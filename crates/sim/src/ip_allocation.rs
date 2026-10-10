use crate::{
    Ipv4InterfaceConfig, NetworkOutletKind, NetworkSim, PortConfig, PortId, SimError, VlanId,
};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

/// Purchased address inventory. The convenience assignment reserves the
/// first usable address for a gateway; ownership alone does not install routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicIpv4Block {
    pub network: Ipv4Addr,
    #[serde(default = "legacy_prefix")]
    pub prefix: u8,
    /// Legacy purchase provenance, never a routing constraint. Zero for new purchases.
    pub uplink: PortId,
}

fn legacy_prefix() -> u8 {
    PublicIpv4Block::PREFIX
}

impl PublicIpv4Block {
    pub const PREFIX: u8 = 29;
    pub const PRICE: i64 = 240;
    pub const PRICE_24: i64 = Self::PRICE * 32;

    pub fn range(self) -> crate::Ipv4Prefix {
        crate::Ipv4Prefix::new(self.network, self.prefix).expect("purchased IPv4 prefix")
    }

    pub fn gateway(self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.network) + 1)
    }

    pub fn host_addresses(self) -> impl Iterator<Item = Ipv4Addr> {
        (u32::from(self.network) + 2..self.range().last()).map(Ipv4Addr::from)
    }

    pub fn contains_host(self, address: Ipv4Addr) -> bool {
        self.range().usable(address) && address != self.gateway()
    }
}

impl NetworkSim {
    /// Count only ranges that are not already covered by any provider pool.
    pub fn available_public_ipv4_pools(&self) -> usize {
        self.available_public_ipv4_subnets(29)
    }

    pub fn available_public_ipv4_subnets(&self, length: u8) -> usize {
        public_subnet_inventory(length)
            .filter(|prefix| self.public_subnet_available(*prefix))
            .count()
    }

    fn public_subnet_available(&self, prefix: crate::Ipv4Prefix) -> bool {
        !self
            .provider()
            .pools()
            .iter()
            .any(|pool| pool.prefix.contains_prefix(prefix) || prefix.contains_prefix(pool.prefix))
            && !self.public_ipv4_blocks.iter().any(|block| {
                block.range().contains_prefix(prefix) || prefix.contains_prefix(block.range())
            })
    }
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
        self.purchase_public_ipv4_pool(uplink, 29)
    }

    pub fn buy_public_ipv4_pool(&mut self) -> Result<PublicIpv4Block, SimError> {
        self.buy_public_ipv4_subnet(29)
    }

    pub fn buy_public_ipv4_subnet(&mut self, length: u8) -> Result<PublicIpv4Block, SimError> {
        self.purchase_public_ipv4_pool(PortId(0), length)
    }

    fn purchase_public_ipv4_pool(
        &mut self,
        provenance: PortId,
        length: u8,
    ) -> Result<PublicIpv4Block, SimError> {
        let price = match length {
            29 => PublicIpv4Block::PRICE,
            24 => PublicIpv4Block::PRICE_24,
            _ => return Err(SimError::InvalidIpv4Prefix),
        };
        if self.money < price {
            return Err(SimError::InsufficientFunds {
                needed: price,
                available: self.money,
            });
        }
        // Documentation and benchmark ranges stay inside the existing simulated
        // network; ownership does not install any public routes.
        let range = public_subnet_inventory(length)
            .find(|prefix| self.public_subnet_available(*prefix))
            .ok_or(SimError::PublicIpv4Exhausted)?;
        let block = PublicIpv4Block {
            network: range.network(),
            prefix: length,
            uplink: provenance,
        };
        self.execute(crate::Command::Provider(crate::ProviderCommand::AddPool(
            crate::AddressPool {
                prefix: range,
                description: format!("Purchased /{length} address inventory"),
            },
        )))?;
        self.money -= price;
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
            .is_some_and(|p| matches!(p.config, PortConfig::Server(_)))
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
            .find(|address| !self.hosting_occupied_addresses().contains(address))
            .ok_or(SimError::PublicIpv4Exhausted)?;
        self.set_ipv4(
            port,
            Ipv4InterfaceConfig::new(address, block.prefix, Some(block.gateway()), VlanId(1)),
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
            .find(|address| !self.hosting_occupied_addresses().contains(address))
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

    fn server_address(&self, port: PortId) -> Option<Ipv4Addr> {
        let PortConfig::Server(config) = &self.port(port)?.config else {
            return None;
        };
        config.ipv4.as_ref().map(|ip| ip.address)
    }
}

fn public_subnet_inventory(length: u8) -> impl Iterator<Item = crate::Ipv4Prefix> {
    // Keep existing /29 inventory stable. Additional /24s use the isolated
    // benchmark space, so a previously bought /29 never prevents a /24 purchase.
    let networks: Vec<_> = match length {
        29 => (0..32).map(|i| Ipv4Addr::new(203, 0, 113, i * 8)).collect(),
        24 => std::iter::once(Ipv4Addr::new(203, 0, 113, 0))
            .chain((0..32).map(|i| Ipv4Addr::new(198, 18, i, 0)))
            .collect(),
        _ => Vec::new(),
    };
    networks
        .into_iter()
        .map(move |network| crate::Ipv4Prefix::new(network, length).expect("shop subnet prefix"))
}
