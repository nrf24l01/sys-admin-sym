use crate::app::{EditorMode, EditorPrompt, TerminalEditor};
use bevy_egui::egui;
use cloud_provider_sim::ConsoleEditorKind;

#[derive(Default)]
pub(super) struct EditorInput {
    pub body: Vec<egui::Event>,
    pub prompt: Vec<egui::Event>,
    pub editable: bool,
    pub quit: bool,
    pub confirm: Option<bool>,
    pub submit: bool,
}

/// Route events in order, then let each TextEdit apply its own editing/cursor rules.
/// Write/quit actions run after the routed text has reached the buffer.
pub(super) fn route(ui: &mut egui::Ui, editor: &mut TerminalEditor) -> EditorInput {
    let mut routed = EditorInput {
        editable: editor.prompt == EditorPrompt::None && editor.mode != EditorMode::Normal,
        ..Default::default()
    };
    let events = ui.input_mut(|input| std::mem::take(&mut input.events));
    let mut unhandled = Vec::new();
    let mut stop = false;
    for event in events {
        if !matches!(
            event,
            egui::Event::Text(_)
                | egui::Event::Paste(_)
                | egui::Event::Cut
                | egui::Event::Copy
                | egui::Event::Key { .. }
        ) {
            unhandled.push(event);
            continue;
        }
        if stop {
            continue;
        }
        if matches!(event, egui::Event::Key { key: egui::Key::Escape, pressed: true, modifiers, .. } if modifiers == egui::Modifiers::NONE)
        {
            editor.escape();
            routed.prompt.clear();
            continue;
        }
        if editor.kind == ConsoleEditorKind::Nano {
            if matches!(event, egui::Event::Key { key: egui::Key::O, pressed: true, modifiers, .. } if modifiers.ctrl && !modifiers.alt)
            {
                if !editor.busy() {
                    editor.nano_write();
                    routed.prompt.clear();
                }
                continue;
            }
            if matches!(event, egui::Event::Key { key: egui::Key::X, pressed: true, modifiers, .. } if modifiers.ctrl && !modifiers.alt)
            {
                if !editor.busy() {
                    routed.quit = true;
                    editor.prompt = EditorPrompt::ConfirmExit;
                    editor.focus = true;
                    routed.prompt.clear();
                }
                continue;
            }
        }
        match editor.prompt {
            EditorPrompt::ConfirmExit => {
                let choice = match &event {
                    egui::Event::Key {
                        key: egui::Key::Y,
                        pressed: true,
                        modifiers,
                        ..
                    } if !modifiers.any() => Some(true),
                    egui::Event::Key {
                        key: egui::Key::N,
                        pressed: true,
                        modifiers,
                        ..
                    } if !modifiers.any() => Some(false),
                    egui::Event::Text(text) if text.eq_ignore_ascii_case("y") => Some(true),
                    egui::Event::Text(text) if text.eq_ignore_ascii_case("n") => Some(false),
                    _ => None,
                };
                if choice.is_some() {
                    routed.confirm = choice;
                    stop = true;
                }
            }
            EditorPrompt::WritePath | EditorPrompt::VimCommand => {
                if matches!(event, egui::Event::Key { key: egui::Key::Enter, pressed: true, modifiers, .. } if !modifiers.any())
                {
                    routed.submit = true;
                    stop = true;
                } else {
                    routed.prompt.push(event);
                }
            }
            EditorPrompt::None if editor.mode != EditorMode::Normal => {
                routed.editable = true;
                routed.body.push(event);
            }
            EditorPrompt::None => match event {
                egui::Event::Text(text) => {
                    for (offset, key) in text.char_indices() {
                        if editor.prompt != EditorPrompt::None {
                            routed.prompt.push(egui::Event::Text(text[offset..].into()));
                            break;
                        }
                        if editor.mode == EditorMode::Insert {
                            routed.editable = true;
                            routed.body.push(egui::Event::Text(text[offset..].into()));
                            break;
                        }
                        editor.vim_normal_key(key);
                        let movement = match key {
                            'h' => Some(egui::Key::ArrowLeft),
                            'j' => Some(egui::Key::ArrowDown),
                            'k' => Some(egui::Key::ArrowUp),
                            'l' => Some(egui::Key::ArrowRight),
                            _ => None,
                        };
                        if let Some(key) = movement {
                            routed.body.push(egui::Event::Key {
                                key,
                                physical_key: None,
                                pressed: true,
                                repeat: false,
                                modifiers: egui::Modifiers::NONE,
                            });
                        }
                    }
                }
                egui::Event::Copy => routed.body.push(event),
                egui::Event::Key {
                    key:
                        egui::Key::ArrowLeft
                        | egui::Key::ArrowRight
                        | egui::Key::ArrowUp
                        | egui::Key::ArrowDown
                        | egui::Key::Home
                        | egui::Key::End
                        | egui::Key::PageUp
                        | egui::Key::PageDown,
                    ..
                } => routed.body.push(event),
                _ => {}
            },
        }
    }
    ui.input_mut(|input| input.events = unhandled);
    routed
}

/// Give only this widget its keyboard events; leave pointer events available to the UI.
pub(super) fn with_events<R>(
    ui: &mut egui::Ui,
    events: Vec<egui::Event>,
    render: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let previous = ui.input_mut(|input| {
        let previous = std::mem::take(&mut input.events);
        input.events = previous
            .iter()
            .filter(|event| {
                !matches!(
                    event,
                    egui::Event::Text(_)
                        | egui::Event::Paste(_)
                        | egui::Event::Cut
                        | egui::Event::Copy
                        | egui::Event::Key { .. }
                )
            })
            .cloned()
            .collect();
        input.events.extend(events);
        previous
    });
    let result = render(ui);
    ui.input_mut(|input| input.events = previous);
    result
}
