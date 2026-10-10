use super::*;

pub(super) struct HostingNetworkAttachment {
    pub uplink: PortId,
    pub template: Ipv4InterfaceConfig,
    pub ranges: Vec<(Ipv4Prefix, Ipv4Prefix)>,
    pub tagged: bool,
}

impl NetworkSim {
    pub(super) fn validate_hosting_node_networks(
        &self,
        node: DeviceId,
        networks: Option<&[HostingNodeNetwork]>,
    ) -> Result<(), String> {
        let device = self
            .device(node)
            .filter(|d| matches!(d.kind, DeviceKind::Server(_)))
            .ok_or("Select a physical virtualization node")?;
        if self.hosting_provider_installed(node) {
            return Err("The main hoster server is not a virtualization node".into());
        }
        let Some(networks) = networks else {
            return Ok(());
        };
        for (i, network) in networks.iter().enumerate() {
            if !device.ports().contains(&network.port)
                || !self
                    .port(network.port)
                    .is_some_and(|p| p.name != "mgmt0" && matches!(p.config, PortConfig::Server(_)))
            {
                return Err(
                    "Hosting networks must use this node's physical data interfaces".into(),
                );
            }
            if network.subnet.length() > 30 || !self.provider.owns(network.subnet) {
                return Err("Select an owned IPv4 subnet (/30 or larger)".into());
            }
            if network.vlan.is_some_and(|v| !(1..4095).contains(&v.0)) {
                return Err("VLAN IDs must be between 1 and 4094".into());
            }
            if networks[..i].iter().any(|other| {
                other.subnet.contains_prefix(network.subnet)
                    || network.subnet.contains_prefix(other.subnet)
            }) {
                return Err("A node's hosting subnet mappings must not overlap".into());
            }
        }
        Ok(())
    }

    /// Guest allocation rules resolve a bridge/VLAN independently of the host's
    /// own IP configuration. The selected wire must have a real reachable router.
    pub(super) fn hosting_network_attachments(
        &self,
        node: DeviceId,
        ranges: &[(Ipv4Prefix, Ipv4Prefix)],
    ) -> Vec<HostingNetworkAttachment> {
        let mut result: Vec<HostingNetworkAttachment> = Vec::new();
        if let Some(networks) = &self.hosting.nodes[&node].networks {
            for network in networks {
                let slices: Vec<_> = ranges
                    .iter()
                    .filter_map(|(range, owner)| {
                        subnet_intersection(*range, network.subnet).map(|slice| (slice, *owner))
                    })
                    .collect();
                let discovery_ranges: Vec<_> = if slices.is_empty() {
                    vec![network.subnet]
                } else {
                    slices.iter().map(|(range, _)| *range).collect()
                };
                for range in discovery_ranges {
                    let Some((uplink, mut template)) =
                        self.find_node_gateway(node, range, Some(*network))
                    else {
                        continue;
                    };
                    // This is a guest subnet template, not a host address. The
                    // ARP probe used 0.0.0.0; use the discovered router to define
                    // subnet membership until provisioning inserts the guest IP.
                    template.address = template.gateway.unwrap();
                    if let Some(existing) = result
                        .iter_mut()
                        .find(|a| a.uplink == uplink && a.template == template)
                    {
                        existing.ranges.extend(&slices);
                        existing.ranges.sort();
                        existing.ranges.dedup();
                        continue;
                    }
                    result.push(HostingNetworkAttachment {
                        uplink,
                        template,
                        ranges: slices.clone(),
                        tagged: network.vlan.is_some(),
                    });
                }
            }
        } else {
            let mut ports = self.device(node).unwrap().ports().to_vec();
            ports.sort();
            for uplink in ports {
                let Some(port) = self
                    .port(uplink)
                    .filter(|p| p.name != "mgmt0" && self.port_link_up(uplink))
                else {
                    continue;
                };
                let PortConfig::Server(config) = &port.config else {
                    continue;
                };
                for template in config
                    .addresses()
                    .filter(|a| a.gateway.is_some_and(|g| a.contains(g)))
                {
                    result.push(HostingNetworkAttachment {
                        uplink,
                        template: template.clone(),
                        ranges: ranges.to_vec(),
                        tagged: config
                            .tagged_vlans
                            .contains(&template.vlan.unwrap_or(VlanId(1))),
                    });
                }
            }
        }
        result
    }

    /// Hosting controls VLAN bridge transport without assigning host IPs.
    /// Keep any VLAN carrying existing guests when the rules are edited.
    pub(super) fn configure_hosting_vlan_interfaces(&mut self, node: DeviceId) {
        let mut tags: std::collections::BTreeMap<PortId, std::collections::BTreeSet<VlanId>> =
            Default::default();
        if let Some(networks) = &self.hosting.nodes[&node].networks {
            for network in networks {
                if let Some(vlan) = network.vlan {
                    tags.entry(network.port).or_default().insert(vlan);
                }
            }
        }
        for vm in self.hosting.vms.values().filter(|v| v.host == node) {
            for port in &vm.interfaces {
                if let Some(uplink) = self.virtual_uplink(*port)
                    && let Some(Port {
                        config: PortConfig::Server(c),
                        ..
                    }) = self.port(*port)
                {
                    tags.entry(uplink).or_default().extend(&c.tagged_vlans);
                }
            }
        }
        let ports = self.device(node).unwrap().ports().to_vec();
        for port in ports {
            if let Some(Port {
                name,
                config: PortConfig::Server(c),
                ..
            }) = self.ports.get_mut(&port)
                && name != "mgmt0"
            {
                let desired = tags.remove(&port).unwrap_or_default();
                if c.tagged_vlans != desired {
                    c.tagged_vlans = desired;
                    self.routing_revision += 1;
                }
            }
        }
    }
}

pub(super) fn subnet_intersection(a: Ipv4Prefix, b: Ipv4Prefix) -> Option<Ipv4Prefix> {
    if a.contains_prefix(b) {
        Some(b)
    } else if b.contains_prefix(a) {
        Some(a)
    } else {
        None
    }
}
