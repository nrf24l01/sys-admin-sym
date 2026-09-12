use super::{EthernetFrame, MacAddress};
use crate::{DeviceKind, NetworkSim, PortConfig, PortId, SwitchPortConfig, SwitchPortMode, VlanId};
use std::collections::{HashSet, VecDeque};

/// A frame arriving at an endpoint port after one or more switch crossings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameDelivery {
    pub port: PortId,
    pub vlan: VlanId,
    pub frame: EthernetFrame,
}

const MAX_FRAME_HOPS: u16 = 64;

impl NetworkSim {
    /// Resolve a physical route through passive patch-panel pairs. The
    /// returned ports include both cable ends and panel sides in traversal
    /// order, which is useful for highlighting a complete cable path.
    pub fn physical_path(&self, start: PortId) -> Vec<PortId> {
        fn walk(
            sim: &NetworkSim,
            current: PortId,
            start: PortId,
            path: &mut Vec<PortId>,
            seen: &mut HashSet<PortId>,
        ) -> bool {
            if path.len() >= 128 || !seen.insert(current) {
                return false;
            }
            path.push(current);
            let terminals = path
                .iter()
                .filter(|p| {
                    !matches!(
                        sim.port(**p).map(|x| &x.config),
                        Some(PortConfig::PatchPanel | PortConfig::CableManager)
                    )
                })
                .count();
            if terminals >= 2 && current != start {
                return true;
            }
            let mut next = Vec::new();
            if let Some(link) = sim.link_for_port(current)
                && let Some(other) = link.other(current)
            {
                next.push(other);
            }
            if let Some(pair) = sim.port(current).and_then(|p| p.paired_port) {
                next.push(pair);
            }
            next.sort();
            for neighbor in next {
                if sim.direct_segment_active(current, neighbor)
                    && walk(sim, neighbor, start, path, seen)
                {
                    return true;
                }
            }
            path.pop();
            seen.remove(&current);
            false
        }
        let terminals: Vec<_> = self
            .ports
            .values()
            .filter(|p| !matches!(p.config, PortConfig::PatchPanel | PortConfig::CableManager))
            .map(|p| p.id)
            .collect();
        let mut terminals = terminals;
        terminals.sort();
        for terminal in terminals {
            let mut path = Vec::new();
            if walk(self, terminal, terminal, &mut path, &mut HashSet::new())
                && path.contains(&start)
            {
                return path;
            }
        }
        Vec::new()
    }

    pub fn physical_link_up(&self, start: PortId) -> bool {
        let path = self.physical_path(start);
        !path.is_empty()
            && path
                .windows(2)
                .all(|pair| self.direct_segment_active(pair[0], pair[1]))
    }

    pub fn physical_link_speed(&self, start: PortId) -> Option<crate::LinkSpeed> {
        if !self.physical_link_up(start) {
            return None;
        }
        self.physical_path(start)
            .windows(2)
            .filter_map(|pair| {
                let a = self.port(pair[0])?;
                let b = self.port(pair[1])?;
                Some(
                    a.advertised_speed
                        .min(a.max_speed)
                        .min(b.advertised_speed)
                        .min(b.max_speed),
                )
            })
            .min()
    }

    fn direct_segment_active(&self, a: PortId, b: PortId) -> bool {
        let (Some(pa), Some(pb)) = (self.port(a), self.port(b)) else {
            return false;
        };
        let active = |p: &crate::Port| {
            p.enabled
                && p.connector.supports_cabling()
                && self.device(p.device).is_some_and(|d| {
                    d.rack.is_some()
                        && (matches!(p.config, PortConfig::PatchPanel | PortConfig::CableManager)
                            || d.powered)
                })
        };
        let cable = self.links.values().any(|l| {
            l.enabled && l.length_cm <= 10_000 && ((l.a == a && l.b == b) || (l.a == b && l.b == a))
        });
        (pa.paired_port == Some(b) || cable) && active(pa) && active(pb)
    }

    pub(crate) fn prepare_runtime(&mut self) {
        self.runtime.prepare(self.topology_revision);
    }

