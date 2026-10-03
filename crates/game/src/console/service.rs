use cloud_provider_sim::{
    Device, DeviceKind, NetworkSim, PortConfig, RemoteDevice, RemoteRequest, RemoteResponse,
};

/// Runs on the simulation worker; all mutations use the existing console engine.
pub struct ConsoleService<'a> {
    sim: &'a mut NetworkSim,
}

impl<'a> ConsoleService<'a> {
    pub fn new(sim: &'a mut NetworkSim) -> Self {
        Self { sim }
    }

    pub fn handle(&mut self, request: RemoteRequest) -> RemoteResponse {
        match request {
            RemoteRequest::List => RemoteResponse::Devices(self.devices()),
            RemoteRequest::Complete { device, input } => {
                if !self.sim.device(device).is_some_and(|device| device.powered) {
                    return RemoteResponse::Error("device is missing or powered off".into());
                }
                RemoteResponse::Completions(self.sim.console_completions(device, &input))
            }
            RemoteRequest::Connect { target } => self.connect(&target),
            RemoteRequest::Run { device, input } => {
                if input.contains(['\n', '\r']) {
                    return RemoteResponse::Error("send one command at a time".into());
                }
                let output = self.sim.execute_console(device, &input);
                RemoteResponse::Output {
                    lines: output.lines,
                    prompt: self.sim.terminal_prompt(device),
                    success: output.success,
                }
            }
        }
    }

    fn devices(&self) -> Vec<RemoteDevice> {
        let mut devices: Vec<_> = self
            .sim
            .devices()
            .filter_map(|device| {
                let kind = match device.kind {
                    DeviceKind::Server(_) => "server",
                    DeviceKind::Switch(_) => "switch",
                    DeviceKind::Router(_) => "router",
                    _ => return None,
                };
                Some(RemoteDevice {
                    id: device.id,
                    name: device.name.clone(),
                    hostname: self
                        .sim
                        .console_hostname(device.id)
                        .unwrap_or_default()
                        .into(),
                    kind: kind.into(),
                    addresses: self.addresses(device),
                    powered: device.powered,
                })
            })
            .collect();
        devices.sort_by_key(|device| device.id);
        devices
    }

    fn addresses(&self, device: &Device) -> Vec<String> {
        let mut addresses: Vec<_> = device
            .ports()
            .iter()
            .filter_map(|id| self.sim.port(*id))
            .flat_map(|port| match &port.config {
                PortConfig::Server(config) => config
                    .ipv4
                    .iter()
                    .map(|ip| ip.address.to_string())
                    .collect::<Vec<_>>(),
                PortConfig::Router(config) => config
                    .interfaces
                    .iter()
                    .filter_map(|interface| interface.address.map(|ip| ip.to_string()))
                    .collect(),
                _ => Vec::new(),
            })
            .collect();
        addresses.sort();
        addresses.dedup();
        addresses
    }

    fn connect(&self, target: &str) -> RemoteResponse {
        let devices = self.devices();
        let candidates: Vec<_> = devices
            .iter()
            .filter(|device| {
                device.id.to_string() == target
                    || device.name.eq_ignore_ascii_case(target)
                    || device.hostname.eq_ignore_ascii_case(target)
                    || device.addresses.iter().any(|address| address == target)
            })
            .collect();
        match candidates.as_slice() {
            [device] if device.powered => RemoteResponse::Connected {
                device: device.id,
                prompt: self.sim.terminal_prompt(device.id),
            },
            [_] => RemoteResponse::Error("device is powered off".into()),
            [] => RemoteResponse::Error(format!("no in-game machine matches '{target}'")),
            _ => RemoteResponse::Error(format!(
                "multiple machines match '{target}'; use a device ID"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloud_provider_sim::{Command, DeviceId, DeviceTemplate, Ipv4InterfaceConfig, SimEvent};

    fn buy(sim: &mut NetworkSim, kind: DeviceTemplate) -> DeviceId {
        match sim.execute(Command::BuyDevice { kind }).unwrap()[0] {
            SimEvent::DeviceAdded(id) => id,
            _ => unreachable!(),
        }
    }

    #[test]
    fn target_resolution_rejects_duplicates_offline_and_missing_devices() {
        let mut sim = NetworkSim::new();
        let first = buy(&mut sim, DeviceTemplate::Switch);
        buy(&mut sim, DeviceTemplate::Switch);
        buy(&mut sim, DeviceTemplate::PatchPanel);
        let mut service = ConsoleService::new(&mut sim);
        let devices = service.devices();
        assert_eq!(devices.len(), 2);
        for (target, expected) in [
            ("Switch".to_string(), "multiple machines"),
            (first.to_string(), "powered off"),
            ("missing".to_string(), "no in-game machine"),
        ] {
            assert!(matches!(service.handle(RemoteRequest::Connect { target }),
                RemoteResponse::Error(error) if error.contains(expected)));
        }
    }

    #[test]
    fn listing_uses_configured_ipv4_and_hostname() {
        let mut sim = NetworkSim::new();
        let id = buy(&mut sim, DeviceTemplate::Server);
        let port = sim.device(id).unwrap().ports()[0];
        sim.execute(Command::SetHostname {
            device: id,
            hostname: "web-1".into(),
        })
        .unwrap();
        sim.execute(Command::SetIpv4 {
            port,
            config: Ipv4InterfaceConfig {
                address: "192.168.10.2".parse().unwrap(),
                prefix: 24,
                gateway: None,
                vlan: None,
            },
        })
        .unwrap();
        let service = ConsoleService::new(&mut sim);
        let devices = service.devices();
        assert_eq!(devices[0].hostname, "web-1");
        assert_eq!(devices[0].addresses, ["192.168.10.2"]);
        for target in ["192.168.10.2", "WEB-1"] {
            assert!(
                matches!(service.connect(target), RemoteResponse::Error(error) if error == "device is powered off")
            );
        }
    }
}
