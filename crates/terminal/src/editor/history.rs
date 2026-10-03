use cloud_provider_sim::DeviceId;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

pub struct HistoryFile {
    path: Option<PathBuf>,
}

impl HistoryFile {
    pub fn new(endpoint: (&str, u16), device: DeviceId) -> Self {
        let directory = std::env::var_os("GAME_SSH_HISTORY_DIR")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .or_else(|| state_directory().map(|path| path.join("game-ssh")));
        Self {
            path: directory.map(|directory| {
                let mut identity = std::collections::hash_map::DefaultHasher::new();
                endpoint.hash(&mut identity);
                directory.join(format!("{:016x}-{}.history", identity.finish(), device.0))
            }),
        }
    }

    pub fn load(&mut self, editor: &mut super::LineEditor) {
        let Some(path) = &self.path else { return };
        if !path.exists() {
            return;
        }
        if let Err(error) = editor.load_history(path) {
            eprintln!("game-ssh: could not load command history: {error}");
        }
    }

    pub fn save(&mut self, editor: &mut super::LineEditor) {
        let Some(path) = &self.path else { return };
        let result = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| editor.append_history(path).map_err(std::io::Error::other));
        if let Err(error) = result {
            eprintln!("game-ssh: could not save command history: {error}");
            self.path = None;
        }
    }
}

fn state_directory() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .map(|home| PathBuf::from(home).join("AppData/Local"))
            })
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_STATE_HOME")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
            })
    }
}
