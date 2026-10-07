//! Cached GPU textures. Pixel conversion/resampling runs on background workers.
mod prepare;

use bevy::{
    asset::{AssetId, RenderAssetUsages},
    image::ImageLoaderSettings,
    prelude::*,
    tasks::{
        AsyncComputeTaskPool, Task,
        futures_lite::future::{block_on, poll_once},
    },
};
use bevy_egui::{EguiContexts, EguiTextureHandle, egui};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

pub(super) fn load(assets: &AssetServer, path: &'static str) -> Handle<Image> {
    assets
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            settings.asset_usage = RenderAssetUsages::MAIN_WORLD;
        })
        .load(path)
}

#[derive(Clone)]
struct GpuTexture {
    image: Handle<Image>,
    texture: Option<egui::TextureId>,
}
impl GpuTexture {
    fn id(&mut self, contexts: &mut EguiContexts) -> egui::TextureId {
        *self.texture.get_or_insert_with(|| {
            contexts.add_image(EguiTextureHandle::Strong(self.image.clone()))
        })
    }
}
struct PreparedImage {
    cpu: Option<Arc<Image>>,
    full: GpuTexture,
    thumbnails: BTreeMap<u32, GpuTexture>,
}

pub(super) struct ImageTextures {
    sources: HashSet<AssetId<Image>>,
    thumbnail_sources: HashSet<AssetId<Image>>,
    prepared: HashMap<AssetId<Image>, PreparedImage>,
    pending: HashMap<AssetId<Image>, Task<Option<prepare::Prepared>>>,
    thumbnail_pending: HashMap<(AssetId<Image>, u32), Task<Option<Image>>>,
    used: HashSet<AssetId<Image>>,
    edge: u32,
}
impl Default for ImageTextures {
    fn default() -> Self {
        Self {
            sources: HashSet::new(),
            thumbnail_sources: HashSet::new(),
            prepared: HashMap::new(),
            pending: HashMap::new(),
            thumbnail_pending: HashMap::new(),
            used: HashSet::new(),
            edge: 1024,
        }
    }
}

impl ImageTextures {
    pub fn track(&mut self, sources: impl IntoIterator<Item = (AssetId<Image>, bool)>) {
        for (id, thumbnail) in sources {
            self.sources.insert(id);
            if thumbnail {
                self.thumbnail_sources.insert(id);
            }
        }
    }

    /// Called by the asset/update system. Drawing only reads completed cache entries.
    pub fn update(
        &mut self,
        assets: &mut Assets<Image>,
        contexts: &mut EguiContexts,
        events: impl IntoIterator<Item = AssetEvent<Image>>,
        resized_edge: Option<u32>,
    ) {
        if let Some(edge) = resized_edge {
            self.edge = edge;
        }
        for event in events {
            let id = match event {
                AssetEvent::Added { id } | AssetEvent::LoadedWithDependencies { id } => id,
                AssetEvent::Modified { id } => {
                    if self.sources.contains(&id) {
                        self.invalidate(id, contexts);
                    }
                    id
                }
                AssetEvent::Removed { id } => {
                    self.invalidate(id, contexts);
                    continue;
                }
                AssetEvent::Unused { .. } => continue,
            };
            if !self.sources.contains(&id)
                || self.pending.contains_key(&id)
                || self.prepared.contains_key(&id)
            {
                continue;
            }
            // Transfer decoded pixels without copying them on the UI thread or
            // generating a Modified event that would invalidate our own cache.
            let Some(source) = assets.get_mut_untracked(id) else {
                continue;
            };
            let Some(data) = source.data.take() else {
                continue;
            };
            let mut owned = source.clone();
            owned.data = Some(data);
            let edge = if self.thumbnail_sources.contains(&id) {
                self.edge
            } else {
                0
            };
            self.pending.insert(
                id,
                AsyncComputeTaskPool::get().spawn(async move { prepare::prepare(owned, edge) }),
            );
        }
        let mut completed = Vec::new();
        self.pending.retain(|id, task| {
            if let Some(result) = block_on(poll_once(task)) {
                completed.push((*id, result));
                false
            } else {
                true
            }
        });
        for (id, result) in completed {
            let Some(result) = result else {
                warn!("Cannot prepare equipment image {id:?}");
                continue;
            };
            let full = GpuTexture {
                image: assets.add(result.full),
                texture: None,
            };
            let thumbnail = result.thumbnail.map_or_else(
                || full.clone(),
                |image| GpuTexture {
                    image: assets.add(image),
                    texture: None,
                },
            );
            self.prepared.insert(
                id,
                PreparedImage {
                    cpu: result.cpu,
                    full,
                    thumbnails: BTreeMap::from([(result.edge, thumbnail)]),
                },
            );
        }
        let mut completed = Vec::new();
        self.thumbnail_pending.retain(|key, task| {
            if let Some(result) = block_on(poll_once(task)) {
                completed.push((*key, result));
                false
            } else {
                true
            }
        });
        for ((id, edge), result) in completed {
            if let Some(prepared) = self.prepared.get_mut(&id) {
                let thumbnail = result.map_or_else(
                    || prepared.full.clone(),
                    |image| GpuTexture {
                        image: assets.add(image),
                        texture: None,
                    },
                );
                prepared.thumbnails.insert(edge, thumbnail);
            }
        }
        // At most one job per source/resolution. Returning to an earlier window
        // size reuses its cached thumbnail; input and scrolling never resample.
        for (&id, prepared) in &self.prepared {
            let key = (id, self.edge);
            if self.thumbnail_sources.contains(&id)
                && !prepared.thumbnails.contains_key(&self.edge)
                && !self.thumbnail_pending.contains_key(&key)
            {
                let Some(source) = prepared.cpu.clone() else {
                    continue;
                };
                let edge = self.edge;
                self.thumbnail_pending.insert(
                    key,
                    AsyncComputeTaskPool::get()
                        .spawn(async move { prepare::thumbnail(&source, edge) }),
                );
            }
        }
    }