    /// Transmit one Ethernet frame from `egress`, forwarding it through the
    /// configured physical topology. Switches learn source MACs per VLAN;
    /// endpoint deliveries are returned in deterministic port order.
    pub fn transmit_frame(&mut self, egress: PortId, frame: EthernetFrame) -> Vec<FrameDelivery> {
        self.runtime.prepare(self.topology_revision);
        if self.ingress_vlan(egress, &frame).is_none() {
            return vec![];
        }
        let initial = frame;
        let mut queue = VecDeque::new();
        let mut seen: HashSet<(PortId, MacAddress, MacAddress, VlanId)> = HashSet::new();
        let mut deliveries = Vec::new();
        queue.push_back((egress, initial, 0u16));

        while let Some((out, frame, hops)) = queue.pop_front() {
            if hops >= MAX_FRAME_HOPS {
                continue;
            }
            let Some(link) = self.link_for_port(out).cloned() else {
                continue;
            };
            let Some(in_port) = link.other(out) else {
                continue;
            };
            if !self.physical_link_up(out) || !self.physical_link_up(in_port) {
                continue;
            }
            if !self.endpoint_link_vlan_matches(out, in_port, frame.vlan) {
                continue;
            }
            self.runtime.send_frame(out, in_port);
            if let Some(pair) = self.port(in_port).and_then(|p| p.paired_port) {
                if !self.port_link_up(pair) {
                    continue;
                }
                queue.push_back((pair, frame, hops + 1));
                continue;
            }
            let wire_vlan = frame.vlan;
            let Some(vlan) = self.ingress_vlan(in_port, &frame) else {
                continue;
            };
            let mut received = frame;
            received.vlan = Some(vlan);
            let Some(port) = self.port(in_port) else {
                continue;
            };
            let device = port.device;

            match self.device(device).map(|d| &d.kind) {
                Some(DeviceKind::Switch(_)) => {
                    self.runtime
                        .learn_mac(device, vlan, received.source, in_port);
                    let mut targets =
                        self.switch_targets(device, in_port, vlan, received.destination);
                    targets.sort();
                    for target in targets {
                        let key = (target, received.source, received.destination, vlan);
                        if !seen.insert(key) {
                            continue;
                        }
                        let mut emitted = received;
                        emitted.vlan = self.egress_vlan(target, vlan);
                        queue.push_back((target, emitted, hops + 1));
                    }
                }
                Some(DeviceKind::Server(_)) | Some(DeviceKind::Router(_)) => {
                    let own = MacAddress::for_port(in_port);
                    if received.destination == own || is_broadcast(received.destination) {
                        deliveries.push(FrameDelivery {
                            port: in_port,
                            vlan,
                            frame: EthernetFrame {
                                vlan: wire_vlan,
                                ..received
                            },
                        });
                    }
                }
                Some(DeviceKind::PatchPanel(_))
                | Some(DeviceKind::CableManager(_))
                | Some(DeviceKind::Ups(_))
                | Some(DeviceKind::Pdu(_)) => {}
                None => {}
            }
        }
        deliveries.sort_by_key(|delivery| delivery.port);
        deliveries
    }

    fn ingress_vlan(&self, port: PortId, frame: &EthernetFrame) -> Option<VlanId> {
        let p = self.port(port)?;
        match &p.config {
            PortConfig::Switch(config) => match &config.mode {
                SwitchPortMode::Access { vlan } => {
                    let effective = vlan.unwrap_or(VlanId(1));
                    (frame.vlan.is_none() && self.switch_has_vlan(p.device, effective))
                        .then_some(effective)
                }
                SwitchPortMode::Trunk {
                    native_vlan,
                    allowed,
                } => match frame.vlan {
                    Some(vlan)
                        if allowed.contains(&vlan) && self.switch_has_vlan(p.device, vlan) =>
                    {
                        Some(vlan)
                    }
                    None if native_vlan.is_some_and(|v| {
                        allowed.contains(&v) && self.switch_has_vlan(p.device, v)
                    }) =>
                    {
                        *native_vlan
                    }
                    _ => None,
                },
            },
            PortConfig::Server(config) => frame.vlan.is_none().then_some(
                config
                    .ipv4
                    .as_ref()
                    .and_then(|ip| ip.vlan)
                    .unwrap_or(VlanId(1)),
            ),
            PortConfig::Router(config) => {
                if let Some(vlan) = frame.vlan {
                    config
                        .interfaces
                        .iter()
                        .any(|i| i.vlan == Some(vlan))
                        .then_some(vlan)
                } else {
                    let mut vlans = config.interfaces.iter().filter_map(|i| i.vlan);
                    let vlan = vlans.next().unwrap_or(VlanId(1));
                    (vlans.all(|other| other == vlan)).then_some(vlan)
                }
            }
            PortConfig::PatchPanel | PortConfig::CableManager => None,
        }
    }

