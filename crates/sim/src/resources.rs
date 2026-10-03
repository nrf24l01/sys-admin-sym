use crate::{DeviceId, DeviceKind, NetworkOutletKind, NetworkSim, PortId};
use std::collections::{HashSet, VecDeque};

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
    /// Whether a router's chosen WAN interface has a live physical path to a room uplink.
    pub(crate) fn wan_reaches_uplink(&self, source: PortId) -> bool {
        let mut queue = VecDeque::from([source]);
        let mut seen = HashSet::new();
        while let Some(port) = queue.pop_front() {
            if !seen.insert(port) || !self.port_link_up(port) {
                continue;
            }
            for endpoint in self.physical_path(port) {
                if self
                    .network_outlet(endpoint)
                    .is_some_and(|outlet| matches!(outlet.kind, NetworkOutletKind::Uplink { .. }))
                {
                    return true;
                }
                if let Some(owner) = self.port(endpoint).and_then(|p| self.device(p.device))
                    && matches!(owner.kind, DeviceKind::Switch(_))
                {
                    queue.extend(
                        owner
                            .ports()
                            .iter()
                            .copied()
                            .filter(|id| self.port_link_up(*id)),
                    );
                }
            }
        }
        false
    }

    pub fn management_path_reaches(&self, source: PortId, target: DeviceId) -> bool {
        let mut queue = VecDeque::from([source]);
        let mut seen = HashSet::new();
        while let Some(port) = queue.pop_front() {
            if !seen.insert(port) || !self.port_link_up(port) {
                continue;
            }
            for endpoint in self.physical_path(port) {
                if let Some(outlet) = self.network_outlet(endpoint) {
                    if matches!(outlet.kind, NetworkOutletKind::Lan { .. }) {
                        queue.extend(
                            self.network_outlets
                                .iter()
                                .filter(|other| {
                                    matches!(other.kind, NetworkOutletKind::Lan { .. })
                                        && self.port_link_up(other.port)
                                })
                                .map(|other| other.port),
                        );
                    }
                    continue;
                }
                let Some(owner) = self.port(endpoint).and_then(|p| self.device(p.device)) else {
                    continue;
                };
                if owner.id == target && owner.powered {
                    return true;
                }
                if matches!(owner.kind, DeviceKind::Switch(_)) {
                    queue.extend(
                        owner
                            .ports()
                            .iter()
                            .copied()
                            .filter(|id| self.port_link_up(*id)),
                    );
                }
            }
        }
        false
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
        let mut queue = VecDeque::from([source]);
        let mut seen = HashSet::new();
        while let Some(port) = queue.pop_front() {
            if !seen.insert(port) {
                continue;
            }
            let path = self.physical_path(port);
            for endpoint in path {
                if let Some(outlet) = self.network_outlet(endpoint) {
                    if matches!(outlet.kind, NetworkOutletKind::Uplink { .. }) == global {
                        return true;
                    }
                    if matches!(outlet.kind, NetworkOutletKind::Lan { .. }) {
                        queue.extend(
                            self.network_outlets
                                .iter()
                                .filter(|other| {
                                    matches!(other.kind, NetworkOutletKind::Lan { .. })
                                        && self.port_link_up(other.port)
                                })
                                .map(|other| other.port),
                        );
                    }
                    continue;
                }
                let Some(p) = self.port(endpoint) else {
                    continue;
                };
                let Some(device) = self.device(p.device) else {
                    continue;
                };
                if matches!(device.kind, DeviceKind::Switch(_) | DeviceKind::Router(_)) {
                    queue.extend(
                        device
                            .ports()
                            .iter()
                            .copied()
                            .filter(|id| self.port_link_up(*id)),
                    );
                }
            }
        }
        false
    }
}
