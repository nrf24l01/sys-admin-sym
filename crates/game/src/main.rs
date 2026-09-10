use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy_egui::EguiPlugin;

mod app;
mod persistence;
mod plugins;
mod ui;

fn main() {
    // Cargo preserves the caller's working directory. Resolve the shared workspace
    // assets from this crate so `cargo run` works from either the workspace root or
    // `crates/game`.
    let asset_root = format!("{}/../../assets", env!("CARGO_MANIFEST_DIR"));

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: asset_root,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Cloud Provider Simulator".into(),
                        resolution: (1600, 900).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(EguiPlugin::default())
        .add_plugins(app::GamePlugin)
        .run();
}
