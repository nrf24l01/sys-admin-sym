use super::{ArpPacket, EthernetFrame, EthernetPayload, MacAddress};
use crate::{NetworkSim, PortConfig, PortId, ReachabilityFailure, SwitchPortMode, VlanId};
use std::collections::HashSet;
use std::net::Ipv4Addr;

impl NetworkSim {
    pub fn interface_ipv4(&self, port: PortId, vlan: VlanId) -> Option<Ipv4Addr> {
        match &self.port(port)?.config {
            PortConfig::Server(config) => config
                .ipv4
                .as_ref()
                .filter(|i| i.vlan.unwrap_or(VlanId(1)) == vlan)
                .map(|i| i.address),
            PortConfig::Router(config) => config
                .interfaces
                .iter()
                .find(|i| i.vlan.unwrap_or(VlanId(1)) == vlan)
                .and_then(|i| i.address),
            PortConfig::Switch(_) | PortConfig::PatchPanel | PortConfig::CableManager => None,
        }
    }

    pub fn wire_vlan_for(&self, port: PortId, vlan: VlanId) -> Option<VlanId> {
        let p = self.port(port)?;
        match &p.config {
            PortConfig::Server(_) => None,
            PortConfig::Switch(config) => match config.mode {
                SwitchPortMode::Access { vlan: access } if access.unwrap_or(VlanId(1)) == vlan => {
                    None
                }
                SwitchPortMode::Trunk {
                    native_vlan,
                    ref allowed,
                } if allowed.contains(&vlan) => (native_vlan != Some(vlan)).then_some(vlan),
                _ => None,
            },
            PortConfig::Router(config) => {
                let iface = config
                    .interfaces
                    .iter()
                    .find(|i| i.vlan.unwrap_or(VlanId(1)) == vlan)?;
                let peer = self.link_for_port(port)?.other(port)?;
                let trunk = matches!(self.port(peer)?.config, PortConfig::Switch(ref c) if matches!(c.mode, SwitchPortMode::Trunk { .. }))
                    || matches!(self.port(peer)?.config, PortConfig::Router(_))
                        && config
                            .interfaces
                            .iter()
                            .filter_map(|i| i.vlan)
                            .collect::<std::collections::BTreeSet<_>>()
                            .len()
                            > 1;
                (trunk && iface.vlan.is_some()).then_some(vlan)
            }
            PortConfig::PatchPanel | PortConfig::CableManager => None,
        }
    }

    pub fn resolve_neighbor(
        &mut self,
        port: PortId,
        target: Ipv4Addr,
        vlan: VlanId,
    ) -> Result<MacAddress, ReachabilityFailure> {
        self.prepare_runtime();
        if let Some(mac) = self.runtime.arp.get(&(port, target, vlan)).copied() {
            return Ok(mac);
        }
        let source_ip = self
            .interface_ipv4(port, vlan)
            .ok_or(ReachabilityFailure::NoAddress)?;
        let request = EthernetFrame {
            source: MacAddress::for_port(port),
            destination: MacAddress([0xff; 6]),
            vlan: self.wire_vlan_for(port, vlan),
            payload: EthernetPayload::Arp(ArpPacket::Request {
                sender_ip: source_ip,
                sender_mac: MacAddress::for_port(port),
                target_ip: target,
                target_mac: None,
            }),
        };
        let requests = self.transmit_frame(port, request);
        let mut replies = HashSet::new();
        for delivery in requests {
            let Some(ip) = self.interface_ipv4(delivery.port, delivery.vlan) else {
                continue;
            };
            if ip != target {
                continue;
            }
            let reply = EthernetFrame {
                source: MacAddress::for_port(delivery.port),
                destination: request.source,
                vlan: self.wire_vlan_for(delivery.port, delivery.vlan),
                payload: EthernetPayload::Arp(ArpPacket::Reply {
                    sender_ip: ip,
                    sender_mac: MacAddress::for_port(delivery.port),
                    target_ip: source_ip,
                    target_mac: request.source,
                }),
            };
            let returned = self.transmit_frame(delivery.port, reply);
            if returned.iter().any(|d| {
                d.port == port && d.vlan == vlan
                    && matches!(d.frame.payload,
                        EthernetPayload::Arp(ArpPacket::Reply { sender_ip, sender_mac, target_ip, target_mac })
                            if sender_ip == ip && sender_mac == reply.source && target_ip == source_ip && target_mac == request.source)
            }) {
                replies.insert(reply.source);
            }
        }
        match replies.len() {
            1 => {
                let mac = *replies.iter().next().unwrap();
                self.runtime.learn_arp(port, target, vlan, mac);
                Ok(mac)
            }
            0 => Err(ReachabilityFailure::DestinationNotFound),
            _ => Err(ReachabilityFailure::AddressConflict),
        }
    }

