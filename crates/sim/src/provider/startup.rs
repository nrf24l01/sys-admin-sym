use crate::*;
use serde::{Deserialize, Serialize};

/// A device's persistent configuration, separate from carrier contracts and leases.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct DeviceNetworkConfig {
    bindings: Vec<InterfaceBinding>,
    sessions: Vec<BgpSession>,
    policies: Vec<PolicyAttachment>,
    preferences: Vec<RoutePreference>,
    dhcp: Vec<DhcpPool>,
    management: Option<SwitchManagement>,
    spanning_tree: bool,
}

impl NetworkSim {
    pub(crate) fn device_network_config(&self, device: DeviceId) -> DeviceNetworkConfig {
        let ports = self.devices[&device].ports();
        let p = &self.provider;
        DeviceNetworkConfig {
            bindings: p
                .bindings
                .iter()
                .filter(|b| ports.contains(&b.port))
                .copied()
                .collect(),
            sessions: p
                .sessions
                .iter()
                .filter(|s| ports.contains(&s.port))
                .cloned()
                .collect(),
            policies: p
                .policies
                .iter()
                .filter(|a| ports.contains(&a.policy.port))
                .cloned()
                .collect(),
            preferences: p
                .preferences
                .iter()
                .filter(|r| r.router == device)
                .copied()
                .collect(),
            dhcp: p
                .dhcp
                .iter()
                .filter(|d| ports.contains(&d.server))
                .copied()
                .collect(),
            management: p.management.iter().find(|m| m.switch == device).copied(),
            spanning_tree: p.spanning_tree_enabled(device),
        }
    }

    pub(crate) fn restore_device_network(&mut self, device: DeviceId, saved: DeviceNetworkConfig) {
        let ports = self.devices[&device].ports();
        let p = &mut self.provider;
        p.bindings.retain(|b| !ports.contains(&b.port));
        p.bindings.extend(saved.bindings);
        p.sessions.retain(|s| !ports.contains(&s.port));
        p.sessions.extend(saved.sessions);
        p.policies.retain(|a| !ports.contains(&a.policy.port));
        p.policies.extend(saved.policies);
        p.preferences.retain(|r| r.router != device);
        p.preferences.extend(saved.preferences);
        p.dhcp.retain(|d| !ports.contains(&d.server));
        p.dhcp.extend(saved.dhcp);
        p.management.retain(|m| m.switch != device);
        p.management.extend(saved.management);
        p.spanning_tree_disabled.retain(|id| *id != device);
        if !saved.spanning_tree {
            p.spanning_tree_disabled.push(device);
        }
        // Leases are operational state, never restored from startup-config.
        p.leases
            .retain(|l| !ports.contains(&l.client) && !ports.contains(&l.server));
        self.reconcile_provider();
    }
}
