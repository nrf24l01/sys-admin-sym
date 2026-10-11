use super::*;
use std::collections::BTreeSet;

pub(super) fn validate_policy(policy: &HypervisorPolicy) -> Result<(), String> {
    for ratio in [
        policy.cpu_overcommit_percent,
        policy.ram_overcommit_percent,
        policy.storage_overcommit_percent,
        policy.io_overcommit_percent,
        policy.network_overcommit_percent,
    ] {
        if !(100..=10000).contains(&ratio) {
            return Err("Overcommit must be between 100% and 10000%".into());
        }
    }
    Ok(())
}

impl NetworkSim {
    pub fn hosting_node_group(&self, node: DeviceId) -> Option<&HostingNodeGroup> {
        self.hosting
            .node_groups
            .values()
            .find(|g| g.nodes.contains(&node))
    }

    /// Resolve live membership, deduplicating explicit and grouped selections.
    pub fn hosting_product_nodes(&self, product: &HostingProduct) -> BTreeSet<DeviceId> {
        let mut nodes = product.eligible_nodes.clone();
        for id in &product.eligible_groups {
            if let Some(group) = self.hosting.node_groups.get(id) {
                nodes.extend(&group.nodes);
            }
        }
        nodes
    }

    pub(super) fn check_hosting_group_override(&self, node: DeviceId) -> Result<(), String> {
        if self.hosting_node_group(node).is_some() {
            Err("Edit shared settings in the node's group or remove it from the group first".into())
        } else {
            Ok(())
        }
    }

    fn validate_group_member(&self, node: DeviceId) -> Result<(), String> {
        if !matches!(
            self.device(node).map(|d| &d.kind),
            Some(DeviceKind::Server(_))
        ) || self.hosting_provider_installed(node)
        {
            return Err("Group members must be physical virtualization nodes".into());
        }
        Ok(())
    }

    fn group_node_networks(
        &self,
        group: &HostingNodeGroup,
        node: DeviceId,
    ) -> Result<Option<Vec<HostingNodeNetwork>>, String> {
        let Some(networks) = &group.networks else {
            return Ok(None);
        };
        let device = self.device(node).ok_or("Unknown group member")?;
        let resolved = networks
            .iter()
            .map(|network| {
                let port = device
                    .ports()
                    .iter()
                    .copied()
                    .find(|id| {
                        self.port(*id).is_some_and(|p| {
                            p.name == network.interface
                                && p.name != "mgmt0"
                                && matches!(p.config, PortConfig::Server(_))
                        }) && self.virtual_uplink(*id).is_none()
                    })
                    .ok_or_else(|| {
                        format!(
                            "Node {} is missing data interface {}",
                            device.name, network.interface
                        )
                    })?;
                Ok(HostingNodeNetwork {
                    port,
                    subnet: network.subnet,
                    vlan: network.vlan,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        self.validate_hosting_node_networks(node, Some(&resolved))?;
        Ok(Some(resolved))
    }

    pub fn validate_hosting_node_group(&self, group: &HostingNodeGroup) -> Result<(), String> {
        self.validate_group(group, None)
    }

    fn validate_group(
        &self,
        group: &HostingNodeGroup,
        moving: Option<DeviceId>,
    ) -> Result<(), String> {
        if group.id != 0 && !self.hosting.node_groups.contains_key(&group.id) {
            return Err("Unknown node group".into());
        }
        if group.name.trim().is_empty() || group.name.len() > 100 {
            return Err("Group name must contain 1–100 bytes of text".into());
        }
        validate_policy(&group.policy)?;
        if group.ipv4_ranges.as_ref().is_some_and(|ranges| {
            ranges
                .iter()
                .any(|r| r.length() > 30 || !self.provider.owns(*r))
        }) {
            return Err("Group ranges must be owned IPv4 subnets (/30 or larger)".into());
        }
        // Empty groups still need valid rules, even without a port to resolve yet.
        if let Some(networks) = &group.networks {
            for (i, network) in networks.iter().enumerate() {
                if network.interface.trim().is_empty()
                    || network.interface.len() > 100
                    || network.interface == "mgmt0"
                {
                    return Err("Select a physical data interface name for group networks".into());
                }
                if network.subnet.length() > 30 || !self.provider.owns(network.subnet) {
                    return Err("Group networks must use owned IPv4 subnets (/30 or larger)".into());
                }
                if network.vlan.is_some_and(|v| !(1..4095).contains(&v.0)) {
                    return Err("VLAN IDs must be between 1 and 4094".into());
                }
                if networks[..i].iter().any(|n| {
                    n.subnet.contains_prefix(network.subnet)
                        || network.subnet.contains_prefix(n.subnet)
                }) {
                    return Err("A group's hosting subnet mappings must not overlap".into());
                }
            }
        }
        for node in &group.nodes {
            self.validate_group_member(*node)?;
            if Some(*node) != moving
                && self
                    .hosting_node_group(*node)
                    .is_some_and(|g| g.id != group.id)
            {
                return Err("Node already belongs to another group; move it using the node's group selector".into());
            }
            self.group_node_networks(group, *node)?;
        }
        Ok(())
    }

    fn apply_group_member(
        &mut self,
        group: &HostingNodeGroup,
        node: DeviceId,
        networks: Option<Vec<HostingNodeNetwork>>,
    ) {
        let config = self.hosting.nodes.entry(node).or_default();
        config.policy = group.policy.clone();
        config.ipv4_ranges = group.ipv4_ranges.clone();
        config.networks = networks;
    }

    pub(super) fn save_hosting_node_group(
        &mut self,
        mut group: HostingNodeGroup,
    ) -> Result<(), String> {
        self.validate_hosting_node_group(&group)?;
        let resolved = group
            .nodes
            .iter()
            .map(|node| {
                self.group_node_networks(&group, *node)
                    .map(|networks| (*node, networks))
            })
            .collect::<Result<Vec<_>, _>>()?;
        // All checks precede IDs, membership and node-setting mutations.
        if group.id == 0 {
            group.id = self.hosting.id();
        }
        group.name = group.name.trim().to_string();
        for (node, networks) in resolved {
            self.apply_group_member(&group, node, networks);
        }
        self.hosting.node_groups.insert(group.id, group);
        Ok(())
    }

    pub(super) fn set_hosting_node_group(
        &mut self,
        node: DeviceId,
        id: Option<u64>,
    ) -> Result<(), String> {
        self.validate_group_member(node)?;
        let target = id
            .map(|id| {
                self.hosting
                    .node_groups
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| "Unknown node group".to_string())
            })
            .transpose()?;
        let resolved = if let Some(group) = &target {
            let mut proposed = group.clone();
            proposed.nodes.insert(node);
            self.validate_group(&proposed, Some(node))?;
            self.group_node_networks(group, node)?
        } else {
            None
        };
        for group in self.hosting.node_groups.values_mut() {
            group.nodes.remove(&node);
        }
        if let Some(group) = target {
            self.hosting
                .node_groups
                .get_mut(&group.id)
                .unwrap()
                .nodes
                .insert(node);
            self.apply_group_member(&group, node, resolved);
        }
        Ok(())
    }

    pub(super) fn delete_hosting_node_group(&mut self, id: u64) -> Result<(), String> {
        self.hosting
            .node_groups
            .remove(&id)
            .ok_or("Unknown node group")?;
        for product in self.hosting.products.values_mut() {
            product.eligible_groups.remove(&id);
        }
        Ok(())
    }
}
