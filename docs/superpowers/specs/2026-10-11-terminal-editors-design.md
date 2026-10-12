# In-Game Linux Terminal Editors

## Goal

Allow players to edit files on simulated Linux servers from the in-game terminal, especially network configuration files such as `/etc/network/interfaces`. Support `nano FILE` and `vim FILE` with editor-specific interaction, while keeping all file reads and writes inside the simulated guest filesystem.

## User experience

From a server console, `sudo nano /etc/network/interfaces` opens a nano-style editor and `sudo vim /etc/network/interfaces` opens a Vim-style editor. Existing files load into the editor; a missing file opens as a new empty buffer. The editor occupies the terminal view and offers familiar keys: nano saves with Ctrl+O and exits with Ctrl+X; Vim starts in normal mode, enters insert mode with `i`, saves with `:w`, quits with `:q`, and saves then quits with `:wq`. Unsaved changes must be confirmed before discarding them. A visible status/footer explains the active mode and shortcuts.

The docked and floating in-game terminal windows share the same editor state and behavior. Closing and reopening an editor must not silently discard unsaved content. The ordinary shell resumes after the editor exits. Existing command execution, history, completion, and terminal output remain available outside editor mode.

## Architecture and data flow

- The UI keeps an editor session associated with the console device, including path, editor kind, buffer, dirty state, and Vim mode/status prompt state.
- Opening an editor is an explicit simulation-worker request. The worker resolves the active console target (including an SSH session), validates the server is powered on, reads the guest filesystem, and returns either file contents or a clear error.
- Saving sends the path and complete buffer to the simulation worker. The worker writes only to the target guest's simulated filesystem using existing path normalization and file I/O rules, and reports errors back to the UI. The host filesystem is never accessed.
- Editor launch is recognized for server consoles only. IOS devices continue to treat `nano` and `vim` as unsupported commands.
- Both editor interfaces share buffer and save plumbing. Their keyboard state machines and displayed shortcuts remain distinct; Vim implements normal and insert modes plus the essential `:` write/quit commands, while nano implements its save and exit prompts.
- Saves are atomic from the user's perspective: a failed write leaves the editor open with its buffer intact and marks the buffer dirty. A successful save updates the simulated file and clears the dirty state.

## Scope and boundaries

This provides practical text editing for simulated server files, not a complete reproduction of GNU nano or Vim. Initial support is limited to opening one path, basic text insertion/deletion and cursor navigation, saving, quitting, dirty-buffer confirmation, nano Ctrl+O/Ctrl+X prompts, and Vim normal/insert mode with `i`, `Esc`, `:w`, `:q`, and `:wq`. Advanced Vim motions, macros, syntax highlighting, shell escapes, search/replace, permissions escalation beyond the existing command context, and editor support in the standalone `game-ssh` client are out of scope.

The editor uses the existing in-game terminal UI. The selected implementation must keep file contents and editor state in the game/simulation process and must not execute host `nano` or `vim` binaries.

## Errors and recovery

Missing parent directories, invalid paths, powered-off devices, non-server targets, and simulated filesystem write failures are shown in the terminal/editor. Failed opens return to the command prompt without creating a file. Failed saves retain the buffer. Quitting with unsaved changes offers save, discard, or cancel; closing the terminal window itself must preserve the draft in console state.

## Verification

- Simulation tests prove editor file reads and writes target the simulated guest filesystem, support creating a file, preserve exact multiline contents, reject invalid paths, and resolve the remote guest when opened through an active SSH session.
- UI tests exercise nano save/exit prompts, Vim normal/insert and `:w`/`:q`/`:wq`, dirty-buffer protection, and shared docked/floating editor state.
- Integration coverage opens `/etc/network/interfaces` in an editor, saves valid static configuration, restarts networking, and verifies the address and route are applied. A failed save must preserve the unsaved draft.
- Run focused terminal and server-OS tests, then required `make check` verification.
