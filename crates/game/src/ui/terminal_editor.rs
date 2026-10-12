use crate::app::{EditorIntent, EditorMode, EditorPrompt, TerminalEditor};
use bevy_egui::egui;
use cloud_provider_sim::ConsoleEditorKind;
mod input;
#[cfg(test)]
mod tests;

/// Shared by docked and floating consoles; keyboard events belong to the focused editor.
pub(super) fn show(
    ui: &mut egui::Ui,
    editor: &mut TerminalEditor,
    console: cloud_provider_sim::DeviceId,
) -> Option<EditorIntent> {
    let text_id = egui::Id::new(("guest-editor", console.0));
    let prompt_id = text_id.with("prompt");
    let confirm_id = text_id.with("confirm");
    // egui may surrender focus on Escape before rendering a newly focused widget,
    // because its focus-lock filter takes effect only after one complete frame.
    let escape = ui.input(|i| i.events.iter().any(|event| matches!(event,
        egui::Event::Key { key: egui::Key::Escape, pressed: true, modifiers, .. } if !modifiers.any())));
    let accepts_input = ui.is_visible() && !ui.is_sizing_pass();
    let focused = accepts_input
        && (editor.focus
            || ui.memory(|m| {
                let ids = [text_id, prompt_id, confirm_id];
                ids.iter().any(|id| m.has_focus(*id))
                    || escape
                        && m.focused().is_none()
                        && ids.iter().any(|id| m.had_focus_last_frame(*id))
            }));
    let routed = if focused {
        input::route(ui, editor)
    } else {
        input::EditorInput {
            editable: editor.prompt == EditorPrompt::None && editor.mode != EditorMode::Normal,
            ..Default::default()
        }
    };
    let mut intent = None;
    ui.horizontal(|ui| {
        ui.strong(if editor.kind == ConsoleEditorKind::Nano {
            "nano"
        } else {
            "vim"
        });
        ui.monospace(&editor.path);
        if editor.dirty() {
            ui.label("[Modified]");
        }
        if editor.busy() {
            ui.spinner();
        }
    });
    let controls_height = if editor.prompt == EditorPrompt::None {
        70.0
    } else {
        105.0
    };
    let height = (ui.available_height() - controls_height).max(40.0);
    egui::ScrollArea::both()
        .id_salt(text_id.with("scroll"))
        .auto_shrink([false, false])
        .max_height(height)
        .min_scrolled_height(height)
        .show(ui, |ui| {
            let original = (!routed.editable).then(|| editor.buffer.clone());
            // Text typed before a mode/prompt transition still belongs to the buffer.
            let body_input = !routed.body.is_empty();
            if accepts_input && (body_input || editor.focus && editor.prompt == EditorPrompt::None)
            {
                ui.memory_mut(|m| {
                    if !m.has_focus(text_id) {
                        m.request_focus(text_id);
                    }
                });
                if editor.prompt == EditorPrompt::None {
                    editor.focus = false;
                }
            }
            input::with_events(ui, routed.body, |ui| {
                egui::TextEdit::multiline(&mut editor.buffer)
                    .id(text_id)
                    .font(egui::TextStyle::Monospace)
                    .code_editor()
                    .cursor_at_end(false)
                    .event_filter(editor_event_filter())
                    .desired_width(f32::INFINITY)
                    .desired_rows(1)
                    .interactive(editor.prompt == EditorPrompt::None || body_input)
                    .show(ui);
            });
            if let Some(original) = original {
                editor.buffer = original;
            }
        });
    // Save and quit see the newly edited buffer, including text typed in this frame.
    if routed.quit && !editor.dirty() {
        intent = Some(EditorIntent::Close);
    }
    match routed.confirm {
        Some(true) => intent = editor.confirm_save(),
        Some(false) => intent = Some(EditorIntent::Close),
        None => {}
    }
    ui.separator();
    match editor.prompt {
        EditorPrompt::WritePath | EditorPrompt::VimCommand => {
            let write_path = editor.prompt == EditorPrompt::WritePath;
            ui.horizontal(|ui| {
                ui.label(if write_path {
                    "File name to write:"
                } else {
                    ":"
                });
                if accepts_input && (editor.focus || !routed.prompt.is_empty()) {
                    ui.memory_mut(|m| {
                        if !m.has_focus(prompt_id) {
                            m.request_focus(prompt_id);
                        }
                    });
                    editor.focus = false;
                }
                input::with_events(ui, routed.prompt, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut editor.prompt_text)
                            .id(prompt_id)
                            .event_filter(editor_event_filter())
                            .font(egui::TextStyle::Monospace)
                            .desired_width(ui.available_width() - 65.0),
                    );
                });
                if ui
                    .button(if write_path { "Write" } else { "Run" })
                    .clicked()
                    || routed.submit
                {
                    intent = editor.submit_prompt();
                }
            });
            ui.weak("Enter confirm · Esc cancel");
        }
        EditorPrompt::ConfirmExit => {
            let label = ui.label("Save modified buffer before exiting?");
            let response = ui.interact(
                label.rect,
                confirm_id,
                egui::Sense::focusable_noninteractive(),
            );
            if accepts_input && editor.focus {
                if !ui.memory(|m| m.has_focus(confirm_id)) {
                    response.request_focus();
                }
                editor.focus = false;
            }
            if ui.memory(|m| m.has_focus(confirm_id)) {
                ui.memory_mut(|m| m.set_focus_lock_filter(confirm_id, editor_event_filter()));
            }
            ui.horizontal(|ui| {
                if ui.button("Yes (Y)").clicked() {
                    intent = editor.confirm_save();
                }
                if ui.button("No (N) — discard").clicked() {
                    intent = Some(EditorIntent::Close);
                }
                if ui.button("Cancel (Esc)").clicked() {
                    editor.escape();
                }
            });
        }
        EditorPrompt::None => {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!editor.busy(), egui::Button::new("Save"))
                    .clicked()
                {
                    if editor.kind == ConsoleEditorKind::Nano {
                        editor.nano_write();
                    } else {
                        intent = editor.vim_command("w");
                    }
                }
                if ui
                    .add_enabled(!editor.busy(), egui::Button::new("Exit"))
                    .clicked()
                {
                    intent = editor.request_quit();
                }
                ui.weak(match editor.mode {
                    EditorMode::Nano => "Ctrl+O Write Out · Ctrl+X Exit",
                    EditorMode::Normal => {
                        "NORMAL · i Insert · h/j/k/l or arrows Move · :w Save · :q Quit"
                    }
                    EditorMode::Insert => "INSERT · Esc Normal",
                });
            });
        }
    }
    if !editor.status.is_empty() {
        ui.monospace(&editor.status);
    }
    intent
}

fn editor_event_filter() -> egui::EventFilter {
    egui::EventFilter {
        tab: true,
        horizontal_arrows: true,
        vertical_arrows: true,
        escape: true,
    }
}