    fn egress_vlan(&self, port: PortId, vlan: VlanId) -> Option<VlanId> {
        match &self.port(port)?.config {
            PortConfig::Switch(config) => match &config.mode {
                SwitchPortMode::Access { .. } => None,
                SwitchPortMode::Trunk {
                    native_vlan,
                    allowed,
                } => allowed
                    .contains(&vlan)
                    .then_some(vlan)
                    .filter(|v| Some(*v) != *native_vlan),
            },
            PortConfig::Server(_) => None,
            PortConfig::Router(config) => {
                let distinct = config
                    .interfaces
                    .iter()
                    .filter_map(|interface| interface.vlan)
                    .collect::<std::collections::BTreeSet<_>>();
                (distinct.len() > 1 && distinct.contains(&vlan)).then_some(vlan)
            }
            PortConfig::PatchPanel | PortConfig::CableManager => None,
        }
    }

    fn switch_targets(
        &self,
        device: crate::DeviceId,
        ingress: PortId,
        vlan: VlanId,
        destination: MacAddress,
    ) -> Vec<PortId> {
        let ports = self.device(device).map(|d| d.ports()).unwrap_or(&[]);
        if !is_broadcast(destination)
            && let Some(learned) = self.runtime.learned_mac(device, vlan, destination)
        {
            if learned != ingress && self.carries_vlan(learned, vlan) && self.port_link_up(learned)
            {
                return vec![learned];
            }
            return vec![];
        }
        ports
            .iter()
            .copied()
            .filter(|p| *p != ingress && self.port_link_up(*p) && self.carries_vlan(*p, vlan))
            .collect()
    }

    fn carries_vlan(&self, port: PortId, vlan: VlanId) -> bool {
        match &self.port(port).map(|p| &p.config) {
            Some(PortConfig::Switch(c)) => {
                c.mode.carries(vlan)
                    && self
                        .port(port)
                        .is_some_and(|p| self.switch_has_vlan(p.device, vlan))
            }
            Some(PortConfig::Server(c)) => c
                .ipv4
                .as_ref()
                .map_or(vlan == VlanId(1), |i| i.vlan.unwrap_or(VlanId(1)) == vlan),
            Some(PortConfig::Router(c)) => c.interfaces.iter().any(|i| {
                if i.vlan == Some(vlan) {
                    return true;
                }
                if i.vlan.is_some() || vlan != VlanId(1) {
                    return false;
                }
                // An untagged physical router interface belongs on an access
                // VLAN 1 peer only; it is not an implicit trunk member.
                self.link_for_port(port)
                    .and_then(|link| link.other(port))
                    .and_then(|peer| self.port(peer))
                    .is_some_and(|peer| {
                        matches!(
                            peer.config,
                            PortConfig::Switch(SwitchPortConfig {
                            mode: SwitchPortMode::Access { vlan }
                        })
                            if vlan.unwrap_or(VlanId(1)) == VlanId(1)
                        )
                    })
            }),
            None => false,
            Some(PortConfig::PatchPanel) | Some(PortConfig::CableManager) => false,
        }
    }

    fn switch_has_vlan(&self, device: crate::DeviceId, vlan: VlanId) -> bool {
        matches!(self.device(device).map(|d| &d.kind), Some(DeviceKind::Switch(sw)) if sw.vlans.iter().any(|entry| entry.id == vlan))
    }

    fn endpoint_link_vlan_matches(
        &self,
        out: PortId,
        input: PortId,
        wire_vlan: Option<VlanId>,
    ) -> bool {
        let Some(source) = self.port(out) else {
            return false;
        };
        let Some(destination) = self.port(input) else {
            return false;
        };
        if wire_vlan.is_some() {
            return true;
        }
        let PortConfig::Server(config) = &source.config else {
            return true;
        };
        let source_vlan = config
            .ipv4
            .as_ref()
            .and_then(|ip| ip.vlan)
            .unwrap_or(VlanId(1));
        match &destination.config {
            PortConfig::Switch(config) => match config.mode {
                SwitchPortMode::Access { vlan } => vlan.unwrap_or(VlanId(1)) == source_vlan,
                SwitchPortMode::Trunk { native_vlan, .. } => native_vlan == Some(source_vlan),
            },
            _ => true,
        }
    }
}

