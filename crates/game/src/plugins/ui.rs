use crate::app::GameSet;
use crate::ui::main_ui;
use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(EguiPrimaryContextPass, main_ui)
            .add_systems(Update, clear_stale_notice.in_set(GameSet::Presentation));
    }
}

fn clear_stale_notice() {
    // Notices intentionally remain visible until the next action in the MVP.
}
