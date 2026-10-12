use super::*;
use cloud_provider_sim::{ConsoleEditorFile, DeviceId};

fn editor(kind: ConsoleEditorKind) -> TerminalEditor {
    TerminalEditor::new(ConsoleEditorFile {
        device: DeviceId(1),
        kind,
        path: "/etc/network/interfaces".into(),
        contents: "".into(),
    })
}
fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}
fn frame(
    ctx: &egui::Context,
    editor: &mut TerminalEditor,
    events: Vec<egui::Event>,
) -> Option<EditorIntent> {
    let mut intent = None;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 500.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            intent = show(ui, editor, DeviceId(1));
        },
    );
    output.textures_delta.clear();
    intent
}
#[test]
fn unfocused_exit_confirmation_does_not_consume_another_consoles_keys() {
    let ctx = egui::Context::default();
    let mut editor = editor(ConsoleEditorKind::Nano);
    frame(&ctx, &mut editor, vec![]);
    editor.buffer = "unsaved".into();
    editor.request_quit();
    frame(&ctx, &mut editor, vec![]);
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("other-console")));
    let intent = frame(
        &ctx,
        &mut editor,
        vec![key(egui::Key::N, egui::Modifiers::NONE)],
    );
    assert!(intent.is_none());
    assert_eq!(editor.buffer, "unsaved");
    assert_eq!(editor.prompt, EditorPrompt::ConfirmExit);
}
#[test]
fn vim_preserves_input_batched_with_mode_switches() {
    let ctx = egui::Context::default();
    let mut editor = editor(ConsoleEditorKind::Vim);
    frame(&ctx, &mut editor, vec![]);
    frame(
        &ctx,
        &mut editor,
        vec![
            egui::Event::Text("i".into()),
            egui::Event::Text("hello".into()),
        ],
    );
    assert_eq!(editor.buffer, "hello");
    editor.escape();
    frame(&ctx, &mut editor, vec![]);
    frame(
        &ctx,
        &mut editor,
        vec![egui::Event::Text(":".into()), egui::Event::Text("w".into())],
    );
    assert_eq!(editor.prompt_text, "w");
}
#[test]
fn nano_yes_confirmation_does_not_change_the_save_filename() {
    let ctx = egui::Context::default();
    let mut editor = editor(ConsoleEditorKind::Nano);
    frame(&ctx, &mut editor, vec![]);
    editor.buffer = "draft".into();
    editor.request_quit();
    frame(&ctx, &mut editor, vec![]);
    frame(
        &ctx,
        &mut editor,
        vec![
            key(egui::Key::Y, egui::Modifiers::NONE),
            egui::Event::Text("y".into()),
        ],
    );
    assert_eq!(editor.prompt_text, "/etc/network/interfaces");
    let intent = frame(
        &ctx,
        &mut editor,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    assert!(
        matches!(intent, Some(EditorIntent::Save { path, .. }) if path == "/etc/network/interfaces")
    );
}
#[test]
fn vim_escape_respects_order_with_batched_text() {
    let ctx = egui::Context::default();
    let mut editor = editor(ConsoleEditorKind::Vim);
    frame(&ctx, &mut editor, vec![]);
    frame(
        &ctx,
        &mut editor,
        vec![
            egui::Event::Text("i".into()),
            key(egui::Key::Escape, egui::Modifiers::NONE),
        ],
    );
    assert_eq!(editor.mode, EditorMode::Normal);
    frame(&ctx, &mut editor, vec![egui::Event::Text("i".into())]);
    frame(
        &ctx,
        &mut editor,
        vec![
            egui::Event::Text("x".into()),
            key(egui::Key::Escape, egui::Modifiers::NONE),
        ],
    );
    assert_eq!(editor.buffer, "x");
    assert_eq!(editor.mode, EditorMode::Normal);
}
#[test]
fn vim_keyboard_enters_insert_writes_text_and_saves_from_command_prompt() {
    let ctx = egui::Context::default();
    let mut editor = editor(ConsoleEditorKind::Vim);
    frame(&ctx, &mut editor, vec![]);
    frame(&ctx, &mut editor, vec![egui::Event::Text("x".into())]);
    assert!(editor.buffer.is_empty());
    frame(&ctx, &mut editor, vec![egui::Event::Text("i".into())]);
    assert_eq!(editor.mode, EditorMode::Insert);
    assert!(editor.buffer.is_empty());
    frame(
        &ctx,
        &mut editor,
        vec![egui::Event::Text("auto eth0\n".into())],
    );
    assert_eq!(editor.buffer, "auto eth0\n");
    frame(
        &ctx,
        &mut editor,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    frame(&ctx, &mut editor, vec![egui::Event::Text(":".into())]);
    assert_eq!(editor.prompt, EditorPrompt::VimCommand);
    frame(&ctx, &mut editor, vec![egui::Event::Text("wq".into())]);
    let intent = frame(
        &ctx,
        &mut editor,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    assert!(
        matches!(intent, Some(EditorIntent::Save { contents, .. }) if contents == "auto eth0\n")
    );
}
#[test]
fn nano_keyboard_writes_and_exit_requires_confirmation() {
    let ctx = egui::Context::default();
    let mut editor = editor(ConsoleEditorKind::Nano);
    frame(&ctx, &mut editor, vec![]);
    frame(&ctx, &mut editor, vec![egui::Event::Text("draft\n".into())]);
    frame(
        &ctx,
        &mut editor,
        vec![key(egui::Key::O, egui::Modifiers::CTRL)],
    );
    assert_eq!(editor.prompt, EditorPrompt::WritePath);
    let intent = frame(
        &ctx,
        &mut editor,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    assert!(matches!(intent, Some(EditorIntent::Save { contents, .. }) if contents == "draft\n"));
    editor.finish_save(Ok(editor.path.clone()));
    frame(&ctx, &mut editor, vec![]);
    frame(&ctx, &mut editor, vec![egui::Event::Text("more".into())]);
    frame(
        &ctx,
        &mut editor,
        vec![key(egui::Key::X, egui::Modifiers::CTRL)],
    );
    assert_eq!(editor.prompt, EditorPrompt::ConfirmExit);
    frame(
        &ctx,
        &mut editor,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(editor.buffer.contains("more"));
    assert_eq!(editor.prompt, EditorPrompt::None);
}

#[test]
fn invisible_editor_layout_does_not_process_keyboard_events() {
    for prompt in [false, true] {
        let ctx = egui::Context::default();
        let mut editor = editor(ConsoleEditorKind::Nano);
        frame(&ctx, &mut editor, vec![]);
        if prompt {
            editor.nano_write();
            frame(&ctx, &mut editor, vec![]);
        }
        let original_buffer = editor.buffer.clone();
        let original_prompt = editor.prompt_text.clone();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Text("hidden text".into())],
                ..Default::default()
            },
            |ui| {
                ui.set_invisible();
                show(ui, &mut editor, DeviceId(1));
            },
        );
        output.textures_delta.clear();
        assert_eq!(editor.buffer, original_buffer, "prompt={prompt}");
        assert_eq!(editor.prompt_text, original_prompt, "prompt={prompt}");
    }
}
