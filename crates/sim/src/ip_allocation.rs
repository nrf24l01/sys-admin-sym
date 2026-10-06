use crate::{
    Ipv4InterfaceConfig, NetworkOutletKind, NetworkSim, PortConfig, PortId, SimError, VlanId,
};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

/// Purchased /29 address inventory. The convenience assignment reserves the
/// first usable address for a gateway; ownership alone does not install routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicIpv4Block {
    pub network: Ipv4Addr,
    /// Legacy purchase provenance, never a routing constraint. Zero for new purchases.
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
        self.purchase_public_ipv4_pool(uplink)
    }

    pub fn buy_public_ipv4_pool(&mut self) -> Result<PublicIpv4Block, SimError> {
        self.purchase_public_ipv4_pool(PortId(0))
    }

    fn purchase_public_ipv4_pool(
        &mut self,
        provenance: PortId,
    ) -> Result<PublicIpv4Block, SimError> {
        if self.money < PublicIpv4Block::PRICE {
            return Err(SimError::InsufficientFunds {
                needed: PublicIpv4Block::PRICE,
                available: self.money,
            });
        }
        // TEST-NET-3 is reserved for documentation, so these simulated addresses
        // cannot be mistaken for genuinely routed Internet addresses.
        let network = (0..32)
            .map(|index| Ipv4Addr::new(203, 0, 113, index * 8))
            .find(|network| {
                let prefix =
                    crate::Ipv4Prefix::new(*network, PublicIpv4Block::PREFIX).expect("/29");
                !self
                    .public_ipv4_blocks
                    .iter()
                    .any(|b| b.network == *network)
                    && !self.provider().pools().iter().any(|p| {
                        p.prefix.contains_prefix(prefix) || prefix.contains_prefix(p.prefix)
                    })
            })
            .ok_or(SimError::PublicIpv4Exhausted)?;
        let block = PublicIpv4Block {
            network,
            uplink: provenance,
        };
        self.execute(crate::Command::Provider(crate::ProviderCommand::AddPool(
            crate::AddressPool {
                prefix: crate::Ipv4Prefix::new(block.network, PublicIpv4Block::PREFIX)
                    .map_err(SimError::Provider)?,
                description: "Purchased /29 address inventory".into(),
            },
        )))?;
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
            .flat_map(|port| match &port.config {
                PortConfig::Server(config) => config
                    .addresses()
                    .filter(|ip| block.contains_host(ip.address))
                    .map(|ip| (ip.address, port.id))
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect();
        assigned.sort_by_key(|(address, _)| *address);
        assigned
    }

    fn ipv4_in_use(&self, address: Ipv4Addr) -> bool {
        self.ports.values().any(|port| match &port.config {
            PortConfig::Server(config) => config.addresses().any(|ip| ip.address == address),
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
}
