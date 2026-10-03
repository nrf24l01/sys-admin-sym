use crate::app::{SettingsWindowState, UiAction};
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
    egui::Window::new("Settings")
        .id(egui::Id::new("game_settings"))
        .open(&mut open)
        .collapsible(false)
        .default_width(430.0)
        .show(viewport, |ui| {
            ui.heading("Console connection");
            egui::Grid::new("console_settings_fields")
                .num_columns(2)
                .spacing([16.0, 10.0])
                .show(ui, |ui| {
                    ui.label("Host");
                    ui.text_edit_singleline(&mut state.host);
                    ui.end_row();
                    ui.label("Port");
                    ui.text_edit_singleline(&mut state.port);
                    ui.end_row();
                    ui.label("Password");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.password)
                            .password(!state.show_password),
                    );
                    ui.end_row();
                });
            ui.checkbox(&mut state.show_password, "Show password");
            ui.horizontal(|ui| {
                if ui.button("Apply settings").clicked() {
                    match state.config() {
                        Ok(config) => {
                            actions.write(UiAction::ApplyConsoleSettings(config));
                        }
                        Err(error) => state.error = Some(error),
                    }
                }
                if ui.button("Reset fields").clicked() {
                    *state = SettingsWindowState::from_config(&settings.console);
                    state.open = true;
                }
            });
            if let Some(error) = &state.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
            if let Some(error) = &settings.console_error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            } else {
                ui.label(format!(
                    "Listening on {}:{}",
                    settings.console.host, settings.console.port
                ));
            }
            ui.separator();
            ui.heading("Game");
            ui.horizontal(|ui| {
                for (label, action) in [
                    ("Save", UiAction::Save),
                    ("Load", UiAction::Load),
                    ("New game", UiAction::NewGame),
                ] {
                    if ui.button(label).clicked() {
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
