use crate::app::{SettingsWindowState, UiAction};
use crate::localization::tr;
use crate::settings::GameSettings;
use bevy::prelude::MessageWriter;
use bevy_egui::egui;

pub fn show(
    viewport: &mut egui::Ui,
    state: &mut SettingsWindowState,
    settings: &GameSettings,
    actions: &mut MessageWriter<UiAction>,
) {
    if !state.open {
        return;
    }
    let mut open = state.open;
    egui::Window::new(tr("settings.title"))
        .id(egui::Id::new("game_settings"))
        .open(&mut open)
        .collapsible(false)
        .default_width(430.0)
        .show(viewport, |ui| {
            ui.horizontal(|ui| {
                ui.label(tr("settings.language"));
                let mut language = settings.language.clone();
                egui::ComboBox::from_id_salt("ui-language")
                    .selected_text(
                        settings
                            .localization
                            .languages()
                            .find(|(code, _)| *code == language)
                            .map_or("English", |(_, name)| name),
                    )
                    .show_ui(ui, |ui| {
                        for (code, name) in settings.localization.languages() {
                            ui.selectable_value(&mut language, code.to_owned(), name);
                        }
                    });
                if language != settings.language {
                    actions.write(UiAction::SelectLanguage(language));
                }
            });
            ui.separator();
            ui.heading(tr("ui.console-connection"));
            egui::Grid::new("console_settings_fields")
                .num_columns(2)
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    ui.label(tr("ui.host"));
                    ui.text_edit_singleline(&mut state.host);
                    ui.end_row();
                    ui.label(tr("ui.port"));
                    ui.text_edit_singleline(&mut state.port);
                    ui.end_row();
                    ui.label(tr("ui.password"));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.password)
                            .password(!state.show_password),
                    );
                    ui.end_row();
                });
            ui.checkbox(&mut state.show_password, tr("ui.show-password"));
            ui.horizontal(|ui| {
                if ui.button(tr("ui.apply-settings")).clicked() {
                    match state.config() {
                        Ok(config) => {
                            actions.write(UiAction::ApplyConsoleSettings(config));
                        }
                        Err(error) => state.error = Some(error),
                    }
                }
                if ui.button(tr("ui.reset-fields")).clicked() {
                    *state = SettingsWindowState::from_config(&settings.console);
                    state.open = true;
                }
            });
            if let Some(error) = &state.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error.render());
            }
            if let Some(error) = &settings.console_error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            } else {
                ui.label(crate::localization::tr_args(
                    "settings.listening",
                    &[
                        (settings.console.host).to_string(),
                        (settings.console.port).to_string(),
                    ],
                ));
            }
            ui.separator();
            ui.heading(tr("ui.game"));
            ui.horizontal(|ui| {
                for (label, action) in [
                    ("game.save", UiAction::Save),
                    ("game.load", UiAction::Load),
                    ("game.new", UiAction::NewGame),
                ] {
                    if ui.button(tr(label)).clicked() {
                        actions.write(action);
                    }
                }
            });
        });
    state.open = open;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{ConsoleSettings, SettingsStore};
    use bevy::{
        ecs::system::SystemState,
        prelude::{Messages, World},
    };

    #[test]
    fn separate_settings_window_contains_game_controls_and_emits_actions() {
        let context = egui::Context::default();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut writer = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let settings = GameSettings {
            console: ConsoleSettings::default(),
            language: "en".into(),
            localization: crate::localization::Localization::default(),
            console_error: None,
            store: SettingsStore::new(std::env::temp_dir().join("unused-settings-test.json")),
        };
        let mut state = SettingsWindowState {
            open: true,
            ..Default::default()
        };
        let mut positions = std::collections::HashMap::new();
        for _ in 0..2 {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 800.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    show(
                        ui,
                        &mut state,
                        &settings,
                        &mut writer.get_mut(&mut world).unwrap(),
                    )
                },
            );
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape {
                    let label = text.galley.text();
                    assert_ne!(label, "game", "password must be masked");
                    if ["Apply settings", "Save", "Load", "New game"].contains(&label) {
                        positions.insert(
                            label.to_string(),
                            egui::Rect::from_min_size(text.pos, text.galley.size()).center(),
                        );
                    }
                }
            }
            output.textures_delta.clear();
        }
        assert_eq!(positions.len(), 4);
        for label in ["Save", "Load", "New game", "Apply settings"] {
            let position = positions[label];
            for pressed in [true, false] {
                let mut output = context.run_ui(
                    egui::RawInput {
                        events: vec![
                            egui::Event::PointerMoved(position),
                            egui::Event::PointerButton {
                                pos: position,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        ..Default::default()
                    },
                    |ui| {
                        show(
                            ui,
                            &mut state,
                            &settings,
                            &mut writer.get_mut(&mut world).unwrap(),
                        )
                    },
                );
                output.textures_delta.clear();
            }
        }
        let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        assert_eq!(actions.len(), 4);
        assert!(matches!(actions[0], UiAction::Save));
        assert!(matches!(actions[1], UiAction::Load));
        assert!(matches!(actions[2], UiAction::NewGame));
        assert!(
            matches!(&actions[3], UiAction::ApplyConsoleSettings(config) if config.port == 47655)
        );
    }
}

#[cfg(test)]
mod localization_tests {
    use super::*;
    use crate::{
        localization::Localization,
        settings::{ConsoleSettings, SettingsStore},
    };
    use bevy::{
        ecs::system::SystemState,
        prelude::{Messages, World},
    };

    #[test]
    fn russian_settings_render_language_names_and_translated_controls() {
        let mut localization = Localization::default();
        localization.select("ru").unwrap();
        let _scope = localization.enter();
        let settings = GameSettings {
            console: ConsoleSettings::default(),
            language: "en".into(),
            localization,
            console_error: None,
            store: SettingsStore::new(std::env::temp_dir().join("unused-ru-settings.json")),
        };
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut writer = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut state = SettingsWindowState {
            open: true,
            ..Default::default()
        };
        let context = egui::Context::default();
        let mut labels = Vec::new();
        for _ in 0..2 {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 800.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    show(
                        ui,
                        &mut state,
                        &settings,
                        &mut writer.get_mut(&mut world).unwrap(),
                    )
                },
            );
            output.textures_delta.clear();
            for shape in output.shapes {
                if let egui::Shape::Text(text) = shape.shape {
                    labels.push(text.galley.text().to_owned());
                }
            }
        }
        for expected in [
            "Настройки",
            "Язык",
            "Подключение консоли",
            "Сохранить",
            "Загрузить",
            "Новая игра",
            "English",
        ] {
            assert!(
                labels.iter().any(|label| label == expected),
                "missing {expected}: {labels:?}"
            );
        }
        assert!(
            !labels
                .iter()
                .any(|label| label == "Settings" || label == "game")
        );
    }
}
