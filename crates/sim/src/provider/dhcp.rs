use crate::*;

impl NetworkSim {
    /// Semantic DHCP exchange over real Ethernet forwarding; no implicit room server.
    pub fn request_dhcp(&mut self, client: PortId) -> Result<std::net::Ipv4Addr, SimError> {
        if !matches!(
            self.port(client).map(|p| &p.config),
            Some(PortConfig::Server(_))
        ) {
            return Err(SimError::WrongPortType);
        }
        let requests = self.transmit_frame(
            client,
            EthernetFrame {
                qos: crate::FrameQos::default(),
                source: MacAddress::for_port(client),
                destination: MacAddress([255; 6]),
                vlan: None,
                payload: EthernetPayload::DhcpDiscover,
            },
        );
        let mut servers: Vec<_> = self
            .provider
            .dhcp
            .iter()
            .copied()
            .filter(|pool| {
                requests
                    .iter()
                    .any(|d| d.port == pool.server && d.vlan == pool.vlan)
            })
            .collect();
        servers.sort_by_key(|p| (p.server, p.vlan));
        let mut occupied = self.occupied_ipv4_addresses(client);
        for pool in servers {
            if let Some(gateway) = pool.gateway {
                occupied.insert(gateway);
            }
            let previous = self
                .provider
                .leases
                .iter()
                .find(|l| {
                    l.client == client
                        && l.server == pool.server
                        && l.vlan == pool.vlan
                        && l.address >= pool.first
                        && l.address <= pool.last
                        && !occupied.contains(&l.address)
                })
                .map(|l| l.address);
            let address = previous.or_else(|| {
                let mut value = u32::from(pool.first);
                for used in occupied.range(pool.first..=pool.last) {
                    if u32::from(*used) == value {
                        value = value.checked_add(1)?;
                    } else if u32::from(*used) > value {
                        break;
                    }
                }
                (value <= u32::from(pool.last)).then_some(value.into())
            });
            let Some(address) = address.filter(|a| Some(*a) != pool.gateway) else {
                continue;
            };
            let replies = self.transmit_frame(
                pool.server,
                EthernetFrame {
                    qos: crate::FrameQos::default(),
                    source: MacAddress::for_port(pool.server),
                    destination: MacAddress::for_port(client),
                    vlan: self.wire_vlan_for(pool.server, pool.vlan),
                    payload: EthernetPayload::DhcpOffer { address },
                },
            );
            if !replies.iter().any(|d| d.port == client) {
                continue;
            }
            self.set_ipv4(
                client,
                Ipv4InterfaceConfig {
                    vlan: None,
                    ..Ipv4InterfaceConfig::new(
                        address,
                        pool.prefix.length(),
                        pool.gateway,
                        VlanId(1),
                    )
                },
            )?;
            self.provider.leases.retain(|l| l.client != client);
            self.provider.leases.push(DhcpLease {
                client,
                server: pool.server,
                vlan: pool.vlan,
                address,
            });
            return Ok(address);
        }
        Err(SimError::Provider(
            "no DHCP lease available: no reachable configured server or pool exhausted".into(),
        ))
    }
}
