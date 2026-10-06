use crate::*;

impl NetworkSim {
    pub(crate) fn reconcile_provider(&mut self) {
        let ports = &self.ports;
        let devices = &self.devices;
        let p = &mut self.provider;
        p.bindings.retain(|b| ports.contains_key(&b.port));
        p.policies.retain(|a| ports.contains_key(&a.policy.port));
        p.sessions.retain(|s| {
            ports.contains_key(&s.port) && p.circuits.iter().any(|c| c.port == s.circuit)
        });
        p.preferences
            .retain(|r| devices.contains_key(&r.router) && ports.contains_key(&r.egress));

        p.management.retain(|m| devices.contains_key(&m.switch));
        p.spanning_tree_disabled
            .retain(|id| devices.contains_key(id));
        p.dhcp.retain(|d| ports.contains_key(&d.server));
        p.leases
            .retain(|l| ports.contains_key(&l.client) && ports.contains_key(&l.server));
    }

    pub(crate) fn migrate_provider_inventory(&mut self) {
        for route in std::mem::take(&mut self.provider.routes) {
            if let Some(device) = self.devices.get_mut(&route.router)
                && let DeviceKind::Router(router) = &mut device.kind
                && router.ports.contains(&route.port)
                && !router.domain_routes.contains(&route)
            {
                router.domain_routes.push(route);
            }
        }
        if self.provider.legacy_inventory_imported {
            return;
        }
        for block in &self.public_ipv4_blocks {
            let prefix =
                Ipv4Prefix::new(block.network, PublicIpv4Block::PREFIX).expect("legacy /29 prefix");
            if !self
                .provider
                .pools
                .iter()
                .any(|p| p.prefix.contains_prefix(prefix) || prefix.contains_prefix(p.prefix))
            {
                self.provider.pools.push(AddressPool {
                    prefix,
                    description: "Imported IPv4 allocation".into(),
                });
            }
        }
        self.provider.legacy_inventory_imported = true;
    }
}
