use crate::*;

#[derive(Default)]
pub(super) struct ManagementChecks {
    pub switch: Option<DeviceId>,
    pub cabled: bool,
    pub vlan: bool,
    pub addressed: bool,
    pub reachable: bool,
}
impl NetworkSim {
    pub(super) fn guide_management(
        &self,
        controller: Option<DeviceId>,
        node: Option<DeviceId>,
    ) -> ManagementChecks {
        let (Some(controller), Some(node)) = (controller, node) else {
            return ManagementChecks::default();
        };
        let ports = |device| {
            self.device(device)
                .map(|d| {
                    d.ports()
                        .iter()
                        .copied()
                        .filter(|id| {
                            self.port(*id).is_some_and(|p| {
                                p.name != "mgmt0" && matches!(p.config, PortConfig::Server(_))
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        let endpoint = |port| {
            let status = self.link_status(port);
            let peer = status.path.last().copied().filter(|id| {
                self.port(*id)
                    .is_some_and(|p| matches!(p.config, PortConfig::Switch(_)))
            });
            (peer, status.speed.is_some())
        };
        let registration = self.hosting.nodes.get(&node).and_then(|n| n.manager);
        let private_pair = |a, b| {
            let addresses = |port| match &self.port(port).unwrap().config {
                PortConfig::Server(c) => {
                    let values: Vec<_> = c
                        .addresses()
                        .filter(|a| a.address.is_private())
                        .map(|a| Some(a.clone()))
                        .collect();
                    if values.is_empty() {
                        vec![None]
                    } else {
                        values
                    }
                }
                _ => vec![None],
            };
            let ca = addresses(a);
            let na = addresses(b);
            ca.iter()
                .flat_map(|c| na.iter().map(move |n| (c, n)))
                .max_by_key(|(c, n)| {
                    let registered = registration.is_some_and(|r| {
                        r.server == controller && c.as_ref().is_some_and(|c| c.address == r.address)
                    });
                    let compatible = c.as_ref().zip(n.as_ref()).is_some_and(|(c, n)| {
                        c.address != n.address && c.contains(n.address) && n.contains(c.address)
                    });
                    (
                        u32::from(registered) * 4 + u32::from(compatible) * 2,
                        std::cmp::Reverse(c.as_ref().map(|c| c.address)),
                        std::cmp::Reverse(n.as_ref().map(|n| n.address)),
                    )
                })
                .map(|(c, n)| (c.clone(), n.clone()))
                .unwrap()
        };
        let mut pairs = Vec::new();
        for a in ports(controller) {
            for b in ports(node) {
                let (ap, au) = endpoint(a);
                let (bp, bu) = endpoint(b);
                let switch = ap
                    .zip(bp)
                    .filter(|(ap, bp)| {
                        self.port(*ap).unwrap().device == self.port(*bp).unwrap().device
                    })
                    .map(|(ap, _)| self.port(ap).unwrap().device);
                let (ca, na) = private_pair(a, b);
                let addresses = ca.as_ref().zip(na.as_ref()).filter(|(c, n)| {
                    c.address != n.address && c.contains(n.address) && n.contains(c.address)
                });
                let score = u32::from(addresses.is_some()) * 100
                    + u32::from(registration.is_some_and(|r| {
                        r.server == controller
                            && ca.as_ref().is_some_and(|c| c.address == r.address)
                    })) * 200
                    + u32::from(switch.is_some()) * 20
                    + u32::from(self.port(a).unwrap().name == "eth1") * 2
                    + u32::from(self.port(b).unwrap().name == "eth1") * 2;
                pairs.push((std::cmp::Reverse(score), a, b, ap, bp, au && bu, switch));
            }
        }
        pairs.sort();
        let Some((_, a, b, ap, bp, up, switch)) = pairs.first().copied() else {
            return ManagementChecks::default();
        };
        let (ca, na) = private_pair(a, b);
        let native_vlan = |switch_port, server_port, address: &Option<Ipv4InterfaceConfig>| {
            let wire = match &self.port(server_port)?.config {
                PortConfig::Server(c) => address
                    .as_ref()
                    .and_then(|a| a.vlan)
                    .filter(|v| c.tagged_vlans.contains(v)),
                _ => None,
            };
            if !self.endpoint_link_vlan_matches(server_port, switch_port, wire) {
                return None;
            }
            match &self.port(switch_port)?.config {
                PortConfig::Switch(c) => match &c.mode {
                    SwitchPortMode::Access { vlan } if wire.is_none() => {
                        Some(vlan.unwrap_or(VlanId(1)))
                    }
                    SwitchPortMode::Trunk {
                        native_vlan,
                        allowed,
                    } => wire.or(*native_vlan).filter(|v| allowed.contains(v)),
                    _ => None,
                },
                _ => None,
            }
        };
        let vlan = switch
            .and_then(|id| {
                let vlan = native_vlan(ap?, a, &ca)?;
                if vlan == VlanId(1) || native_vlan(bp?, b, &na) != Some(vlan) {
                    return None;
                }
                let declared = match &self.device(id)?.kind {
                    DeviceKind::Switch(s) => &s.vlans,
                    DeviceKind::Router(r) => &r.vlans,
                    _ => return None,
                };
                declared.iter().any(|v| v.id == vlan).then_some(vlan)
            })
            .is_some();
        let addressed = ca.as_ref().zip(na.as_ref()).is_some_and(|(ca, na)| {
            ca.address != na.address && ca.contains(na.address) && na.contains(ca.address)
                && [ca.address, na.address].iter().all(|address| self.ports().filter(|p| matches!(&p.config, PortConfig::Server(c) if c.addresses().any(|a| a.address == *address))).count() == 1)
        });
        let reachable = addressed
            && ca.as_ref().zip(na.as_ref()).is_some_and(|(ca, na)| {
                let route_a = self.server_route_selection(controller, na.address, None);
                let route_b = self.server_route_selection(node, ca.address, None);
                route_a.is_some_and(|r| {
                    r.port == a
                        && r.source.address.is_private()
                        && self
                            .probe_ipv4_address(a, r.source.address, na.address)
                            .reachable
                }) && route_b.is_some_and(|r| {
                    r.port == b
                        && r.source.address.is_private()
                        && self
                            .probe_ipv4_address(b, r.source.address, ca.address)
                            .reachable
                })
            });
        ManagementChecks {
            switch,
            cabled: switch.is_some() && up,
            vlan: vlan && up,
            addressed,
            reachable,
        }
    }
}