fn is_broadcast(mac: MacAddress) -> bool {
    mac.0 == [0xff; 6]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ArpPacket, Command, DeviceId, DeviceTemplate, EthernetPayload, Ipv4InterfaceConfig,
        OutletId, PowerEndpoint, RACK_C13_OUTLETS, RackId, SourceId,
    };
    use std::net::Ipv4Addr;

    fn setup() -> NetworkSim {
        let mut sim = NetworkSim::new();
        sim.execute(Command::BuyCableSupply {
            supply: crate::CableSupply::CableBox305m,
        })
        .unwrap();
        sim.execute(Command::BuyCableSupply {
            supply: crate::CableSupply::Rj45Pack20,
        })
        .unwrap();
        sim
    }

    fn device(sim: &mut NetworkSim, template: DeviceTemplate, unit: u8) -> DeviceId {
        let id = match sim.execute(Command::BuyDevice { kind: template }).unwrap()[0] {
            crate::SimEvent::DeviceAdded(id) => id,
            _ => unreachable!(),
        };
        sim.execute(Command::PlaceDevice {
            device: id,
            rack: RackId(1),
            unit,
        })
        .unwrap();
        if matches!(
            template,
            DeviceTemplate::PatchPanel | DeviceTemplate::CableManager
        ) {
            return id;
        }
        let outlet = (0..RACK_C13_OUTLETS as u8)
            .map(|index| OutletId {
                source: SourceId::Rack(RackId(1)),
                index,
            })
            .find(|outlet| !sim.power.connections.contains_key(outlet))
            .unwrap();
        sim.execute(Command::ConnectPower {
            outlet,
            endpoint: PowerEndpoint::Device(id),
        })
        .unwrap();
        sim.execute(Command::SetPower {
            device: id,
            powered: true,
        })
        .unwrap();
        id
    }

    fn frame(source: PortId, destination: MacAddress, vlan: Option<VlanId>) -> EthernetFrame {
        EthernetFrame {
            source: MacAddress::for_port(source),
            destination,
            vlan,
            payload: EthernetPayload::Arp(ArpPacket::Request {
                sender_ip: Ipv4Addr::new(192, 0, 2, 1),
                sender_mac: MacAddress::for_port(source),
                target_ip: Ipv4Addr::new(192, 0, 2, 2),
                target_mac: None,
            }),
        }
    }

    #[test]
    fn broadcast_floods_same_vlan_and_isolates_other_vlan() {
        let mut sim = setup();
        let sw = device(&mut sim, DeviceTemplate::Switch, 1);
        let a = device(&mut sim, DeviceTemplate::Server, 2);
        let b = device(&mut sim, DeviceTemplate::Server, 3);
        let c = device(&mut sim, DeviceTemplate::Server, 4);
        let [pa, pb, pc] = [a, b, c].map(|id| sim.device(id).unwrap().ports()[0]);
        for vlan in [10, 20] {
            sim.execute(Command::CreateVlan {
                switch: sw,
                vlan: crate::Vlan {
                    id: VlanId(vlan),
                    name: format!("VLAN{vlan}"),
                },
            })
            .unwrap();
        }
        for (p, vlan) in [(pa, 10), (pb, 10), (pc, 20)] {
            sim.execute(Command::SetIpv4 {
                port: p,
                config: Ipv4InterfaceConfig::new(
                    Ipv4Addr::new(192, 0, vlan as u8, 1),
                    24,
                    None,
                    VlanId(vlan),
                ),
            })
            .unwrap();
        }
        let [s1, s2, s3] = sim.device(sw).unwrap().ports()[..3] else {
            unreachable!()
        };
        for (p, vlan) in [(s1, 10), (s2, 10), (s3, 20)] {
            sim.execute(Command::SetSwitchPortMode {
                port: p,
                mode: SwitchPortMode::Access {
                    vlan: Some(VlanId(vlan)),
                },
            })
            .unwrap();
        }
        for (x, y) in [(pa, s1), (pb, s2), (pc, s3)] {
            sim.execute(Command::Connect { a: x, b: y }).unwrap();
        }
        let deliveries = sim.transmit_frame(pa, frame(pa, MacAddress([0xff; 6]), None));
        assert_eq!(
            deliveries.iter().map(|d| d.port).collect::<Vec<_>>(),
            vec![pb]
        );
    }

    #[test]
    fn learned_unicast_excludes_third_host() {
        let mut sim = setup();
        let sw = device(&mut sim, DeviceTemplate::Switch, 1);
        let ids = [2, 3, 4].map(|u| device(&mut sim, DeviceTemplate::Server, u));
        let ports = ids.map(|id| sim.device(id).unwrap().ports()[0]);
        let switch_ports = sim.device(sw).unwrap().ports()[..3].to_vec();
        for (p, sp) in ports.into_iter().zip(switch_ports) {
            sim.execute(Command::Connect { a: p, b: sp }).unwrap();
        }
        let bmac = MacAddress::for_port(ports[1]);
        sim.transmit_frame(ports[1], frame(ports[1], MacAddress([0xff; 6]), None));
        let deliveries = sim.transmit_frame(ports[0], frame(ports[0], bmac, None));
        assert_eq!(
            deliveries.iter().map(|d| d.port).collect::<Vec<_>>(),
            vec![ports[1]]
        );
    }

    #[test]
    fn trunk_without_native_vlan_rejects_untagged_frame() {
        let mut sim = setup();
        let sw = device(&mut sim, DeviceTemplate::Switch, 1);
        let server = device(&mut sim, DeviceTemplate::Server, 2);
        let sp = sim.device(sw).unwrap().ports()[0];
        let ep = sim.device(server).unwrap().ports()[0];
        sim.execute(Command::SetSwitchPortMode {
            port: sp,
            mode: SwitchPortMode::Trunk {
                native_vlan: None,
                allowed: vec![VlanId(1)],
            },
        })
        .unwrap();
        sim.execute(Command::Connect { a: ep, b: sp }).unwrap();
        assert!(
            sim.transmit_frame(ep, frame(ep, MacAddress([0xff; 6]), None))
                .is_empty()
        );
    }

    #[test]
    fn switch_loop_is_bounded() {
        let mut sim = setup();
        let a = device(&mut sim, DeviceTemplate::Switch, 1);
        let b = device(&mut sim, DeviceTemplate::Switch, 2);
        let ap = sim.device(a).unwrap().ports()[..2].to_vec();
        let bp = sim.device(b).unwrap().ports()[..2].to_vec();
        for (p, q) in [(ap[0], bp[0]), (ap[1], bp[1])] {
            sim.execute(Command::Connect { a: p, b: q }).unwrap();
        }
        let deliveries = sim.transmit_frame(ap[0], frame(ap[0], MacAddress([0xff; 6]), None));
        assert!(deliveries.is_empty());
        assert!(sim.port_telemetry(ap[0]).tx_frames < 100);
    }

    #[test]
    fn passive_patch_panel_is_end_to_end_and_reports_full_path() {
        let mut sim = setup();
        let sw = device(&mut sim, DeviceTemplate::Switch, 1);
        let panel = device(&mut sim, DeviceTemplate::PatchPanel, 2);
        let server = device(&mut sim, DeviceTemplate::Server, 3);
        let switch_port = sim.device(sw).unwrap().ports()[0];
        let rear = sim.device(panel).unwrap().ports()[0];
        let front = sim.device(panel).unwrap().ports()[1];
        let server_port = sim.device(server).unwrap().ports()[0];
        sim.execute(Command::Connect {
            a: switch_port,
            b: front,
        })
        .unwrap();
        sim.execute(Command::Connect {
            a: rear,
            b: server_port,
        })
        .unwrap();
        sim.ports.get_mut(&server_port).unwrap().max_speed = crate::LinkSpeed::Mbps100;
        let expected = vec![switch_port, front, rear, server_port];
        assert_eq!(sim.physical_path(switch_port), expected);
        assert_eq!(sim.physical_path(front), expected);
        assert!(sim.physical_link_up(switch_port));
        assert_eq!(
            sim.physical_link_speed(switch_port),
            Some(crate::LinkSpeed::Mbps100)
        );
        assert_eq!(
            sim.transmit_frame(switch_port, frame(switch_port, MacAddress([0xff; 6]), None))
                .len(),
            1
        );
        assert!(sim.port_telemetry(switch_port).tx_frames > 0);
        assert!(sim.port_telemetry(front).rx_frames > 0);
        assert!(sim.port_telemetry(rear).tx_frames > 0);
        assert!(sim.port_telemetry(server_port).rx_frames > 0);
        let baseline = sim.clone();
        let first = sim.link_for_port(switch_port).unwrap().id;
        sim.execute(Command::Disconnect { link: first }).unwrap();
        assert!(!sim.physical_link_up(server_port));
        let second = baseline.link_for_port(server_port).unwrap().id;
        let mut other = baseline;
        other.execute(Command::Disconnect { link: second }).unwrap();
        assert!(!other.physical_link_up(switch_port));
    }
}
