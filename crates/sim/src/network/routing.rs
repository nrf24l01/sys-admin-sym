use crate::{PortId, VlanId};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouterInterface {
    pub name: String,
    pub port: PortId,
    pub vlan: Option<VlanId>,
    pub address: Option<Ipv4Addr>,
    pub prefix: u8,
    pub dhcp: bool,
    pub internet_connected: bool,
}

impl RouterInterface {
    pub fn lan(
        name: impl Into<String>,
        port: PortId,
        vlan: VlanId,
        address: Ipv4Addr,
        prefix: u8,
    ) -> Self {
        Self {
            name: name.into(),
            port,
            vlan: Some(vlan),
            address: Some(address),
            prefix,
            dhcp: false,
            internet_connected: false,
        }
    }

    pub fn wan(name: impl Into<String>, port: PortId) -> Self {
        Self {
            name: name.into(),
            port,
            vlan: None,
            address: None,
            prefix: 0,
            dhcp: true,
            internet_connected: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Route {
    pub network: Ipv4Addr,
    pub prefix: u8,
    pub via: Option<Ipv4Addr>,
    pub egress: PortId,
}
