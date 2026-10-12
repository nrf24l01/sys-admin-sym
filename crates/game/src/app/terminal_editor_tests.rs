use super::*;
use cloud_provider_sim::{ConsoleEditorFile, ConsoleEditorKind, DeviceId};

fn editor(kind: ConsoleEditorKind) -> TerminalEditor {
    TerminalEditor::new(ConsoleEditorFile {
        device: DeviceId(1),
        kind,
        path: "/etc/network/interfaces".into(),
        contents: "old\n".into(),
    })
}

#[test]
fn vim_modes_write_quit_and_unsaved_protection() {
    let mut e = editor(ConsoleEditorKind::Vim);
    assert_eq!(e.mode, EditorMode::Normal);
    e.vim_normal_key('i');
    assert_eq!(e.mode, EditorMode::Insert);
    e.buffer = "new\n".into();
    e.escape();
    assert_eq!(e.mode, EditorMode::Normal);
    assert!(e.vim_command("q").is_none());
    assert_eq!(e.prompt, EditorPrompt::ConfirmExit);
    e.escape();
    assert!(
        matches!(e.vim_command("wq"), Some(EditorIntent::Save { ref contents, .. }) if contents == "new\n")
    );
    assert!(e.finish_save(Ok("/etc/network/interfaces".into())));
    assert!(!e.dirty());
}

#[test]
fn nano_write_prompt_and_cancelled_exit_retain_buffer() {
    let mut e = editor(ConsoleEditorKind::Nano);
    e.buffer = "new\n".into();
    e.nano_write();
    assert_eq!(e.prompt, EditorPrompt::WritePath);
    assert_eq!(e.prompt_text, e.path);
    assert!(matches!(e.submit_prompt(), Some(EditorIntent::Save { .. })));
    assert!(!e.finish_save(Ok(e.path.clone())));
    e.buffer.push_str("draft");
    assert!(e.request_quit().is_none());
    assert_eq!(e.prompt, EditorPrompt::ConfirmExit);
    e.escape();
    assert_eq!(e.buffer, "new\ndraft");
    assert_eq!(e.prompt, EditorPrompt::None);
}

#[test]
fn failed_save_and_edits_during_save_are_not_lost() {
    let mut e = editor(ConsoleEditorKind::Vim);
    e.buffer = "new\n".into();
    e.vim_command("wq").unwrap();
    assert!(!e.finish_save(Err("server is powered off".into())));
    assert!(e.dirty());
    assert_eq!(e.buffer, "new\n");
    assert!(e.status.contains("powered off"));
    e.vim_command("wq").unwrap();
    e.buffer.push_str("later");
    assert!(!e.finish_save(Ok(e.path.clone())));
    assert!(e.dirty());
    assert_eq!(e.buffer, "new\nlater");
}
