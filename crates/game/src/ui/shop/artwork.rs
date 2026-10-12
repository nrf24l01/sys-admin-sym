use super::catalog::Offer;
use bevy_egui::egui;
use cloud_provider_sim::PurchaseItem;

#[derive(Clone)]
pub(in crate::ui) struct ShopTextures {
    pub servers:
        std::sync::Arc<std::collections::BTreeMap<String, crate::ui::server_models::ServerShopIds>>,
    pub use_full: bool,
    pub server: egui::TextureId,
    pub router: egui::TextureId,
    pub switch: egui::TextureId,
    pub switch_10g: egui::TextureId,
    pub ups: egui::TextureId,
    pub pdu: egui::TextureId,
    pub products: egui::TextureId,
    pub optics: crate::ui::optics::ShopTextures,
    /// Server, router, 4G switch, UPS, PDU, products, modules, assemblies, 4X switch.
    pub ready: [bool; 9],
    pub full: [egui::TextureId; 9],
}
impl ShopTextures {
    pub(super) fn full_resolution(&self) -> Self {
        let mut result = self.clone();
        result.use_full = true;
        [
            result.server,
            result.router,
            result.switch,
            result.ups,
            result.pdu,
            result.products,
        ] = result.full[..6].try_into().unwrap();
        result.optics.modules = result.full[6];
        result.optics.cables = result.full[7];
        result.switch_10g = result.full[8];
        result
    }
}
#[derive(Debug, Clone, Copy)]
pub(super) struct Artwork {
    pub texture: egui::TextureId,
    pub uv: egui::Rect,
    pub aspect: f32,
    pub ready: bool,
}

pub(super) fn artwork(offer: &Offer, textures: &ShopTextures) -> Option<Artwork> {
    if matches!(
        offer.item,
        PurchaseItem::Transceiver(_) | PurchaseItem::Assembly(_)
    ) {
        return crate::ui::optics::shop_artwork(&offer.id, textures.optics).map(|(texture, uv)| {
            Artwork {
                texture,
                uv,
                ready: textures.ready[if matches!(offer.item, PurchaseItem::Assembly(_)) {
                    7
                } else {
                    6
                }],
                aspect: if matches!(offer.item, PurchaseItem::Assembly(_)) {
                    1.0
                } else {
                    uv.width() / uv.height()
                },
            }
        });
    }
    if let Some(model) = cloud_provider_sim::server_catalog().model(&offer.display_id) {
        let t = textures.servers.get(&model.id)?;
        let [l, top, r, b] = model.artwork.front.uv;
        let [w, h] = model.artwork.front.source_size;
        return Some(Artwork {
            texture: if textures.use_full {
                t.full
            } else {
                t.thumbnail
            },
            uv: egui::Rect::from_min_max(egui::pos2(l, top), egui::pos2(r, b)),
            aspect: w as f32 * (r - l) / (h as f32 * (b - top)),
            ready: t.ready,
        });
    }
    let rect = |a, b, c, d| egui::Rect::from_min_max(egui::pos2(a, b), egui::pos2(c, d));
    let panel = |texture, uv: egui::Rect, source: egui::Vec2, ready| {
        Some(Artwork {
            texture,
            uv,
            aspect: source.x * uv.width() / (source.y * uv.height()),
            ready,
        })
    };
    match offer.display_id.as_str() {
        "cisco_isr_c1111" => panel(
            textures.router,
            rect(0.0, 193.0 / 683.0, 1.0, 480.0 / 683.0),
            egui::vec2(2172.0, 724.0),
            textures.ready[1],
        ),
        "cisco_catalyst_c1000" => panel(
            textures.switch,
            rect(0.0, 210.0 / 666.0, 1.0, 434.0 / 666.0),
            egui::vec2(2200.0, 715.0),
            textures.ready[2],
        ),
        "switch_10g" => panel(
            textures.switch_10g,
            rect(8.0 / 500.0, 203.0 / 400.0, 495.0 / 500.0, 251.0 / 400.0),
            egui::vec2(500.0, 400.0),
            textures.ready[8],
        ),
        "apc_smt1500" => panel(
            textures.ups,
            rect(90.0 / 2048.0, 0.0, 1958.0 / 2048.0, 0.5),
            egui::vec2(2048.0, 768.0),
            textures.ready[3],
        ),
        "rack_pdu" => panel(
            textures.pdu,
            rect(78.0 / 2048.0, 128.0 / 768.0, 1970.0 / 2048.0, 370.0 / 768.0),
            egui::vec2(2048.0, 768.0),
            textures.ready[4],
        ),
        id => {
            let cell = match cloud_provider_sim::server_catalog()
                .part(id)
                .map(|p| &p.kind)
            {
                Some(cloud_provider_sim::ServerPartKind::Cpu { .. }) => 0,
                Some(cloud_provider_sim::ServerPartKind::Ram { .. }) => 1,
                _ => match id {
                    "xeon_e_2434" => 0,
                    "ddr5_ecc_16gb" | "ddr5_ecc_32gb" => 1,
                    "intel_i350_t4" => 2,
                    "intel_x520_da2" => 3,
                    "enterprise_hdd_2tb" => 4,
                    "enterprise_ssd_960gb" => 5,
                    "ethernet_cable_box" => 6,
                    "rj45_connectors" => 7,
                    "patch_panel" => 8,
                    "fiber_panel" => 9,
                    "cable_manager" => 10,
                    "public_ipv4_pool" | "public_ipv4_pool_24" => 11,
                    _ => return None,
                },
            };
            Some(Artwork {
                texture: textures.products,
                uv: egui::Rect::from_min_size(
                    egui::pos2((cell % 4) as f32 / 4.0, (cell / 4) as f32 / 3.0),
                    egui::vec2(0.25, 1.0 / 3.0),
                ),
                aspect: 1.0,
                ready: textures.ready[5],
            })
        }
    }
}

