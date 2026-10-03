//! Persistent Linux guest model. All commands operate on this guest and the
//! simulated network; the host machine is never used as a command backend.
mod command_registry;
mod commands;
mod filesystem;
mod network;
mod services;
mod shell;
mod syntax;

pub use command_registry::{CommandRegistry, CompletionContext, LinuxCommand};
pub use filesystem::{GuestFile, GuestFilesystem};
pub use network::{LinuxRoute, RouteSelection};
use serde::{Deserialize, Serialize};
pub use shell::LinuxShell;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxService {
    pub enabled: bool,
    pub active: bool,
    pub log: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerOs {
    pub filesystem: GuestFilesystem,
    pub cwd: String,
    pub environment: BTreeMap<String, String>,
    pub history: Vec<String>,
    pub last_status: u8,
    pub routes: Vec<LinuxRoute>,
    pub services: BTreeMap<String, LinuxService>,
    pub loopback_up: bool,
    #[serde(skip)]
    pub(crate) shell_depth: usize,
}

impl Default for ServerOs {
    fn default() -> Self {
        Self {
            filesystem: GuestFilesystem::default(),
            cwd: "/root".into(),
            environment: BTreeMap::from([
                ("HOME".into(), "/root".into()),
                ("USER".into(), "root".into()),
                ("SHELL".into(), "/bin/bash".into()),
                (
                    "PATH".into(),
                    "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".into(),
                ),
            ]),
            history: Vec::new(),
            last_status: 0,
            routes: Vec::new(),
            loopback_up: true,
            shell_depth: 0,
            services: ["networking", "ssh", "systemd-resolved"]
                .into_iter()
                .map(|name| {
                    (
                        name.into(),
                        LinuxService {
                            enabled: true,
                            active: true,
                            log: vec![format!("Started {name}.service")],
                        },
                    )
                })
                .collect(),
        }
    }
}

impl ServerOs {
    pub fn prompt(&self, hostname: &str) -> String {
        let path = if self.cwd == "/root" { "~" } else { &self.cwd };
        format!("root@{hostname}:{path}#")
    }
}
