# Terminal Editors Implementation Plan

**Goal:** Edit simulated server files with nano and modal Vim in docked/floating game consoles.
**Architecture:** Simulation resolves and validates editor launch/file I/O. Worker requests return a pinned guest/path buffer. Shared UI editor state handles distinct nano/Vim keys and asynchronous saves.
**Spec:** `docs/superpowers/specs/2026-10-11-terminal-editors-design.md`
**Execution:** Native implementation in this session; user explicitly requested proceeding without more approval gates.

## Global Constraints

Keep all file I/O in simulated guests. Preserve unsaved drafts and existing saves. Use existing worker boundaries. Run `make check`. Do not touch `.VSCodeCounter/`.

## Review Focus

Quoted/relative paths must resolve on the actual SSH target. Failed save must retain draft. Pending save must not clear newer edits. Powered-off/deleted targets must fail clearly. Nano and Vim controls must not submit shell commands.

### Task 1: Guest editor I/O

Files: `crates/sim/src/server_os/{mod,editor}.rs`, `crates/sim/tests/server_os.rs`.
Interfaces: `ConsoleEditorKind`, `ConsoleEditorFile`; `NetworkSim::open_console_editor(source, input) -> Option<Result<ConsoleEditorFile, String>>`; `NetworkSim::save_console_editor_file(target, path, contents) -> Result<String, String> (returns the normalized saved path)`.
- [x] Add failing tests for quoted/relative paths, exact multiline save, creation, invalid paths and SSH resolution.
- [x] Implement launch parsing and guest-only reads/writes with pinned target and powered-server validation.
- [x] Run focused server OS tests.

### Task 2: Interactive editor and worker integration

Files: `crates/game/src/app/{state,messages}.rs`, `crates/game/src/plugins/simulation.rs`, `crates/game/src/ui/{terminal_renderer,terminal_editor}.rs`, editor tests.
Interfaces: shared `TerminalEditor` state; editor-open and editor-save worker responses; save request carries captured buffer and target/path.
- [x] Add failing behavior tests for Vim mode/commands, nano save/exit and dirty/pending-save recovery.
- [x] Intercept editor launches on worker before shell execution; return editor buffer and pin writes to opened guest.
- [x] Render the same editor in both console views, with nano prompts and Vim normal/insert/command modes. Keep drafts across window closure and report save failures in place.
- [x] Run game editor and worker tests.

### Task 3: User documentation and full verification

Files: `docs/PROVIDER_NETWORK_GUIDE.md` and command-help/completion where appropriate.
- [x] Document `/etc/network/interfaces` editing, save/quit keys and `systemctl restart networking`.
- [x] Verify real editor-to-network restart integration, inspect changes and run `make check`.

Verification: `make check VK_DRIVER_FILES=/tmp/sys-admin-drive-lavapipe/lvp_icd.json` passed, including formatting, whitespace, Clippy, workspace Rust/Python tests, build, and game-window/authenticated-console smoke checks. Keyboard regressions cover docked/floating focus handoff and invisible editor layout.
