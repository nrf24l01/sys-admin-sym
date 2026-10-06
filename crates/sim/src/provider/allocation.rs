use crate::*;
use std::{collections::BTreeSet, net::Ipv4Addr};

impl NetworkSim {
    /// Shared reservations for both owned-address allocation and DHCP.
    pub(crate) fn occupied_ipv4_addresses(&self, except: PortId) -> BTreeSet<Ipv4Addr> {
        let mut occupied = BTreeSet::new();
        for endpoint in self.ports() {
            if endpoint.id == except {
                continue;
            }
            match &endpoint.config {
                PortConfig::Server(c) => occupied.extend(c.addresses().map(|a| a.address)),
                PortConfig::Router(c) => {
                    occupied.extend(c.interfaces.iter().filter_map(|i| i.address))
                }
                _ => {}
            }
        }
        occupied.extend(self.provider.circuits().iter().map(|c| c.address));
        occupied.extend(
            self.devices()
                .filter_map(|d| self.switch_management(d.id).map(|m| m.address)),
        );
        occupied.extend(
            self.provider
                .leases()
                .iter()
                .filter(|l| l.client != except)
                .map(|l| l.address),
        );
        occupied
    }

    /// Allocate an address from owned inventory without inventing connectivity.
    pub fn allocate_ipv4(
        &mut self,
        port: PortId,
        prefix: Ipv4Prefix,
        gateway: Option<Ipv4Addr>,
        vlan: Option<VlanId>,
    ) -> Result<Ipv4Addr, SimError> {
        if !self.provider.owns(prefix) {
            return Err(SimError::PublicIpv4BlockNotOwned);
        }
        let Some(Port {
            config: PortConfig::Server(config),
            ..
        }) = self.port(port)
        else {
            return Err(SimError::WrongPortType);
        };
        if gateway.is_some_and(|g| !prefix.usable(g)) {
            return Err(SimError::InvalidIpv4Gateway);
        }
        if vlan.is_some_and(|v| !(1..4095).contains(&v.0)) {
            return Err(SimError::InvalidIpv4Vlan);
        }
        let existing = config
            .addresses()
            .find(|a| prefix.usable(a.address) && Some(a.address) != gateway)
            .map(|a| a.address);
        let mut occupied = self.occupied_ipv4_addresses(port);
        occupied.extend(gateway);
        let first = u32::from(prefix.network()) + u32::from(prefix.length() < 31);
        let last = prefix.last() - u32::from(prefix.length() < 31);
        let mut candidate = first;
        for address in occupied.range(Ipv4Addr::from(first)..=Ipv4Addr::from(last)) {
            let value = u32::from(*address);
            if value == candidate {
                candidate = candidate
                    .checked_add(1)
                    .ok_or(SimError::PublicIpv4Exhausted)?;
            } else if value > candidate {
                break;
            }
        }
        let address = existing
            .filter(|a| !occupied.contains(a))
            .or_else(|| (candidate <= last).then(|| Ipv4Addr::from(candidate)))
            .ok_or(SimError::PublicIpv4Exhausted)?;
        self.set_ipv4(
            port,
            Ipv4InterfaceConfig {
                address,
                prefix: prefix.length(),
                gateway,
                vlan,
            },
        )?;
        Ok(address)
    }
}
