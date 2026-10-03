use crate::{
    Ipv4InterfaceConfig, MacAddress, NetworkOutletKind, NetworkSim, PortConfig, PortId, SimError,
    VlanId,
};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

/// A simulated provider-routed /29. The first usable address is the gateway;
/// the remaining five usable addresses may be assigned to servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicIpv4Block {
    pub network: Ipv4Addr,
    pub uplink: PortId,
}

impl PublicIpv4Block {
    pub const PREFIX: u8 = 29;
    pub const PRICE: i64 = 240;

    pub fn gateway(self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.network) + 1)
    }

    pub fn host_addresses(self) -> impl Iterator<Item = Ipv4Addr> {
        (2..=6).map(move |offset| Ipv4Addr::from(u32::from(self.network) + offset))
    }

    pub fn contains_host(self, address: Ipv4Addr) -> bool {
        self.host_addresses().any(|host| host == address)
    }
}

impl NetworkSim {
    pub fn public_ipv4_blocks(&self) -> &[PublicIpv4Block] {
        &self.public_ipv4_blocks
    }

    pub fn buy_public_ipv4_block(&mut self, uplink: PortId) -> Result<PublicIpv4Block, SimError> {
        if !self
            .network_outlet(uplink)
            .is_some_and(|outlet| matches!(outlet.kind, NetworkOutletKind::Uplink { .. }))
        {
            return Err(SimError::InvalidPublicUplink);
        }
        if self.money < PublicIpv4Block::PRICE {
            return Err(SimError::InsufficientFunds {
                needed: PublicIpv4Block::PRICE,
                available: self.money,
            });
        }
        // TEST-NET-3 is reserved for documentation, so these simulated addresses
        // cannot be mistaken for genuinely routed Internet addresses.
        let index = self.public_ipv4_blocks.len();
        if index >= 32 {
            return Err(SimError::PublicIpv4Exhausted);
        }
        let block = PublicIpv4Block {
            network: Ipv4Addr::new(203, 0, 113, (index * 8) as u8),
            uplink,
        };
        self.money -= PublicIpv4Block::PRICE;
        self.public_ipv4_blocks.push(block);
        Ok(block)
    }

    pub fn assign_public_ipv4(
        &mut self,
        port: PortId,
        network: Ipv4Addr,
    ) -> Result<Ipv4Addr, SimError> {
        let block = self
            .public_ipv4_blocks
            .iter()
            .find(|block| block.network == network)
            .copied()
            .ok_or(SimError::PublicIpv4BlockNotOwned)?;
        if !self
            .port(port)
            .is_some_and(|p| p.name != "mgmt0" && matches!(p.config, PortConfig::Server(_)))
        {
            return Err(SimError::WrongPortType);
        }
        if let Some(address) = self.server_address(port)
            && block.contains_host(address)
        {
            return Ok(address);
        }
        let address = block
            .host_addresses()
            .find(|address| !self.ipv4_in_use(*address))
            .ok_or(SimError::PublicIpv4Exhausted)?;
        self.set_ipv4(
            port,
            Ipv4InterfaceConfig::new(
                address,
                PublicIpv4Block::PREFIX,
                Some(block.gateway()),
                VlanId(1),
            ),
        )?;
        Ok(address)
    }

    pub fn assign_lan_ipv4(&mut self, port: PortId) -> Result<Ipv4Addr, SimError> {
        if !matches!(
            self.port(port).map(|p| &p.config),
            Some(PortConfig::Server(_))
        ) {
            return Err(SimError::WrongPortType);
        }
        if let Some(address) = self.server_address(port)
            && address.octets()[..2] == [10, 0]
        {
            return Ok(address);
        }
        let address = (10..=u16::MAX)
            .map(|host| Ipv4Addr::new(10, 0, (host >> 8) as u8, host as u8))
            .find(|address| !self.ipv4_in_use(*address))
            .ok_or(SimError::LanIpv4Exhausted)?;
        self.set_ipv4(port, Ipv4InterfaceConfig::new(address, 16, None, VlanId(1)))?;
        Ok(address)
    }

    pub fn public_block_for_address(&self, address: Ipv4Addr) -> Option<PublicIpv4Block> {
        self.public_ipv4_blocks
            .iter()
            .copied()
            .find(|block| block.contains_host(address))
    }

