//! Guest-only file access for interactive console editors.
use super::{
    GuestFilesystem, LinuxShell,
    syntax::{ShellSyntax, ShellToken},
};
use crate::{DeviceId, DeviceKind, NetworkSim};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleEditorKind {
    Nano,
    Vim,
}

#[derive(Debug, Clone)]
pub struct ConsoleEditorFile {
    pub device: DeviceId,
    pub kind: ConsoleEditorKind,
    pub path: String,
    pub contents: String,
}

impl NetworkSim {
    /// Recognize standalone editor commands; other shell commands remain untouched.
    pub fn open_console_editor(
        &mut self,
        source: DeviceId,
        input: &str,
    ) -> Option<Result<ConsoleEditorFile, String>> {
        let mut raw_words = input.split_whitespace();
        let first = raw_words.next()?;
        let candidate = if first.rsplit('/').next()? == "sudo" {
            raw_words.next()?
        } else {
            first
        };
        if !matches!(candidate.rsplit('/').next()?, "nano" | "vim" | "vi") {
            return None;
        }
        let tokens = match ShellSyntax::tokenize(input) {
            Ok(tokens) => tokens,
            Err(error) => return Some(Err(error)),
        };
        let mut words = tokens.iter();
        let Some(ShellToken::Word(mut command)) = words.next().cloned() else {
            return None;
        };
        if command.rsplit('/').next()? == "sudo" {
            let Some(ShellToken::Word(next)) = words.next() else {
                return None;
            };
            command = next.clone();
        }
        let kind = match command.rsplit('/').next()? {
            "nano" => ConsoleEditorKind::Nano,
            "vim" | "vi" => ConsoleEditorKind::Vim,
            _ => return None,
        };
        let path = match (words.next(), words.next()) {
            (Some(ShellToken::Word(path)), None) if !path.is_empty() && !path.starts_with('-') => {
                path.clone()
            }
            _ => {
                return Some(Err(format!(
                    "usage: {command} FILE (interactive in-game console)"
                )));
            }
        };
        Some((|| {
            let mut device = source;
            let mut visited = HashSet::new();
            loop {
                self.validate_editor_guest(device)?;
                if !visited.insert(device) {
                    return Err("SSH session cycle".into());
                }
                let Some(target) = self.ssh_sessions.get(&device).copied() else {
                    break;
                };
                device = target;
            }
            let expanded = LinuxShell::expand(self, device, &path);
            let path = self.editor_path(device, &expanded)?;
            let os = self.guest_mut(device);
            let contents = if os.filesystem.entries.contains_key(&path) {
                os.filesystem.read(&path)?
            } else {
                String::new()
            };
            os.history.push(input.into());
            if os.history.len() > 1000 {
                os.history.remove(0);
            }
            self.record_guest_work(device, 200, contents.len() as u64);
            Ok(ConsoleEditorFile {
                device,
                kind,
                path,
                contents,
            })
        })())
    }

    pub fn save_console_editor_file(
        &mut self,
        device: DeviceId,
        path: &str,
        contents: &str,
    ) -> Result<String, String> {
        self.validate_editor_guest(device)?;
        let path = self.editor_path(device, path)?;
        self.guest_mut(device)
            .filesystem
            .write(&path, contents, false)?;
        self.record_guest_work(device, 200, contents.len() as u64);
        Ok(path)
    }

    fn validate_editor_guest(&self, device: DeviceId) -> Result<(), String> {
        let guest = self
            .device(device)
            .ok_or("editor: server no longer exists")?;
        if !guest.powered {
            return Err("editor: server is powered off".into());
        }
        if !matches!(guest.kind, DeviceKind::Server(_)) {
            return Err("editor: Linux server required".into());
        }
        Ok(())
    }

    fn editor_path(&mut self, device: DeviceId, path: &str) -> Result<String, String> {
        if path.is_empty() || path.contains(['\0', '\n', '\r']) {
            return Err("editor: invalid file path".into());
        }
        let os = self.guest_mut(device);
        let path = GuestFilesystem::normalize(&os.cwd, path);
        if os
            .filesystem
            .entries
            .get(&path)
            .is_some_and(|file| file.directory)
        {
            return Err(format!("{path}: Is a directory"));
        }
        let parent =
            path.rsplit_once('/').map_or(
                "/",
                |(parent, _)| if parent.is_empty() { "/" } else { parent },
            );
        if !os
            .filesystem
            .entries
            .get(parent)
            .is_some_and(|file| file.directory)
        {
            return Err(format!("{parent}: No such directory"));
        }
        Ok(path)
    }
}
