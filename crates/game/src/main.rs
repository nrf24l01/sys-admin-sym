use bevy::prelude::*;
use bevy_egui::EguiPlugin;

mod app;
mod persistence;
mod plugins;
mod ui;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Cloud Provider Simulator".into(),
                resolution: (1600, 900).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(app::GamePlugin)
        .run();
}
