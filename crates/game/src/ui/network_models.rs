//! Catalog-driven network equipment artwork.
use super::server_models::{ServerFaceIds, ServerFaces, ServerShopIds};
use super::textures::{self, ImageTextures};
use bevy::prelude::*;
use bevy_egui::EguiContexts;
use cloud_provider_sim::network_device_catalog;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Default)]
pub(super) struct NetworkImages(pub BTreeMap<String, ServerFaces>);

impl NetworkImages {
    pub fn load(&mut self, assets: &AssetServer) {
        self.0 = network_device_catalog()
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