    pub fn texture(
        &mut self,
        source: &Handle<Image>,
        contexts: &mut EguiContexts,
    ) -> egui::TextureId {
        self.prepared
            .get_mut(&source.id())
            .map_or(egui::TextureId::User(u64::MAX), |prepared| {
                self.used.insert(prepared.full.image.id());
                prepared.full.id(contexts)
            })
    }

    pub fn thumbnail(
        &mut self,
        source: &Handle<Image>,
        contexts: &mut EguiContexts,
    ) -> egui::TextureId {
        let Some(prepared) = self.prepared.get_mut(&source.id()) else {
            return egui::TextureId::User(u64::MAX);
        };
        let closest = *prepared
            .thumbnails
            .keys()
            .min_by_key(|edge| edge.abs_diff(self.edge))
            .unwrap();
        let thumbnail = prepared.thumbnails.get_mut(&closest).unwrap();
        self.used.insert(thumbnail.image.id());
        thumbnail.id(contexts)
    }

    /// Keep GPU images cached, but don't make egui rebuild bindings for hidden
    /// views and inactive thumbnail sizes on every rendered frame.
    pub fn retain_used(&mut self, contexts: &mut EguiContexts) {
        for prepared in self.prepared.values_mut() {
            for texture in
                std::iter::once(&mut prepared.full).chain(prepared.thumbnails.values_mut())
            {
                if !self.used.contains(&texture.image.id()) && texture.texture.take().is_some() {
                    contexts.remove_image(texture.image.id());
                }
            }
        }
        self.used.clear();
    }

    pub fn contains(&self, source: &Handle<Image>) -> bool {
        self.prepared.contains_key(&source.id())
    }

    fn invalidate(&mut self, source: AssetId<Image>, contexts: &mut EguiContexts) {
        self.pending.remove(&source);
        self.thumbnail_pending.retain(|(id, _), _| *id != source);
        if let Some(prepared) = self.prepared.remove(&source) {
            contexts.remove_image(prepared.full.image.id());
            for thumbnail in prepared.thumbnails.values() {
                contexts.remove_image(thumbnail.image.id());
            }
        }
    }
}

pub(super) fn thumbnail_edge(window: &Window) -> u32 {
    let base = if window.width() <= 900.0 {
        512.0
    } else {
        1024.0
    };
    ((base * window.scale_factor()).ceil() as u32)
        .next_power_of_two()
        .clamp(512, 2048)
}

#[cfg(test)]
mod tests;
