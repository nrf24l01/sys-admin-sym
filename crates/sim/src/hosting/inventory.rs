use super::*;
use std::net::Ipv4Addr;

impl NetworkSim {
    pub fn hosting_ipv4_assignments(&self) -> Vec<HostingIpv4Assignment> {
        let mut rows = std::collections::BTreeMap::new();
        for address in self.hosting_occupied_addresses() {
            if let Some(pool) = self
                .provider
                .pools()
                .iter()
                .find(|p| p.prefix.usable(address))
            {
                rows.insert(
                    address,
                    HostingIpv4Assignment {
                        address,
                        range: pool.prefix,
                        service: None,
                        customer: None,
                        vm: None,
                        node: None,
                        port: None,
                    },
                );
            }
        }
        for port in self.ports() {
            let addresses: Vec<_> = match &port.config {
                PortConfig::Server(c) => c.addresses().map(|a| a.address).collect(),
                PortConfig::Router(c) => c.interfaces.iter().filter_map(|a| a.address).collect(),
                _ => vec![],
            };
            for address in addresses {
                if let Some(row) = rows.get_mut(&address) {
                    row.node = Some(port.device);
                    row.port = Some(port.id);
                }
            }
        }
        for device in self.devices() {
            if let Some(management) = self.switch_management(device.id)
                && let Some(row) = rows.get_mut(&management.address)
            {
                row.node = Some(device.id);
            }
        }
        for lease in self.provider.leases() {
            if let Some(row) = rows.get_mut(&lease.address) {
                row.port = Some(lease.client);
                row.node = self.port(lease.client).map(|p| p.device);
            }
        }
        for service in self
            .hosting
            .services
            .values()
            .filter(|s| s.terminated_at_ms.is_none())
        {
            let Some(lease) = self.hosting_service_provisioning(service.id) else {
                continue;
            };
            let vm = service.vm.and_then(|id| self.hosting.vms.get(&id));
            for address in lease.ips {
                if let Some(row) = rows.get_mut(&address) {
                    row.service = Some(service.id);
                    row.customer = Some(service.customer);
                    row.node = Some(lease.host);
                    row.vm = vm.map(|v| v.id);
                    row.port = vm.and_then(|v| v.interfaces.first().copied());
                }
            }
        }
        rows.into_values().collect()
    }
    pub fn hosting_management_connected(&self, host: DeviceId) -> bool {
        let Some(manager) = self.hosting.nodes.get(&host).and_then(|n| n.manager) else {
            return false;
        };
        self.hosting_provider_server() == Some(manager.server)
            && self.hosting_management_controller(manager.address) == Some(manager.server)
            && self.hosting_management_reaches(host, manager.address)
    }
    pub(crate) fn hosting_management_reaches(&self, host: DeviceId, target: Ipv4Addr) -> bool {
        let Some(controller) = self.hosting_management_controller(target) else {
            return false;
        };
        if host == controller
            || self.hosting_provider_installed(host)
            || !self.hosting_server_operational(host)
        {
            return false;
        }
        let Some(route) = self.server_route_selection(host, target, None) else {
            return false;
        };
        let mgmt = route.port;
        if !route.source.address.is_private() || !self.port_link_up(mgmt) {
            return false;
        }
        let revision = (self.topology_revision, self.routing_revision);
        if let Some(connected) = self
            .runtime
            .hosting_management
            .get(revision, (host, target))
        {
            return connected;
        }
        let mut probe = self.clone();
        probe.runtime.traffic_probe = true;
        let connected = probe.transmit_icmp(mgmt, target).reachable;
        self.runtime
            .hosting_management
            .insert(revision, (host, target), connected);
        connected
    }
    pub fn hosting_node_eligible(&self, host: DeviceId) -> bool {
        self.hosting
            .nodes
            .get(&host)
            .is_some_and(|n| n.prepared_at_ms.is_some())
            && self.hosting_management_connected(host)
    }
    pub fn hosting_capacity(&self, host: DeviceId, storage: HostingStorage) -> NodeCapacity {
        let Some(Device {
            kind: DeviceKind::Server(s),
            powered: true,
            ..
        }) = self.device(host)
        else {
            return NodeCapacity::default();
        };
        let mut c = NodeCapacity {
            cpu_mhz: self.server_resources(host).compute_mhz as f64,
            network_mbps: s.ports.iter().filter(|id| {
                if let Some(networks) = self.hosting.nodes.get(&host).and_then(|n| n.networks.as_ref()) {
                    networks.iter().any(|n| n.port == **id)
                } else {
                    self.port(**id).is_some_and(|p| matches!(&p.config, PortConfig::Server(c)
                        if c.addresses().any(|a| a.gateway.is_some_and(|g| a.contains(g))
                            && Ipv4Prefix::new(a.address, a.prefix).is_ok_and(|subnet|
                                self.provider.pools().iter().any(|pool| subnet.contains_prefix(pool.prefix) || pool.prefix.contains_prefix(subnet))))))
                }
            }).filter_map(|id| self.port_link_speed(*id)).map(|speed| f64::from(speed.mbps())).sum(),
            ..Default::default()
        };
        if let Some(h) = &s.hardware {
            c.ram_mb = h
                .ram
                .iter()
                .filter_map(|id| server_catalog().parts.iter().find(|p| &p.id == id))
                .filter_map(|p| match p.kind {
                    ServerPartKind::Ram { capacity_gb, .. } => {
                        Some(f64::from(capacity_gb) * 1024.0)
                    }
                    _ => None,
                })
                .sum();
            for drive in h
                .drives
                .iter()
                .flatten()
                .filter_map(|id| drive_catalog().drives.iter().find(|d| &d.id == id))
                .filter(|d| {
                    matches!(
                        (storage, d.kind),
                        (HostingStorage::Hdd, DriveKind::Hdd)
                            | (HostingStorage::Ssd, DriveKind::Ssd)
                    )
                })
            {
                c.disk_gb += f64::from(drive.capacity_gb);
                c.iops += f64::from(drive.read_iops.min(drive.write_iops));
                c.disk_mb_s += f64::from(drive.read_mb_s.min(drive.write_mb_s));
            }
        } else {
            // Legacy assembled servers have catalog CPU/RAM but no invented disks.
            c.ram_mb = server_catalog()
                .parts
                .iter()
                .find_map(|p| match p.kind {
                    ServerPartKind::Ram { capacity_gb, .. } => {
                        Some(f64::from(capacity_gb) * 1024.0)
                    }
                    _ => None,
                })
                .unwrap_or(0.0);
        }
        c
    }
    pub fn validate_hosting_product(
        &self,
        p: &HostingProduct,
        publishing: bool,
    ) -> Result<(), String> {
        if p.name.trim().is_empty()
            || p.name.len() > 100
            || p.monthly_price_cents == 0
            || p.monthly_price_cents > 100_000_000
            || p.vcpu == 0
            || p.vcpu > 4096
            || p.ram_mb < 128
            || p.ram_mb > 1_000_000_000
            || p.disk_gb == 0
            || p.disk_gb > 1_000_000_000
            || p.port_mbps == 0
            || p.port_mbps > 100000
            || p.cpu_weight == 0
            || p.cpu_weight > 10000
            || p.cpu_limit_percent == 0
            || p.cpu_limit_percent > 100
            || p.cpu_burst_percent < p.cpu_limit_percent
            || p.cpu_burst_percent > 100
            || p.disk_iops == 0
            || p.disk_mb_s == 0
            || p.ipv4_count > 16
            || p.traffic_gb == Some(0)
        {
            return Err("Invalid name, price, resource capacity or limit".into());
        }
        if p.category == HostingCategory::DedicatedCpu && p.cpu_policy != CpuPolicy::Dedicated {
            return Err("Dedicated CPU products must reserve CPU".into());
        }
        if p.eligible_nodes.iter().any(|id| {
            !matches!(
                self.device(*id).map(|d| &d.kind),
                Some(DeviceKind::Server(_))
            )
        }) {
            return Err("Placement must contain physical servers".into());
        }
        if p.ipv4_ranges
            .iter()
            .any(|range| !self.provider.owns(*range) || range.length() > 30)
        {
            return Err("Select usable owned IPv4 ranges (/30 or larger)".into());
        }
        if publishing
            && (p.eligible_nodes.is_empty() || p.ipv4_count > 0 && p.ipv4_ranges.is_empty())
        {
            return Err("Select eligible servers and IPv4 ranges before publishing".into());
        }
        Ok(())
    }
    /// Includes host/router/DHCP use, gateway reservations, stopped guests and
    /// paid services retaining their addresses while their VM is destroyed.
    pub fn hosting_range_usage(&self, prefix: Ipv4Prefix) -> HostingRangeUsage {
        let total = u64::from(prefix.last()) - u64::from(u32::from(prefix.network())) + 1
            - if prefix.length() < 31 { 2 } else { 0 };
        let occupied = self.hosting_occupied_addresses();
        let allocated = occupied.iter().filter(|ip| prefix.usable(**ip)).count() as u64;
        HostingRangeUsage {
            prefix,
            total,
            allocated,
            available: total.saturating_sub(allocated),
        }
    }
    pub(crate) fn hosting_occupied_addresses(&self) -> std::collections::BTreeSet<Ipv4Addr> {
        let mut used = self.occupied_ipv4_addresses(PortId(0));
        for pool in self.provider.pools() {
            used.insert(Ipv4Addr::from(u32::from(pool.prefix.network()) + 1));
        }
        for port in self.ports() {
            if let PortConfig::Server(c) = &port.config {
                used.extend(c.addresses().filter_map(|a| a.gateway));
            }
        }
        used
    }
}