    pub fn source_address_conflict(&mut self, port: PortId, vlan: VlanId) -> bool {
        self.prepare_runtime();
        let Some(address) = self.interface_ipv4(port, vlan) else {
            return false;
        };
        let probe = EthernetFrame {
            source: MacAddress::for_port(port),
            destination: MacAddress([0xff; 6]),
            vlan: self.wire_vlan_for(port, vlan),
            payload: EthernetPayload::Arp(ArpPacket::Request {
                sender_ip: address,
                sender_mac: MacAddress::for_port(port),
                target_ip: address,
                target_mac: None,
            }),
        };
        self.transmit_frame(port, probe)
            .into_iter()
            .any(|delivery| {
                delivery.port != port
                    && self.interface_ipv4(delivery.port, delivery.vlan) == Some(address)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Command, DeviceTemplate, Ipv4InterfaceConfig, OutletId, PowerEndpoint, RackId, SimEvent,
        SourceId,
    };

    fn router() -> (NetworkSim, PortId) {
        let mut sim = NetworkSim::new();
        let router = match sim
            .execute(Command::BuyDevice {
                kind: DeviceTemplate::Router,
            })
            .unwrap()[0]
        {
            SimEvent::DeviceAdded(id) => id,
            _ => unreachable!(),
        };
        sim.execute(Command::PlaceDevice {
            device: router,
            rack: RackId(1),
            unit: 1,
        })
        .unwrap();
        sim.execute(Command::ConnectPower {
            outlet: OutletId {
                source: SourceId::Rack(RackId(1)),
                index: 0,
            },
            endpoint: PowerEndpoint::Device(router),
        })
        .unwrap();
        sim.execute(Command::SetPower {
            device: router,
            powered: true,
        })
        .unwrap();
        let port = sim.device(router).unwrap().ports()[2];
        (sim, port)
    }

    #[test]
    fn router_subinterfaces_resolve_by_vlan() {
        let (mut sim, port) = router();
        for (vlan, address) in [(10, [192, 0, 2, 1]), (20, [198, 51, 100, 1])] {
            sim.execute(Command::ConfigureRouterInterface {
                port,
                name: format!("LAN{vlan}"),
                vlan: Some(VlanId(vlan)),
                address: Some(address.into()),
                prefix: 24,
                internet_connected: false,
            })
            .unwrap();
        }
        assert_eq!(
            sim.interface_ipv4(port, VlanId(10)),
            Some([192, 0, 2, 1].into())
        );
        assert_eq!(
            sim.interface_ipv4(port, VlanId(20)),
            Some([198, 51, 100, 1].into())
        );
        assert_eq!(sim.interface_ipv4(port, VlanId(30)), None);
    }

    #[test]
    fn duplicate_source_address_is_detected_on_same_vlan() {
        let mut sim = NetworkSim::new();
        let mut ports = Vec::new();
        for unit in 1..=2 {
            let server = match sim
                .execute(Command::BuyDevice {
                    kind: DeviceTemplate::Server,
                })
                .unwrap()[0]
            {
                SimEvent::DeviceAdded(id) => id,
                _ => unreachable!(),
            };
            sim.execute(Command::PlaceDevice {
                device: server,
                rack: RackId(1),
                unit,
            })
            .unwrap();
            sim.execute(Command::ConnectPower {
                outlet: OutletId {
                    source: SourceId::Rack(RackId(1)),
                    index: unit - 1,
                },
                endpoint: PowerEndpoint::Device(server),
            })
            .unwrap();
            sim.execute(Command::SetPower {
                device: server,
                powered: true,
            })
            .unwrap();
            ports.push(sim.device(server).unwrap().ports()[0]);
        }
        let config = Ipv4InterfaceConfig::new([192, 0, 2, 7].into(), 24, None, VlanId(1));
        for port in ports.iter().copied() {
            sim.execute(Command::SetIpv4 {
                port,
                config: config.clone(),
            })
            .unwrap();
        }
        assert!(!sim.source_address_conflict(ports[0], VlanId(1)));
        sim.execute(Command::BuyCableSupply {
            supply: crate::CableSupply::CableBox305m,
        })
        .unwrap();
        sim.execute(Command::BuyCableSupply {
            supply: crate::CableSupply::Rj45Pack20,
        })
        .unwrap();
        sim.execute(Command::Connect {
            a: ports[0],
            b: ports[1],
        })
        .unwrap();
        assert!(sim.source_address_conflict(ports[0], VlanId(1)));
    }
}