pub(super) fn image(ui: &mut egui::Ui, offer: &Offer, textures: &ShopTextures, size: egui::Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 6.0, ui.visuals().faint_bg_color);
    if let Some(art) = artwork(offer, textures).filter(|art| art.ready) {
        let bounds = rect.shrink(12.0);
        let size = super::layout::image_size(ui, &offer.id, bounds.size(), art.aspect);
        let image_rect = egui::Rect::from_center_size(bounds.center(), size);
        ui.painter()
            .image(art.texture, image_rect, art.uv, egui::Color32::WHITE);
        if let PurchaseItem::ServerModel {
            model_id,
            configuration,
        } = &offer.item
        {
            let model = cloud_provider_sim::server_catalog()
                .model(model_id)
                .unwrap();
            let mut drives = vec![None; model.chassis.drive_bays.len()];
            if *configuration == cloud_provider_sim::ServerConfiguration::FullPack {
                for drive in &model.full_pack.drives {
                    drives[drive.bay] = Some(drive.drive_id.clone());
                }
            }
            crate::ui::server_drive_faces::paint(ui.painter(), image_rect, model, &drives, false);
        }
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            crate::localization::tr("shop.image-unavailable"),
            egui::FontId::proportional(12.0),
            ui.visuals().weak_text_color(),
        );
    }
}

#[cfg(test)]
pub(super) fn test_textures() -> ShopTextures {
    let texture = egui::TextureId::User(1);
    ShopTextures {
        use_full: false,
        servers: std::sync::Arc::new(
            cloud_provider_sim::server_catalog()
                .models
                .iter()
                .map(|m| {
                    (
                        m.id.clone(),
                        crate::ui::server_models::ServerShopIds {
                            thumbnail: texture,
                            full: texture,
                            ready: true,
                        },
                    )
                })
                .collect(),
        ),
        server: texture,
        router: texture,
        switch: texture,
        switch_10g: texture,
        ups: texture,
        pdu: texture,
        products: texture,
        ready: [true; 9],
        full: [texture; 9],
        optics: crate::ui::optics::ShopTextures {
            modules: texture,
            cables: texture,
        },
    }
}