    pub fn public_assignments(&self, block: PublicIpv4Block) -> Vec<(Ipv4Addr, PortId)> {
        let mut assigned: Vec<_> = self
            .ports
            .values()
            .filter_map(|port| {
                let PortConfig::Server(config) = &port.config else {
                    return None;
                };
                let address = config.ipv4.as_ref()?.address;
                block.contains_host(address).then_some((address, port.id))
            })
            .collect();
        assigned.sort_by_key(|(address, _)| *address);
        assigned
    }

    fn ipv4_in_use(&self, address: Ipv4Addr) -> bool {
        self.ports.values().any(|port| match &port.config {
            PortConfig::Server(config) => {
                config.ipv4.as_ref().is_some_and(|ip| ip.address == address)
            }
            PortConfig::Router(config) => config
                .interfaces
                .iter()
                .any(|ip| ip.address == Some(address)),
            _ => false,
        })
    }

    fn server_address(&self, port: PortId) -> Option<Ipv4Addr> {
        let PortConfig::Server(config) = &self.port(port)?.config else {
            return None;
        };
        config.ipv4.as_ref().map(|ip| ip.address)
    }

    /// Simulate a packet entering a purchased uplink and reaching an assigned
    /// server through the actual cable, switch and VLAN forwarding path.
    pub fn ping_from_internet(&mut self, address: Ipv4Addr) -> crate::ReachabilityResult {
        use crate::{
            EthernetFrame, EthernetPayload, Hop, IcmpMessage, Ipv4Packet, ReachabilityFailure,
            ReachabilityResult,
        };
        let failed = |reason| ReachabilityResult {
            reachable: false,
            hops: vec![],
            failure: Some(reason),
        };
        let Some(block) = self.public_block_for_address(address) else {
            return failed(ReachabilityFailure::NoRoute);
        };
        let targets: Vec<_> = self
            .ports
            .values()
            .filter(|port| match &port.config {
                PortConfig::Server(config) if port.name != "mgmt0" => {
                    config.ipv4.as_ref().is_some_and(|ip| {
                        ip.address == address
                            && ip.prefix == PublicIpv4Block::PREFIX
                            && ip.gateway == Some(block.gateway())
                    })
                }
                _ => false,
            })
            .map(|port| port.id)
            .collect();
        if targets.len() != 1 {
            return failed(if targets.is_empty() {
                ReachabilityFailure::DestinationNotFound
            } else {
                ReachabilityFailure::AddressConflict
            });
        }
        let target = targets[0];
        if !self.port_up(target) {
            return failed(ReachabilityFailure::DestinationDown);
        }
        let source = Ipv4Addr::new(198, 51, 100, 1);
        let request = EthernetFrame {
            source: MacAddress::for_port(block.uplink),
            destination: MacAddress::for_port(target),
            vlan: None,
            payload: EthernetPayload::Ipv4 {
                packet: Ipv4Packet {
                    source,
                    destination: address,
                    ttl: 64,
                    protocol: 1,
                },
                icmp: IcmpMessage::EchoRequest {
                    identifier: 1,
                    sequence: 1,
                },
            },
        };
        let arrived = self
            .transmit_frame(block.uplink, request)
            .iter()
            .any(|delivery| delivery.port == target);
        if !arrived {
            return failed(ReachabilityFailure::NoPhysicalLink);
        }
        let reply = EthernetFrame {
            source: MacAddress::for_port(target),
            destination: MacAddress::for_port(block.uplink),
            vlan: None,
            payload: EthernetPayload::Ipv4 {
                packet: Ipv4Packet {
                    source: address,
                    destination: source,
                    ttl: 64,
                    protocol: 1,
                },
                icmp: IcmpMessage::EchoReply {
                    identifier: 1,
                    sequence: 1,
                },
            },
        };
        if !self
            .transmit_frame(target, reply)
            .iter()
            .any(|delivery| delivery.port == block.uplink)
        {
            return failed(ReachabilityFailure::NoRoute);
        }
        ReachabilityResult {
            reachable: true,
            hops: vec![Hop {
                device: crate::NetworkOutlet::OWNER,
                ingress: Some(block.uplink),
                egress: Some(target),
                note: "public uplink route".into(),
            }],
            failure: None,
        }
    }
}
