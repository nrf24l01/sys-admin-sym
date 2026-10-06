use super::simulation::SimulationWorker;
use crate::app::{GameSet, SettingsWindowState, UiAction, UiState};
use crate::localization::Localization;
use crate::settings::{GameSettings, SettingsFile, SettingsStore};
use bevy::prelude::*;

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        let store = SettingsStore::new(
            std::env::current_dir()
                .unwrap_or_default()
                .join("cloud-provider-settings.json"),
        );
        let loaded = store.load();
        let error = loaded.as_ref().err().cloned();
        let mut saved = loaded.unwrap_or_default();
        let mut localization = Localization::load(&crate::asset_root().join("locales"));
        if localization.select(&saved.language).is_err() {
            saved.language = "en".into();
            localization.select("en").expect("English fallback");
        }
        app.insert_resource(UiState {
            settings: SettingsWindowState::from_config(&saved.console),
            notice: error.map(|error| (error.into(), false)),
            ..Default::default()
        })
        .insert_resource(GameSettings {
            console: saved.console,
            language: saved.language,
            localization,
            console_error: None,
            store,
        })
        .add_systems(Update, apply_settings.in_set(GameSet::Settings))
        .add_systems(Startup, localize_window_title)
        .add_systems(Update, localize_window_title.after(GameSet::Settings));
    }
}

fn apply_settings(
    mut actions: MessageReader<UiAction>,
    mut worker: ResMut<SimulationWorker>,
    mut settings: ResMut<GameSettings>,
    mut ui: ResMut<UiState>,
) {
    for action in actions.read() {
        if let UiAction::SelectLanguage(language) = action {
            let result = settings.select_language(language);
            match result {
                Ok(()) => {
                    ui.settings.error = None;
                    ui.notice = Some(("settings.language-changed".into(), true));
                }
                Err(error) => ui.settings.error = Some(error.into()),
            }
            continue;
        }
        let UiAction::ApplyConsoleSettings(config) = action else {
            continue;
        };
        let previous = settings.console.clone();
        let config = config.clone();
        let result = worker.configure_console(config.clone()).and_then(|_| {
            if let Err(error) = settings.store.save(&SettingsFile {
                console: config.clone(),
                language: settings.language.clone(),
            }) {
                return match worker.configure_console(previous.clone()) {
                    Ok(()) => Err(error),
                    Err(restore_error) => Err(format!("{error}; {restore_error}")),
                };
            }
            Ok(())
        });
        settings.console_error = worker.console_error().map(str::to_owned);
        match result {
            Ok(()) => {
                settings.console = config.clone();
                ui.settings.error = None;
                ui.notice = Some(("settings.applied".into(), true));
            }
            Err(error) => {
                ui.settings.error = Some(error.clone().into());
                ui.notice = Some((error.into(), false));
            }
        }
    }
}

fn localize_window_title(settings: Res<GameSettings>, mut windows: Query<&mut Window>) {
    if settings.is_changed() {
        for mut window in &mut windows {
            window.title = settings.localization.text("app.title");
        }
    }
}
