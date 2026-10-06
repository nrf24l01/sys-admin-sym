use crate::*;
use std::net::Ipv4Addr;

impl NetworkSim {
    pub fn bgp_state(&self, session: &BgpSession) -> BgpState {
        if !session.enabled {
            return BgpState::Disabled;
        }
        let Some(circuit) = self.provider.circuit(session.circuit) else {
            return BgpState::PeerMismatch;
        };
        if !circuit.enabled || !self.port_link_up(session.port) || !self.port_link_up(circuit.port)
        {
            return BgpState::LinkDown;
        }
        if session.peer_asn != circuit.asn || session.local_asn == session.peer_asn {
            return BgpState::PeerMismatch;
        }
        if circuit
            .offered_routes
            .iter()
            .filter(|prefix| {
                session
                    .import_prefixes
                    .iter()
                    .any(|allowed| allowed.contains_prefix(**prefix))
            })
            .count()
            > session.max_prefixes
        {
            return BgpState::PrefixLimit;
        }
        let Some(local) = self.interface_ipv4(session.port, session.vlan) else {
            return BgpState::PeerMismatch;
        };
        if !Ipv4Prefix::new(circuit.address, circuit.prefix)
            .is_ok_and(|p| p.usable(local) && local != circuit.address)
        {
            return BgpState::PeerMismatch;
        }
        let mut probe = self.clone();
        if probe
            .resolve_neighbor(session.port, circuit.address, session.vlan)
            .ok()
            != Some(MacAddress::for_port(circuit.port))
        {
            return BgpState::LinkDown;
        }
        BgpState::Established
    }

    pub fn bgp_export_accepted(&self, session: &BgpSession, prefix: Ipv4Prefix) -> bool {
        if prefix.length() > 24
            || !Self::simulated_public_address(prefix.network())
            || !self.provider.owns(prefix)
            || !session.export_prefixes.contains(&prefix)
        {
            return false;
        }
        let Some(circuit) = self.provider.circuit(session.circuit) else {
            return false;
        };
        if !circuit.authorizations.iter().any(|a| {
            a.origin_asn == session.local_asn
                && a.prefix.contains_prefix(prefix)
                && prefix.length() <= a.max_length
        }) {
            return false;
        }
        // An announcement needs a route in the originating router's own table.
        let Some(device) = self.port(session.port).and_then(|p| self.device(p.device)) else {
            return false;
        };
        let DeviceKind::Router(router) = &device.kind else {
            return false;
        };
        let domain = self.provider.domain(session.port, session.vlan);
        router.interfaces.iter().any(|i| {
            i.address.is_some_and(|ip| prefix.contains(ip))
                && i.prefix == prefix.length()
                && self.provider.domain(i.port, i.vlan.unwrap_or(VlanId(1))) == domain
                && self.port_link_up(i.port)
        }) || (domain == RoutingDomain(0)
            && router.routes.iter().any(|r| {
                r.network == prefix.network()
                    && r.prefix == prefix.length()
                    && self.port_link_up(r.egress)
                    && router.interfaces.iter().any(|i| {
                        i.port == r.egress
                            && self.provider.domain(i.port, i.vlan.unwrap_or(VlanId(1))) == domain
                    })
            }))
            || router.domain_routes.iter().any(|r| {
                r.router == device.id
                    && r.domain == domain
                    && r.prefix == prefix
                    && self.port_link_up(r.port)
            })
    }

    /// An upstream has its own routing table. Static contracts and accepted BGP
    /// exports install routes to the provider edge, never directly to a server MAC.
    pub(crate) fn upstream_route(
        &self,
        circuit_port: PortId,
        destination: Ipv4Addr,
    ) -> Option<UpstreamRoute> {
        let circuit = self.provider.circuit(circuit_port).filter(|c| c.enabled)?;
        let mut candidates: Vec<_> = circuit
            .routes
            .iter()
            .copied()
            .filter(|r| r.prefix.contains(destination))
            .collect();
        if let Ok(prefix) = Ipv4Prefix::new(circuit.address, circuit.prefix)
            && prefix.usable(destination)
            && destination != circuit.address
        {
            candidates.push(UpstreamRoute {
                prefix,
                next_hop: destination,
            });
        }
        for session in self
            .provider
            .sessions()
            .iter()
            .filter(|s| s.circuit == circuit_port)
        {
            if self.bgp_state(session) != BgpState::Established {
                continue;
            }
            let Some(next_hop) = self.interface_ipv4(session.port, session.vlan) else {
                continue;
            };
            for prefix in session
                .export_prefixes
                .iter()
                .copied()
                .filter(|p| p.contains(destination) && self.bgp_export_accepted(session, *p))
            {
                candidates.push(UpstreamRoute { prefix, next_hop });
            }
        }
        candidates.sort_by_key(|r| (std::cmp::Reverse(r.prefix.length()), r.next_hop));
        candidates.into_iter().next()
    }

