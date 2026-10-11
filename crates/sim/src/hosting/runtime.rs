use super::*;
use std::collections::BTreeMap;

/// Weighted max-min scheduling, redistributing idle guests' unused allocation.
fn fair_schedule(demand: &[f64], weights: &[f64], capacity: f64) -> Vec<f64> {
    let mut allocation = vec![0.0; demand.len()];
    let mut order: Vec<_> = (0..demand.len()).collect();
    order.sort_by(|a, b| (demand[*a] / weights[*a]).total_cmp(&(demand[*b] / weights[*b])));
    let mut left = capacity.max(0.0);
    let mut total_weight: f64 = weights.iter().sum();
    for (pos, i) in order.iter().enumerate() {
        let share = left * weights[*i] / total_weight.max(0.001);
        if demand[*i] <= share {
            allocation[*i] = demand[*i];
            left -= demand[*i];
            total_weight -= weights[*i];
        } else {
            for j in &order[pos..] {
                allocation[*j] = left * weights[*j] / total_weight.max(0.001);
            }
            break;
        }
    }
    allocation
}
fn noise(seed: u64, tick: u64) -> f64 {
    let mut x = seed.wrapping_add(tick.wrapping_mul(0x9e3779b97f4a7c15));
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    ((x ^ (x >> 31)) >> 11) as f64 / (1u64 << 53) as f64
}
pub fn customer_workload(
    customer: &HostingCustomer,
    vm: &VirtualMachine,
    now_ms: u64,
) -> WorkloadDemand {
    let req = &customer.requirements;
    let hour = (now_ms % 86_400_000) as f64 / 3_600_000.0;
    let daily = 0.35 + 0.65 * ((hour - 15.0) * std::f64::consts::PI / 12.0).cos().max(0.0);
    let random = noise(customer.workload_seed, now_ms / HOSTING_TICK_MS);
    let spike = if random > 0.95 {
        2.5
    } else {
        0.6 + random * 0.8
    };
    let (cpu, memory, io, network) = match customer.category {
        CustomerCategory::Website => (0.3, 0.4, 0.2, 0.4),
        CustomerCategory::Developer => (if random > 0.6 { 1.0 } else { 0.05 }, 0.55, 0.4, 0.15),
        CustomerCategory::Ecommerce => (0.65, 0.65, 0.55, 0.6),
        CustomerCategory::GameServer => (0.9, 0.65, 0.2, 0.8),
        CustomerCategory::Saas => (0.7, 0.7, 0.5, 0.7),
        CustomerCategory::Database => (0.7, 0.8, 0.9, 0.4),
        CustomerCategory::FileStorage => (0.2, 0.5, 0.8, 0.95),
        CustomerCategory::Backup => (
            0.1,
            0.3,
            if hour < 6.0 { 0.9 } else { 0.05 },
            if hour < 6.0 { 1.0 } else { 0.05 },
        ),
        CustomerCategory::HighPerformance => (1.0, 0.8, 0.8, 0.8),
    };
    let idle = matches!(
        customer.category,
        CustomerCategory::Website | CustomerCategory::Developer | CustomerCategory::FileStorage
    ) && random < 0.15
        && daily < 0.5;
    let intensity = if idle { 0.0 } else { daily * spike };
    let target_disk = req.disk_gb * 0.8;
    WorkloadDemand {
        cpu_mhz: req.cpu_mhz * cpu * intensity,
        memory_mb: req.ram_mb * memory * (0.6 + intensity * 0.4),
        cache_mb: req.ram_mb * 0.15,
        read_iops: req.iops * io * intensity * 0.6,
        write_iops: req.iops * io * intensity * 0.4,
        disk_mb_s: req.iops * io * intensity * 0.016,
        disk_growth_gb_s: (target_disk - vm.disk_used_gb).max(0.0) / (5.0 * 86400.0) * intensity,
        network_mbps: req.network_mbps * network * intensity,
    }
}
impl NetworkSim {
    pub(crate) fn tick_hosting_workloads(&mut self) {
        let seconds = HOSTING_TICK_MS as f64 / 1000.0;
        let previous_states: Vec<_> = self.hosting.vms.values().map(|v| v.power).collect();
        let mut by_host: BTreeMap<DeviceId, Vec<u64>> = BTreeMap::new();
        // Measure delivery against work permitted by the purchased contract.
        // Customer requests above a quota are not provider capacity failures.
        let mut entitled: BTreeMap<u64, WorkloadDemand> = BTreeMap::new();
        for vm in self.hosting.vms.values_mut() {
            let cumulative = vm.metrics.completed_cpu_mhz_seconds;
            let traffic = vm.metrics.traffic_bytes;
            let disk_queue = vm.metrics.disk_queue_ops;
            vm.metrics = VmMetrics {
                completed_cpu_mhz_seconds: cumulative,
                traffic_bytes: traffic,
                disk_queue_ops: disk_queue,
                ..Default::default()
            };
            vm.demand = if vm.power == VmPower::Running {
                vm.workload_override.clone().unwrap_or_else(|| {
                    customer_workload(
                        &self.hosting.customers[&vm.customer],
                        vm,
                        self.hosting.now_ms,
                    )
                })
            } else {
                WorkloadDemand::default()
            };
            by_host.entry(vm.host).or_default().push(vm.id);
            entitled.insert(vm.id, WorkloadDemand::default());
        }
        let node_status: BTreeMap<_, _> = by_host
            .keys()
            .map(|host| {
                (
                    *host,
                    (
                        self.hosting_server_operational(*host),
                        self.hosting_node_eligible(*host),
                    ),
                )
            })
            .collect();
        for (host, ids) in by_host {
            let powered = node_status[&host].0;
            if !powered {
                continue;
            }
            let policy = self.hosting.nodes[&host].policy.clone();
            let cap = self.hosting_capacity(host);
            let cpu_capacity = (cap.cpu_mhz - policy.reserved_cpu_mhz as f64).max(0.0);
            let memory_capacity = (cap.ram_mb - policy.reserved_ram_mb as f64).max(0.0);
            let weights: Vec<_> = ids
                .iter()
                .map(|id| f64::from(self.hosting.vms[id].plan.cpu_weight))
                .collect();
            let mut dedicated = vec![0.0; ids.len()];
            let mut limited = vec![0.0; ids.len()];
            for (i, id) in ids.iter().enumerate() {
                let vm = self.hosting.vms.get_mut(id).unwrap();
                if vm.power == VmPower::Running {
                    vm.host_memory_failure = false;
                }
                let base =
                    f64::from(vm.plan.vcpu) * VCPU_MHZ * f64::from(vm.plan.cpu_limit_percent)
                        / 100.0;
                let burst =
                    f64::from(vm.plan.vcpu) * VCPU_MHZ * f64::from(vm.plan.cpu_burst_percent)
                        / 100.0;
                let limit = base + (burst - base) * (vm.burst_credit_seconds / seconds).min(1.0);
                limited[i] = vm.demand.cpu_mhz.min(limit);
                entitled.get_mut(id).unwrap().cpu_mhz = limited[i];
                vm.metrics.cpu_throttled =
                    ((vm.demand.cpu_mhz - limited[i]) / vm.demand.cpu_mhz.max(0.001)).max(0.0);
                if vm.plan.cpu_policy == CpuPolicy::Dedicated {
                    dedicated[i] = limited[i];
                }
            }
            let mut memory_demand = Vec::new();
            for id in &ids {
                let vm = self.hosting.vms.get_mut(id).unwrap();
                let working = vm.demand.memory_mb.min(vm.plan.ram_mb as f64);
                let cache = vm.demand.cache_mb;
                // Guest allocation failures reclaim/kill guest processes. They
                // never power off the VM, disable its NIC, or consume host RAM
                // beyond the purchased quota. Host OOM is handled below.
                vm.metrics.guest_memory_limited_mb = (vm.demand.memory_mb - working).max(0.0);
                memory_demand.push((working + cache).min(vm.plan.ram_mb as f64));
            }
            let pressure = memory_demand.iter().sum::<f64>() > memory_capacity;
            if pressure && policy.ballooning {
                for (i, id) in ids.iter().enumerate() {
                    let vm = self.hosting.vms.get_mut(id).unwrap();
                    let working = vm.demand.memory_mb.min(vm.plan.ram_mb as f64);
                    let reclaim = (memory_demand[i] - working).max(0.0);
                    memory_demand[i] -= reclaim;
                    vm.metrics.ballooned_mb = reclaim;
                }
            }
            // Reclaim cache, then swap cold pages. Exhausting both kills the
            // largest working set; virtual allocations alone never trigger OOM.
            let usable_swap = (policy.swap_mb as f64)
                .min((cap.disk_gb - policy.reserved_disk_gb as f64).max(0.0) * 1024.0);
            while memory_demand.iter().sum::<f64>() > memory_capacity + usable_swap {
                let Some((victim, _)) = memory_demand
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| **d > 0.0)
                    .max_by(|a, b| a.1.total_cmp(b.1))
                else {
                    break;
                };
                memory_demand[victim] = 0.0;
                let vm = self.hosting.vms.get_mut(&ids[victim]).unwrap();
                vm.power = VmPower::Oom;
                vm.host_memory_failure = true;
                vm.customer_restart_at_ms = self.hosting.now_ms + HOSTING_TICK_MS;
                self.hosting.nodes.get_mut(&host).unwrap().usage.oom_events += 1;
            }
            let resident = fair_schedule(&memory_demand, &weights, memory_capacity);
            for (i, id) in ids.iter().enumerate() {
                if self.hosting.vms[id].power != VmPower::Running {
                    dedicated[i] = 0.0;
                    limited[i] = 0.0;
                }
            }
            // Dedicated guests receive their reservation first. If hardware was
            // removed after placement, reservations compete rather than invent CPU.
            let reserved = fair_schedule(&dedicated, &weights, cpu_capacity);
            let residual: Vec<_> = limited
                .iter()
                .zip(&reserved)
                .map(|(a, b)| (a - b).max(0.0))
                .collect();
            let shared = fair_schedule(
                &residual,
                &weights,
                cpu_capacity - reserved.iter().sum::<f64>(),
            );
            for (i, id) in ids.iter().enumerate() {
                let vm = self.hosting.vms.get_mut(id).unwrap();
                if vm.power != VmPower::Running {
                    vm.demand = WorkloadDemand::default();
                    continue;
                }
                vm.metrics.resident_mb = resident[i];
                vm.metrics.swapped_mb = (memory_demand[i] - resident[i]).max(0.0);
                let swap_factor = if vm.metrics.swapped_mb > 0.0 {
                    resident[i] / memory_demand[i].max(1.0)
                } else {
                    1.0
                };
                vm.metrics.cpu_used_mhz = (reserved[i] + shared[i]) * swap_factor;
                vm.metrics.cpu_ready =
                    ((limited[i] - reserved[i] - shared[i]) / limited[i].max(0.001)).max(0.0);
                let base =
                    f64::from(vm.plan.vcpu) * VCPU_MHZ * f64::from(vm.plan.cpu_limit_percent)
                        / 100.0;
                if vm.metrics.cpu_used_mhz > base {
                    let extra = f64::from(vm.plan.vcpu)
                        * VCPU_MHZ
                        * f64::from(vm.plan.cpu_burst_percent - vm.plan.cpu_limit_percent)
                        / 100.0;
                    vm.burst_credit_seconds = (vm.burst_credit_seconds
                        - seconds * (vm.metrics.cpu_used_mhz - base) / extra.max(1.0))
                    .max(0.0);
                } else {
                    vm.burst_credit_seconds = (vm.burst_credit_seconds
                        + seconds * (1.0 - vm.metrics.cpu_used_mhz / base.max(1.0)))
                    .min(f64::from(vm.plan.burst_seconds));
                }
                vm.metrics.completed_cpu_mhz_seconds += vm.metrics.cpu_used_mhz * seconds;
            }
            let disk_capacity =
                (cap.disk_gb - policy.reserved_disk_gb as f64 - policy.swap_mb as f64 / 1024.0)
                    .max(0.0);
            let disk_ids = &ids;
            let disk_weights: Vec<_> = disk_ids
                .iter()
                .map(|id| f64::from(self.hosting.vms[id].plan.cpu_weight))
                .collect();
            let iops: Vec<_> = disk_ids
                .iter()
                .map(|id| {
                    let v = &self.hosting.vms[id];
                    if v.power != VmPower::Running {
                        return 0.0;
                    }
                    (v.demand.read_iops
                        + v.demand.write_iops
                        + v.metrics.swapped_mb * 1024.0 / 4.0 / seconds
                        + v.metrics.disk_queue_ops / seconds)
                        .min(f64::from(v.plan.disk_iops))
                })
                .collect();
            let throughput: Vec<_> = disk_ids
                .iter()
                .map(|id| {
                    self.hosting.vms[id]
                        .demand
                        .disk_mb_s
                        .min(f64::from(self.hosting.vms[id].plan.disk_mb_s))
                })
                .collect();
            let io_allocation = fair_schedule(&iops, &disk_weights, cap.iops);
            let mb_allocation = fair_schedule(&throughput, &disk_weights, cap.disk_mb_s);
            let mut free = (disk_capacity
                - disk_ids
                    .iter()
                    .map(|id| self.hosting.vms[id].physical_disk_gb)
                    .sum::<f64>())
            .max(0.0);
            let requested_writes: Vec<_> = disk_ids
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    let v = &self.hosting.vms[id];
                    let io_fraction = if iops[i] > 0.0 {
                        (io_allocation[i] / iops[i]).min(1.0)
                    } else {
                        1.0
                    };
                    (v.demand.disk_growth_gb_s * seconds * io_fraction)
                        .min(mb_allocation[i] * io_fraction * seconds / 1000.0)
                })
                .collect();
            let growth: Vec<_> = disk_ids
                .iter()
                .zip(&requested_writes)
                .map(|(id, requested)| {
                    requested.min(
                        (self.hosting.vms[id].plan.disk_gb as f64
                            - self.hosting.vms[id].disk_used_gb)
                            .max(0.0),
                    )
                })
                .collect();
            let requested_growth: Vec<_> = disk_ids
                .iter()
                .zip(&growth)
                .map(|(id, growth)| {
                    if self.hosting.vms[id].plan.thin {
                        *growth
                    } else {
                        0.0
                    }
                })
                .collect();
            let growth_allocation = fair_schedule(&requested_growth, &disk_weights, free);
            for (i, id) in disk_ids.iter().enumerate() {
                let vm = self.hosting.vms.get_mut(id).unwrap();
                if vm.power != VmPower::Running {
                    continue;
                }
                let io_fraction = if iops[i] > 0.0 {
                    (io_allocation[i] / iops[i]).min(1.0)
                } else {
                    1.0
                };
                let mb_fraction = if throughput[i] > 0.0 {
                    (mb_allocation[i] / throughput[i]).min(1.0)
                } else {
                    1.0
                };
                vm.metrics.disk_iops = io_allocation[i] * mb_fraction;
                vm.metrics.disk_mb_s = mb_allocation[i] * io_fraction;
                let delivery = entitled.get_mut(id).unwrap();
                delivery.read_iops = iops[i];
                delivery.disk_mb_s = throughput[i];
                vm.metrics.disk_ready = 1.0 - io_fraction.min(mb_fraction);
                let incoming_ops = (vm.demand.read_iops + vm.demand.write_iops) * seconds
                    + vm.metrics.swapped_mb * 256.0;
                let queue = (vm.metrics.disk_queue_ops + incoming_ops
                    - vm.metrics.disk_iops * seconds)
                    .max(0.0);
                // A bounded guest queue rejects excess operations instead of
                // unboundedly accumulating simulation objects.
                vm.metrics.disk_queue_ops = queue.min(f64::from(vm.plan.disk_iops) * 30.0);
                vm.metrics.disk_latency_ms = if vm.metrics.disk_iops > 0.0 {
                    1000.0 / vm.metrics.disk_iops
                        + vm.metrics.disk_queue_ops / vm.metrics.disk_iops * 1000.0
                } else {
                    30000.0
                };
                let accepted = if vm.plan.thin {
                    growth_allocation[i].min(free)
                } else {
                    growth[i]
                };
                vm.metrics.guest_disk_full_gb = (requested_writes[i] - growth[i]).max(0.0);
                vm.metrics.write_failed_gb = (growth[i] - accepted).max(0.0);
                vm.disk_used_gb += accepted;
                if vm.plan.thin {
                    vm.physical_disk_gb += accepted;
                    free -= accepted;
                }
            }
        }
        let mut flows: Vec<_> = self
            .hosting
            .vms
            .values()
            .filter(|v| v.power == VmPower::Running && node_status[&v.host].0)
            .map(|v| {
                let traffic_limit = v.plan.traffic_gb.map_or(f64::INFINITY, |gb| {
                    gb.saturating_mul(1_000_000_000)
                        .saturating_sub(v.metrics.traffic_bytes) as f64
                        / (125000.0 * seconds)
                });
                TrafficFlow {
                    id: v.id,
                    source: v.interfaces[0],
                    source_ipv4: None,
                    destination: std::net::Ipv4Addr::new(198, 51, 100, 1),
                    demand_mbps: v.demand.network_mbps,
                    limit_mbps: f64::from(v.plan.port_mbps).min(traffic_limit),
                }
            })
            .collect();
        for flow in &flows {
            entitled.get_mut(&flow.id).unwrap().network_mbps =
                flow.demand_mbps.min(flow.limit_mbps);
        }
        let site_flow = self.hosting_site_flow();
        flows.extend(site_flow);
        let traffic = self.simulate_traffic(&flows, seconds);
        let site = traffic.get(&u64::MAX).copied().unwrap_or_default();
        self.hosting.site_traffic = HostingSiteTraffic {
            bytes: self.hosting.site_traffic.bytes.saturating_add(site.bytes),
            demand_mbps: site_flow.map_or(0.0, |flow| flow.demand_mbps),
            delivered_mbps: site.delivered_mbps,
            latency_ms: site.latency_ms,
            loss: site.loss,
        };
        let paused_guests: Vec<_> = self
            .hosting
            .vms
            .values()
            .filter(|vm| {
                vm.power != VmPower::Running && !vm.host_memory_failure && node_status[&vm.host].1
            })
            .map(|vm| vm.id)
            .collect();
        let restart_network = self.hosting_restart_network_status(&paused_guests);
        let mut mean_quality = 0.0;
        let mut count = 0;
        for vm in self.hosting.vms.values_mut() {
            let permitted = &entitled[&vm.id];
            if let Some(t) = traffic.get(&vm.id) {
                vm.metrics.network_mbps = t.delivered_mbps;
                vm.metrics.network_latency_ms = t.latency_ms;
                // No packets are offered after the customer's allowance is
                // exhausted; losses on neighboring flows are not this VM's loss.
                vm.metrics.packet_loss = if permitted.network_mbps > 0.0 {
                    t.loss
                } else {
                    0.0
                };
                vm.metrics.reachable = t.reachable;
                vm.metrics.traffic_bytes = vm.metrics.traffic_bytes.saturating_add(t.bytes);
            }
            let (operational, manageable) = node_status[&vm.host];
            let network_available = if vm.power == VmPower::Running {
                vm.metrics.reachable
            } else {
                restart_network.get(&vm.id).copied().unwrap_or(false)
            };
            vm.metrics.provider_fault = if !operational {
                Some(HostingProviderFault::HostUnavailable)
            } else if vm.host_memory_failure {
                Some(HostingProviderFault::HostMemoryExhausted)
            } else if vm.power != VmPower::Running && !manageable {
                Some(HostingProviderFault::ManagementUnavailable)
            } else if !network_available {
                Some(HostingProviderFault::PublicNetworkUnavailable)
            } else {
                None
            };
            let cpu = delivery_ratio(vm.metrics.cpu_used_mhz, permitted.cpu_mhz);
            let memory = if vm.metrics.swapped_mb > 0.0 {
                (vm.metrics.resident_mb / (vm.metrics.resident_mb + vm.metrics.swapped_mb)).powi(2)
            } else {
                1.0
            };
            let io = delivery_ratio(vm.metrics.disk_iops, permitted.read_iops)
                .min(delivery_ratio(vm.metrics.disk_mb_s, permitted.disk_mb_s));
            let network = delivery_ratio(vm.metrics.network_mbps, permitted.network_mbps);
            let customer = self.hosting.customers.get_mut(&vm.customer).unwrap();
            let latency = if permitted.network_mbps > 0.0 {
                (customer.requirements.max_latency_ms / vm.metrics.network_latency_ms.max(1.0))
                    .min(1.0)
            } else {
                1.0
            };
            let w = quality_weights(customer.category);
            let quality = if vm.metrics.provider_fault.is_some() {
                0.0
            } else {
                cpu.powf(w[0])
                    * memory.powf(w[1])
                    * io.powf(w[2])
                    * (network * latency).powf(w[3])
                    * if vm.metrics.write_failed_gb > 0.0 {
                        0.2
                    } else {
                        1.0
                    }
            };
            vm.metrics.quality = quality;
            let service = self.hosting.services.get_mut(&vm.service).unwrap();
            let alpha = 1.0 - (-seconds / 86400.0).exp();
            service.satisfaction += (quality - service.satisfaction) * alpha;
            service.observed_quality += (quality - service.observed_quality) * alpha;
            service.availability +=
                (f64::from(vm.metrics.provider_fault.is_none()) - service.availability) * alpha;
            customer.satisfaction = service.satisfaction;
            customer.churn_probability =
                ((customer.performance_expectation - customer.satisfaction).max(0.0)
                    / customer.performance_expectation)
                    .powi(2)
                    .clamp(0.0, 1.0);
            // Simulated customers recover their own guests through the same
            // management connectivity required by interactive VM operations.
            // A host OOM recovery waits at least one interval and is assessed
            // again against actual RAM at the next workload scheduling pass.
            let restart_at = if vm.power == VmPower::Rebooting {
                vm.reboot_until_ms
            } else {
                vm.customer_restart_at_ms
            };
            if vm.power != VmPower::Running && manageable && restart_at <= self.hosting.now_ms {
                vm.power = VmPower::Running;
            }
            mean_quality += quality;
            count += 1;
        }
        // An operator-destroyed paid guest is a provider outage until recreated
        // or refunded. Customer shutdowns above retain their restart capability.
        let alpha = 1.0 - (-seconds / 86400.0).exp();
        for service in self
            .hosting
            .services
            .values_mut()
            .filter(|s| s.terminated_at_ms.is_none() && s.vm.is_none())
        {
            service.satisfaction *= 1.0 - alpha;
            service.observed_quality *= 1.0 - alpha;
            service.availability *= 1.0 - alpha;
            let customer = self.hosting.customers.get_mut(&service.customer).unwrap();
            customer.satisfaction = service.satisfaction;
            customer.churn_probability =
                ((customer.performance_expectation - customer.satisfaction).max(0.0)
                    / customer.performance_expectation)
                    .powi(2)
                    .clamp(0.0, 1.0);
            count += 1;
        }
        if count > 0 {
            self.hosting.reputation += (mean_quality / f64::from(count) - self.hosting.reputation)
                * (1.0 - (-seconds / (3.0 * 86400.0)).exp());
        }
        if self
            .hosting
            .vms
            .values()
            .map(|v| v.power)
            .ne(previous_states)
        {
            self.topology_revision += 1;
        }
        self.refresh_hosting_usage();
    }
}
fn delivery_ratio(delivered: f64, permitted: f64) -> f64 {
    if permitted > 0.0 {
        (delivered / permitted).clamp(0.0, 1.0)
    } else {
        1.0
    }
}
fn quality_weights(category: CustomerCategory) -> [f64; 4] {
    match category {
        CustomerCategory::GameServer | CustomerCategory::HighPerformance => [0.5, 0.15, 0.1, 0.25],
        CustomerCategory::Database => [0.2, 0.35, 0.4, 0.05],
        CustomerCategory::FileStorage | CustomerCategory::Backup => [0.05, 0.1, 0.4, 0.45],
        _ => [0.3, 0.25, 0.2, 0.25],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scheduler_redistributes_idle_cpu_by_weight() {
        assert_eq!(
            fair_schedule(&[1.0, 100.0, 100.0], &[1.0, 1.0, 2.0], 10.0),
            vec![1.0, 3.0, 6.0]
        );
        assert_eq!(
            fair_schedule(&[1.0, 2.0], &[1.0, 1.0], 10.0),
            vec![1.0, 2.0]
        );
    }
}
