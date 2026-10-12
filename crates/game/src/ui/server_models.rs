//! Catalog-driven server artwork; adding a JSON model needs no renderer branch.
use super::textures::{self, ImageTextures};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use cloud_provider_sim::server_catalog;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Default)]
pub(super) struct ServerImages(pub BTreeMap<String, ServerFaces>);
pub(super) struct ServerFaces {
    pub front: Handle<Image>,
    pub rear: Handle<Image>,
}
#[derive(Clone, Copy)]
pub(super) struct ServerFaceIds {
    pub front: egui::TextureId,
    pub rear: egui::TextureId,
}
#[derive(Clone, Copy)]
pub(super) struct ServerShopIds {
    pub thumbnail: egui::TextureId,
    pub full: egui::TextureId,
    pub ready: bool,
}

impl ServerImages {
    pub fn load(&mut self, assets: &AssetServer) {
        self.0 = server_catalog()
            .models
            .iter()
            .map(|model| {
                (
                    model.id.clone(),
                    ServerFaces {
                        front: textures::load(assets, &model.artwork.front.path),
                        rear: textures::load(assets, &model.artwork.rear.path),
                    },
                )
            })
            .collect();
    }
    pub fn sources(&self) -> impl Iterator<Item = (AssetId<Image>, bool)> + '_ {
        self.0
            .values()
            .flat_map(|face| [(face.front.id(), true), (face.rear.id(), false)])
    }
    pub fn rack_ids(
        &self,
        images: &mut ImageTextures,
        contexts: &mut EguiContexts,
        active: &BTreeSet<String>,
    ) -> Arc<BTreeMap<String, ServerFaceIds>> {
        Arc::new(
            self.0
                .iter()
                .filter(|(id, _)| active.contains(*id))
                .map(|(id, face)| {
                    (
                        id.clone(),
                        ServerFaceIds {
                            front: images.texture(&face.front, contexts),
                            rear: images.texture(&face.rear, contexts),
                        },
                    )
                })
                .collect(),
        )
    }
    pub fn shop_ids(
        &self,
        images: &mut ImageTextures,
        contexts: &mut EguiContexts,
        active: &BTreeMap<String, bool>,
    ) -> Arc<BTreeMap<String, ServerShopIds>> {
        Arc::new(
            self.0
                .iter()
                .filter(|(id, _)| active.contains_key(*id))
                .map(|(id, face)| {
                    let thumbnail = images.thumbnail(&face.front, contexts);
                    let full = if active[id] {
                        images.texture(&face.front, contexts)
                    } else {
                        thumbnail
                    };
                    (
                        id.clone(),
                        ServerShopIds {
                            thumbnail,
                            full,
                            ready: images.contains(&face.front),
                        },
                    )
                })
                .collect(),
        )
    }
}
