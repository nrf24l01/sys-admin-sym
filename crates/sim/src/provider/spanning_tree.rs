use crate::*;
use std::collections::{BTreeMap, HashSet};

impl NetworkSim {
    /// Converged loop-free Ethernet forwarding. This deliberately does not model
    /// BPDU timing: a deterministic minimum spanning forest is recomputed on changes.
    pub fn spanning_tree_blocked_ports(&self, vlan: VlanId) -> Vec<PortId> {
        let mut edges = Vec::new();
        for port in self
            .ports()
            .filter(|p| matches!(p.config, PortConfig::Switch(_)))
        {
            if !self.carries_vlan(port.id, vlan) {
                continue;
            }
            let path = self.physical_path(port.id);
            let Some(peer) = path
                .first()
                .filter(|id| **id != port.id)
                .or_else(|| path.last().filter(|id| **id != port.id))
            else {
                continue;
            };
            let Some(other) = self
                .port(*peer)
                .filter(|p| matches!(p.config, PortConfig::Switch(_)))
            else {
                continue;
            };
            if port.id >= other.id || !self.carries_vlan(other.id, vlan) {
                continue;
            }
            if !self.endpoint_link_vlan_matches(
                port.id,
                other.id,
                self.wire_vlan_for(port.id, vlan),
            ) {
                continue;
            }
            edges.push((
                std::cmp::Reverse(self.port_link_speed(port.id).map_or(0, |s| s.mbps())),
                port.id,
                other.id,
                port.device,
                other.device,
            ));
        }
        edges.sort();
        let mut components = BTreeMap::new();
        fn root(components: &BTreeMap<DeviceId, DeviceId>, mut node: DeviceId) -> DeviceId {
            while let Some(parent) = components.get(&node) {
                if *parent == node {
                    break;
                }
                node = *parent;
            }
            node
        }
        let mut blocked = Vec::new();
        for (_, a, b, da, db) in edges {
            let ra = root(&components, da);
            let rb = root(&components, db);
            if ra == rb {
                // One blocked endpoint suffices; never pretend a disabled bridge participates.
                if self.provider.spanning_tree_enabled(db) {
                    blocked.push(b);
                } else if self.provider.spanning_tree_enabled(da) {
                    blocked.push(a);
                }
            } else {
                components.insert(ra.max(rb), ra.min(rb));
            }
        }
        blocked.sort();
        blocked
    }

    pub(crate) fn bridge_port_blocked(&mut self, port: PortId, vlan: VlanId) -> bool {
        if !self.runtime.spanning_tree.contains_key(&vlan) {
            let blocked: HashSet<_> = self.spanning_tree_blocked_ports(vlan).into_iter().collect();
            self.runtime.spanning_tree.insert(vlan, blocked);
        }
        self.runtime
            .spanning_tree
            .get(&vlan)
            .is_some_and(|ports| ports.contains(&port))
    }
}
