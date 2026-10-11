use super::*;
use std::collections::BTreeSet;
use std::net::Ipv4Addr;

impl NetworkSim {
    /// Pair each permitted slice with its product range for allocation ownership.
    pub(crate) fn hosting_node_product_ranges(
        &self,
        node: DeviceId,
        product: &HostingProduct,
    ) -> Vec<(Ipv4Prefix, Ipv4Prefix)> {
        let policy = self
            .hosting
            .nodes
            .get(&node)
            .and_then(|n| n.ipv4_ranges.as_ref());
        let mut ranges = BTreeSet::new();
        for product_range in &product.ipv4_ranges {
            if let Some(policy) = policy {
                for node_range in policy {
                    if product_range.contains_prefix(*node_range) {
                        ranges.insert((*node_range, *product_range));
                    } else if node_range.contains_prefix(*product_range) {
                        ranges.insert((*product_range, *product_range));
                    }
                }
            } else {
                ranges.insert((*product_range, *product_range));
            }
        }
        ranges.into_iter().collect()
    }

    pub(crate) fn auto_configure_hosting_nodes(&mut self) {
        let nodes: Vec<_> = self.hosting.nodes.keys().copied().collect();
        for node in nodes {
            if !self.hosting_node_eligible(node) {
                continue;
            }
            self.configure_hosting_vlan_interfaces(node);
            // Explicit rules attach guests to a port/VLAN. They must not consume
            // a public address for the host or depend on a host IPv4 alias.
            if self.hosting.nodes[&node].networks.is_some() {
                self.release_unused_node_addresses(node, &BTreeSet::new());
                continue;
            }
            let ranges = self.hosting.nodes[&node]
                .ipv4_ranges
                .clone()
                .unwrap_or_else(|| {
                    self.hosting
                        .products
                        .values()
                        .filter(|p| p.published && self.hosting_product_nodes(p).contains(&node))
                        .flat_map(|p| p.ipv4_ranges.iter().copied())
                        .collect()
                });
            self.release_unused_node_addresses(node, &ranges);
            let restricted = self.hosting.nodes[&node].ipv4_ranges.is_some();
            for range in ranges {
                if !self.provider.owns(range) {
                    continue;
                }
                let configured = self.device(node).unwrap().ports().iter().any(|id| {
                    let port = self.port(*id).unwrap();
                    port.name != "mgmt0" && self.port_link_up(*id)
                        && matches!(&port.config, PortConfig::Server(c) if c.addresses().any(|a|
                            (!restricted || range.usable(a.address))
                                && Ipv4Prefix::new(a.address, a.prefix).is_ok_and(|r| r.contains_prefix(range))
                                && a.gateway.is_some_and(|g| a.contains(g))
                        ))
                });
                if configured {
                    continue;
                }
                let Some((port, template)) = self.find_node_gateway(node, range, None) else {
                    continue;
                };
                let PortConfig::Server(original) = &self.port(port).unwrap().config else {
                    continue;
                };
                let mut config = original.clone();
                // The existing allocator reserves a unique address in the slice.
                // The actual router subnet, which can be larger, defines its NIC mask.
                let mut gateways: BTreeSet<_> = self
                    .provider()
                    .pools()
                    .iter()
                    .map(|p| Ipv4Addr::from(u32::from(p.prefix.network()) + 1))
                    .collect();
                gateways.extend(self.ports().flat_map(|p| match &p.config {
                    PortConfig::Server(c) => c.addresses().filter_map(|a| a.gateway).collect(),
                    _ => Vec::new(),
                }));
                // Keep addresses on other VLANs, including existing guest uplinks.
                gateways.extend(
                    config
                        .addresses()
                        .filter(|a| a.vlan != template.vlan)
                        .map(|a| a.address),
                );
                let Ok(address) =
                    self.allocate_ipv4_reserved(port, range, None, template.vlan, &gateways)
                else {
                    continue;
                };
                let replacement = Ipv4InterfaceConfig {
                    address,
                    ..template
                };
                let existed = config.addresses().any(|a| a.address == address);
                if config.ipv4.as_ref().is_none_or(|a| a.address == address) {
                    config.ipv4 = Some(replacement);
                } else if let Some(alias) = config
                    .additional_ipv4
                    .iter_mut()
                    .find(|a| a.address == address)
                {
                    *alias = replacement;
                } else {
                    config.additional_ipv4.push(replacement);
                }
                self.ports.get_mut(&port).unwrap().config = PortConfig::Server(config);
                if !existed {
                    self.hosting
                        .nodes
                        .get_mut(&node)
                        .unwrap()
                        .auto_addresses
                        .insert(address, port);
                }
                self.routing_revision += 1;
            }
        }
    }

