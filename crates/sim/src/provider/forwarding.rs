use crate::*;
use std::{cmp::Reverse, net::Ipv4Addr};

#[derive(Debug, Clone, Copy)]
struct Candidate {
    prefix: u8,
    preference: u32,
    port: PortId,
    vlan: VlanId,
    next: Ipv4Addr,
}

impl NetworkSim {
    pub(crate) fn frame_permitted(
        &mut self,
        port: PortId,
        vlan: VlanId,
        ingress: bool,
        frame: EthernetFrame,
    ) -> bool {
        match frame.payload {
            EthernetPayload::Ipv4 { packet, .. } => {
                self.packet_permitted(port, vlan, ingress, packet)
            }
            EthernetPayload::Arp(
                ArpPacket::Request {
                    sender_ip,
                    sender_mac,
                    ..
                }
                | ArpPacket::Reply {
                    sender_ip,
                    sender_mac,
                    ..
                },
            ) if ingress => {
                let permit = sender_mac == frame.source
                    && self
                        .provider
                        .policies()
                        .iter()
                        .filter(|a| a.ingress && a.policy.port == port && a.policy.vlan == vlan)
                        .all(|a| {
                            sender_ip.is_unspecified()
                                || a.policy.allowed_sources.is_empty()
                                || a.policy
                                    .allowed_sources
                                    .iter()
                                    .any(|p| p.contains(sender_ip))
                        });
                if !permit {
                    self.runtime.policy_drops += 1;
                }
                permit
            }
            _ => true,
        }
    }

    pub(crate) fn packet_permitted(
        &mut self,
        port: PortId,
        vlan: VlanId,
        ingress: bool,
        packet: Ipv4Packet,
    ) -> bool {
        let permit = self
            .provider
            .policies()
            .iter()
            .filter(|a| a.ingress == ingress && a.policy.port == port && a.policy.vlan == vlan)
            .all(|a| a.policy.permits(packet, ingress));
        if !permit {
            self.runtime.policy_drops += 1;
        }
        permit
    }

    pub(crate) fn router_route_in_domain(
        &self,
        router: DeviceId,
        source: Ipv4Addr,
        destination: Ipv4Addr,
        domain: RoutingDomain,
    ) -> Option<(PortId, VlanId, Ipv4Addr)> {
        let DeviceKind::Router(config) = &self.device(router)?.kind else {
            return None;
        };
        let mut candidates = Vec::new();
        for i in &config.interfaces {
            let vlan = i.vlan.unwrap_or(VlanId(1));
            if self.provider.domain(i.port, vlan) != domain || !self.port_link_up(i.port) {
                continue;
            }
            if i.address
                .is_some_and(|a| same_subnet(a, destination, i.prefix))
            {
                candidates.push(Candidate {
                    prefix: i.prefix,
                    preference: 0,
                    port: i.port,
                    vlan,
                    next: destination,
                });
            }
            for r in config.routes.iter().filter(|r| {
                domain == RoutingDomain(0)
                    && r.egress == i.port
                    && same_subnet(r.network, destination, r.prefix)
            }) {
                if r.via
                    .is_some_and(|via| !i.address.is_some_and(|a| same_subnet(a, via, i.prefix)))
                {
                    continue;
                }
                let preference = self
                    .provider
                    .preferences
                    .iter()
                    .find(|p| {
                        p.router == router
                            && p.egress == r.egress
                            && p.via == r.via
                            && p.prefix.network() == r.network
                            && p.prefix.length() == r.prefix
                    })
                    .map_or(1, |p| p.preference);
                candidates.push(Candidate {
                    prefix: r.prefix,
                    preference,
                    port: r.egress,
                    vlan,
                    next: r.via.unwrap_or(destination),
                });
            }
            for route in config.domain_routes.iter().filter(|r| {
                r.router == router
                    && r.domain == domain
                    && r.port == i.port
                    && r.vlan == vlan
                    && r.prefix.contains(destination)
            }) {
                if route
                    .next_hop
                    .is_some_and(|n| !i.address.is_some_and(|a| same_subnet(a, n, i.prefix)))
                {
                    continue;
                }
                if route.track_neighbor
                    && let Some(next) = route.next_hop
                {
                    let mut probe = self.clone();
                    if probe
                        .resolve_neighbor(route.port, next, route.vlan)
                        .is_err()
                    {
                        continue;
                    }
                }
                candidates.push(Candidate {
                    prefix: route.prefix.length(),
                    preference: route.preference,
                    port: route.port,
                    vlan: route.vlan,
                    next: route.next_hop.unwrap_or(destination),
                });
            }
            for session in self
                .provider
                .sessions()
                .iter()
                .filter(|s| s.port == i.port && s.vlan == vlan)
            {
                if self.bgp_state(session) != BgpState::Established {
                    continue;
                }
                let circuit = self.provider.circuit(session.circuit)?;
                for prefix in circuit.offered_routes.iter().filter(|p| {
                    p.contains(destination)
                        && session
                            .import_prefixes
                            .iter()
                            .any(|allowed| allowed.contains_prefix(**p))
                }) {
                    candidates.push(Candidate {
                        prefix: prefix.length(),
                        preference: session.preference,
                        port: i.port,
                        vlan,
                        next: circuit.address,
                    });
                }
            }
        }
        candidates.sort_by_key(|c| (Reverse(c.prefix), c.preference, c.port, c.vlan, c.next));
        candidates.dedup_by_key(|c| (c.prefix, c.preference, c.port, c.vlan, c.next));
        let best = candidates.first()?;
        let equal = candidates
            .iter()
            .take_while(|c| c.prefix == best.prefix && c.preference == best.preference)
            .count();
        // Stable per-flow choice; serialization and HashMap iteration cannot change it.
        let index =
            (u64::from(u32::from(source)) ^ u64::from(u32::from(destination))) % equal as u64;
        let selected = candidates[index as usize];
        Some((selected.port, selected.vlan, selected.next))
    }
}
