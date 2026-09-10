use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReachabilityResult {
    pub reachable: bool,
    pub hops: Vec<Hop>,
    pub failure: Option<ReachabilityFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hop {
    pub device: DeviceId,
    pub ingress: Option<PortId>,
    pub egress: Option<PortId>,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReachabilityFailure {
    SourceDown,
    NoAddress,
    NoPhysicalLink,
    VlanBlocked,
    NoGateway,
    NoRoute,
    DestinationNotFound,
    DestinationDown,
    AddressConflict,
}

impl NetworkSim {
    pub fn ping(&self, source: PortId, destination: Ipv4Addr) -> ReachabilityResult {
        match self.check_reachability(source, destination) {
            Ok(hops) => ReachabilityResult {
                reachable: true,
                hops,
                failure: None,
            },
            Err((failure, hops)) => ReachabilityResult {
                reachable: false,
                hops,
                failure: Some(failure),
            },
        }
    }

    pub fn duplicate_addresses(&self) -> Vec<(Ipv4Addr, Vec<PortId>)> {
        let mut found: HashMap<Ipv4Addr, Vec<PortId>> = HashMap::new();
        for port in self.ports.values() {
            if !self.device_active(port.device) {
                continue;
            }
            match &port.config {
                PortConfig::Server(v) => {
                    if let Some(ip) = &v.ipv4 {
                        found.entry(ip.address).or_default().push(port.id);
                    }
                }
                PortConfig::Router(v) => {
                    for iface in &v.interfaces {
                        if let Some(ip) = iface.address {
                            found.entry(ip).or_default().push(port.id);
                        }
                    }
                }
                PortConfig::Switch(_) => {}
            }
        }
        found
            .into_iter()
            .filter(|(_, ports)| ports.len() > 1)
            .collect()
    }

    fn check_reachability(
        &self,
        source: PortId,
        destination: Ipv4Addr,
    ) -> Result<Vec<Hop>, (ReachabilityFailure, Vec<Hop>)> {
        let port = self
            .ports
            .get(&source)
            .ok_or((ReachabilityFailure::SourceDown, vec![]))?;
        if !port.enabled || !self.device_active(port.device) {
            return Err((ReachabilityFailure::SourceDown, vec![]));
        }
        let config = match &port.config {
            PortConfig::Server(v) => v
                .ipv4
                .as_ref()
                .ok_or((ReachabilityFailure::NoAddress, vec![]))?,
            _ => return Err((ReachabilityFailure::NoAddress, vec![])),
        };
        if self
            .duplicate_addresses()
            .iter()
            .any(|(ip, _)| *ip == config.address || *ip == destination)
        {
            return Err((
                ReachabilityFailure::AddressConflict,
                self.one_hop(source, "duplicate IPv4 address"),
            ));
        }

        let destinations = self.server_ports_with_ip(destination);
        if config.contains(destination) {
            return self.reach_local(source, config.vlan, &destinations);
        }

        let gateway = config.gateway.ok_or((
            ReachabilityFailure::NoGateway,
            self.one_hop(source, "default gateway is not configured"),
        ))?;
        if !config.contains(gateway) {
            return Err((
                ReachabilityFailure::NoGateway,
                self.one_hop(source, "gateway is outside the local subnet"),
            ));
        }
        let gateways = self.router_interfaces_with_ip(gateway, config.vlan);
        let (router, mut path) = self.reach_router(source, config.vlan, &gateways)?;

        if destinations.is_empty() {
            let has_wan = self
                .devices
                .get(&router)
                .is_some_and(|device| match &device.kind {
                    DeviceKind::Router(v) => v.interfaces.iter().any(|i| i.internet_connected),
                    _ => false,
                });
            if has_wan && !is_private(destination) {
                path.push(Hop {
                    device: router,
                    ingress: None,
                    egress: None,
                    note: "NAT → internet".into(),
                });
                return Ok(path);
            }
            return Err((
                if is_private(destination) {
                    ReachabilityFailure::DestinationNotFound
                } else {
                    ReachabilityFailure::NoRoute
                },
                path,
            ));
        }

        let Some((target_port, target_cfg)) = destinations.iter().find_map(|id| {
            let p = self.ports.get(id)?;
            let PortConfig::Server(v) = &p.config else {
                return None;
            };
            v.ipv4.as_ref().map(|cfg| (*id, cfg))
        }) else {
            return Err((ReachabilityFailure::DestinationNotFound, path));
        };

        let router_ifaces = self
            .devices
            .get(&router)
            .and_then(|d| match &d.kind {
                DeviceKind::Router(v) => Some(&v.interfaces),
                _ => None,
            })
            .expect("router located");
        let Some(out) = router_ifaces.iter().find(|i| {
            i.address
                .is_some_and(|address| same_subnet(address, destination, i.prefix))
                && i.vlan == Some(target_cfg.vlan)
        }) else {
            return Err((ReachabilityFailure::NoRoute, path));
        };
        match self.l2_path(out.port, target_port, target_cfg.vlan) {
            Some(p) => {
                path.extend(self.hops_from_ports(&p, format!("routed VLAN {}", target_cfg.vlan.0)));
                Ok(path)
            }
            None if !self.device_active(self.ports[&target_port].device) => {
                Err((ReachabilityFailure::DestinationDown, path))
            }
            None => Err((ReachabilityFailure::VlanBlocked, path)),
        }
    }

    fn reach_local(
        &self,
        source: PortId,
        vlan: VlanId,
        destinations: &[PortId],
    ) -> Result<Vec<Hop>, (ReachabilityFailure, Vec<Hop>)> {
        if destinations.is_empty() {
            return Err((
                ReachabilityFailure::DestinationNotFound,
                self.one_hop(source, "destination not found"),
            ));
        }
        for target in destinations {
            if let Some(path) = self.l2_path(source, *target, vlan) {
                return Ok(self.hops_from_ports(&path, format!("VLAN {}", vlan.0)));
            }
        }
        let failure = if destinations
            .iter()
            .all(|p| !self.device_active(self.ports[p].device))
        {
            ReachabilityFailure::DestinationDown
        } else if destinations.iter().any(|p| self.physical_path(source, *p)) {
            ReachabilityFailure::VlanBlocked
        } else {
            ReachabilityFailure::NoPhysicalLink
        };
        Err((failure, self.one_hop(source, "local delivery failed")))
    }

    fn reach_router(
        &self,
        source: PortId,
        vlan: VlanId,
        gateways: &[(DeviceId, PortId)],
    ) -> Result<(DeviceId, Vec<Hop>), (ReachabilityFailure, Vec<Hop>)> {
        for (router, port) in gateways {
            if let Some(path) = self.l2_path(source, *port, vlan) {
                return Ok((
                    *router,
                    self.hops_from_ports(&path, format!("gateway VLAN {}", vlan.0)),
                ));
            }
        }
        let failure = if gateways.is_empty() {
            ReachabilityFailure::NoGateway
        } else if gateways.iter().any(|(_, p)| self.physical_path(source, *p)) {
            ReachabilityFailure::VlanBlocked
        } else {
            ReachabilityFailure::NoPhysicalLink
        };
        Err((failure, self.one_hop(source, "gateway unreachable")))
    }

    fn device_active(&self, id: DeviceId) -> bool {
        self.devices
            .get(&id)
            .is_some_and(|v| v.powered && v.rack.is_some())
    }

    fn server_ports_with_ip(&self, address: Ipv4Addr) -> Vec<PortId> {
        self.ports
            .values()
            .filter_map(|p| match &p.config {
                PortConfig::Server(v) if v.ipv4.as_ref().is_some_and(|i| i.address == address) => {
                    Some(p.id)
                }
                _ => None,
            })
            .collect()
    }

    fn router_interfaces_with_ip(
        &self,
        address: Ipv4Addr,
        vlan: VlanId,
    ) -> Vec<(DeviceId, PortId)> {
        self.ports
            .values()
            .filter_map(|p| match &p.config {
                PortConfig::Router(v)
                    if v.interfaces
                        .iter()
                        .any(|i| i.address == Some(address) && i.vlan == Some(vlan))
                        && self.device_active(p.device) =>
                {
                    Some((p.device, p.id))
                }
                _ => None,
            })
            .collect()
    }

    fn port_carries(&self, id: PortId, vlan: VlanId) -> bool {
        let Some(port) = self.ports.get(&id) else {
            return false;
        };
        if !port.enabled || !self.device_active(port.device) {
            return false;
        }
        match &port.config {
            PortConfig::Server(v) => v.ipv4.as_ref().is_some_and(|i| i.vlan == vlan),
            PortConfig::Switch(v) => v.mode.carries(vlan),
            PortConfig::Router(v) => v.interfaces.iter().any(|i| i.vlan == Some(vlan)),
        }
    }

    fn l2_path(&self, from: PortId, to: PortId, vlan: VlanId) -> Option<Vec<PortId>> {
        self.bfs_ports(from, to, |sim, p| sim.port_carries(p, vlan), true)
    }

    fn physical_path(&self, from: PortId, to: PortId) -> bool {
        self.bfs_ports(
            from,
            to,
            |sim, p| {
                sim.ports
                    .get(&p)
                    .is_some_and(|v| v.enabled && sim.device_active(v.device))
            },
            false,
        )
        .is_some()
    }

    fn bfs_ports<F>(
        &self,
        from: PortId,
        to: PortId,
        allowed: F,
        vlan_aware: bool,
    ) -> Option<Vec<PortId>>
    where
        F: Fn(&Self, PortId) -> bool,
    {
        if !allowed(self, from) || !allowed(self, to) {
            return None;
        }
        let mut queue = VecDeque::from([from]);
        let mut previous = HashMap::new();
        let mut visited = HashSet::from([from]);
        while let Some(current) = queue.pop_front() {
            if current == to {
                let mut path = vec![to];
                let mut cursor = to;
                while cursor != from {
                    cursor = previous[&cursor];
                    path.push(cursor);
                }
                path.reverse();
                return Some(path);
            }
            let mut next = Vec::new();
            if let Some(link) = self.link_for_port(current).filter(|v| v.enabled)
                && let Some(other) = link.other(current)
            {
                next.push(other);
            }
            if let Some(port) = self.ports.get(&current)
                && let Some(Device {
                    kind: DeviceKind::Switch(sw),
                    ..
                }) = self.devices.get(&port.device)
                && (!vlan_aware || sw.vlans.iter().any(|v| self.port_carries(current, v.id)))
            {
                next.extend(sw.ports.iter().copied().filter(|p| *p != current));
            }
            for candidate in next {
                if allowed(self, candidate) && visited.insert(candidate) {
                    previous.insert(candidate, current);
                    queue.push_back(candidate);
                }
            }
        }
        None
    }

    fn one_hop(&self, port: PortId, note: impl Into<String>) -> Vec<Hop> {
        self.ports
            .get(&port)
            .map(|p| {
                vec![Hop {
                    device: p.device,
                    ingress: None,
                    egress: Some(port),
                    note: note.into(),
                }]
            })
            .unwrap_or_default()
    }

    fn hops_from_ports(&self, ports: &[PortId], note: String) -> Vec<Hop> {
        let mut hops = Vec::new();
        for port in ports {
            let Some(p) = self.ports.get(port) else {
                continue;
            };
            if hops.last().is_some_and(|h: &Hop| h.device == p.device) {
                if let Some(last) = hops.last_mut() {
                    last.egress = Some(*port);
                }
            } else {
                hops.push(Hop {
                    device: p.device,
                    ingress: Some(*port),
                    egress: Some(*port),
                    note: note.clone(),
                });
            }
        }
        hops
    }
}

fn is_private(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_loopback() || ip.is_link_local()
}
