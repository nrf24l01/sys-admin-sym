use crate::*;
use std::collections::{BTreeMap, BTreeSet, HashSet};

impl NetworkSim {
    /// Converged loop-free Ethernet forwarding. This deliberately does not model
    /// BPDU timing: a deterministic minimum spanning forest is recomputed on changes.
    pub fn spanning_tree_blocked_ports(&self, vlan: VlanId) -> Vec<PortId> {
        let mut edges = Vec::new();
        let mut bundles = BTreeSet::new();
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
            let (a, b, speed) = if let Some(member) = self.channel_member(port.id) {
                let pairs = self.channel_active_pairs(port.id);
                let Some((a, b)) = pairs.first().copied() else {
                    continue;
                };
                if !bundles.insert((a.min(b), a.max(b))) {
                    continue;
                }
                (a, b, self.channel_capacity_mbps(port.device, member.group))
            } else {
                if !self.channel_forwarding(other.id) {
                    continue;
                }
                (
                    port.id,
                    other.id,
                    self.port_link_speed(port.id).map_or(0, |s| s.mbps()),
                )
            };
            edges.push((std::cmp::Reverse(speed), a, b, port.device, other.device));
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
        blocked
            .into_iter()
            .flat_map(|port| {
                if self.channel_member(port).is_some() {
                    self.channel_active_pairs(port)
                        .into_iter()
                        .map(|(local, _)| local)
                        .collect()
                } else {
                    vec![port]
                }
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
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
