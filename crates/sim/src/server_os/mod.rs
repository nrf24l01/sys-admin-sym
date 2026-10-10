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

impl crate::NetworkSim {
    pub(crate) fn server_power_transition(&mut self, device: crate::DeviceId, powered: bool) {
        if !powered {
            self.ssh_sessions
                .retain(|source, target| *source != device && *target != device);
            self.runtime.power_activity.cpu.remove(&device);
            self.runtime.power_activity.storage.remove(&device);
            for id in self.devices[&device].ports() {
                self.runtime.power_activity.network.remove(id);
                if let Some(port) = self.ports.get_mut(id)
                    && let crate::PortConfig::Server(config) = &mut port.config
                {
                    config.ipv4 = None;
                    config.additional_ipv4.clear();
                }
            }
        }
        // A guest not opened yet boots with the usual default OS state.
        let Some(os) = self.server_operating_systems.get_mut(&device) else {
            return;
        };
        for service in os.services.values_mut() {
            service.active = powered && service.enabled;
            service.log.push(
                if powered {
                    "Started after power restored"
                } else {
                    "Stopped: server power lost"
                }
                .into(),
            );
        }
        os.routes.clear();
        os.loopback_up = powered;
        let configure = powered && os.services.get("networking").is_some_and(|s| s.enabled);
        let ports = self.devices[&device].ports().to_vec();
        for id in ports {
            if let Some(port) = self.ports.get_mut(&id) {
                self.runtime.power_activity.network.remove(&id);
                if let crate::PortConfig::Server(config) = &mut port.config {
                    config.ipv4 = None;
                    config.additional_ipv4.clear();
                }
            }
        }
        if configure
            && let Err(error) = services::LinuxServices::configure_network(self, device, None)
            && let Some(service) = self.guest_mut(device).services.get_mut("networking")
        {
            service.active = false;
            service
                .log
                .push(format!("Network configuration failed: {error}"));
        }
    }
}
