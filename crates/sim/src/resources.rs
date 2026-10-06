use crate::{DeviceId, DeviceKind, NetworkSim, PortId};

/// Capacity credited to a network. CPU is measured in aggregate core MHz;
/// memory is generation and module weighted equivalent GB.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ServerResources {
    pub compute_mhz: u64,
    pub memory_score_gb: u64,
    pub network_mbps: u64,
}

impl ServerResources {
    pub fn add(&mut self, other: Self) {
        self.compute_mhz += other.compute_mhz;
        self.memory_score_gb += other.memory_score_gb;
        self.network_mbps += other.network_mbps;
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DataCenterResources {
    pub lan: ServerResources,
    pub global: ServerResources,
}

impl NetworkSim {
    pub fn management_path_reaches(&self, source: PortId, target: DeviceId) -> bool {
        let Some(device) = self.device(target) else {
            return false;
        };
        match &device.kind {
            DeviceKind::Switch(_) => self
                .switch_management(target)
                .is_some_and(|m| self.ping(source, m.address).reachable),
            DeviceKind::Server(_) => device
                .ports()
                .iter()
                .filter_map(|p| self.port(*p))
                .filter(|p| p.name == "mgmt0")
                .any(|p| match &p.config {
                    crate::PortConfig::Server(c) => c
                        .addresses()
                        .any(|a| self.ping(source, a.address).reachable),
                    _ => false,
                }),
            DeviceKind::Router(r) => r
                .interfaces
                .iter()
                .filter_map(|i| i.address)
                .any(|a| self.ping(source, a).reachable),
            _ => false,
        }
    }

    pub fn server_resources(&self, id: DeviceId) -> ServerResources {
        let Some(device) = self.device(id) else {
            return ServerResources::default();
        };
        let DeviceKind::Server(server) = &device.kind else {
            return ServerResources::default();
        };
        if !device.powered
            || server
                .hardware
                .as_ref()
                .is_some_and(|hardware| !hardware.ready())
        {
            return ServerResources::default();
        }
        let (compute_mhz, memory_score_gb) = if let Some(hardware) = &server.hardware {
            (hardware.compute_mhz(), hardware.memory_score_gb())
        } else {
            let catalog = crate::server_catalog();
            let cpu = catalog
                .parts
                .iter()
                .find_map(|part| match part.kind {
                    crate::ServerPartKind::Cpu {
                        cores,
                        frequency_mhz,
                        ..
                    } => Some(u64::from(cores) * u64::from(frequency_mhz)),
                    _ => None,
                })
                .unwrap_or(0);
            let memory = catalog
                .parts
                .iter()
                .find_map(|part| part.kind.memory_score_gb())
                .unwrap_or(0);
            (cpu, memory)
        };
        let network_mbps = server
            .ports
            .iter()
            .filter_map(|port| {
                (self.port(*port).is_some_and(|p| p.name != "mgmt0"))
                    .then(|| {
                        self.port_link_speed(*port)
                            .map(|speed| u64::from(speed.mbps()))
                    })
                    .flatten()
            })
            .sum();
        ServerResources {
            compute_mhz,
            memory_score_gb,
            network_mbps,
        }
    }

    pub fn datacenter_resources(&self) -> DataCenterResources {
        let mut totals = DataCenterResources::default();
        for device in self.devices() {
            let DeviceKind::Server(server) = &device.kind else {
                continue;
            };
            let resources = self.server_resources(device.id);
            if resources.network_mbps == 0 {
                continue;
            }
            let connected = server
                .ports
                .iter()
                .copied()
                .filter(|id| self.port(*id).is_some_and(|p| p.name != "mgmt0"))
                .any(|port| self.network_reaches(port, false));
            if connected {
                totals.lan.add(resources);
            }
            let global = server
                .ports
                .iter()
                .copied()
                .filter(|id| self.port(*id).is_some_and(|p| p.name != "mgmt0"))
                .any(|port| self.network_reaches(port, true));
            if global {
                totals.global.add(resources);
            }
        }
        totals
    }

    pub(crate) fn network_reaches(&self, source: PortId, global: bool) -> bool {
        if global {
            return self
                .ping(source, std::net::Ipv4Addr::new(198, 51, 100, 1))
                .reachable;
        }
        self.ports()
            .filter(|p| p.id != source)
            .any(|p| match &p.config {
                crate::PortConfig::Server(c) => c
                    .addresses()
                    .any(|a| self.ping(source, a.address).reachable),
                crate::PortConfig::Router(c) => c
                    .interfaces
                    .iter()
                    .filter_map(|i| i.address)
                    .any(|a| self.ping(source, a).reachable),
                _ => false,
            })
    }
}
