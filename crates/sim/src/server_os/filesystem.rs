use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestFile {
    pub contents: String,
    pub directory: bool,
    pub mode: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestFilesystem {
    pub entries: BTreeMap<String, GuestFile>,
}

impl Default for GuestFilesystem {
    fn default() -> Self {
        let mut fs = Self {
            entries: BTreeMap::new(),
        };
        for directory in [
            "/",
            "/root",
            "/home",
            "/etc",
            "/etc/network",
            "/etc/ssh",
            "/tmp",
            "/var",
            "/var/log",
            "/usr",
            "/usr/bin",
            "/bin",
            "/sbin",
            "/dev",
            "/proc",
        ] {
            fs.entries.insert(
                directory.into(),
                GuestFile {
                    contents: String::new(),
                    directory: true,
                    mode: 0o755,
                },
            );
        }
        for (path, contents) in [
            (
                "/etc/os-release",
                "NAME=\"Debian GNU/Linux\"\nVERSION_ID=\"12\"\nPRETTY_NAME=\"Debian GNU/Linux 12 (simulated)\"\n",
            ),
            ("/etc/hosts", "127.0.0.1 localhost\n"),
            (
                "/etc/resolv.conf",
                "# Configure simulated DNS resolvers here\n",
            ),
            (
                "/etc/network/interfaces",
                "auto lo\niface lo inet loopback\n\n# Example static configuration:\n# auto eth0\n# iface eth0 inet static\n#     address 10.0.0.10/16\n#     gateway 10.0.0.1\n",
            ),
            ("/etc/ssh/sshd_config", "Port 22\nPermitRootLogin yes\n"),
            ("/etc/passwd", "root:x:0:0:root:/root:/bin/bash\n"),
            ("/etc/group", "root:x:0:\n"),
            ("/var/log/syslog", "Linux guest initialized\n"),
        ] {
            fs.write(path, contents, false)
                .expect("default guest filesystem");
        }
        fs
    }
}

impl GuestFilesystem {
    pub fn normalize(cwd: &str, path: &str) -> String {
        let path = if path == "~" {
            "/root".into()
        } else if let Some(rest) = path.strip_prefix("~/") {
            format!("/root/{rest}")
        } else {
            path.to_owned()
        };
        let absolute = if path.starts_with('/') {
            path
        } else {
            format!("{cwd}/{path}")
        };
        let mut parts = Vec::new();
        for part in absolute.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                value => parts.push(value),
            }
        }
        format!("/{}", parts.join("/"))
    }

    pub fn read(&self, path: &str) -> Result<String, String> {
        let file = self
            .entries
            .get(path)
            .ok_or_else(|| format!("{path}: No such file or directory"))?;
        if file.directory {
            return Err(format!("{path}: Is a directory"));
        }
        Ok(file.contents.clone())
    }

    pub fn write(&mut self, path: &str, contents: &str, append: bool) -> Result<(), String> {
        let parent =
            path.rsplit_once('/').map_or(
                "/",
                |(parent, _)| if parent.is_empty() { "/" } else { parent },
            );
        if !self.entries.get(parent).is_some_and(|file| file.directory) {
            return Err(format!("{parent}: No such directory"));
        }
        let file = self.entries.entry(path.into()).or_insert(GuestFile {
            contents: String::new(),
            directory: false,
            mode: 0o644,
        });
        if file.directory {
            return Err(format!("{path}: Is a directory"));
        }
        if append {
            file.contents.push_str(contents);
        } else {
            file.contents = contents.into();
        }
        Ok(())
    }

    pub fn mkdir(&mut self, path: &str, parents: bool) -> Result<(), String> {
        if let Some(file) = self.entries.get(path) {
            return if parents && file.directory {
                Ok(())
            } else {
                Err(format!("{path}: File exists"))
            };
        }
        let parent =
            path.rsplit_once('/').map_or(
                "/",
                |(parent, _)| if parent.is_empty() { "/" } else { parent },
            );
        if parents && !self.entries.contains_key(parent) {
            self.mkdir(parent, true)?;
        }
        if !self.entries.get(parent).is_some_and(|file| file.directory) {
            return Err(format!("{parent}: No such directory"));
        }
        self.entries.insert(
            path.into(),
            GuestFile {
                contents: String::new(),
                directory: true,
                mode: 0o755,
            },
        );
        Ok(())
    }

    pub fn list(&self, path: &str) -> Result<Vec<(String, GuestFile)>, String> {
        let file = self
            .entries
            .get(path)
            .ok_or_else(|| format!("{path}: No such file or directory"))?;
        if !file.directory {
            return Ok(vec![(
                path.rsplit('/').next().unwrap_or(path).into(),
                file.clone(),
            )]);
        }
        let prefix = if path == "/" {
            "/".into()
        } else {
            format!("{path}/")
        };
        Ok(self
            .entries
            .iter()
            .filter_map(|(name, file)| {
                let relative = name.strip_prefix(&prefix)?;
                (!relative.is_empty() && !relative.contains('/'))
                    .then(|| (relative.into(), file.clone()))
            })
            .collect())
    }

    pub fn remove(&mut self, path: &str, recursive: bool, force: bool) -> Result<(), String> {
        if path == "/" {
            return Err("refusing to remove guest root directory".into());
        }
        let Some(file) = self.entries.get(path) else {
            return if force {
                Ok(())
            } else {
                Err(format!("{path}: No such file or directory"))
            };
        };
        if file.directory && !recursive {
            return Err(format!("{path}: Is a directory"));
        }
        let prefix = format!("{path}/");
        self.entries
            .retain(|name, _| name != path && !(recursive && name.starts_with(&prefix)));
        Ok(())
    }
}
