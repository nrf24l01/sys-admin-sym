//! Uniform virtual rows keep scroll offsets and widget identities stable.
use bevy_egui::egui;

#[derive(Clone, Copy, PartialEq)]
struct LayoutKey {
    width: f32,
    list: bool,
    variants: bool,
}
#[derive(Clone, Copy)]
pub(super) struct ProductLayout {
    key: LayoutKey,
    pub columns: usize,
    pub row_height: f32,
}
impl ProductLayout {
    pub fn is_list(self) -> bool {
        self.key.list
    }
    pub fn cached(ui: &egui::Ui, list: bool, variants: bool) -> Self {
        let key = LayoutKey {
            width: ui.available_width(),
            list,
            variants,
        };
        let id = ui.id().with("shop-product-layout");
        ui.ctx().data_mut(|data| {
            if let Some(cached) = data.get_temp::<Self>(id).filter(|cached| cached.key == key) {
                return cached;
            }
            let layout = Self {
                key,
                columns: if list {
                    1
                } else {
                    (key.width / 280.0).floor().clamp(1.0, 3.0) as usize
                },
                row_height: if list { 256.0 } else { 336.0 } + if variants { 28.0 } else { 0.0 },
            };
            data.insert_temp(id, layout);
            layout
        })
    }
}

#[derive(Clone, Copy)]
struct ImageFit {
    bounds: egui::Vec2,
    aspect: f32,
    size: egui::Vec2,
}

pub(super) fn image_size(ui: &egui::Ui, id: &str, bounds: egui::Vec2, aspect: f32) -> egui::Vec2 {
    let id = ui.id().with(("shop-image-fit", id));
    ui.ctx().data_mut(|data| {
        if let Some(cached) = data
            .get_temp::<ImageFit>(id)
            .filter(|cached| cached.bounds == bounds && cached.aspect == aspect)
        {
            return cached.size;
        }
        let width = bounds.x.min(bounds.y * aspect);
        let size = egui::vec2(width, width / aspect);
        data.insert_temp(
            id,
            ImageFit {
                bounds,
                aspect,
                size,
            },
        );
        size
    })
}
