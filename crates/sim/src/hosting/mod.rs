//! Hosting runs in the authoritative simulation, with physical-host resource pools
//! and ordinary IPv4 ports attached to a host bridge in the Ethernet engine.
mod addressing;
mod admission;
mod groups;
mod inventory;
mod lifecycle;
mod market;
mod network;
mod node_networks;
mod provider_server;
mod provisioning;
mod runtime;
mod types;
use crate::*;
pub use market::customer_requirements;
pub(crate) use provider_server::MainHosterCommand;
pub use runtime::customer_workload;
pub use types::*;

impl NetworkSim {
    pub fn configure_hosting(&mut self, command: HostingCommand) -> Result<(), String> {
        match command {
            HostingCommand::SetProviderSiteAddress(address) => {
                self.set_hosting_provider_site_address(address)?
            }
            HostingCommand::SaveProduct(mut p) => {
                if p.id != 0 && !self.hosting.products.contains_key(&p.id) {
                    return Err("Unknown product".into());
                }
                self.validate_hosting_product(&p, p.published)?;
                if p.id == 0 {
                    p.id = self.hosting.id();
                }
                self.hosting.products.insert(p.id, p);
            }
            HostingCommand::Publish { product, published } => {
                let p = self
                    .hosting
                    .products
                    .get(&product)
                    .ok_or("Unknown product")?;
                if published {
                    self.validate_hosting_product(p, true)?;
                }
                self.hosting.products.get_mut(&product).unwrap().published = published;
            }
            HostingCommand::DeleteProduct(id) => {
                self.hosting.products.remove(&id).ok_or("Unknown product")?;
            }
            HostingCommand::SaveNodeGroup(group) => self.save_hosting_node_group(group)?,
            HostingCommand::DeleteNodeGroup(id) => self.delete_hosting_node_group(id)?,
            HostingCommand::SetNodeGroup { node, group } => {
                self.set_hosting_node_group(node, group)?;
            }
            HostingCommand::SetNodePolicy { node, policy } => {
                self.check_hosting_group_override(node)?;
                if !matches!(
                    self.device(node).map(|d| &d.kind),
                    Some(DeviceKind::Server(_))
                ) {
                    return Err("Select a physical server".into());
                }
                groups::validate_policy(&policy)?;
                self.hosting.nodes.entry(node).or_default().policy = policy;
            }
            HostingCommand::SetNodeIpv4Ranges { node, ranges } => {
                self.check_hosting_group_override(node)?;
                if !matches!(
                    self.device(node).map(|d| &d.kind),
                    Some(DeviceKind::Server(_))
                ) || self.hosting_provider_installed(node)
                {
                    return Err("Select a physical virtualization node".into());
                }
                if ranges.as_ref().is_some_and(|ranges| {
                    ranges
                        .iter()
                        .any(|r| r.length() > 30 || !self.provider.owns(*r))
                }) {
                    return Err("Node ranges must be owned IPv4 subnets (/30 or larger)".into());
                }
                self.hosting.nodes.entry(node).or_default().ipv4_ranges = ranges;
            }
            HostingCommand::SetNodeNetworks { node, networks } => {
                self.check_hosting_group_override(node)?;
                self.validate_hosting_node_networks(node, networks.as_deref())?;
                self.hosting.nodes.entry(node).or_default().networks = networks;
            }
            HostingCommand::SetMarket(config) => {
                if config.population == 0
                    || config.population > 1_000_000_000
                    || config.periods.is_empty()
                    || config
                        .periods
                        .iter()
                        .any(|p| p.months == 0 || p.months > 120 || p.weight == 0)
                    || config.time_scale > 86400
                    || config.prospects_per_day > 100000
                {
                    return Err("Set positive subscription periods (1–120 months) and weights; market size ≤100000/day and clock scale ≤86400".into());
                }
                self.hosting.market = config;
            }
            HostingCommand::SetVmPower { vm, power } => {
                if !matches!(power, VmPower::Running | VmPower::Stopped) {
                    return Err("Use start or shutdown".into());
                }
                self.check_vm_management(vm)?;
                let machine = self.hosting.vms.get_mut(&vm).unwrap();
                machine.power = power;
                if power == VmPower::Stopped {
                    machine.customer_restart_at_ms = self.hosting.now_ms + HOSTING_TICK_MS;
                    machine.metrics.reachable = false;
                }
                self.topology_revision += 1;
            }
            HostingCommand::RebootVm(vm) => {
                self.check_vm_management(vm)?;
                let machine = self.hosting.vms.get_mut(&vm).unwrap();
                machine.power = VmPower::Rebooting;
                machine.reboot_until_ms = self.hosting.now_ms + HOSTING_TICK_MS;
                self.topology_revision += 1;
            }
            HostingCommand::DestroyServiceVm(service) => {
                self.destroy_hosting_vm(service)?;
            }
            HostingCommand::RecreateServiceVm(service) => {
                self.recreate_hosting_vm(service)?;
            }
            HostingCommand::RefundService(service) => {
                self.refund_hosting_service(service)?;
            }
            HostingCommand::TerminateService(id) => {
                if let Some(vm) = self.hosting.services.get(&id).and_then(|s| s.vm) {
                    self.check_vm_management(vm)?;
                }
                self.terminate_hosting_service(id)?;
            }
            HostingCommand::Order {
                order,
                product,
                category,
            } => {
                self.purchase_hosting(order, product, category)?;
            }
        }
        self.auto_configure_hosting_nodes();
        self.refresh_hosting_usage();
        self.refresh_hosting_market();
        Ok(())
    }
    fn check_vm_management(&self, vm: u64) -> Result<(), String> {
        let host = self.hosting.vms.get(&vm).ok_or("Unknown VM")?.host;
        if !self.hosting_node_eligible(host) {
            return Err(
                "Node must be powered, prepared and reachable through its management network"
                    .into(),
            );
        }
        Ok(())
    }
    pub fn prepare_hosting_node(
        &mut self,
        host: DeviceId,
        management_ip: std::net::Ipv4Addr,
    ) -> Result<(), String> {
        let controller = self.hosting_management_controller(management_ip)
            .ok_or("vm-prepare: address must belong to an operational main hoster server; run ./setup-main-hoster-server.sh there first")?;
        if !self.hosting_management_reaches(host, management_ip) {
            return Err(
                "vm-prepare: main hoster server must be reachable through its management network"
                    .into(),
            );
        }
        let node = self.hosting.nodes.entry(host).or_default();
        node.prepared_at_ms.get_or_insert(self.hosting.now_ms);
        node.manager = Some(HostingManagerRegistration {
            server: controller,
            address: management_ip,
        });
        self.auto_configure_hosting_nodes();
        Ok(())
    }
    pub(crate) fn advance_hosting(&mut self, ms: u64) {
        let mut remaining = ms.saturating_mul(u64::from(self.hosting.market.time_scale));
        if self.hosting.products.is_empty()
            && self.hosting.nodes.is_empty()
            && self.hosting.vms.is_empty()
            && self.hosting.services.is_empty()
        {
            // Empty calendars can skip workload ticks, including very large time jumps.
            let total = u128::from(self.hosting.pending_ms) + u128::from(remaining);
            let elapsed = total / u128::from(HOSTING_TICK_MS) * u128::from(HOSTING_TICK_MS);
            self.hosting.now_ms = self
                .hosting
                .now_ms
                .saturating_add(elapsed.min(u128::from(u64::MAX)) as u64);
            self.hosting.pending_ms = (total % u128::from(HOSTING_TICK_MS)) as u64;
            self.bank.advance(remaining, &mut self.money);
            return;
        }
        self.bank.collect_due(&mut self.money);
        while remaining > 0 {
            let step = remaining.min(HOSTING_TICK_MS - self.hosting.pending_ms);
            self.bank.advance(step, &mut self.money);
            self.hosting.pending_ms += step;
            remaining -= step;
            if self.hosting.pending_ms == HOSTING_TICK_MS {
                self.hosting.pending_ms = 0;
                self.hosting.now_ms = self.hosting.now_ms.saturating_add(HOSTING_TICK_MS);
                self.auto_configure_hosting_nodes();
                self.tick_hosting_workloads();
                self.tick_hosting_subscriptions();
                self.refresh_hosting_market();
                self.tick_hosting_market();
            }
        }
    }
    pub(crate) fn refresh_hosting_usage(&mut self) {
        let inactive: Vec<_> = self
            .runtime
            .aggregate_network_sources
            .keys()
            .copied()
            .filter(|port| {
                self.port(*port).is_none()
                    || self
                        .virtual_uplink(*port)
                        .is_some_and(|_| !self.virtual_port_running(*port))
            })
            .collect();
        for port in inactive {
            self.remove_aggregate_traffic_source(port);
        }

        for n in self.hosting.nodes.values_mut() {
            let oom = n.usage.oom_events;
            n.usage = NodeUsage {
                oom_events: oom,
                ..Default::default()
            };
        }
        for vm in self.hosting.vms.values() {
            let usage = &mut self.hosting.nodes.entry(vm.host).or_default().usage;
            usage.sold_vcpu += u64::from(vm.plan.vcpu);
            usage.sold_ram_mb += vm.plan.ram_mb;
            usage.virtual_disk_gb += vm.plan.disk_gb;
            usage.physical_disk_gb += vm.physical_disk_gb;
            if vm.power != VmPower::Running {
                continue;
            }
            usage.cpu_demand_mhz += vm.demand.cpu_mhz;
            usage.cpu_used_mhz += vm.metrics.cpu_used_mhz;
            usage.memory_used_mb += vm.metrics.resident_mb;
            usage.swap_used_mb += vm.metrics.swapped_mb;
            usage.disk_iops += vm.metrics.disk_iops;
            usage.disk_mb_s += vm.metrics.disk_mb_s;
            usage.network_mbps += vm.metrics.network_mbps;
        }
    }
}