    pub(crate) fn transit_delivery(
        &mut self,
        circuit: PortId,
        packet: Ipv4Packet,
        icmp: IcmpMessage,
        hops: &mut Vec<Hop>,
        depth: u8,
    ) -> bool {
        let gateway_echo = self
            .provider
            .circuit(circuit)
            .is_some_and(|c| c.enabled && c.address == packet.destination);
        if !gateway_echo
            && (!Self::simulated_public_address(packet.source)
                || !Self::simulated_public_address(packet.destination))
        {
            return false;
        }
        // Return routing is required even for a request originating inside the DC.
        let Some(route) = self.upstream_route(circuit, packet.source) else {
            return false;
        };
        if matches!(icmp, IcmpMessage::EchoReply { .. }) {
            return gateway_echo || self.provider.external.available(packet.destination);
        }
        if !gateway_echo && !self.provider.external.responds(packet.destination) {
            return false;
        }
        let IcmpMessage::EchoRequest {
            identifier,
            sequence,
        } = icmp
        else {
            return false;
        };
        let Ok(mac) = self.resolve_neighbor(circuit, route.next_hop, VlanId(1)) else {
            return false;
        };
        hops.push(Hop {
            device: NetworkOutlet::OWNER,
            ingress: Some(circuit),
            egress: Some(circuit),
            note: "upstream routed echo reply".into(),
        });
        self.deliver(
            circuit,
            VlanId(1),
            mac,
            Ipv4Packet {
                source: packet.destination,
                destination: packet.source,
                ttl: 64,
                protocol: 1,
            },
            IcmpMessage::EchoReply {
                identifier,
                sequence,
            },
            hops,
            depth + 1,
        )
    }

    pub(crate) fn simulated_public_address(ip: Ipv4Addr) -> bool {
        // Documentation ranges are deliberately usable in this isolated simulator.
        !ip.is_private()
            && !ip.is_loopback()
            && !ip.is_link_local()
            && !ip.is_multicast()
            && !ip.is_broadcast()
            && !ip.is_unspecified()
            && ip.octets()[0] != 0
            && ip.octets()[0] < 240
            && !(ip.octets()[0] == 100 && (64..128).contains(&ip.octets()[1]))
    }

    pub fn ping_from_internet(&mut self, address: Ipv4Addr) -> ReachabilityResult {
        self.ping_from_external(Ipv4Addr::new(198, 51, 100, 1), address)
    }

    pub fn ping_from_external(
        &mut self,
        source: Ipv4Addr,
        address: Ipv4Addr,
    ) -> ReachabilityResult {
        if !self.provider.external.available(source) {
            return ReachabilityResult {
                reachable: false,
                hops: vec![],
                failure: Some(ReachabilityFailure::SourceDown),
            };
        }
        let mut candidates: Vec<_> = self
            .provider
            .circuits()
            .iter()
            .filter_map(|c| self.upstream_route(c.port, address).map(|r| (c.port, r)))
            .collect();
        candidates.sort_by_key(|(port, r)| (std::cmp::Reverse(r.prefix.length()), *port));
        let mut last_hops = Vec::new();
        for (port, route) in candidates {
            let mut hops = Vec::new();
            if !self.port_link_up(port) {
                continue;
            }
            let Ok(mac) = self.resolve_neighbor(port, route.next_hop, VlanId(1)) else {
                continue;
            };
            let packet = Ipv4Packet {
                source,
                destination: address,
                ttl: 64,
                protocol: 1,
            };
            if self.deliver(
                port,
                VlanId(1),
                mac,
                packet,
                IcmpMessage::EchoRequest {
                    identifier: 1,
                    sequence: 1,
                },
                &mut hops,
                0,
            ) {
                return ReachabilityResult {
                    reachable: true,
                    hops,
                    failure: None,
                };
            }
            last_hops = hops;
        }
        ReachabilityResult {
            reachable: false,
            hops: last_hops,
            failure: Some(ReachabilityFailure::NoRoute),
        }
    }
}
