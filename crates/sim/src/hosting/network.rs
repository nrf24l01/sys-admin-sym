use super::*;
use std::collections::BTreeMap;

impl NetworkSim {
    /// Test whether stopped guests could use their network after a customer start.
    /// A single read-only probe shares the existing packet/return-route engine;
    /// real guest power, utilization and traffic counters remain unchanged.
    pub(crate) fn hosting_restart_network_status(&self, ids: &[u64]) -> BTreeMap<u64, bool> {
        if ids.is_empty() {
            return BTreeMap::new();
        }
        let mut probe = self.clone();
        let mut flows = Vec::with_capacity(ids.len());
        for id in ids {
            let vm = probe.hosting.vms.get_mut(id).unwrap();
            vm.power = VmPower::Running;
            if let Some(source) = vm.interfaces.first().copied() {
                flows.push(TrafficFlow {
                    id: *id,
                    source,
                    source_ipv4: None,
                    destination: std::net::Ipv4Addr::new(198, 51, 100, 1),
                    demand_mbps: 0.0,
                    limit_mbps: 0.0,
                });
            }
        }
        probe.topology_revision += 1;
        probe
            .simulate_traffic(&flows, 0.0)
            .into_iter()
            .map(|(id, traffic)| (id, traffic.reachable))
            .collect()
    }
    /// Keep installed hosting controllers and prepared nodes connected when retiring
    /// the old synthetic management NIC. Other hardware keeps main's normal migration.
    pub(crate) fn migrate_hosting_management_ports(&mut self) {
        let mut legacy: Vec<_> = self
            .ports
            .values()
            .filter(|p| {
                p.name == "mgmt0"
                    && (self.hosting.nodes.contains_key(&p.device)
                        || self.hosting_provider_installed(p.device))
            })
            .map(|p| (p.id, p.device))
            .collect();
        legacy.sort_unstable();
        for (old, host) in legacy {
            let Some(new) = self.devices[&host].ports().iter().copied().find(|id| {
                self.ports.get(id).is_some_and(|p| p.name != "mgmt0" && p.connector == PortConnector::Rj45
                    && matches!(&p.config, PortConfig::Server(c) if c.addresses().next().is_none()))
                    && !self.links.values().any(|l| l.a == *id || l.b == *id)
            }) else { continue; };
            let config = self.ports[&old].config.clone();
            let enabled = self.ports[&old].enabled;
            let name = self.ports[&new].name.clone();
            self.ports.get_mut(&new).unwrap().config = config;
            self.ports.get_mut(&new).unwrap().enabled = enabled;
            for link in self.links.values_mut() {
                if link.a == old {
                    link.a = new;
                }
                if link.b == old {
                    link.b = new;
                }
            }
            if let Some(link) = self.port_links.remove(&old) {
                self.port_links.insert(new, link);
            }
            if let Some(os) = self.server_operating_systems.get_mut(&host) {
                for route in &mut os.routes {
                    if route.port == old {
                        route.port = new;
                    }
                }
                if let Ok(config) = os.filesystem.read("/etc/network/interfaces") {
                    let config = config.replace("mgmt0", &name);
                    let _ = os
                        .filesystem
                        .write("/etc/network/interfaces", &config, false);
                }
            }
        }
    }

