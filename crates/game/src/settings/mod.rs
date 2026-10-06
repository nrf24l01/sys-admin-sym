mod config;
mod store;

pub use config::ConsoleSettings;
pub use store::{SettingsFile, SettingsStore};

#[derive(bevy::prelude::Resource)]
pub struct GameSettings {
    pub console: ConsoleSettings,
    pub language: String,
    pub localization: crate::localization::Localization,
    pub console_error: Option<String>,
    pub store: SettingsStore,
}

impl GameSettings {
    /// Save the preference before exposing the new language to the next UI frame.
    pub fn select_language(&mut self, language: &str) -> Result<(), String> {
        self.localization.validate_language(language)?;
        let config = SettingsFile {
            console: self.console.clone(),
            language: language.to_owned(),
        };
        self.store.save(&config)?;
        self.localization.select(language)?;
        self.language = config.language;
        Ok(())
    }
}