    pub(super) fn find_node_gateway(
        &self,
        node: DeviceId,
        range: Ipv4Prefix,
        mapping: Option<HostingNodeNetwork>,
    ) -> Option<(PortId, Ipv4InterfaceConfig)> {
        let mut data_ports = self.device(node)?.ports().to_vec();
        data_ports.sort();
        let mut gateways: Vec<_> =
            self.ports()
                .flat_map(|port| match &port.config {
                    PortConfig::Router(c) => {
                        c.interfaces
                            .iter()
                            .filter_map(|i| {
                                let address = i.address?;
                                let subnet = Ipv4Prefix::new(address, i.prefix).ok()?;
                                (subnet.usable(address) && subnet.contains_prefix(range)).then_some(
                                    (port.id, address, i.prefix, i.vlan.unwrap_or(VlanId(1))),
                                )
                            })
                            .collect::<Vec<_>>()
                    }
                    _ => Vec::new(),
                })
                .collect();
        gateways.sort();
        for port in data_ports {
            let p = self.port(port)?;
            if p.name == "mgmt0"
                || !self.port_link_up(port)
                || mapping.is_some_and(|m| m.port != port)
            {
                continue;
            }
            let PortConfig::Server(config) = &p.config else {
                continue;
            };
            let vlan = mapping.and_then(|m| m.vlan).or_else(|| {
                config
                    .addresses()
                    .find(|a| !config.tagged_vlans.contains(&a.vlan.unwrap_or(VlanId(1))))
                    .and_then(|a| a.vlan)
            });
            for (router_port, gateway, mask, gateway_vlan) in &gateways {
                if *gateway_vlan != vlan.unwrap_or(VlanId(1)) {
                    continue;
                }
                if self.discover_ipv4_neighbor(port, *gateway, vlan.unwrap_or(VlanId(1)))
                    == Some(MacAddress::for_port(*router_port))
                {
                    return Some((
                        port,
                        Ipv4InterfaceConfig {
                            address: Ipv4Addr::UNSPECIFIED,
                            prefix: *mask,
                            gateway: Some(*gateway),
                            vlan,
                        },
                    ));
                }
            }
        }
        None
    }

    fn release_unused_node_addresses(&mut self, node: DeviceId, ranges: &BTreeSet<Ipv4Prefix>) {
        let old = self.hosting.nodes[&node].auto_addresses.clone();
        for (address, port) in old {
            let allowed = self.port(port).is_some_and(|p| match &p.config {
                PortConfig::Server(c) => {
                    c.addresses().any(|a| a.address == address)
                        && ranges.iter().any(|r| r.usable(address))
                }
                _ => false,
            });
            if allowed
                || self.hosting.vms.values().any(|vm| {
                    vm.host == node
                        && vm
                            .interfaces
                            .iter()
                            .any(|v| self.virtual_uplink(*v) == Some(port))
                })
            {
                continue;
            }
            if let Some(Port {
                config: PortConfig::Server(config),
                ..
            }) = self.ports.get_mut(&port)
            {
                config.additional_ipv4.retain(|a| a.address != address);
                if config.ipv4.as_ref().is_some_and(|a| a.address == address) {
                    config.ipv4 = if config.additional_ipv4.is_empty() {
                        None
                    } else {
                        Some(config.additional_ipv4.remove(0))
                    };
                }
            }
            self.hosting
                .nodes
                .get_mut(&node)
                .unwrap()
                .auto_addresses
                .remove(&address);
            self.routing_revision += 1;
        }
    }
}
