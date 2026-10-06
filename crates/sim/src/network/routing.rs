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

/// Normalize legacy records that represented untagged VLAN 1 as both None and Some(1).
/// Prefer an addressed record over a placeholder, then the latest configuration.
pub(crate) fn normalize_router_interfaces(interfaces: &mut Vec<RouterInterface>) {
    let mut selected = std::collections::BTreeMap::new();
    for (index, interface) in interfaces.iter().enumerate() {
        let key = (interface.port, interface.vlan.unwrap_or(VlanId(1)));
        let candidate = (interface.address.is_some(), index);
        selected
            .entry(key)
            .and_modify(|current| {
                if candidate > *current {
                    *current = candidate;
                }
            })
            .or_insert(candidate);
    }
    let mut index = 0;
    interfaces.retain(|interface| {
        let keep = selected[&(interface.port, interface.vlan.unwrap_or(VlanId(1)))].1 == index;
        index += 1;
        keep
    });
}
