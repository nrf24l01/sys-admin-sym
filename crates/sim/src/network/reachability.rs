use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    TtlExpired,
}

impl NetworkSim {
    pub fn transmit_icmp(&mut self, source: PortId, destination: Ipv4Addr) -> ReachabilityResult {
        self.ping_ttl_mut(source, destination, 64)
    }
    pub fn ping(&self, source: PortId, destination: Ipv4Addr) -> ReachabilityResult {
        let mut s = self.clone();
        s.transmit_icmp(source, destination)
    }
    pub fn ping_with_ttl(
        &self,
        source: PortId,
        destination: Ipv4Addr,
        ttl: u8,
    ) -> ReachabilityResult {
        let mut s = self.clone();
        s.ping_ttl_mut(source, destination, ttl)
    }
    fn ping_ttl_mut(
        &mut self,
        source: PortId,
        destination: Ipv4Addr,
        ttl: u8,
    ) -> ReachabilityResult {
        let bad = |f, h| ReachabilityResult {
            reachable: false,
            hops: h,
            failure: Some(f),
        };
        if !self.port_up(source) {
            let powered = self
                .port(source)
                .and_then(|port| self.device(port.device))
                .is_some_and(|device| device.powered);
            return bad(
                if self.link_for_port(source).is_some() && !powered {
                    ReachabilityFailure::SourceDown
                } else {
                    ReachabilityFailure::NoPhysicalLink
                },
                vec![],
            );
        }
        let Some(device) = self.port(source).map(|port| port.device) else {
            return bad(ReachabilityFailure::SourceDown, vec![]);
        };
        let Some(route) = self.server_route_selection(device, destination, Some(source)) else {
            return bad(
                if self.server_ipv4(source).is_none() {
                    ReachabilityFailure::NoAddress
                } else {
                    ReachabilityFailure::NoGateway
                },
                vec![],
            );
        };
        let src = route.source;
        let source_vlan = src.vlan.unwrap_or(VlanId(1));
        if self.address_conflict(source, source_vlan, src.address) {
            return bad(ReachabilityFailure::AddressConflict, vec![]);
        }
        let next = route.next_hop;
        let mut hops = vec![];
        let Some(mac) = self.resolve_arp(source, next, source_vlan, &mut hops) else {
            return bad(self.local_failure(source, next), hops);
        };
        let p = Ipv4Packet {
            source: src.address,
            destination,
            ttl,
            protocol: 1,
        };
        let ok = self.deliver(
            source,
            source_vlan,
            mac,
            p,
            IcmpMessage::EchoRequest {
                identifier: 1,
                sequence: 1,
            },
            &mut hops,
            0,
        );
        if ok {
            ReachabilityResult {
                reachable: true,
                hops,
                failure: None,
            }
        } else {
            bad(
                if hops.iter().any(|h| h.note.contains("TTL")) {
                    ReachabilityFailure::TtlExpired
                } else if self.server_ports_with_ip(destination).iter().any(|p| {
                    self.server_ipv4(*p)
                        .is_some_and(|i| !i.contains(src.address) && i.gateway.is_none())
                }) {
                    ReachabilityFailure::NoGateway
                } else {
                    ReachabilityFailure::DestinationNotFound
                },
                hops,
            )
        }
    }
    pub fn ping_router(&self, router: DeviceId, destination: Ipv4Addr) -> ReachabilityResult {
        let mut s = self.clone();
        s.ping_router_mut(router, destination)
    }
    pub fn ping_router_mut(
        &mut self,
        router: DeviceId,
        destination: Ipv4Addr,
    ) -> ReachabilityResult {
        self.ping_router_domain_mut(router, RoutingDomain(0), destination)
    }

