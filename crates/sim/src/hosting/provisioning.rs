use super::admission::shortfall;
use super::*;
use std::net::Ipv4Addr;

pub(super) struct Placement {
    pub(super) host: DeviceId,
    uplink: PortId,
    addresses: Vec<(Ipv4Addr, Ipv4Prefix)>,
    template: Ipv4InterfaceConfig,
    thin: bool,
    tagged: bool,
}

impl NetworkSim {
    pub fn hosting_availability(&self, product: u64) -> Result<DeviceId, String> {
        let p = self
            .hosting
            .products
            .get(&product)
            .ok_or("Unknown product")?;
        self.hosting_placement(p).map(|place| place.host)
    }
    fn hosting_placement(&self, p: &HostingProduct) -> Result<Placement, String> {
        self.hosting_provider_site_status()?;
        if !p.published {
            return Err("Product is unpublished".into());
        }
        self.validate_hosting_product(p, true)?;
        self.hosting_resource_placement(p, None)
    }
    pub(super) fn hosting_resource_placement(
        &self,
        p: &HostingProduct,
        retained: Option<(u64, &HostingServiceProvisioning)>,
    ) -> Result<Placement, String> {
        let occupied = if let Some((service, _)) = retained {
            self.occupied_ipv4_addresses_except_service(PortId(0), Some(service))
        } else {
            self.hosting_occupied_addresses()
        };
        let mut nodes: Vec<_> = if let Some((_, lease)) = retained {
            vec![lease.host]
        } else {
            p.eligible_nodes.iter().copied().collect()
        };
        nodes.sort_by_key(|id| {
            (
                self.hosting
                    .vms
                    .values()
                    .filter(|vm| vm.host == *id)
                    .count(),
                *id,
            )
        });
        let mut reason = "No powered, prepared node with management connectivity".to_string();
        let mut capacity_failures = Vec::new();
        for host in nodes {
            if !self.hosting_node_eligible(host) {
                continue;
            }
            let ranges = self.hosting_node_product_ranges(host, p);
            if p.ipv4_count > 0 && ranges.is_empty() {
                reason = "Node IPv4 policy allows none of the product's selected ranges".into();
                continue;
            }
            let node = &self.hosting.nodes[&host];
            let policy = &node.policy;
            let cap = self.hosting_capacity(host, p.storage);
            let existing: Vec<_> = self
                .hosting
                .vms
                .values()
                .filter(|v| v.host == host)
                .collect();
            if existing
                .iter()
                .any(|v| v.plan.category == HostingCategory::DedicatedServer)
                || p.category == HostingCategory::DedicatedServer && !existing.is_empty()
            {
                reason = "Dedicated server requires an exclusive node".into();
                continue;
            }
            let thin = p.thin
                && policy.thin_provisioning
                && p.category != HostingCategory::DedicatedServer;
            let mut dedicated = existing
                .iter()
                .filter(|v| v.plan.cpu_policy == CpuPolicy::Dedicated)
                .map(|v| f64::from(v.plan.vcpu) * VCPU_MHZ)
                .sum::<f64>();
            let mut shared = existing
                .iter()
                .filter(|v| v.plan.cpu_policy == CpuPolicy::Shared)
                .map(|v| f64::from(v.plan.vcpu) * VCPU_MHZ)
                .sum::<f64>();
            if p.cpu_policy == CpuPolicy::Dedicated {
                dedicated += f64::from(p.vcpu) * VCPU_MHZ;
            } else {
                shared += f64::from(p.vcpu) * VCPU_MHZ;
            }
            let cpu = (cap.cpu_mhz - policy.reserved_cpu_mhz as f64).max(0.0);
            let memory = (cap.ram_mb - policy.reserved_ram_mb as f64).max(0.0);
            let sold_ram: u64 = existing.iter().map(|v| v.plan.ram_mb).sum();
            let storage_vms: Vec<_> = existing
                .iter()
                .filter(|v| v.plan.storage == p.storage)
                .collect();
            let disk =
                (cap.disk_gb - policy.reserved_disk_gb as f64 - policy.swap_mb as f64 / 1024.0)
                    .max(0.0);
            let virtual_disk: u64 = storage_vms.iter().map(|v| v.plan.disk_gb).sum();
            let physical_disk: f64 = storage_vms.iter().map(|v| v.physical_disk_gb).sum();
            let initial = if thin {
                p.disk_gb.min(1) as f64
            } else {
                p.disk_gb as f64
            };
            let iops: u64 = storage_vms
                .iter()
                .map(|v| u64::from(v.plan.disk_iops))
                .sum();
            let throughput: u64 = storage_vms
                .iter()
                .map(|v| u64::from(v.plan.disk_mb_s))
                .sum();
            let bandwidth: u64 = existing.iter().map(|v| u64::from(v.plan.port_mbps)).sum();
            let dedicated_request = if p.cpu_policy == CpuPolicy::Dedicated {
                f64::from(p.vcpu) * VCPU_MHZ
            } else {
                0.0
            };
            let shared_request = if p.cpu_policy == CpuPolicy::Shared {
                f64::from(p.vcpu) * VCPU_MHZ
            } else {
                0.0
            };
            let storage = match p.storage {
                HostingStorage::Hdd => "HDD",
                HostingStorage::Ssd => "SSD",
            };
            let failures: Vec<_> = [
                shortfall("Dedicated CPU", dedicated - dedicated_request, dedicated_request, cpu, "MHz", || {
                    format!("Physical CPU {:.2} MHz, hypervisor reservation {} MHz; dedicated CPU cannot be overcommitted.", cap.cpu_mhz, policy.reserved_cpu_mhz)
                }),
                shortfall("Shared CPU", shared - shared_request, shared_request,
                    (cpu - dedicated).max(0.0) * f64::from(policy.cpu_overcommit_percent) / 100.0, "MHz", || {
                    format!("Physical CPU {:.2} MHz, hypervisor reservation {} MHz, dedicated CPU after this order {:.2} MHz; CPU overcommit {}%.", cap.cpu_mhz, policy.reserved_cpu_mhz, dedicated, policy.cpu_overcommit_percent)
                }),
                shortfall("RAM", sold_ram as f64, p.ram_mb as f64,
                    memory * f64::from(policy.ram_overcommit_percent) / 100.0, "MiB", || {
                    format!("Physical RAM {:.2} MiB, hypervisor reservation {} MiB; RAM overcommit {}%.", cap.ram_mb, policy.reserved_ram_mb, policy.ram_overcommit_percent)
                }),
                shortfall("Physical storage", physical_disk, initial, disk, "GB", || {
                    format!("{storage}: installed {:.2} GB, hypervisor reservation {} GB, swap reservation {:.2} GB; {} provisioning requires {:.2} GB initially.", cap.disk_gb, policy.reserved_disk_gb, policy.swap_mb as f64 / 1024.0, if thin { "thin" } else { "thick" }, initial)
                }),
                shortfall("Virtual disk capacity", virtual_disk as f64, p.disk_gb as f64,
                    disk * f64::from(policy.storage_overcommit_percent) / 100.0, "GB", || {
                    format!("{storage}: usable physical storage {:.2} GB; storage overcommit {}%; {} provisioning.", disk, policy.storage_overcommit_percent, if thin { "thin" } else { "thick" })
                }),
                shortfall("Disk IOPS", iops as f64, f64::from(p.disk_iops),
                    cap.iops * f64::from(policy.io_overcommit_percent) / 100.0, "IOPS", || {
                    format!("{storage}: physical capacity {:.2} IOPS; storage performance overcommit {}%.", cap.iops, policy.io_overcommit_percent)
                }),
                shortfall("Disk throughput", throughput as f64, f64::from(p.disk_mb_s),
                    cap.disk_mb_s * f64::from(policy.io_overcommit_percent) / 100.0, "MB/s", || {
                    format!("{storage}: physical throughput {:.2} MB/s; storage performance overcommit {}%.", cap.disk_mb_s, policy.io_overcommit_percent)
                }),
                shortfall("Network bandwidth", bandwidth as f64, f64::from(p.port_mbps),
                    cap.network_mbps * f64::from(policy.network_overcommit_percent) / 100.0, "Mbps", || {
                    format!("Physical NIC capacity {:.2} Mbps; network overcommit {}%.", cap.network_mbps, policy.network_overcommit_percent)
                }),
            ].into_iter().flatten().collect();
            if !failures.is_empty() {
                reason = format!(
                    "{} (node #{}): insufficient capacity under its overcommit policy\n{}",
                    self.device(host).unwrap().name,
                    host.0,
                    failures.join("\n")
                );
                capacity_failures.push(reason.clone());
                continue;
            }
            reason = if node.networks.is_some() {
                "No guest subnet rule has a reachable gateway on its selected port/VLAN"
            } else {
                "Node needs a connected data interface with IPv4 subnet and gateway configuration"
            }
            .into();
            for attachment in self.hosting_network_attachments(host, &ranges) {
                let super::node_networks::HostingNetworkAttachment {
                    uplink,
                    template,
                    ranges,
                    tagged,
                } = attachment;
                if ranges.is_empty() && p.ipv4_count > 0 {
                    continue;
                }
                let mut addresses = Vec::new();
                if let Some((_, lease)) = retained {
                    if lease.ips.len() != usize::from(p.ipv4_count)
                        || lease.ips.len() != lease.ip_ranges.len()
                        || lease.ips.iter().zip(&lease.ip_ranges).any(|(ip, range)| {
                            !template.contains(*ip)
                                || !self.provider.owns(*range)
                                || !range.usable(*ip)
                                || !ranges.iter().any(|(slice, _)| slice.usable(*ip))
                                || occupied.contains(ip)
                                || template.gateway == Some(*ip)
                        })
                    {
                        reason = "Reserved service IPv4 cannot be used on this node's data network or current address policy".into();
                        continue;
                    }
                    return Ok(Placement {
                        host,
                        uplink,
                        addresses: lease
                            .ips
                            .iter()
                            .copied()
                            .zip(lease.ip_ranges.iter().copied())
                            .collect(),
                        template,
                        thin,
                        tagged,
                    });
                }
                for (range, owner) in &ranges {
                    let first = u32::from(range.network()) + 1;
                    let last = range.last() - 1;
                    if !template.contains(Ipv4Addr::from(first))
                        || !template.contains(Ipv4Addr::from(last))
                    {
                        continue;
                    }
                    let mut candidate = first;
                    // Scan gaps in the occupied set, never an entire large prefix.
                    for used in occupied
                        .range(Ipv4Addr::from(first)..=Ipv4Addr::from(last))
                        .map(|a| u32::from(*a))
                        .chain(std::iter::once(last.saturating_add(1)))
                    {
                        while candidate < used && addresses.len() < usize::from(p.ipv4_count) {
                            let address = Ipv4Addr::from(candidate);
                            if !addresses.iter().any(|(a, _)| *a == address) {
                                addresses.push((address, *owner));
                            }
                            candidate += 1;
                        }
                        candidate = candidate.max(used.saturating_add(1));
                        if addresses.len() == usize::from(p.ipv4_count) {
                            break;
                        }
                    }
                    if addresses.len() == usize::from(p.ipv4_count) {
                        break;
                    }
                }
                if addresses.len() == usize::from(p.ipv4_count) {
                    return Ok(Placement {
                        host,
                        uplink,
                        addresses,
                        template,
                        thin,
                        tagged,
                    });
                }
                reason = "No free IPv4 in selected ranges on this node's data network".into();
            }
        }
        if !capacity_failures.is_empty() {
            if capacity_failures.last() != Some(&reason) {
                capacity_failures.push(format!("Other placement checks: {reason}"));
            }
            reason = capacity_failures.join("\n\n");
        }
        Err(reason)
    }
    /// The single simulation owner validates everything before committing. Failed
    /// orders consume no IDs, RNG, addresses, resources or money. Successful order
    /// keys remain idempotent even after a service has been terminated.
    pub fn purchase_hosting(
        &mut self,
        order: u64,
        product: u64,
        category: CustomerCategory,
    ) -> Result<u64, String> {
        self.purchase_hosting_profile(order, product, category, None)
    }
    pub(crate) fn purchase_hosting_profile(
        &mut self,
        order: u64,
        product: u64,
        category: CustomerCategory,
        profile: Option<HostingCustomer>,
    ) -> Result<u64, String> {
        if let Some(service) = self.hosting.order_services.get(&order) {
            if self.hosting.services[service].product != product {
                return Err("Order key already belongs to another product".into());
            }
            return Ok(*service);
        }
        let plan = self
            .hosting
            .products
            .get(&product)
            .ok_or("Unknown product")?
            .clone();
        let place = self.hosting_placement(&plan)?;
        let months = self.hosting.duration();
        let amount = plan.monthly_price_cents * u64::from(months);
        let expires = self.hosting.now_ms + MONTH_MS * u64::from(months);
        let customer_id = self.hosting.id();
        let service_id = self.hosting.id();
        let vm_id = self.hosting.id();
        let mut customer = if let Some(profile) = profile {
            profile
        } else {
            let variation = self.hosting.random();
            super::market::customer_profile(category, variation, self.hosting.rng_state)
        };
        customer.id = customer_id;
        customer.services.insert(service_id);
        let vm = self.create_hosting_vm(vm_id, customer_id, service_id, &plan, expires, place);
        let provisioning = HostingServiceProvisioning {
            plan: vm.plan.clone(),
            host: vm.host,
            ips: vm.ips.clone(),
            ip_ranges: vm.ip_ranges.clone(),
        };
        self.hosting.vms.insert(vm_id, vm);
        self.hosting.customers.insert(customer_id, customer);
        self.hosting.services.insert(
            service_id,
            HostingService {
                id: service_id,
                customer: customer_id,
                product,
                vm: Some(vm_id),
                provisioning: Some(provisioning),
                retained_traffic_bytes: 0,
                purchased_at_ms: self.hosting.now_ms,
                expires_at_ms: expires,
                monthly_price_cents: plan.monthly_price_cents,
                renewals: 0,
                terminated_at_ms: None,
                satisfaction: 1.0,
                observed_quality: 1.0,
                availability: 1.0,
            },
        );
        self.record_hosting_payment(HostingPayment {
            order,
            service: service_id,
            at_ms: self.hosting.now_ms,
            months,
            amount_cents: amount,
            renewal: false,
        });
        self.hosting.market_stats.entry(product).or_default().sales += 1;
        self.topology_revision += 1;
        self.routing_revision += 1;
        self.refresh_hosting_usage();
        Ok(service_id)
    }
    pub(super) fn create_hosting_vm(
        &mut self,
        vm_id: u64,
        customer_id: u64,
        service_id: u64,
        plan: &HostingProduct,
        expires: u64,
        place: Placement,
    ) -> VirtualMachine {
        let port = self.alloc_port(
            place.host,
            format!("vnet{vm_id}"),
            PortConnector::Rj45,
            PortConfig::Server(ServerPortConfig::default()),
        );
        let ips: Vec<_> = place.addresses.iter().map(|(a, _)| *a).collect();
        let addresses: Vec<_> = place
            .addresses
            .iter()
            .map(|(a, _)| Ipv4InterfaceConfig {
                address: *a,
                ..place.template.clone()
            })
            .collect();
        if let PortConfig::Server(config) = &mut self.ports.get_mut(&port).unwrap().config {
            if place.tagged {
                config
                    .tagged_vlans
                    .insert(place.template.vlan.unwrap_or(VlanId(1)));
            }
            config.ipv4 = addresses.first().cloned();
            config.additional_ipv4 = addresses.into_iter().skip(1).collect();
        }
        self.add_host_bridge(port, place.uplink);
        // Preserve routing-domain isolation on virtual interfaces.
        let vlan = place.template.vlan.unwrap_or(VlanId(1));
        self.provider_bind_virtual(port, place.uplink, vlan);
        let physical_disk_gb = if place.thin { 1.0 } else { plan.disk_gb as f64 };
        VirtualMachine {
            id: vm_id,
            customer: customer_id,
            host: place.host,
            product: plan.id,
            service: service_id,
            plan: HostingProduct {
                thin: place.thin,
                ..plan.clone()
            },
            interfaces: vec![port],
            ips,
            ip_ranges: place.addresses.iter().map(|(_, range)| *range).collect(),
            physical_disk_gb,
            disk_used_gb: 1.0,
            power: VmPower::Running,
            host_memory_failure: false,
            customer_restart_at_ms: 0,
            expires_at_ms: expires,
            demand: WorkloadDemand::default(),
            workload_override: None,
            metrics: VmMetrics {
                quality: 1.0,
                ..Default::default()
            },
            burst_credit_seconds: f64::from(plan.burst_seconds),
            reboot_until_ms: 0,
        }
    }
    pub(crate) fn record_hosting_payment(&mut self, p: HostingPayment) {
        self.hosting.revenue_remainder_cents += p.amount_cents;
        self.money += (self.hosting.revenue_remainder_cents / 100) as i64;
        self.hosting.revenue_remainder_cents %= 100;
        if !p.renewal {
            self.hosting.order_services.insert(p.order, p.service);
        }
        self.hosting.payments.push(p);
    }
    pub fn terminate_hosting_service(&mut self, id: u64) -> Result<(), String> {
        let service = self
            .hosting
            .services
            .get_mut(&id)
            .ok_or("Unknown service")?;
        if service.terminated_at_ms.is_some() {
            return Ok(());
        }
        service.terminated_at_ms = Some(self.hosting.now_ms);
        let customer = service.customer;
        let product = service.product;
        let machine = service.vm.take();
        if let Some(vm) = machine {
            self.remove_hosting_vm(vm);
        }
        self.hosting
            .customers
            .get_mut(&customer)
            .unwrap()
            .services
            .remove(&id);
        self.hosting
            .market_stats
            .entry(product)
            .or_default()
            .cancellations += 1;
        self.topology_revision += 1;
        self.routing_revision += 1;
        self.refresh_hosting_usage();
        Ok(())
    }
}