    pub(crate) fn rebuild_hosting_indexes(&mut self) {
        if self.hosting.clock_defaults_version < HOSTING_CLOCK_DEFAULTS_VERSION {
            // Upgrade the former one-minute default once; keep custom rates and pause.
            if self.hosting.market.time_scale == 60 {
                self.hosting.market.time_scale = DEFAULT_HOSTING_TIME_SCALE;
            }
            self.hosting.clock_defaults_version = HOSTING_CLOCK_DEFAULTS_VERSION;
        }
        if let Some(legacy) = self.hosting.provider_server.take() {
            self.hosting
                .provider_site_ipv4
                .get_or_insert(legacy.public_ipv4);
        }
        self.hosting.bridge_members.clear();
        self.hosting.order_services = self
            .hosting
            .payments
            .iter()
            .filter(|p| !p.renewal)
            .map(|p| (p.order, p.service))
            .collect();
        for (port, uplink) in &self.hosting.bridges {
            self.hosting
                .bridge_members
                .entry(*uplink)
                .or_default()
                .insert(MacAddress::for_port(*port), *port);
        }
    }
    pub(crate) fn add_host_bridge(&mut self, port: PortId, uplink: PortId) {
        self.hosting.bridges.insert(port, uplink);
        self.hosting
            .bridge_members
            .entry(uplink)
            .or_default()
            .insert(MacAddress::for_port(port), port);
    }
    pub(crate) fn remove_host_bridge(&mut self, port: PortId) {
        if let Some(uplink) = self.hosting.bridges.remove(&port)
            && let Some(members) = self.hosting.bridge_members.get_mut(&uplink)
        {
            members.remove(&MacAddress::for_port(port));
        }
    }

    pub(crate) fn virtual_uplink(&self, port: PortId) -> Option<PortId> {
        self.hosting.bridges.get(&port).copied()
    }
    pub(crate) fn virtual_port_running(&self, port: PortId) -> bool {
        self.port(port)
            .and_then(|p| p.name.strip_prefix("vnet"))
            .and_then(|s| s.parse::<u64>().ok())
            .and_then(|id| self.hosting.vms.get(&id))
            .is_some_and(|vm| vm.power == VmPower::Running)
    }
    pub(crate) fn virtual_link_status(&self, port: PortId) -> Option<LinkStatus> {
        let uplink = self.virtual_uplink(port)?;
        if !self.virtual_port_running(port) || !self.port(port).is_some_and(|p| p.enabled) {
            return Some(LinkStatus::down(LinkFault::Disabled, vec![port]));
        }
        let mut status = self.link_status(uplink);
        status.path.insert(0, port);
        Some(status)
    }
    pub(crate) fn bridge_deliveries(
        &mut self,
        uplink: PortId,
        frame: EthernetFrame,
        vlan: VlanId,
        path: &[PortId],
    ) -> Vec<FrameDelivery> {
        let ports: Vec<_> = self
            .hosting
            .bridge_members
            .get(&uplink)
            .map(|members| {
                if frame.destination == MacAddress([255; 6]) {
                    members.values().copied().collect()
                } else {
                    members
                        .get(&frame.destination)
                        .copied()
                        .into_iter()
                        .collect()
                }
            })
            .unwrap_or_default();
        ports
            .into_iter()
            .filter_map(|port| {
                if !self.virtual_port_running(port) {
                    return None;
                }
                let PortConfig::Server(config) = &self.port(port)?.config else {
                    return None;
                };
                if config
                    .ipv4
                    .as_ref()
                    .map_or(VlanId(1), |i| i.vlan.unwrap_or(VlanId(1)))
                    != vlan
                {
                    return None;
                }
                self.runtime.rx(port);
                let mut path = path.to_vec();
                path.push(port);
                Some(FrameDelivery {
                    port,
                    frame,
                    vlan,
                    path,
                })
            })
            .collect()
    }
    pub(crate) fn virtual_route(
        &self,
        port: PortId,
        destination: std::net::Ipv4Addr,
    ) -> Option<RouteSelection> {
        self.virtual_uplink(port)?;
        let PortConfig::Server(config) = &self.port(port)?.config else {
            return None;
        };
        let source = config
            .addresses()
            .find(|a| a.contains(destination))
            .or(config.ipv4.as_ref())?
            .clone();
        let (next_hop, prefix) = if source.contains(destination) {
            (destination, source.prefix)
        } else {
            (source.gateway?, 0)
        };
        Some(RouteSelection {
            port,
            source,
            next_hop,
            prefix,
            metric: 0,
        })
    }
}
