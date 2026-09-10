use crate::VlanId;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ipv4InterfaceConfig {
    pub address: Ipv4Addr,
    pub prefix: u8,
    pub gateway: Option<Ipv4Addr>,
    pub vlan: VlanId,
}

impl Ipv4InterfaceConfig {
    pub fn new(address: Ipv4Addr, prefix: u8, gateway: Option<Ipv4Addr>, vlan: VlanId) -> Self {
        Self {
            address,
            prefix,
            gateway,
            vlan,
        }
    }

    pub fn contains(&self, address: Ipv4Addr) -> bool {
        same_subnet(self.address, address, self.prefix)
    }

    pub fn validate(&self) -> bool {
        self.prefix <= 32 && !self.address.is_unspecified() && self.vlan.0 > 0 && self.vlan.0 < 4095
    }
}

pub fn same_subnet(a: Ipv4Addr, b: Ipv4Addr, prefix: u8) -> bool {
    if prefix > 32 {
        return false;
    }
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    (u32::from(a) & mask) == (u32::from(b) & mask)
}