    pub fn ping_router_domain_mut(
        &mut self,
        router: DeviceId,
        domain: RoutingDomain,
        destination: Ipv4Addr,
    ) -> ReachabilityResult {
        let s = self;
        let Some((port, vlan, next)) =
            s.router_route_in_domain(router, Ipv4Addr::UNSPECIFIED, destination, domain)
        else {
            return ReachabilityResult {
                reachable: false,
                hops: vec![],
                failure: Some(ReachabilityFailure::NoRoute),
            };
        };
        let Some(address) = s.interface_ipv4(port, vlan) else {
            return ReachabilityResult {
                reachable: false,
                hops: vec![],
                failure: Some(ReachabilityFailure::NoAddress),
            };
        };
        let mut h = vec![Hop {
            device: router,
            ingress: None,
            egress: Some(port),
            note: "router-originated echo request".into(),
        }];
        let Some(mac) = s.resolve_arp(port, next, vlan, &mut h) else {
            return ReachabilityResult {
                reachable: false,
                hops: h,
                failure: Some(ReachabilityFailure::DestinationNotFound),
            };
        };
        let p = Ipv4Packet {
            source: address,
            destination,
            ttl: 64,
            protocol: 1,
        };
        let ok = s.deliver(
            port,
            vlan,
            mac,
            p,
            IcmpMessage::EchoRequest {
                identifier: 1,
                sequence: 1,
            },
            &mut h,
            0,
        );
        ReachabilityResult {
            reachable: ok,
            hops: h,
            failure: (!ok).then_some(ReachabilityFailure::DestinationNotFound),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn deliver(
        &mut self,
        out: PortId,
        vlan: VlanId,
        mac: MacAddress,
        p: Ipv4Packet,
        icmp: IcmpMessage,
        h: &mut Vec<Hop>,
        depth: u8,
    ) -> bool {
        if depth > 32 {
            return false;
        }
        let f = EthernetFrame {
            source: MacAddress::for_port(out),
            destination: mac,
            vlan: self.wire_vlan_for(out, vlan),
            payload: EthernetPayload::Ipv4 { packet: p, icmp },
        };
        let deliveries = self.transmit_frame(out, f);
        for d in deliveries {
            for path_port in &d.path {
                if let Some(port) = self.port(*path_port) {
                    let outgoing = self.link_for_port(*path_port).is_some_and(|l| {
                        d.path
                            .windows(2)
                            .any(|pair| pair[0] == *path_port && l.other(pair[0]) == Some(pair[1]))
                    });
                    self.hop(
                        h,
                        port.device,
                        (!outgoing).then_some(*path_port),
                        outgoing.then_some(*path_port),
                        "Ethernet forwarding",
                    );
                }
            }
            let Some(port) = self.port(d.port).cloned() else {
                continue;
            };
            self.hop(h, port.device, Some(d.port), None, "IPv4 delivery");
            match port.config {
                PortConfig::Server(_) => {
                    if self.interface_has_ipv4(d.port, d.vlan, p.destination) {
                        match icmp {
                            IcmpMessage::EchoRequest {
                                identifier,
                                sequence,
                            } => {
                                let Some(route) = self.server_route_in_domain(
                                    port.device,
                                    p.source,
                                    None,
                                    Some(self.provider.domain(d.port, d.vlan)),
                                ) else {
                                    continue;
                                };
                                let interface_vlan = route.source.vlan.unwrap_or(VlanId(1));
                                let Some(m) =
                                    self.resolve_arp(route.port, route.next_hop, interface_vlan, h)
                                else {
                                    continue;
                                };
                                let rp = Ipv4Packet {
                                    source: p.destination,
                                    destination: p.source,
                                    ttl: 64,
                                    protocol: 1,
                                };
                                if self.deliver(
                                    route.port,
                                    interface_vlan,
                                    m,
                                    rp,
                                    IcmpMessage::EchoReply {
                                        identifier,
                                        sequence,
                                    },
                                    h,
                                    depth + 1,
                                ) {
                                    return true;
                                }
                            }
                            IcmpMessage::EchoReply { .. } => return true,
                            _ => {}
                        }
                    }
                }
                PortConfig::Router(_) => {
                    if self.route_router(port.device, d.port, d.vlan, p, icmp, h, depth) {
                        return true;
                    }
                }
                PortConfig::Switch(_) => {
                    if self.switch_management_delivery(d.port, d.vlan, p, icmp, h, depth) {
                        return true;
                    }
                }
                PortConfig::Infrastructure => {
                    if self.transit_delivery(d.port, p, icmp, h, depth) {
                        return true;
                    }
                }
                PortConfig::PatchPanel | PortConfig::CableManager => {}
            }
        }
        false
    }
    #[allow(clippy::too_many_arguments)]
    fn route_router(
        &mut self,
        r: DeviceId,
        ing: PortId,
        ingress_vlan: VlanId,
        p: Ipv4Packet,
        icmp: IcmpMessage,
        h: &mut Vec<Hop>,
        depth: u8,
    ) -> bool {
        let domain = self.provider.domain(ing, ingress_vlan);
        let is = self.router_interfaces(r);
        if is
            .iter()
            .find(|(_, i)| {
                i.address == Some(p.destination)
                    && self.provider.domain(i.port, i.vlan.unwrap_or(VlanId(1))) == domain
            })
            .is_some()
        {
            if matches!(icmp, IcmpMessage::EchoReply { .. }) {
                return true;
            }
            if let IcmpMessage::EchoRequest {
                identifier,
                sequence,
            } = icmp
            {
                let Some((ro, rv, rn)) =
                    self.router_route_in_domain(r, p.destination, p.source, domain)
                else {
                    return false;
                };
                let Some(m) = self.resolve_arp(ro, rn, rv, h) else {
                    return false;
                };
                let rp = Ipv4Packet {
                    source: p.destination,
                    destination: p.source,
                    ttl: 64,
                    protocol: 1,
                };
                return self.deliver(
                    ro,
                    rv,
                    m,
                    rp,
                    IcmpMessage::EchoReply {
                        identifier,
                        sequence,
                    },
                    h,
                    depth + 1,
                );
            }
        }
        if p.ttl <= 1 {
            self.hop(h, r, Some(ing), None, "TTL expired");
            return false;
        }
        let Some((o, v, n)) = self.router_route_in_domain(r, p.source, p.destination, domain)
        else {
            return false;
        };
        let Some(m) = self.resolve_arp(o, n, v, h) else {
            return false;
        };
        self.hop(h, r, Some(ing), Some(o), format!("routed VLAN {}", v.0));
        let mut fp = p;
        fp.ttl -= 1;
        self.deliver(o, v, m, fp, icmp, h, depth + 1)
    }
    fn resolve_arp(
        &mut self,
        port: PortId,
        target: Ipv4Addr,
        vlan: VlanId,
        h: &mut Vec<Hop>,
    ) -> Option<MacAddress> {
        let m = self.resolve_neighbor(port, target, vlan).ok()?;
        h.push(Hop {
            device: self.ports[&port].device,
            ingress: None,
            egress: Some(port),
            note: "ARP resolved".into(),
        });
        Some(m)
    }
    fn server_ipv4(&self, p: PortId) -> Option<Ipv4InterfaceConfig> {
        match &self.ports.get(&p)?.config {
            PortConfig::Server(c) => c.ipv4.clone(),
            _ => None,
        }
    }
    fn port_vlan_available(&self, p: PortId, vlan: VlanId) -> bool {
        let Some(peer) = self.link_for_port(p).and_then(|l| l.other(p)) else {
            return false;
        };
        match &self.ports.get(&peer).map(|x| &x.config) {
            Some(PortConfig::Switch(c)) => c.mode.carries(vlan),
            _ => true,
        }
    }
    fn port_ipv4(&self, p: PortId) -> Option<Ipv4Addr> {
        self.server_ipv4(p)
            .map(|x| x.address)
            .or_else(|| match &self.ports.get(&p)?.config {
                PortConfig::Router(c) => c.interfaces.iter().find_map(|i| i.address),
                _ => None,
            })
            .or_else(|| {
                self.router_interfaces(self.ports.get(&p)?.device)
                    .into_iter()
                    .find(|(id, _)| *id == p)
                    .and_then(|(_, i)| i.address)
            })
    }
    fn router_interfaces(&self, d: DeviceId) -> Vec<(PortId, RouterInterface)> {
        match self.device(d).map(|x| &x.kind) {
            Some(DeviceKind::Router(r)) => {
                r.interfaces.iter().map(|i| (i.port, i.clone())).collect()
            }
            _ => vec![],
        }
    }
    fn router_interfaces_all(&self) -> Vec<(PortId, RouterInterface)> {
        self.devices
            .keys()
            .flat_map(|id| self.router_interfaces(*id))
            .collect()
    }
    pub(crate) fn port_up(&self, p: PortId) -> bool {
        self.ports
            .get(&p)
            .is_some_and(|x| x.enabled && self.device_active(x.device) && self.port_link_up(p))
    }
    fn device_active(&self, d: DeviceId) -> bool {
        self.devices.get(&d).is_some_and(|x| {
            x.rack.is_some()
                && (x.powered
                    || matches!(
                        x.kind,
                        DeviceKind::PatchPanel(_) | DeviceKind::CableManager(_)
                    ))
        })
    }
    fn hop(
        &self,
        h: &mut Vec<Hop>,
        d: DeviceId,
        i: Option<PortId>,
        e: Option<PortId>,
        n: impl Into<String>,
    ) {
        if let Some(x) = h.last_mut().filter(|x| x.device == d) {
            x.ingress = i.or(x.ingress);
            x.egress = e.or(x.egress);
            x.note = n.into()
        } else {
            h.push(Hop {
                device: d,
                ingress: i,
                egress: e,
                note: n.into(),
            })
        }
    }
    fn local_failure(&self, p: PortId, target: Ipv4Addr) -> ReachabilityFailure {
        if self.server_ports_with_ip(target).is_empty() {
            let vlan = self.server_ipv4(p).map(|i| i.vlan.unwrap_or(VlanId(1)));
            let router_target = self
                .router_interfaces_all()
                .iter()
                .any(|(_, i)| i.address == Some(target));
            if (router_target
                && vlan.is_some_and(|v| {
                    !self.router_interfaces_all().into_iter().any(|(rp, i)| {
                        i.vlan.unwrap_or(VlanId(1)) == v && self.port_vlan_available(rp, v)
                    })
                }))
                || (!target.is_private()
                    && vlan.is_some_and(|v| {
                        !self.router_interfaces_all().into_iter().any(|(rp, i)| {
                            i.vlan.unwrap_or(VlanId(1)) == v && self.port_vlan_available(rp, v)
                        })
                    }))
            {
                ReachabilityFailure::VlanBlocked
            } else {
                ReachabilityFailure::DestinationNotFound
            }
        } else if !self.port_link_up(p) {
            ReachabilityFailure::NoPhysicalLink
        } else {
            ReachabilityFailure::VlanBlocked
        }
    }
    fn server_ports_with_ip(&self, ip: Ipv4Addr) -> Vec<PortId> {
        self.ports
            .values()
            .filter_map(|p| self.address_config(p.id, ip).map(|_| p.id))
            .filter(|p| self.port_up(*p))
            .collect()
    }
    pub fn duplicate_addresses(&self) -> Vec<(Ipv4Addr, Vec<PortId>)> {
        let mut m = HashMap::new();
        for p in self.ports.values() {
            if let PortConfig::Server(config) = &p.config {
                for ip in config.addresses() {
                    m.entry(ip.address).or_insert_with(Vec::new).push(p.id);
                }
            } else if let Some(ip) = self.port_ipv4(p.id) {
                m.entry(ip).or_insert_with(Vec::new).push(p.id)
            }
        }
        m.into_iter()
            .filter(|(_, v): &(_, Vec<_>)| v.len() > 1)
            .collect()
    }
}
