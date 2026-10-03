use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy_egui::EguiPlugin;

mod app;
mod persistence;
mod plugins;
mod ui;

fn asset_root_for(
    executable: &std::path::Path,
    workspace_assets: &std::path::Path,
) -> std::path::PathBuf {
    let beside_executable = executable.parent().map(|parent| parent.join("assets"));
    beside_executable
        .filter(|path| path.is_dir())
        .unwrap_or_else(|| workspace_assets.to_path_buf())
}

fn asset_root() -> std::path::PathBuf {
    let workspace_assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    std::env::current_exe()
        .ok()
        .map(|executable| asset_root_for(&executable, &workspace_assets))
        .unwrap_or(workspace_assets)
}

fn main() {
    let asset_root = asset_root().to_string_lossy().into_owned();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_assets_next_to_executable_take_precedence() {
        let bundle =
            std::env::temp_dir().join(format!("cloud-provider-assets-{}", std::process::id()));
        let assets = bundle.join("assets");
        std::fs::create_dir_all(&assets).unwrap();
        let executable = bundle.join("cloud-provider-sim");
        let fallback = std::path::Path::new("/source/assets");
        assert_eq!(asset_root_for(&executable, fallback), assets);
        std::fs::remove_dir_all(&bundle).unwrap();
        assert_eq!(asset_root_for(&executable, fallback), fallback);
    }
}
