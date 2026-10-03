mod config;
mod store;

pub use config::ConsoleSettings;
pub use store::SettingsStore;

#[derive(bevy::prelude::Resource)]
pub struct GameSettings {
    pub console: ConsoleSettings,
    pub console_error: Option<String>,
    pub store: SettingsStore,
}
