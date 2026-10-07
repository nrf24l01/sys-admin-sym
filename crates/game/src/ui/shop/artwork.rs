use super::catalog::Offer;
use bevy_egui::egui;
use cloud_provider_sim::PurchaseItem;

#[derive(Clone, Copy)]
pub(in crate::ui) struct ShopTextures {
    pub server: egui::TextureId,
    pub router: egui::TextureId,
    pub switch: egui::TextureId,
    pub ups: egui::TextureId,
    pub pdu: egui::TextureId,
    pub products: egui::TextureId,
    pub optics: crate::ui::optics::ShopTextures,
    /// Server, router, switch, UPS, PDU, products, modules, assemblies.
    pub ready: [bool; 8],
    pub full: [egui::TextureId; 8],
}
impl ShopTextures {
    pub(super) fn full_resolution(mut self) -> Self {
        [
            self.server,
            self.router,
            self.switch,
            self.ups,
            self.pdu,
            self.products,
        ] = self.full[..6].try_into().unwrap();
        self.optics.modules = self.full[6];
        self.optics.cables = self.full[7];
        self
    }
}
#[derive(Debug, Clone, Copy)]
pub(super) struct Artwork {
    pub texture: egui::TextureId,
    pub uv: egui::Rect,
    pub aspect: f32,
    pub ready: bool,
}

pub(super) fn artwork(offer: &Offer, textures: ShopTextures) -> Option<Artwork> {
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
        "dell_r360" => panel(
            textures.server,
            rect(0.009, 0.48, 0.995, 0.84),
            egui::vec2(2048.0, 512.0),
            textures.ready[0],
        ),
        "cisco_isr_c1111" => panel(
            textures.router,
            rect(0.0, 193.0 / 683.0, 1.0, 480.0 / 683.0),
            egui::vec2(2172.0, 724.0),
            textures.ready[1],
        ),
        "cisco_catalyst_c1000" | "switch_10g" => panel(
            textures.switch,
            rect(0.0, 210.0 / 666.0, 1.0, 434.0 / 666.0),
            egui::vec2(2200.0, 715.0),
            textures.ready[2],
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
            let cell = match id {
                "xeon_e_2434" => 0,
                "ddr5_ecc_16gb" => 1,
                "intel_i350_t4" => 2,
                "intel_x520_da2" => 3,
                "enterprise_hdd_2tb" => 4,
                "enterprise_ssd_960gb" => 5,
                "ethernet_cable_box" => 6,
                "rj45_connectors" => 7,
                "patch_panel" => 8,
                "fiber_panel" => 9,
                "cable_manager" => 10,
                "public_ipv4_pool" => 11,
                _ => return None,
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

pub(super) fn image(ui: &mut egui::Ui, offer: &Offer, textures: ShopTextures, size: egui::Vec2) {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 6.0, ui.visuals().faint_bg_color);
    if let Some(art) = artwork(offer, textures).filter(|art| art.ready) {
        let bounds = rect.shrink(12.0);
        let size = super::layout::image_size(ui, &offer.id, bounds.size(), art.aspect);
        let image_rect = egui::Rect::from_center_size(bounds.center(), size);
        ui.painter()
            .image(art.texture, image_rect, art.uv, egui::Color32::WHITE);
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
        server: texture,
        router: texture,
        switch: texture,
        ups: texture,
        pdu: texture,
        products: texture,
        ready: [true; 8],
        full: [texture; 8],
        optics: crate::ui::optics::ShopTextures {
            modules: texture,
            cables: texture,
        },
    }
}
