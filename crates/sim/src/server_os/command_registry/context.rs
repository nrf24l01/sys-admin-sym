use super::CommandRegistry;
use crate::*;

/// Immutable world and parsed arguments available to a command's suggestion method.
pub struct CompletionContext<'a> {
    pub registry: &'a CommandRegistry,
    pub sim: &'a NetworkSim,
    pub device: DeviceId,
    pub args: &'a [String],
    pub partial: &'a str,
}

impl<'a> CompletionContext<'a> {
    pub fn new(
        registry: &'a CommandRegistry,
        sim: &'a NetworkSim,
        device: DeviceId,
        args: &'a [String],
        partial: &'a str,
    ) -> Self {
        Self {
            registry,
            sim,
            device,
            args,
            partial,
        }
    }
    pub fn with_args(&self, args: &'a [String]) -> Self {
        Self::new(self.registry, self.sim, self.device, args, self.partial)
    }
    pub fn choices(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }
    pub fn last(&self) -> Option<&str> {
        self.args.last().map(String::as_str)
    }
    pub fn interfaces(&self) -> Vec<String> {
        std::iter::once("lo".into())
            .chain(
                self.sim
                    .device(self.device)
                    .into_iter()
                    .flat_map(|device| device.ports())
                    .filter_map(|id| self.sim.port(*id))
                    .map(|port| port.name.clone()),
            )
            .collect()
    }
    pub fn addresses(&self) -> Vec<String> {
        let mut values = Vec::new();
        for port in self.sim.ports() {
            match &port.config {
                PortConfig::Server(config) => {
                    for ip in config.addresses() {
                        values.push(ip.address.to_string());
                        if let Some(gateway) = ip.gateway {
                            values.push(gateway.to_string());
                        }
                    }
                }
                PortConfig::Router(config) => values.extend(
                    config
                        .interfaces
                        .iter()
                        .filter_map(|interface| interface.address.map(|ip| ip.to_string())),
                ),
                _ => {}
            }
        }
        values.extend(
            self.sim
                .ios_configs
                .values()
                .filter_map(|config| config.management_ip.map(|ip| ip.to_string())),
        );
        values
    }
    pub fn cidrs(&self) -> Vec<String> {
        self.sim
            .device(self.device)
            .into_iter()
            .flat_map(|device| device.ports())
            .filter_map(|id| self.sim.port(*id))
            .flat_map(|port| match &port.config {
                PortConfig::Server(config) => config
                    .addresses()
                    .map(|ip| format!("{}/{}", ip.address, ip.prefix))
                    .collect(),
                _ => Vec::new(),
            })
            .collect()
    }
    pub fn drives(&self) -> Vec<String> {
        let Some(DeviceKind::Server(server)) =
            self.sim.device(self.device).map(|device| &device.kind)
        else {
            return Vec::new();
        };
        server
            .hardware
            .as_ref()
            .into_iter()
            .flat_map(|hardware| &hardware.drives)
            .flatten()
            .enumerate()
            .map(|(index, _)| format!("/dev/{}", crate::linux_drive_name(index)))
            .collect()
    }
    pub fn units(&self) -> Vec<String> {
        let default = ServerOs::default();
        self.sim
            .server_os(self.device)
            .unwrap_or(&default)
            .services
            .keys()
            .cloned()
            .flat_map(|name| [format!("{name}.service"), name])
            .collect()
    }
    pub fn variables(&self) -> Vec<String> {
        let default = ServerOs::default();
        self.sim
            .server_os(self.device)
            .unwrap_or(&default)
            .environment
            .keys()
            .cloned()
            .collect()
    }
    pub fn hosts(&self) -> Vec<String> {
        let mut values = self.addresses();
        values.extend(self.sim.devices().filter_map(|device| match &device.kind {
            DeviceKind::Server(server) => Some(server.hostname.clone()),
            _ => None,
        }));
        let default = GuestFilesystem::default();
        let fs = self
            .sim
            .server_os(self.device)
            .map_or(&default, |os| &os.filesystem);
        if let Ok(contents) = fs.read("/etc/hosts") {
            for line in contents.lines() {
                values.extend(
                    line.split('#')
                        .next()
                        .unwrap_or("")
                        .split_whitespace()
                        .skip(1)
                        .map(str::to_owned),
                );
            }
        }
        values
    }
    pub fn paths(&self, directories: bool) -> Vec<String> {
        let default = GuestFilesystem::default();
        let os = self.sim.server_os(self.device);
        let fs = os.map_or(&default, |os| &os.filesystem);
        let cwd = os.map_or("/root", |os| os.cwd.as_str());
        let prefix = self
            .partial
            .rsplit_once('/')
            .map_or("", |(parent, _)| &self.partial[..parent.len() + 1]);
        let directory =
            GuestFilesystem::normalize(cwd, if prefix.is_empty() { "." } else { prefix });
        fs.list(&directory)
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, file)| !directories || file.directory)
            .map(|(name, file)| format!("{prefix}{name}{}", if file.directory { "/" } else { "" }))
            .collect()
    }
    pub fn finish(&self, candidates: Vec<String>) -> Vec<String> {
        let mut values: Vec<_> = candidates
            .into_iter()
            .filter(|value| value.starts_with(self.partial))
            .map(|value| {
                value
                    .chars()
                    .flat_map(|c| {
                        if c.is_whitespace()
                            || matches!(c, '\\' | '\'' | '"' | '$' | ';' | '|' | '&' | '<' | '>')
                        {
                            vec!['\\', c]
                        } else {
                            vec![c]
                        }
                    })
                    .collect::<String>()
            })
            .collect();
        values.sort();
        values.dedup();
        values
    }
}