pub(crate) struct VmHostCommand;
impl LinuxCommand for VmHostCommand {
    fn name(&self) -> &'static str {
        "vm-host"
    }
    fn execute(
        &self,
        sim: &mut NetworkSim,
        device: DeviceId,
        args: &[String],
        _: &str,
    ) -> Result<String, String> {
        match args.first().map(String::as_str) {
            Some("prepare") if args.len() == 2 => {
                let address = args[1]
                    .parse()
                    .map_err(|_| "usage: ./vm-prepare.sh <management server IPv4>")?;
                sim.prepare_hosting_node(device, address)?;
                Ok("Hypervisor prepared. Existing VMs preserved.\n".into())
            }
            Some("status") => Ok(format!(
                "Prepared: {}\nManagement: {}\nManager: {:?}\n{:?}\n",
                sim.hosting
                    .nodes
                    .get(&device)
                    .is_some_and(|n| n.prepared_at_ms.is_some()),
                sim.hosting_management_connected(device),
                sim.hosting.nodes.get(&device).and_then(|n| n.manager),
                sim.hosting.nodes.get(&device).map(|n| &n.usage)
            )),
            _ => Err(
                "usage: vm-host prepare <management server IPv4>|status (or ./vm-prepare.sh <IP>)"
                    .into(),
            ),
        }
    }
    fn suggest(&self, _: &CompletionContext<'_>) -> Vec<String> {
        vec!["prepare".into(), "status".into()]
    }
}
