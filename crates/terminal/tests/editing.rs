#[cfg(unix)]
#[test]
fn interactive_terminal_editing_history_and_completion() {
    let status = std::process::Command::new("python3")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/editing.py"))
        .arg(env!("CARGO_BIN_EXE_game-ssh"))
        .status()
        .expect("run PTY integration test");
    assert!(status.success(), "terminal editing integration test failed");
}
