use crate::app::GameSet;
use crate::ui::{EquipmentImages, load_equipment_images, main_ui};
use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EquipmentImages>()
            .add_systems(Startup, (setup_camera, load_equipment_images))
            .add_systems(EguiPrimaryContextPass, main_ui)
            .add_systems(Update, clear_stale_notice.in_set(GameSet::Presentation));
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn clear_stale_notice() {
    // Notices intentionally remain visible until the next action in the MVP.
}
