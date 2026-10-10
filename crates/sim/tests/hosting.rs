use cloud_provider_sim::*;
use std::net::Ipv4Addr;
fn ip(s: &str) -> Ipv4Addr {
    s.parse().unwrap()
}
fn prefix(s: &str) -> Ipv4Prefix {
    s.parse().unwrap()
}
fn apply(sim: &mut NetworkSim, c: HostingCommand) {
    sim.execute(Command::Hosting(c)).unwrap();
}
fn power(sim: &mut NetworkSim, id: DeviceId, unit: u8) {
    let rack = RackId(if unit > 4 { 2 } else { 1 });
    let rack_unit = if unit > 4 { unit - 4 } else { unit };
    sim.execute(Command::PlaceDevice {
        device: id,
        rack,
        unit: rack_unit,
    })
    .unwrap();
    sim.execute(Command::ConnectPower {
        outlet: OutletId {
            source: SourceId::Rack(rack),
            index: rack_unit - 1,
        },
        endpoint: PowerEndpoint::Device(id),
    })
    .unwrap();
    sim.execute(Command::SetPower {
        device: id,
        powered: true,
    })
    .unwrap();
}
struct Lab {
    sim: NetworkSim,
    host: DeviceId,
    controller: DeviceId,
    controller_public: PortId,
    controller_management: PortId,
    mgmt: PortId,
    data: PortId,
    transit: PortId,
    plan: u64,
}
impl Lab {
    fn new() -> Self {
        let mut sim = NetworkSim::new();
        sim.money = 100000;
        sim.hosting.market.prospects_per_day = 0;
        sim.hosting.market.time_scale = 1;
        sim.hosting.market.periods = vec![SubscriptionOption {
            months: 1,
            weight: 1,
        }];
        for supply in [CableSupply::CableBox305m, CableSupply::Rj45Pack20] {
            sim.execute(Command::BuyCableSupply { supply }).unwrap();
        }
        let SimEvent::DeviceAdded(host) = sim.execute(Command::BuyServerFullPack).unwrap()[0]
        else {
            panic!()
        };
        let SimEvent::DeviceAdded(router) = sim
            .execute(Command::BuyDevice {
                kind: DeviceTemplate::Router,
            })
            .unwrap()[0]
        else {
            panic!()
        };
        power(&mut sim, host, 1);
        power(&mut sim, router, 2);
        let data = sim.device(host).unwrap().ports()[0];
        let mgmt = sim
            .device(host)
            .unwrap()
            .ports()
            .iter()
            .copied()
            .find(|p| sim.port(*p).unwrap().name == "mgmt0")
            .unwrap();
        let rp = sim.device(router).unwrap().ports().to_vec();
        for (port, address, bits) in [
            (rp[0], "192.0.2.2", 30),
            (rp[2], "203.0.113.1", 24),
            (rp[3], "10.0.0.1", 24),
        ] {
            sim.execute(Command::ConfigureRouterInterface {
                port,
                name: format!("p{}", port.0),
                vlan: None,
                address: Some(ip(address)),
                prefix: bits,
                internet_connected: false,
            })
            .unwrap();
        }
        for (port, address, bits, gateway) in [
            (data, "203.0.113.100", 24, Some(ip("203.0.113.1"))),
            (mgmt, "10.0.0.2", 24, None),
        ] {
            sim.execute(Command::SetIpv4 {
                port,
                config: Ipv4InterfaceConfig::new(ip(address), bits, gateway, VlanId(1)),
            })
            .unwrap();
        }
        let transit = sim
            .network_outlets()
            .filter(|o| matches!(o.kind, NetworkOutletKind::Uplink { .. }))
            .min_by_key(|o| o.port)
            .unwrap()
            .port;
        for (a, b) in [(data, rp[2]), (mgmt, rp[3]), (rp[0], transit)] {
            sim.execute(Command::Connect { a, b }).unwrap();
        }
        sim.execute(Command::Provider(ProviderCommand::SetTransit(
            TransitCircuit {
                port: transit,
                name: "Transit".into(),
                address: ip("192.0.2.1"),
                prefix: 30,
                asn: 64501,
                capacity_mbps: 100,
                enabled: true,
                routes: vec![UpstreamRoute {
                    prefix: prefix("203.0.113.0/24"),
                    next_hop: ip("192.0.2.2"),
                }],
                offered_routes: vec![prefix("0.0.0.0/0")],
                authorizations: vec![],
            },
        )))
        .unwrap();
        sim.execute(Command::SetStaticRoute {
            router,
            route: Route {
                network: ip("0.0.0.0"),
                prefix: 0,
                via: Some(ip("192.0.2.1")),
                egress: rp[0],
            },
        })
        .unwrap();
        sim.buy_public_ipv4_pool().unwrap();
        sim.buy_public_ipv4_pool().unwrap();
        sim.buy_public_ipv4_pool().unwrap();
        let SimEvent::DeviceAdded(controller) = sim.execute(Command::BuyServerFullPack).unwrap()[0]
        else {
            panic!()
        };
        power(&mut sim, controller, 3);
        let controller_public = sim.device(controller).unwrap().ports()[0];
        let controller_management = sim
            .device(controller)
            .unwrap()
            .ports()
            .iter()
            .copied()
            .find(|p| sim.port(*p).unwrap().name == "mgmt0")
            .unwrap();
        for (port, address, gateway) in [
            (controller_public, "203.0.113.18", Some(ip("203.0.113.1"))),
            (controller_management, "10.0.0.10", None),
        ] {
            sim.execute(Command::SetIpv4 {
                port,
                config: Ipv4InterfaceConfig::new(ip(address), 24, gateway, VlanId(1)),
            })
            .unwrap();
        }
        for (unit, node_port, router_port, control_port) in [
            (4, data, rp[2], controller_public),
            (5, mgmt, rp[3], controller_management),
        ] {
            let link = sim.link_for_port(node_port).unwrap().id;
            sim.execute(Command::Disconnect { link }).unwrap();
            let SimEvent::DeviceAdded(switch) = sim
                .execute(Command::BuyDevice {
                    kind: DeviceTemplate::Switch,
                })
                .unwrap()[0]
            else {
                panic!()
            };
            power(&mut sim, switch, unit);
            let ports = sim.device(switch).unwrap().ports().to_vec();
            for (a, b) in [
                (node_port, ports[0]),
                (router_port, ports[1]),
                (control_port, ports[2]),
            ] {
                sim.execute(Command::Connect { a, b }).unwrap();
            }
        }
        let setup = sim.execute_console(controller, "./setup-main-hoster-server.sh");
        assert!(setup.success, "{:?}", setup.lines);
        apply(
            &mut sim,
            HostingCommand::SetProviderSiteAddress(Some(ip("203.0.113.18"))),
        );
        assert!(sim.hosting_provider_site_status().is_ok());
        let p = HostingProduct {
            eligible_nodes: [host].into(),
            ipv4_ranges: [prefix("203.0.113.0/29"), prefix("203.0.113.8/29")].into(),
            ..Default::default()
        };
        apply(&mut sim, HostingCommand::SaveProduct(p));
        let plan = *sim.hosting.products.keys().next().unwrap();
        apply(
            &mut sim,
            HostingCommand::Publish {
                product: plan,
                published: true,
            },
        );
        Self {
            sim,
            host,
            controller,
            controller_public,
            controller_management,
            mgmt,
            data,
            transit,
            plan,
        }
    }
    fn prepare(&mut self) {
        let output = self
            .sim
            .execute_console(self.host, "./vm-prepare.sh 10.0.0.10");
        assert!(output.success, "{:?}", output.lines);
    }
    fn buy(&mut self, order: u64) -> u64 {
        self.sim
            .purchase_hosting(order, self.plan, CustomerCategory::Website)
            .unwrap()
    }
    fn tick(&mut self) {
        self.sim.advance_time(HOSTING_TICK_MS);
    }
    fn policy(&mut self, change: impl FnOnce(&mut HypervisorPolicy)) {
        let mut policy = self.sim.hosting.nodes[&self.host].policy.clone();
        change(&mut policy);
        apply(
            &mut self.sim,
            HostingCommand::SetNodePolicy {
                node: self.host,
                policy,
            },
        );
    }
    fn vm(&self, service: u64) -> u64 {
        self.sim.hosting.services[&service].vm.unwrap()
    }
    fn demand(&mut self, vm: u64, demand: WorkloadDemand) {
        self.sim.hosting.vms.get_mut(&vm).unwrap().workload_override = Some(demand);
    }
}
#[test]
fn preparation_requires_management_and_survives_reexecution_and_disconnect() {
    let mut l = Lab::new();
    assert!(l.sim.hosting_availability(l.plan).is_err());
    assert!(
        l.sim
            .purchase_hosting(1, l.plan, CustomerCategory::Website)
            .is_err()
    );
    let link = l.sim.link_for_port(l.mgmt).unwrap();
    let router_port = if link.a == l.mgmt { link.b } else { link.a };
    let link = link.id;
    l.sim.execute(Command::Disconnect { link }).unwrap();
    assert!(
        !l.sim
            .execute_console(l.host, "./vm-prepare.sh 10.0.0.10")
            .success
    );
    assert!(
        l.sim
            .hosting
            .nodes
            .get(&l.host)
            .is_none_or(|n| n.prepared_at_ms.is_none())
    );
    l.sim
        .execute(Command::Connect {
            a: l.mgmt,
            b: router_port,
        })
        .unwrap();
    l.prepare();
    assert!(l.sim.hosting_node_eligible(l.host));
    let service = l.buy(1);
    let vm = l.vm(service);
    l.prepare();
    assert_eq!(l.sim.hosting.vms.len(), 1);
    let link = l.sim.link_for_port(l.mgmt).unwrap().id;
    l.sim.execute(Command::Disconnect { link }).unwrap();
    assert!(
        l.sim
            .purchase_hosting(2, l.plan, CustomerCategory::Website)
            .is_err()
    );
    assert!(
        l.sim
            .configure_hosting(HostingCommand::RebootVm(vm))
            .is_err()
    );
    l.tick();
    assert_eq!(l.sim.hosting.vms[&vm].power, VmPower::Running);
    assert!(l.sim.hosting.vms[&vm].metrics.reachable);
}
#[test]
fn multiple_ranges_share_unique_ips_and_exhaustion_and_termination_are_atomic() {
    let mut l = Lab::new();
    l.prepare();
    assert_eq!(l.sim.hosting.products[&l.plan].ipv4_ranges.len(), 2);
    l.policy(|p| {
        p.ram_overcommit_percent = 400;
        p.cpu_overcommit_percent = 1000;
    });
    let mut ips = std::collections::BTreeSet::new();
    for i in 1..=9 {
        let service = l.buy(i);
        let vm = &l.sim.hosting.vms[&l.vm(service)];
        assert!(
            vm.ips
                .iter()
                .all(|ip| vm.plan.ipv4_ranges.iter().any(|p| p.usable(*ip)))
        );
        assert!(ips.insert(vm.ips[0]));
    }
    // Two /29s: first usable is a reserved gateway in each pool; one gateway is
    // already configured, so ten customer addresses remain.
    let service = l.buy(10);
    assert!(ips.insert(l.sim.hosting.vms[&l.vm(service)].ips[0]));
    let before = ron::to_string(&l.sim.hosting).unwrap();
    assert!(
        l.sim
            .purchase_hosting(11, l.plan, CustomerCategory::Website)
            .is_err()
    );
    assert_eq!(ron::to_string(&l.sim.hosting).unwrap(), before);
    let released = l.sim.hosting.vms[&l.vm(service)].ips[0];
    l.sim.terminate_hosting_service(service).unwrap();
    l.sim.terminate_hosting_service(service).unwrap();
    let replacement = l.buy(11);
    assert_eq!(l.sim.hosting.vms[&l.vm(replacement)].ips[0], released);
    let count = l.sim.hosting.vms.len();
    let money = l.sim.money;
    assert_eq!(l.buy(11), replacement);
    assert_eq!(l.sim.money, money);
    assert_eq!(l.sim.hosting.vms.len(), count);
    assert_eq!(l.buy(10), service);
    assert_eq!(l.sim.hosting.vms.len(), count);
}
#[test]
fn virtual_interfaces_use_real_routes_and_transit_congestion() {
    let mut l = Lab::new();
    l.prepare();
    let a = l.buy(1);
    let b = l.buy(2);
    let a = l.vm(a);
    let b = l.vm(b);
    let port = l.sim.hosting.vms[&a].interfaces[0];
    let address = l.sim.hosting.vms[&a].ips[0];
    assert!(l.sim.transmit_icmp(port, ip("198.51.100.1")).reachable);
    assert!(l.sim.ping_from_internet(address).reachable);
    assert!(
        l.sim
            .transmit_icmp(port, l.sim.hosting.vms[&b].ips[0])
            .reachable
    );
    for vm in [a, b] {
        l.demand(
            vm,
            WorkloadDemand {
                cpu_mhz: 100.0,
                memory_mb: 512.0,
                network_mbps: 100.0,
                ..Default::default()
            },
        );
    }
    l.tick();
    for vm in [a, b] {
        let m = &l.sim.hosting.vms[&vm].metrics;
        assert!(m.network_mbps <= 50.01 && m.network_mbps > 49.0, "{m:?}");
        assert!(m.packet_loss > 0.4);
        assert!(m.quality < 1.0);
        assert!(m.network_latency_ms > 0.0);
        assert!(m.traffic_bytes > 0);
    }
    assert!(l.sim.port_telemetry(l.transit).tx_bytes > 0);
    let link = l.sim.link_for_port(l.data).unwrap().id;
    l.sim.execute(Command::Disconnect { link }).unwrap();
    l.tick();
    assert!(!l.sim.hosting.vms[&a].metrics.reachable);
    assert_eq!(l.sim.hosting.vms[&a].metrics.network_mbps, 0.0);
}
#[test]
fn cpu_overcommit_degrades_only_actual_load_and_dedicated_cpu_reserves_capacity() {
    let mut l = Lab::new();
    l.prepare();
    let mut vms = vec![];
    for i in 1..=3 {
        let s = l.buy(i);
        vms.push(l.vm(s));
    }
    assert!(l.sim.hosting.nodes[&l.host].usage.sold_vcpu > 4);
    for id in &vms {
        l.demand(
            *id,
            WorkloadDemand {
                cpu_mhz: 500.0,
                memory_mb: 256.0,
                ..Default::default()
            },
        );
    }
    l.tick();
    for id in &vms {
        assert_eq!(l.sim.hosting.vms[id].metrics.cpu_ready, 0.0);
        assert_eq!(l.sim.hosting.vms[id].metrics.cpu_used_mhz, 500.0);
        assert_eq!(l.sim.hosting.vms[id].metrics.quality, 1.0);
    }
    for id in &vms {
        l.demand(
            *id,
            WorkloadDemand {
                cpu_mhz: 6800.0,
                memory_mb: 256.0,
                ..Default::default()
            },
        );
    }
    l.tick();
    for id in &vms {
        assert!(l.sim.hosting.vms[id].metrics.cpu_ready > 0.3);
        assert!(l.sim.hosting.vms[id].metrics.cpu_used_mhz < 5000.0);
        assert!(l.sim.hosting.vms[id].metrics.quality < 1.0);
    }
    let mut p = l.sim.hosting.products[&l.plan].clone();
    p.category = HostingCategory::DedicatedCpu;
    p.cpu_policy = CpuPolicy::Dedicated;
    p.id = 0;
    apply(&mut l.sim, HostingCommand::SaveProduct(p));
    let product = *l.sim.hosting.products.keys().last().unwrap();
    let s = l
        .sim
        .purchase_hosting(4, product, CustomerCategory::Website)
        .unwrap();
    let dedicated = l.vm(s);
    l.demand(
        dedicated,
        WorkloadDemand {
            cpu_mhz: 6800.0,
            memory_mb: 256.0,
            ..Default::default()
        },
    );
    l.tick();
    assert_eq!(l.sim.hosting.vms[&dedicated].metrics.cpu_used_mhz, 6800.0);
    assert!(
        l.sim
            .purchase_hosting(5, product, CustomerCategory::Website)
            .is_err()
    );
}
#[test]
fn memory_overcommit_reclaims_then_swaps_and_ooms_from_demand() {
    let mut l = Lab::new();
    l.prepare();
    l.policy(|p| p.ram_overcommit_percent = 400);
    let mut p = l.sim.hosting.products[&l.plan].clone();
    p.ram_mb = 12000;
    apply(&mut l.sim, HostingCommand::SaveProduct(p));
    let a = l.buy(1);
    let b = l.buy(2);
    let a = l.vm(a);
    let b = l.vm(b);
    for vm in [a, b] {
        l.demand(
            vm,
            WorkloadDemand {
                cpu_mhz: 100.0,
                memory_mb: 1000.0,
                ..Default::default()
            },
        );
    }
    l.tick();
    assert!(
        l.sim
            .hosting
            .vms
            .values()
            .all(|v| v.power == VmPower::Running && v.metrics.swapped_mb == 0.0)
    );
    for vm in [a, b] {
        l.demand(
            vm,
            WorkloadDemand {
                cpu_mhz: 100.0,
                memory_mb: 8500.0,
                cache_mb: 2000.0,
                ..Default::default()
            },
        );
    }
    l.tick();
    assert!(
        l.sim
            .hosting
            .vms
            .values()
            .all(|v| v.power == VmPower::Running
                && v.metrics.swapped_mb > 0.0
                && v.metrics.quality < 1.0
                && v.metrics.ballooned_mb > 0.0)
    );
    for vm in [a, b] {
        l.demand(
            vm,
            WorkloadDemand {
                cpu_mhz: 100.0,
                memory_mb: 12000.0,
                ..Default::default()
            },
        );
    }
    l.tick();
    assert!(l.sim.hosting.vms.values().any(|v| v.power == VmPower::Oom));
    for v in l
        .sim
        .hosting
        .vms
        .values()
        .filter(|v| v.power == VmPower::Oom)
    {
        assert!(v.host_memory_failure);
        assert_eq!(
            v.metrics.provider_fault,
            Some(HostingProviderFault::HostMemoryExhausted)
        );
        assert_eq!(v.metrics.quality, 0.0);
    }
}
#[test]
fn customer_quota_excess_keeps_vm_online_and_provider_quality_healthy() {
    let mut l = Lab::new();
    l.prepare();
    let mut p = l.sim.hosting.products[&l.plan].clone();
    p.vcpu = 1;
    p.ram_mb = 1024;
    p.cpu_limit_percent = 50;
    p.cpu_burst_percent = 100;
    p.burst_seconds = 10;
    p.disk_iops = 100;
    p.disk_mb_s = 1;
    p.port_mbps = 1;
    apply(&mut l.sim, HostingCommand::SaveProduct(p));
    let s = l.buy(1);
    let vm = l.vm(s);
    l.demand(
        vm,
        WorkloadDemand {
            cpu_mhz: 50000.0,
            memory_mb: 100000.0,
            cache_mb: 20000.0,
            read_iops: 30000.0,
            write_iops: 30000.0,
            disk_mb_s: 100.0,
            network_mbps: 1000.0,
            ..Default::default()
        },
    );
    for _ in 0..12 {
        l.tick();
        let v = &l.sim.hosting.vms[&vm];
        assert_eq!(v.power, VmPower::Running);
        assert!(v.metrics.reachable);
        assert_eq!(v.metrics.resident_mb, 1024.0);
        assert_eq!(v.metrics.guest_memory_limited_mb, 98976.0);
        assert_eq!(v.metrics.swapped_mb, 0.0);
        assert!(v.metrics.cpu_throttled > 0.9);
        assert_eq!(v.metrics.cpu_ready, 0.0);
        assert!(v.metrics.disk_iops <= 100.0);
        assert!(v.metrics.disk_mb_s <= 1.0);
        assert_eq!(v.metrics.network_mbps, 1.0);
        assert_eq!(v.metrics.quality, 1.0);
        assert_eq!(l.sim.hosting.nodes[&l.host].usage.oom_events, 0);
        assert_eq!(l.sim.hosting.services[&s].satisfaction, 1.0);
        assert_eq!(l.sim.hosting.services[&s].availability, 1.0);
        assert_eq!(l.sim.hosting.customers[&v.customer].churn_probability, 0.0);
        assert_eq!(l.sim.hosting.reputation, 1.0);
    }
    let saved = ron::to_string(&l.sim).unwrap();
    let mut loaded: NetworkSim = ron::from_str(&saved).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(
        loaded.hosting.vms[&vm].metrics.guest_memory_limited_mb,
        98976.0
    );
    loaded.advance_time(HOSTING_TICK_MS);
    assert_eq!(loaded.hosting.vms[&vm].power, VmPower::Running);
    assert!(loaded.hosting.vms[&vm].metrics.reachable);
    assert_eq!(loaded.hosting.vms[&vm].metrics.quality, 1.0);
    assert_eq!(loaded.hosting.payments.len(), 1);
}

#[test]
fn restarting_a_saved_memory_stopped_guest_does_not_repeat_guest_quota_shutdown() {
    let mut l = Lab::new();
    l.prepare();
    let s = l.buy(1);
    let vm = l.vm(s);
    l.demand(
        vm,
        WorkloadDemand {
            cpu_mhz: 100.0,
            memory_mb: 10000.0,
            ..Default::default()
        },
    );
    l.sim.hosting.vms.get_mut(&vm).unwrap().power = VmPower::Oom;
    let saved = ron::ser::to_string_pretty(&l.sim, Default::default()).unwrap();
    // Earlier saves have no guest quota or physical disk contention metrics.
    let legacy = saved
        .lines()
        .filter(|line| {
            !line.contains("guest_memory_limited_mb:")
                && !line.contains("guest_disk_full_gb:")
                && !line.contains("disk_ready:")
                && !line.contains("provider_fault:")
                && !line.contains("host_memory_failure:")
                && !line.contains("customer_restart_at_ms:")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut loaded: NetworkSim = ron::from_str(&legacy).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.hosting.vms[&vm].power, VmPower::Oom);
    // The customer, rather than the provider, recovers the saved guest stop.
    loaded.advance_time(HOSTING_TICK_MS);
    assert_eq!(loaded.hosting.vms[&vm].power, VmPower::Running);
    assert_eq!(loaded.hosting.vms[&vm].metrics.provider_fault, None);
    assert_eq!(loaded.hosting.vms[&vm].metrics.quality, 1.0);
    for _ in 0..3 {
        loaded.advance_time(HOSTING_TICK_MS);
        assert_eq!(loaded.hosting.vms[&vm].power, VmPower::Running);
        assert!(loaded.hosting.vms[&vm].metrics.reachable);
        assert_eq!(loaded.hosting.vms[&vm].metrics.quality, 1.0);
    }
    assert_eq!(loaded.hosting.payments.len(), 1);
}

#[test]
fn a_full_guest_disk_rejects_customer_writes_without_provider_write_failures() {
    for thin in [true, false] {
        let mut l = Lab::new();
        l.prepare();
        let mut p = l.sim.hosting.products[&l.plan].clone();
        p.disk_gb = 2;
        p.thin = thin;
        apply(&mut l.sim, HostingCommand::SaveProduct(p));
        let s = l.buy(1);
        let vm = l.vm(s);
        l.demand(
            vm,
            WorkloadDemand {
                cpu_mhz: 100.0,
                memory_mb: 256.0,
                write_iops: 100.0,
                disk_mb_s: 1.0,
                disk_growth_gb_s: 1.0,
                ..Default::default()
            },
        );
        for _ in 0..12 {
            l.tick();
        }
        let v = &l.sim.hosting.vms[&vm];
        assert_eq!(v.disk_used_gb, 2.0);
        assert_eq!(v.physical_disk_gb, 2.0);
        assert!(v.metrics.guest_disk_full_gb > 0.0);
        assert_eq!(v.metrics.write_failed_gb, 0.0);
        assert_eq!(v.metrics.quality, 1.0);
        assert_eq!(v.power, VmPower::Running);
        assert!(v.metrics.reachable);
        assert_eq!(l.sim.hosting.reputation, 1.0);
    }
}

#[test]
fn thin_disks_separate_physical_usage_and_fail_writes_at_exhaustion() {
    let mut l = Lab::new();
    l.prepare();
    let mut p = l.sim.hosting.products[&l.plan].clone();
    p.disk_gb = 1300;
    apply(&mut l.sim, HostingCommand::SaveProduct(p));
    let a = l.buy(1);
    let b = l.buy(2);
    let a = l.vm(a);
    let b = l.vm(b);
    assert_eq!(l.sim.hosting.nodes[&l.host].usage.virtual_disk_gb, 2600);
    assert_eq!(l.sim.hosting.nodes[&l.host].usage.physical_disk_gb, 2.0);
    for vm in [a, b] {
        l.demand(
            vm,
            WorkloadDemand {
                cpu_mhz: 100.0,
                memory_mb: 256.0,
                read_iops: 30000.0,
                write_iops: 30000.0,
                disk_mb_s: 500.0,
                disk_growth_gb_s: 100.0,
                ..Default::default()
            },
        );
    }
    for _ in 0..20 {
        l.tick();
    }
    assert!(l.sim.hosting.nodes[&l.host].usage.physical_disk_gb <= 954.01);
    assert!(
        l.sim
            .hosting
            .vms
            .values()
            .all(|v| v.metrics.write_failed_gb > 0.0
                && v.metrics.guest_disk_full_gb == 0.0
                && v.metrics.quality < 1.0
                && v.metrics.disk_queue_ops > 0.0
                && v.metrics.disk_latency_ms > 1.0)
    );
    let mut thick = l.sim.hosting.products[&l.plan].clone();
    thick.thin = false;
    apply(&mut l.sim, HostingCommand::SaveProduct(thick));
    assert!(
        l.sim
            .purchase_hosting(3, l.plan, CustomerCategory::Website)
            .is_err()
    );
}
#[test]
fn automatic_node_addressing_requires_a_real_gateway_and_resumes_sales_after_reconnection() {
    let mut l = Lab::new();
    l.prepare();
    let mut market = l.sim.hosting.market.clone();
    market.prospects_per_day = 500;
    apply(&mut l.sim, HostingCommand::SetMarket(market));
    let old_link = l.sim.link_for_port(l.data).unwrap().clone();
    let peer = old_link.other(l.data).unwrap();
    l.sim
        .execute(Command::Disconnect { link: old_link.id })
        .unwrap();
    let SimEvent::DeviceAdded(isolated) = l
        .sim
        .execute(Command::BuyDevice {
            kind: DeviceTemplate::Switch,
        })
        .unwrap()[0]
    else {
        panic!()
    };
    power(&mut l.sim, isolated, 6);
    let isolated_port = l.sim.device(isolated).unwrap().ports()[0];
    l.sim
        .execute(Command::Connect {
            a: l.data,
            b: isolated_port,
        })
        .unwrap();
    l.sim
        .execute(Command::ResetPortConfig { port: l.data })
        .unwrap();
    l.tick();
    assert!(l.sim.hosting_provider_site_status().is_ok());
    assert!(l.sim.hosting_node_eligible(l.host));
    let reason = l.sim.hosting_availability(l.plan).unwrap_err();
    assert!(reason.contains("data interface"), "{reason}");
    assert!(!reason.contains("management"), "{reason}");
    assert_eq!(
        l.sim.hosting.market_stats[&l.plan].expected_orders_per_day,
        0.0
    );
    let before = ron::to_string(&l.sim.hosting).unwrap();
    assert_eq!(
        l.sim.purchase_hosting(1, l.plan, CustomerCategory::Website),
        Err(reason)
    );
    assert_eq!(ron::to_string(&l.sim.hosting).unwrap(), before);
    let link = l.sim.link_for_port(l.data).unwrap().id;
    l.sim.execute(Command::Disconnect { link }).unwrap();
    l.sim
        .execute(Command::Connect { a: l.data, b: peer })
        .unwrap();
    l.tick();
    assert!(l.sim.hosting.market_stats[&l.plan].expected_orders_per_day > 0.0);
    l.sim.advance_time(86_400_000);
    assert!(!l.sim.hosting.customers.is_empty());
    assert!(!l.sim.hosting.vms.is_empty());
    assert!(!l.sim.hosting.payments.is_empty());
}

#[test]
fn automatic_node_ips_and_guests_obey_individual_slices_of_a_purchased_24() {
    let mut l = Lab::new();
    let block = l.sim.buy_public_ipv4_subnet(24).unwrap();
    assert_eq!(block.range(), prefix("198.18.0.0/24"));
    let data_switch = l
        .sim
        .port(l.sim.link_for_port(l.data).unwrap().other(l.data).unwrap())
        .unwrap()
        .device;
    let mgmt_switch = l
        .sim
        .port(l.sim.link_for_port(l.mgmt).unwrap().other(l.mgmt).unwrap())
        .unwrap()
        .device;
    let dp = l.sim.device(data_switch).unwrap().ports().to_vec();
    let mp = l.sim.device(mgmt_switch).unwrap().ports().to_vec();
    let router = l
        .sim
        .devices()
        .find(|d| matches!(d.kind, DeviceKind::Router(_)))
        .unwrap()
        .id;
    let router_port = l.sim.device(router).unwrap().ports()[4];
    l.sim
        .execute(Command::ConfigureRouterInterface {
            port: router_port,
            name: "customer-24".into(),
            vlan: None,
            address: Some(ip("198.18.0.1")),
            prefix: 24,
            internet_connected: false,
        })
        .unwrap();
    l.sim
        .execute(Command::BuyCableSupply {
            supply: CableSupply::Rj45Pack20,
        })
        .unwrap();
    l.sim
        .execute(Command::Connect {
            a: router_port,
            b: dp[3],
        })
        .unwrap();
    let mut circuit = l.sim.provider().circuit(l.transit).unwrap().clone();
    circuit.routes.push(UpstreamRoute {
        prefix: block.range(),
        next_hop: ip("192.0.2.2"),
    });
    l.sim
        .execute(Command::Provider(ProviderCommand::SetTransit(circuit)))
        .unwrap();
    let SimEvent::DeviceAdded(second) = l.sim.execute(Command::BuyServerFullPack).unwrap()[0]
    else {
        panic!()
    };
    power(&mut l.sim, second, 6);
    let second_data = l.sim.device(second).unwrap().ports()[0];
    let second_mgmt = *l
        .sim
        .device(second)
        .unwrap()
        .ports()
        .iter()
        .find(|p| l.sim.port(**p).unwrap().name == "mgmt0")
        .unwrap();
    l.sim
        .execute(Command::SetIpv4 {
            port: second_mgmt,
            config: Ipv4InterfaceConfig::new(ip("10.0.0.3"), 24, None, VlanId(1)),
        })
        .unwrap();
    for (a, b) in [(second_data, dp[4]), (second_mgmt, mp[3])] {
        l.sim.execute(Command::Connect { a, b }).unwrap();
    }
    for (node, slice) in [(l.host, "198.18.0.1/29"), (second, "198.18.0.8/29")] {
        apply(
            &mut l.sim,
            HostingCommand::SetNodeIpv4Ranges {
                node,
                ranges: Some([prefix(slice)].into()),
            },
        );
    }
    l.prepare();
    let prepared = l.sim.execute_console(second, "./vm-prepare.sh 10.0.0.10");
    assert!(prepared.success, "{:?}", prepared.lines);
    let mut product = l.sim.hosting.products[&l.plan].clone();
    product.eligible_nodes.insert(second);
    product.ipv4_ranges = [block.range()].into();
    apply(&mut l.sim, HostingCommand::SaveProduct(product));
    let mut addresses = std::collections::BTreeSet::new();
    for (node, data, slice) in [
        (l.host, l.data, "198.18.0.0/29"),
        (second, second_data, "198.18.0.8/29"),
    ] {
        let range = prefix(slice);
        assert_eq!(l.sim.hosting.nodes[&node].ipv4_ranges, Some([range].into()));
        let PortConfig::Server(config) = &l.sim.port(data).unwrap().config else {
            panic!()
        };
        let address = config
            .addresses()
            .find(|a| range.usable(a.address))
            .unwrap();
        assert_eq!(address.prefix, 24);
        assert_eq!(address.gateway, Some(ip("198.18.0.1")));
        assert!(addresses.insert(address.address));
        assert!(
            l.sim.hosting.nodes[&node]
                .auto_addresses
                .contains_key(&address.address)
        );
    }
    let first = l.buy(1);
    let second_service = l.buy(2);
    assert_eq!(l.sim.hosting.vms[&l.vm(first)].host, l.host);
    assert_eq!(l.sim.hosting.vms[&l.vm(second_service)].host, second);
    l.tick();
    for vm in l.sim.hosting.vms.values() {
        let ranges = l.sim.hosting.nodes[&vm.host].ipv4_ranges.as_ref().unwrap();
        assert!(ranges.iter().any(|r| r.usable(vm.ips[0])));
        assert!(addresses.insert(vm.ips[0]));
        assert!(vm.metrics.reachable);
        assert!(vm.demand.cpu_mhz > 0.0);
    }
    let original_ip = l.sim.hosting.vms[&l.vm(first)].ips[0];
    apply(
        &mut l.sim,
        HostingCommand::SetNodeIpv4Ranges {
            node: l.host,
            ranges: Some([prefix("198.18.0.16/29")].into()),
        },
    );
    let third = l.buy(3);
    assert_eq!(l.sim.hosting.vms[&l.vm(third)].host, l.host);
    assert!(prefix("198.18.0.16/29").usable(l.sim.hosting.vms[&l.vm(third)].ips[0]));
    assert_eq!(l.sim.hosting.vms[&l.vm(first)].ips[0], original_ip);
    assert!(l.sim.ping_from_internet(original_ip).reachable);
    let snapshot = ron::to_string(&l.sim).unwrap();
    assert!(
        l.sim
            .configure_hosting(HostingCommand::SetNodeIpv4Ranges {
                node: l.host,
                ranges: Some([prefix("192.0.3.0/29")].into())
            })
            .is_err()
    );
    assert_eq!(ron::to_string(&l.sim).unwrap(), snapshot);
    let mut loaded: NetworkSim = ron::from_str(&snapshot).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(
        ron::to_string(&loaded.hosting).unwrap(),
        ron::to_string(&l.sim.hosting).unwrap()
    );
    assert_eq!(
        loaded
            .purchase_hosting(1, l.plan, CustomerCategory::Website)
            .unwrap(),
        first
    );
    loaded.advance_time(HOSTING_TICK_MS);
    assert_eq!(loaded.hosting.payments.len(), 3);
    assert_eq!(loaded.hosting.vms.len(), 3);
    assert!(loaded.ping_from_internet(original_ip).reachable);
    loaded.terminate_hosting_service(second_service).unwrap();
    apply(
        &mut loaded,
        HostingCommand::SetNodeIpv4Ranges {
            node: second,
            ranges: Some(Default::default()),
        },
    );
    assert!(loaded.hosting.nodes[&second].auto_addresses.is_empty());
    let PortConfig::Server(config) = &loaded.port(second_data).unwrap().config else {
        panic!()
    };
    assert!(config.addresses().next().is_none());
    // Management and the main provider server's public address remain unchanged.
    assert_eq!(
        loaded.interface_ipv4(second_mgmt, VlanId(1)),
        Some(ip("10.0.0.3"))
    );
    assert_eq!(
        loaded.interface_ipv4(l.controller_public, VlanId(1)),
        Some(ip("203.0.113.18"))
    );
}

#[test]
fn node_and_product_ranges_must_intersect_and_failed_orders_leave_no_allocations() {
    let mut l = Lab::new();
    // Preserve a player's broader manual address, while guests use the routed
    // alias automatically configured from the permitted allocation slice.
    l.sim
        .execute(Command::SetIpv4 {
            port: l.data,
            config: Ipv4InterfaceConfig::new(ip("203.0.113.100"), 24, None, VlanId(1)),
        })
        .unwrap();
    l.prepare();
    apply(
        &mut l.sim,
        HostingCommand::SetNodeIpv4Ranges {
            node: l.host,
            ranges: Some([prefix("203.0.113.16/29")].into()),
        },
    );
    let snapshot = ron::to_string(&l.sim).unwrap();
    let reason = l
        .sim
        .purchase_hosting(1, l.plan, CustomerCategory::Website)
        .unwrap_err();
    assert!(reason.contains("Node IPv4 policy"), "{reason}");
    assert_eq!(ron::to_string(&l.sim).unwrap(), snapshot);
    apply(
        &mut l.sim,
        HostingCommand::SetNodeIpv4Ranges {
            node: l.host,
            ranges: Some([prefix("203.0.113.1/29")].into()),
        },
    );
    let service = l.buy(1);
    assert!(prefix("203.0.113.0/29").usable(l.sim.hosting.vms[&l.vm(service)].ips[0]));
    apply(
        &mut l.sim,
        HostingCommand::SetNodeIpv4Ranges {
            node: l.host,
            ranges: Some(Default::default()),
        },
    );
    assert!(l.sim.hosting_availability(l.plan).is_err());
    l.tick();
    assert!(l.sim.hosting.vms[&l.vm(service)].metrics.reachable);
    assert_eq!(
        l.sim.interface_ipv4(l.data, VlanId(1)),
        Some(ip("203.0.113.100"))
    );
}

#[test]
fn every_price_reduction_increases_attractiveness_even_below_half_the_budget() {
    let l = Lab::new();
    let mut plan = l.sim.hosting.products[&l.plan].clone();
    for category in CustomerCategory::ALL {
        let mut previous = f64::INFINITY;
        for cents in [1, 5, 25, 100, 200, 500, 2000, 20000, 100_000_000] {
            plan.monthly_price_cents = cents;
            let score = l.sim.hosting_attractiveness(&plan, category);
            assert!(score.is_finite() && score > 0.0);
            assert!(
                score < previous,
                "{category:?}: price {cents}, score {score} >= {previous}"
            );
            previous = score;
        }
    }
}

#[test]
fn discounted_identical_products_generate_more_expected_orders_and_edits_update_demand() {
    let mut l = Lab::new();
    l.prepare();
    let mut market = l.sim.hosting.market.clone();
    market.prospects_per_day = 200;
    apply(&mut l.sim, HostingCommand::SetMarket(market));
    let mut cheap = l.sim.hosting.products[&l.plan].clone();
    cheap.monthly_price_cents = 100;
    apply(&mut l.sim, HostingCommand::SaveProduct(cheap.clone()));
    let mut expensive = cheap.clone();
    expensive.id = 0;
    expensive.monthly_price_cents = 200;
    apply(&mut l.sim, HostingCommand::SaveProduct(expensive));
    let expensive_id = *l.sim.hosting.products.keys().last().unwrap();
    let stats = &l.sim.hosting.market_stats;
    assert!(stats[&l.plan].attractiveness > stats[&expensive_id].attractiveness);
    assert!(stats[&l.plan].expected_orders_per_day > stats[&expensive_id].expected_orders_per_day);
    let previous_demand = stats[&l.plan].expected_orders_per_day;
    cheap.monthly_price_cents = 50;
    apply(&mut l.sim, HostingCommand::SaveProduct(cheap));
    assert!(l.sim.hosting.market_stats[&l.plan].expected_orders_per_day > previous_demand);
}

#[test]
fn price_sensitivity_changes_discount_value_and_cheap_prices_do_not_hide_bad_quality() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    let mut customer = l.sim.hosting.customers[&l.sim.hosting.services[&service].customer].clone();
    let normal = l.sim.hosting.products[&l.plan].clone();
    let mut cheap = normal.clone();
    cheap.monthly_price_cents = 1;
    customer.price_sensitivity = 0.5;
    let low_sensitivity_gain = l.sim.hosting_attractiveness_for_customer(&cheap, &customer)
        / l.sim
            .hosting_attractiveness_for_customer(&normal, &customer);
    customer.price_sensitivity = 2.0;
    let high_sensitivity_gain = l.sim.hosting_attractiveness_for_customer(&cheap, &customer)
        / l.sim
            .hosting_attractiveness_for_customer(&normal, &customer);
    assert!(high_sensitivity_gain > low_sensitivity_gain);
    let healthy_score = l
        .sim
        .hosting_attractiveness(&normal, CustomerCategory::Website);
    l.sim
        .hosting
        .services
        .get_mut(&service)
        .unwrap()
        .observed_quality = 0.1;
    assert!(
        l.sim
            .hosting_attractiveness(&cheap, CustomerCategory::Website)
            < healthy_score * 0.1
    );
}

#[test]
fn faster_default_clock_advances_workloads_and_paid_renewals_at_every_game_speed() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    let vm = l.vm(service);
    l.sim.hosting.market.time_scale = DEFAULT_HOSTING_TIME_SCALE;
    l.sim
        .hosting
        .services
        .get_mut(&service)
        .unwrap()
        .expires_at_ms = 3 * HOSTING_TICK_MS;
    l.sim.hosting.vms.get_mut(&vm).unwrap().expires_at_ms = 3 * HOSTING_TICK_MS;
    l.demand(
        vm,
        WorkloadDemand {
            cpu_mhz: 500.0,
            memory_mb: 256.0,
            network_mbps: 10.0,
            ..Default::default()
        },
    );
    for speed in SimulationSpeed::ALL {
        let mut sim = l.sim.clone();
        sim.execute(Command::SetSimulationSpeed(speed)).unwrap();
        sim.advance_time(1000);
        assert_eq!(
            sim.hosting.now_ms + sim.hosting.pending_ms,
            510_000 * speed.multiplier()
        );
        let machine = &sim.hosting.vms[&vm];
        assert_eq!(machine.metrics.cpu_used_mhz, 500.0);
        assert!(machine.metrics.network_mbps > 0.0);
        assert!(sim.port_telemetry(l.transit).tx_bytes > 0);
        let renewed = speed.multiplier() >= 2;
        assert_eq!(sim.hosting.services[&service].renewals, u64::from(renewed));
        assert_eq!(sim.hosting.payments.len(), if renewed { 2 } else { 1 });
        if renewed {
            let payment = sim.hosting.payments.last().unwrap();
            assert_eq!(payment.at_ms, 3 * HOSTING_TICK_MS);
            assert!(payment.renewal);
            assert_eq!(machine.expires_at_ms, 3 * HOSTING_TICK_MS + MONTH_MS);
        }
    }
}

#[test]
fn all_game_speed_presets_accelerate_workloads_and_subscriptions_without_changing_results() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    l.sim
        .hosting
        .services
        .get_mut(&service)
        .unwrap()
        .expires_at_ms = 2 * HOSTING_TICK_MS;
    l.sim
        .hosting
        .vms
        .get_mut(&l.vm(service))
        .unwrap()
        .expires_at_ms = 2 * HOSTING_TICK_MS;
    for speed in SimulationSpeed::ALL {
        let mut accelerated = l.sim.clone();
        let mut normal = l.sim.clone();
        accelerated
            .execute(Command::SetSimulationSpeed(speed))
            .unwrap();
        accelerated.advance_time(HOSTING_TICK_MS);
        normal.advance_time(HOSTING_TICK_MS * speed.multiplier());
        assert_eq!(
            accelerated.simulation_time_ms(),
            normal.simulation_time_ms()
        );
        assert_eq!(
            ron::to_string(&accelerated.hosting).unwrap(),
            ron::to_string(&normal.hosting).unwrap()
        );
        if speed.multiplier() >= 2 {
            assert_eq!(accelerated.hosting.services[&service].renewals, 1);
            assert_eq!(accelerated.hosting.payments.len(), 2);
        }
    }
}

#[test]
fn attractiveness_compares_category_price_resources_quality_and_expected_demand() {
    let mut l = Lab::new();
    l.prepare();
    l.sim.hosting.market.prospects_per_day = 200;
    let mut a = l.sim.hosting.products[&l.plan].clone();
    a.monthly_price_cents = 400;
    apply(&mut l.sim, HostingCommand::SaveProduct(a.clone()));
    let mut b = a.clone();
    b.id = 0;
    b.vcpu = 4;
    b.ram_mb = 8192;
    b.disk_gb = 100;
    b.monthly_price_cents = 500;
    apply(&mut l.sim, HostingCommand::SaveProduct(b));
    let id = *l.sim.hosting.products.keys().last().unwrap();
    let b = &l.sim.hosting.products[&id];
    assert!(
        l.sim.hosting_attractiveness(b, CustomerCategory::Developer)
            > l.sim
                .hosting_attractiveness(&a, CustomerCategory::Developer)
                * 1.3
    );
    let small_gain = l.sim.hosting_attractiveness(b, CustomerCategory::Website)
        / l.sim.hosting_attractiveness(&a, CustomerCategory::Website);
    let dev_gain = l.sim.hosting_attractiveness(b, CustomerCategory::Developer)
        / l.sim
            .hosting_attractiveness(&a, CustomerCategory::Developer);
    assert!(dev_gain > small_gain);
    let mut poor = a.clone();
    poor.monthly_price_cents = 20000;
    assert!(
        l.sim
            .hosting_attractiveness(&poor, CustomerCategory::Developer)
            < l.sim
                .hosting_attractiveness(&a, CustomerCategory::Developer)
                * 0.5
    );
    assert!(
        l.sim.hosting.market_stats[&id].expected_orders_per_day
            > l.sim.hosting.market_stats[&a.id].expected_orders_per_day
    );
}
#[test]
fn automatic_market_generates_category_workloads_progressively_and_deterministically() {
    let mut l = Lab::new();
    l.prepare();
    l.sim.hosting.market.prospects_per_day = 500;
    l.policy(|p| p.ram_overcommit_percent = 400);
    assert!(l.sim.hosting.vms.is_empty());
    let mut twin = l.sim.clone();
    l.sim.advance_time(12 * 60 * 60 * 1000);
    twin.advance_time(12 * 60 * 60 * 1000);
    assert!(!l.sim.hosting.vms.is_empty());
    assert!(
        l.sim
            .hosting
            .vms
            .values()
            .any(|v| v.demand.cpu_mhz > 0.0 && v.demand.network_mbps > 0.0)
    );
    assert_eq!(
        ron::to_string(&l.sim.hosting).unwrap(),
        ron::to_string(&twin.hosting).unwrap()
    );
    let customer = l.sim.hosting.customers.values().next().unwrap().clone();
    let vm = l.sim.hosting.vms.values().next().unwrap();
    let mut website = customer.clone();
    website.category = CustomerCategory::Website;
    website.requirements = customer_requirements(website.category);
    let mut other = customer.clone();
    other.category = CustomerCategory::GameServer;
    other.requirements = customer_requirements(other.category);
    assert!(
        customer_workload(&other, vm, 15 * 3600000).cpu_mhz
            > customer_workload(&website, vm, 15 * 3600000).cpu_mhz
    );
}
#[test]
fn subscriptions_choose_configured_periods_renew_on_time_and_cancel_for_quality() {
    let mut l = Lab::new();
    l.prepare();
    l.sim.hosting.market.periods = vec![SubscriptionOption {
        months: 3,
        weight: 1,
    }];
    let s = l.buy(1);
    let vm = l.vm(s);
    assert_eq!(l.sim.hosting.payments[0].months, 3);
    assert_eq!(l.sim.hosting.services[&s].expires_at_ms, 3 * MONTH_MS);
    let expiry = l.sim.hosting.services[&s].expires_at_ms;
    l.sim.hosting.now_ms = expiry - HOSTING_TICK_MS;
    l.demand(
        vm,
        WorkloadDemand {
            cpu_mhz: 100.0,
            memory_mb: 256.0,
            ..Default::default()
        },
    );
    l.tick();
    assert_eq!(l.sim.hosting.services[&s].renewals, 1);
    assert!(l.sim.hosting.payments.last().unwrap().renewal);
    assert_eq!(l.sim.hosting.payments.last().unwrap().at_ms, expiry);
    apply(
        &mut l.sim,
        HostingCommand::SetVmPower {
            vm,
            power: VmPower::Stopped,
        },
    );
    let expiry = l.sim.hosting.services[&s].expires_at_ms;
    l.sim.hosting.now_ms = expiry - HOSTING_TICK_MS;
    let service = l.sim.hosting.services.get_mut(&s).unwrap();
    service.satisfaction = 0.0;
    service.availability = 0.0;
    l.tick();
    assert!(l.sim.hosting.services[&s].terminated_at_ms.is_some());
    assert!(!l.sim.hosting.vms.contains_key(&vm));
}
#[test]
fn save_load_preserves_hosting_and_never_recreates_vms_or_payments() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    l.tick();
    let saved = ron::to_string(&l.sim).unwrap();
    let mut loaded: NetworkSim = ron::from_str(&saved).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(
        ron::to_string(&loaded.hosting).unwrap(),
        ron::to_string(&l.sim.hosting).unwrap()
    );
    assert!(loaded.hosting_node_eligible(l.host));
    assert_eq!(
        loaded
            .purchase_hosting(1, l.plan, CustomerCategory::Website)
            .unwrap(),
        service
    );
    assert_eq!(loaded.hosting.vms.len(), 1);
    assert_eq!(loaded.hosting.payments.len(), 1);
    let vm = &loaded.hosting.vms[&l.vm(service)];
    assert!(loaded.ping(vm.interfaces[0], ip("198.51.100.1")).reachable);
}
#[test]
fn product_edit_delete_and_unpublish_preserve_purchased_specs() {
    let mut l = Lab::new();
    l.prepare();
    let s = l.buy(1);
    let vm = l.vm(s);
    let mut p = l.sim.hosting.products[&l.plan].clone();
    p.vcpu = 1;
    p.monthly_price_cents = 9999;
    apply(&mut l.sim, HostingCommand::SaveProduct(p));
    apply(
        &mut l.sim,
        HostingCommand::Publish {
            product: l.plan,
            published: false,
        },
    );
    assert!(l.sim.hosting_availability(l.plan).is_err());
    apply(&mut l.sim, HostingCommand::DeleteProduct(l.plan));
    assert_eq!(l.sim.hosting.vms[&vm].plan.vcpu, 2);
    assert_eq!(l.sim.hosting.services[&s].monthly_price_cents, 500);
    l.tick();
    assert!(l.sim.hosting.vms[&vm].metrics.reachable);
}
#[test]
fn insufficient_resources_and_invalid_publication_do_not_leak_state() {
    let mut l = Lab::new();
    l.prepare();
    let mut p = l.sim.hosting.products[&l.plan].clone();
    p.ram_mb = 1000000;
    apply(&mut l.sim, HostingCommand::SaveProduct(p));
    let before = ron::to_string(&l.sim).unwrap();
    assert!(
        l.sim
            .purchase_hosting(1, l.plan, CustomerCategory::Website)
            .is_err()
    );
    assert_eq!(ron::to_string(&l.sim).unwrap(), before);
    let invalid = HostingProduct {
        published: true,
        ..Default::default()
    };
    assert!(
        l.sim
            .configure_hosting(HostingCommand::SaveProduct(invalid))
            .is_err()
    );
}

#[test]
fn products_share_a_pool_but_allocate_only_their_selected_addresses() {
    let mut l = Lab::new();
    l.prepare();
    let mut p = l.sim.hosting.products[&l.plan].clone();
    p.id = 0;
    p.ipv4_ranges = [prefix("203.0.113.8/29")].into();
    p.ipv4_count = 2;
    apply(&mut l.sim, HostingCommand::SaveProduct(p));
    let product = *l.sim.hosting.products.keys().last().unwrap();
    let s = l
        .sim
        .purchase_hosting(1, product, CustomerCategory::Website)
        .unwrap();
    let vm = &l.sim.hosting.vms[&l.vm(s)];
    assert_eq!(vm.ips.len(), 2);
    assert!(vm.ips.iter().all(|a| prefix("203.0.113.8/29").usable(*a)));
    let ips = vm.ips.clone();
    let shared = l.buy(2);
    let vm = &l.sim.hosting.vms[&l.vm(shared)];
    assert!(!ips.contains(&vm.ips[0]));
    assert_eq!(
        l.sim
            .hosting_range_usage(prefix("203.0.113.8/29"))
            .available,
        3
    );
}
#[test]
fn cpu_shares_limits_and_burst_credit_change_actual_scheduling() {
    let mut l = Lab::new();
    l.prepare();
    let mut p = l.sim.hosting.products[&l.plan].clone();
    p.cpu_limit_percent = 25;
    p.cpu_burst_percent = 100;
    p.burst_seconds = 300;
    apply(&mut l.sim, HostingCommand::SaveProduct(p));
    let s = l.buy(1);
    let vm = l.vm(s);
    l.demand(
        vm,
        WorkloadDemand {
            cpu_mhz: 6800.0,
            memory_mb: 256.0,
            ..Default::default()
        },
    );
    l.tick();
    assert_eq!(l.sim.hosting.vms[&vm].metrics.cpu_used_mhz, 6800.0);
    l.tick();
    assert_eq!(l.sim.hosting.vms[&vm].metrics.cpu_used_mhz, 1700.0);
    assert!(l.sim.hosting.vms[&vm].metrics.cpu_throttled > 0.7);
    l.sim.terminate_hosting_service(s).unwrap();
    let mut p = l.sim.hosting.products[&l.plan].clone();
    p.vcpu = 4;
    p.cpu_limit_percent = 100;
    p.cpu_burst_percent = 100;
    p.cpu_weight = 100;
    apply(&mut l.sim, HostingCommand::SaveProduct(p.clone()));
    let a = l.buy(2);
    let a = l.vm(a);
    p.cpu_weight = 200;
    apply(&mut l.sim, HostingCommand::SaveProduct(p));
    let b = l.buy(3);
    let b = l.vm(b);
    for id in [a, b] {
        l.demand(
            id,
            WorkloadDemand {
                cpu_mhz: 13600.0,
                memory_mb: 256.0,
                ..Default::default()
            },
        );
    }
    l.tick();
    assert!(
        (l.sim.hosting.vms[&b].metrics.cpu_used_mhz / l.sim.hosting.vms[&a].metrics.cpu_used_mhz
            - 2.0)
            .abs()
            < 0.01
    );
}
#[test]
fn disk_contention_and_thick_reservations_use_installed_drive_ratings() {
    let mut l = Lab::new();
    l.prepare();
    let mut p = l.sim.hosting.products[&l.plan].clone();
    p.disk_iops = 100000;
    p.disk_mb_s = 1000;
    p.thin = false;
    apply(&mut l.sim, HostingCommand::SaveProduct(p));
    let a = l.buy(1);
    let a = l.vm(a);
    let b = l.buy(2);
    let b = l.vm(b);
    assert_eq!(l.sim.hosting.nodes[&l.host].usage.physical_disk_gb, 60.0);
    for id in [a, b] {
        l.demand(
            id,
            WorkloadDemand {
                cpu_mhz: 100.0,
                memory_mb: 256.0,
                read_iops: 30000.0,
                write_iops: 30000.0,
                disk_mb_s: 50.0,
                ..Default::default()
            },
        );
    }
    l.tick();
    assert!(
        l.sim
            .hosting
            .vms
            .values()
            .all(|v| v.metrics.disk_iops <= 15000.01
                && v.metrics.disk_iops > 14999.0
                && v.metrics.disk_queue_ops > 0.0)
    );
    assert!(
        l.sim.hosting.nodes[&l.host].usage.disk_iops
            <= l.sim.hosting_capacity(l.host, HostingStorage::Ssd).iops
    );
    apply(
        &mut l.sim,
        HostingCommand::SetVmPower {
            vm: a,
            power: VmPower::Stopped,
        },
    );
    l.tick();
    assert_eq!(l.sim.hosting.vms[&a].metrics.disk_iops, 0.0);
    assert_eq!(l.sim.hosting.vms[&b].metrics.disk_iops, 30000.0);
}
#[test]
fn stopped_rebooted_and_exclusive_servers_obey_their_lifecycle() {
    let mut l = Lab::new();
    l.prepare();
    let mut p = l.sim.hosting.products[&l.plan].clone();
    p.category = HostingCategory::DedicatedServer;
    p.cpu_policy = CpuPolicy::Dedicated;
    apply(&mut l.sim, HostingCommand::SaveProduct(p));
    let s = l.buy(1);
    let vm = l.vm(s);
    assert!(
        l.sim
            .purchase_hosting(2, l.plan, CustomerCategory::Website)
            .is_err()
    );
    apply(
        &mut l.sim,
        HostingCommand::SetVmPower {
            vm,
            power: VmPower::Stopped,
        },
    );
    l.tick();
    assert_eq!(l.sim.hosting.vms[&vm].demand.cpu_mhz, 0.0);
    assert_eq!(l.sim.hosting.vms[&vm].metrics.network_mbps, 0.0);
    apply(&mut l.sim, HostingCommand::RebootVm(vm));
    assert_eq!(l.sim.hosting.vms[&vm].power, VmPower::Rebooting);
    l.tick();
    assert_eq!(l.sim.hosting.vms[&vm].power, VmPower::Running);
    l.sim.terminate_hosting_service(s).unwrap();
    assert!(l.sim.hosting_availability(l.plan).is_ok());
}
#[test]
fn capacity_diagnostics_identify_each_resource_and_failed_orders_are_atomic() {
    let mut l = Lab::new();
    l.prepare();
    l.buy(1);
    let cap = l.sim.hosting_capacity(l.host, HostingStorage::Ssd);
    let policy = &l.sim.hosting.nodes[&l.host].policy;
    let cpu = cap.cpu_mhz - policy.reserved_cpu_mhz as f64;
    let disk = cap.disk_gb - policy.reserved_disk_gb as f64 - policy.swap_mb as f64 / 1024.0;
    for (gate, resource, unit, context) in [
        (
            0,
            "Dedicated CPU",
            "MHz",
            "dedicated CPU cannot be overcommitted",
        ),
        (1, "Shared CPU", "MHz", "CPU overcommit"),
        (2, "RAM", "MiB", "RAM overcommit"),
        (3, "Physical storage", "GB", "thick provisioning"),
        (4, "Virtual disk capacity", "GB", "storage overcommit"),
        (5, "Disk IOPS", "IOPS", "storage performance overcommit"),
        (
            6,
            "Disk throughput",
            "MB/s",
            "storage performance overcommit",
        ),
        (7, "Network bandwidth", "Mbps", "network overcommit"),
    ] {
        let mut sim = l.sim.clone();
        let mut product = sim.hosting.products[&l.plan].clone();
        match gate {
            0 => {
                product.category = HostingCategory::DedicatedCpu;
                product.cpu_policy = CpuPolicy::Dedicated;
                product.vcpu = (cpu / VCPU_MHZ) as u32 + 1;
            }
            1 => {
                product.vcpu =
                    (cpu * f64::from(policy.cpu_overcommit_percent) / 100.0 / VCPU_MHZ) as u32 + 1
            }
            2 => {
                product.ram_mb = ((cap.ram_mb - policy.reserved_ram_mb as f64)
                    * f64::from(policy.ram_overcommit_percent)
                    / 100.0) as u64
                    + 1
            }
            3 => {
                product.thin = false;
                product.disk_gb = disk as u64 + 1;
            }
            4 => {
                product.disk_gb =
                    (disk * f64::from(policy.storage_overcommit_percent) / 100.0) as u64 + 1
            }
            5 => {
                product.disk_iops =
                    (cap.iops * f64::from(policy.io_overcommit_percent) / 100.0) as u32 + 1
            }
            6 => {
                product.disk_mb_s =
                    (cap.disk_mb_s * f64::from(policy.io_overcommit_percent) / 100.0) as u32 + 1
            }
            7 => {
                product.port_mbps =
                    (cap.network_mbps * f64::from(policy.network_overcommit_percent) / 100.0) as u32
                        + 1
            }
            _ => unreachable!(),
        }
        apply(&mut sim, HostingCommand::SaveProduct(product));
        let before = ron::to_string(&sim).unwrap();
        let reason = sim.hosting_availability(l.plan).unwrap_err();
        let line = reason
            .lines()
            .find(|line| line.starts_with(&format!("{resource}:")))
            .unwrap_or_else(|| panic!("Missing {resource}: {reason}"));
        for detail in [
            "requested",
            "available",
            "committed",
            "after order",
            "limit",
            "Shortfall",
            unit,
            context,
        ] {
            assert!(line.contains(detail), "Missing {detail}: {line}");
        }
        assert!(reason.contains(&format!(
            "{} (node #{})",
            sim.device(l.host).unwrap().name,
            l.host.0
        )));
        assert_eq!(
            sim.purchase_hosting(2, l.plan, CustomerCategory::Website)
                .unwrap_err(),
            reason
        );
        assert_eq!(
            ron::to_string(&sim).unwrap(),
            before,
            "Failed {resource} admission mutated simulation"
        );
    }
}

#[test]
fn ram_diagnostics_account_for_bookings_reservations_and_policy_changes() {
    let mut l = Lab::new();
    l.prepare();
    l.buy(1);
    let cap = l.sim.hosting_capacity(l.host, HostingStorage::Ssd);
    l.policy(|p| {
        p.reserved_ram_mb = cap.ram_mb as u64 - 3584;
        p.ram_overcommit_percent = 100;
    });
    let reason = l.sim.hosting_availability(l.plan).unwrap_err();
    assert!(reason.contains("RAM: requested 2048.00 MiB, available 1536.00 MiB (committed 2048.00, after order 4096.00, limit 3584.00 MiB). Shortfall 512.00 MiB."), "{reason}");
    assert!(
        reason.contains(&format!(
            "Physical RAM {:.2} MiB, hypervisor reservation {} MiB; RAM overcommit 100%",
            cap.ram_mb,
            cap.ram_mb as u64 - 3584
        )),
        "{reason}"
    );
    l.policy(|p| p.ram_overcommit_percent = 200);
    assert!(l.sim.hosting_availability(l.plan).is_ok());
    // An order using exactly the remaining capacity must still be accepted.
    l.policy(|p| {
        p.ram_overcommit_percent = 100;
        p.reserved_ram_mb = cap.ram_mb as u64 - 4096;
    });
    l.buy(2);
    let reason = l.sim.hosting_availability(l.plan).unwrap_err();
    assert!(reason.contains("RAM: requested 2048.00 MiB, available 0.00 MiB (committed 4096.00, after order 6144.00, limit 4096.00 MiB). Shortfall 2048.00 MiB."), "{reason}");
}

#[test]
fn capacity_diagnostics_report_all_failed_resources_on_all_candidate_nodes() {
    let mut l = Lab::new();
    l.prepare();
    let SimEvent::DeviceAdded(second) = l.sim.execute(Command::BuyServerFullPack).unwrap()[0]
    else {
        panic!()
    };
    power(&mut l.sim, second, 6);
    let mgmt = *l
        .sim
        .device(second)
        .unwrap()
        .ports()
        .iter()
        .find(|id| l.sim.port(**id).unwrap().name == "mgmt0")
        .unwrap();
    let peer = l.sim.link_for_port(l.mgmt).unwrap().other(l.mgmt).unwrap();
    let switch = l.sim.port(peer).unwrap().device;
    let switch_port = l.sim.device(switch).unwrap().ports()[3];
    l.sim
        .execute(Command::SetIpv4 {
            port: mgmt,
            config: Ipv4InterfaceConfig::new(ip("10.0.0.3"), 24, None, VlanId(1)),
        })
        .unwrap();
    l.sim
        .execute(Command::Connect {
            a: mgmt,
            b: switch_port,
        })
        .unwrap();
    let output = l.sim.execute_console(second, "./vm-prepare.sh 10.0.0.10");
    assert!(output.success, "{:?}", output.lines);
    let mut product = l.sim.hosting.products[&l.plan].clone();
    product.eligible_nodes.insert(second);
    apply(&mut l.sim, HostingCommand::SaveProduct(product));
    for node in [l.host, second] {
        let cap = l.sim.hosting_capacity(node, HostingStorage::Ssd);
        let mut policy = l.sim.hosting.nodes[&node].policy.clone();
        policy.reserved_cpu_mhz = cap.cpu_mhz as u64 + 1;
        policy.reserved_ram_mb = cap.ram_mb as u64 + 1;
        apply(&mut l.sim, HostingCommand::SetNodePolicy { node, policy });
    }
    let reason = l.sim.hosting_availability(l.plan).unwrap_err();
    let sections: Vec<_> = reason.split("\n\n").collect();
    assert_eq!(sections.len(), 2, "{reason}");
    for (section, node) in sections.iter().zip([l.host, second]) {
        assert!(
            section.contains(&format!(
                "{} (node #{})",
                l.sim.device(node).unwrap().name,
                node.0
            )),
            "{section}"
        );
        assert!(
            section.contains("Shared CPU: requested 6800.00 MHz, available 0.00 MHz"),
            "{section}"
        );
        assert!(
            section.contains("RAM: requested 2048.00 MiB, available 0.00 MiB"),
            "{section}"
        );
    }
}

#[test]
fn network_and_memory_policies_limit_future_sales_without_harming_existing_vms() {
    let mut l = Lab::new();
    l.prepare();
    let s = l.buy(1);
    let vm = l.vm(s);
    l.policy(|p| {
        p.reserved_ram_mb = 15000;
        p.network_overcommit_percent = 100;
    });
    assert!(l.sim.hosting_availability(l.plan).is_err());
    assert!(l.sim.hosting.vms.contains_key(&vm));
    l.policy(|p| p.reserved_ram_mb = 512);
    let mut p = l.sim.hosting.products[&l.plan].clone();
    p.port_mbps = 1000;
    apply(&mut l.sim, HostingCommand::SaveProduct(p));
    assert!(l.sim.hosting_availability(l.plan).is_err());
}
#[test]
fn sustained_provider_network_outage_causes_cancellation_from_observed_quality() {
    let mut l = Lab::new();
    l.prepare();
    let s = l.buy(1);
    let link = l.sim.link_for_port(l.data).unwrap().id;
    l.sim.execute(Command::Disconnect { link }).unwrap();
    l.sim.advance_time(MONTH_MS);
    assert!(l.sim.hosting.services[&s].terminated_at_ms.is_some());
    assert_eq!(l.sim.hosting.payments.len(), 1);
    assert!(l.sim.hosting.services[&s].observed_quality < 0.01);
    assert!(
        l.sim
            .hosting
            .customers
            .values()
            .next()
            .unwrap()
            .churn_probability
            > 0.99
    );
}

#[test]
fn a_customer_stopped_guest_renews_normally_when_provider_infrastructure_works() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    let vm = l.vm(service);
    let expiry = l.sim.hosting.services[&service].expires_at_ms;
    l.sim.hosting.now_ms = expiry - HOSTING_TICK_MS;
    apply(
        &mut l.sim,
        HostingCommand::SetVmPower {
            vm,
            power: VmPower::Stopped,
        },
    );
    l.sim
        .hosting
        .vms
        .get_mut(&vm)
        .unwrap()
        .customer_restart_at_ms = expiry + HOSTING_TICK_MS;
    l.tick();
    assert_eq!(l.sim.hosting.vms[&vm].power, VmPower::Stopped);
    assert_eq!(l.sim.hosting.vms[&vm].metrics.provider_fault, None);
    assert_eq!(l.sim.hosting.services[&service].availability, 1.0);
    assert_eq!(l.sim.hosting.services[&service].satisfaction, 1.0);
    assert_eq!(l.sim.hosting.services[&service].renewals, 1);
    assert_eq!(l.sim.hosting.payments.len(), 2);
    assert_eq!(l.sim.hosting.payments[1].at_ms, expiry);
}

#[test]
fn customer_stops_and_reboots_do_not_penalize_provider_or_require_player_recovery() {
    for state in [VmPower::Stopped, VmPower::Oom, VmPower::Rebooting] {
        let mut l = Lab::new();
        l.prepare();
        let s = l.buy(1);
        let vm = l.vm(s);
        l.demand(
            vm,
            WorkloadDemand {
                cpu_mhz: 100.0,
                memory_mb: 256.0,
                ..Default::default()
            },
        );
        let now = l.sim.hosting.now_ms;
        let machine = l.sim.hosting.vms.get_mut(&vm).unwrap();
        machine.power = state;
        machine.customer_restart_at_ms = now + 3 * HOSTING_TICK_MS;
        machine.reboot_until_ms = now + 3 * HOSTING_TICK_MS;
        l.sim.topology_revision += 1;
        for _ in 0..2 {
            l.tick();
            let v = &l.sim.hosting.vms[&vm];
            assert_eq!(v.power, state);
            assert!(!v.metrics.reachable); // The guest NIC really is down.
            assert_eq!(v.metrics.provider_fault, None);
            assert_eq!(v.metrics.quality, 1.0);
            assert_eq!(v.metrics.cpu_used_mhz, 0.0);
            assert_eq!(v.metrics.resident_mb, 0.0);
            assert_eq!(v.metrics.traffic_bytes, 0);
            assert_eq!(l.sim.hosting.services[&s].availability, 1.0);
            assert_eq!(l.sim.hosting.reputation, 1.0);
        }
        l.tick();
        assert_eq!(l.sim.hosting.vms[&vm].power, VmPower::Running);
        l.tick();
        assert!(l.sim.hosting.vms[&vm].metrics.reachable);
        assert_eq!(l.sim.hosting.vms[&vm].metrics.cpu_used_mhz, 100.0);
        assert_eq!(l.sim.hosting.vms[&vm].metrics.quality, 1.0);
        assert_eq!(l.sim.hosting.payments.len(), 1);
    }
}

#[test]
fn stopped_guests_identify_actual_infrastructure_failures_and_retry_customer_start() {
    for fault in [
        HostingProviderFault::HostUnavailable,
        HostingProviderFault::ManagementUnavailable,
        HostingProviderFault::PublicNetworkUnavailable,
    ] {
        let mut l = Lab::new();
        l.prepare();
        let s = l.buy(1);
        let vm = l.vm(s);
        apply(
            &mut l.sim,
            HostingCommand::SetVmPower {
                vm,
                power: VmPower::Stopped,
            },
        );
        let disconnected = match fault {
            HostingProviderFault::HostUnavailable => {
                l.sim
                    .execute(Command::SetPower {
                        device: l.host,
                        powered: false,
                    })
                    .unwrap();
                None
            }
            _ => {
                let port = if fault == HostingProviderFault::ManagementUnavailable {
                    l.mgmt
                } else {
                    l.data
                };
                let link = l.sim.link_for_port(port).unwrap().clone();
                l.sim
                    .execute(Command::Disconnect { link: link.id })
                    .unwrap();
                Some(link)
            }
        };
        l.tick();
        let machine = &l.sim.hosting.vms[&vm];
        assert_eq!(machine.metrics.provider_fault, Some(fault));
        assert_eq!(machine.metrics.quality, 0.0);
        assert!(l.sim.hosting.services[&s].availability < 1.0);
        if fault != HostingProviderFault::PublicNetworkUnavailable {
            assert_eq!(machine.power, VmPower::Stopped);
        }
        if let Some(link) = disconnected {
            l.sim
                .execute(Command::Connect {
                    a: link.a,
                    b: link.b,
                })
                .unwrap();
        } else {
            l.sim
                .execute(Command::SetPower {
                    device: l.host,
                    powered: true,
                })
                .unwrap();
        }
        l.tick();
        l.tick();
        assert_eq!(l.sim.hosting.vms[&vm].power, VmPower::Running);
        assert!(l.sim.hosting.vms[&vm].metrics.reachable);
        assert_eq!(l.sim.hosting.vms[&vm].metrics.provider_fault, None);
        assert_eq!(l.sim.hosting.payments.len(), 1);
    }
}

#[test]
fn node_ip_diagnostics_list_only_its_actual_guest_interfaces_and_release_deleted_guests() {
    let mut l = Lab::new();
    l.prepare();
    let mut product = l.sim.hosting.products[&l.plan].clone();
    product.ipv4_count = 2;
    apply(&mut l.sim, HostingCommand::SaveProduct(product));
    let service = l.buy(1);
    let vm = l.vm(service);
    let machine = l.sim.hosting.vms[&vm].clone();
    let iface = l.sim.port(machine.interfaces[0]).unwrap().name.clone();
    for command in [
        "ip a",
        "ip -br a",
        "ip -o -4 addr",
        &format!("ip addr show dev {iface}"),
    ] {
        let output = l.sim.execute_console(l.host, command);
        assert!(output.success, "{:?}", output.lines);
        let text = output.lines.join("\n");
        assert!(text.contains(&iface));
        for address in &machine.ips {
            assert!(text.contains(&format!("{address}/24")), "{text}");
        }
    }
    let output = l.sim.execute_console(l.controller, "ip a");
    assert!(output.success);
    assert!(!output.lines.join("\n").contains(&iface));
    let before = ron::to_string(&l.sim.hosting).unwrap();
    let edit = l
        .sim
        .execute_console(l.host, &format!("ip addr flush dev {iface}"));
    assert!(!edit.success);
    assert_eq!(ron::to_string(&l.sim.hosting).unwrap(), before);
    apply(
        &mut l.sim,
        HostingCommand::SetVmPower {
            vm,
            power: VmPower::Stopped,
        },
    );
    let output = l
        .sim
        .execute_console(l.host, &format!("ip a show dev {iface}"));
    assert!(output.lines.join("\n").contains("state DOWN"));
    assert!(
        output
            .lines
            .join("\n")
            .contains(&machine.ips[0].to_string())
    );
    l.sim.terminate_hosting_service(service).unwrap();
    let output = l.sim.execute_console(l.host, "ip a");
    assert!(!output.lines.join("\n").contains(&iface));
    assert!(
        !output
            .lines
            .join("\n")
            .contains(&machine.ips[0].to_string())
    );
}

#[test]
fn ballooning_only_reclaims_cache_and_keeps_working_memory_resident() {
    let mut l = Lab::new();
    l.prepare();
    l.policy(|p| {
        p.reserved_ram_mb = 13384;
        p.ram_overcommit_percent = 200;
        p.swap_mb = 0;
    });
    let a = l.buy(1);
    let a = l.vm(a);
    let b = l.buy(2);
    let b = l.vm(b);
    for vm in [a, b] {
        l.demand(
            vm,
            WorkloadDemand {
                cpu_mhz: 100.0,
                memory_mb: 1500.0,
                cache_mb: 2000.0,
                ..Default::default()
            },
        );
    }
    l.tick();
    for vm in [a, b] {
        assert_eq!(l.sim.hosting.vms[&vm].metrics.resident_mb, 1500.0);
        assert_eq!(l.sim.hosting.vms[&vm].metrics.ballooned_mb, 548.0);
        assert_eq!(l.sim.hosting.vms[&vm].metrics.swapped_mb, 0.0);
    }
}
#[test]
fn traffic_allowance_is_enforced_without_overrunning_a_tick() {
    let mut l = Lab::new();
    l.prepare();
    let mut p = l.sim.hosting.products[&l.plan].clone();
    p.traffic_gb = Some(1);
    apply(&mut l.sim, HostingCommand::SaveProduct(p));
    let s = l.buy(1);
    let vm = l.vm(s);
    l.demand(
        vm,
        WorkloadDemand {
            cpu_mhz: 100.0,
            memory_mb: 256.0,
            network_mbps: 100.0,
            ..Default::default()
        },
    );
    l.tick();
    assert!(l.sim.hosting.vms[&vm].metrics.traffic_bytes <= 1_000_000_000);
    assert!(l.sim.hosting.vms[&vm].metrics.traffic_bytes >= 999_999_999);
    assert_eq!(l.sim.hosting.vms[&vm].metrics.quality, 1.0);
    l.tick();
    assert_eq!(l.sim.hosting.vms[&vm].metrics.quality, 1.0);
    assert!(l.sim.hosting.vms[&vm].metrics.network_mbps < 0.001);
    let mut p = l.sim.hosting.products[&l.plan].clone();
    p.traffic_gb = None;
    p.port_mbps = 1000;
    apply(&mut l.sim, HostingCommand::SaveProduct(p));
    let neighbor = l.buy(2);
    let neighbor = l.vm(neighbor);
    l.demand(
        neighbor,
        WorkloadDemand {
            cpu_mhz: 100.0,
            memory_mb: 256.0,
            network_mbps: 1000.0,
            ..Default::default()
        },
    );
    l.tick();
    assert_eq!(l.sim.hosting.vms[&vm].metrics.network_mbps, 0.0);
    assert_eq!(l.sim.hosting.vms[&vm].metrics.packet_loss, 0.0);
    assert_eq!(l.sim.hosting.vms[&vm].metrics.quality, 1.0);
    assert!(l.sim.hosting.vms[&neighbor].metrics.packet_loss > 0.5);
    assert!(l.sim.hosting.vms[&neighbor].metrics.quality < 1.0);
}

#[test]
fn provider_site_connects_by_public_ip_and_setup_is_local_to_the_console() {
    let mut l = Lab::new();
    assert_eq!(l.sim.hosting.provider_site_ipv4, Some(ip("203.0.113.18")));
    assert_eq!(l.sim.hosting_provider_server(), Some(l.controller));
    assert!(
        l.sim
            .execute_console(l.controller, "systemctl is-active hosting-provider")
            .success
    );
    assert!(
        l.sim
            .execute_console(l.controller, "cat /etc/hosting-provider.conf")
            .lines
            .iter()
            .any(|line| line.contains("listen=0.0.0.0"))
    );
    for invalid in [
        ip("198.51.100.77"),
        ip("10.0.0.10"),
        ip("203.0.113.16"),
        ip("203.0.113.23"),
    ] {
        let before = ron::to_string(&l.sim).unwrap();
        assert!(
            l.sim
                .configure_hosting(HostingCommand::SetProviderSiteAddress(Some(invalid)))
                .is_err()
        );
        assert_eq!(ron::to_string(&l.sim).unwrap(), before);
    }
    // A public address connects only to its actual machine; the UI never installs the app.
    apply(
        &mut l.sim,
        HostingCommand::SetProviderSiteAddress(Some(ip("203.0.113.2"))),
    );
    assert_eq!(l.sim.hosting_provider_server(), None);
    assert!(l.sim.hosting_provider_site_status().is_err());
    assert!(!l.sim.hosting_provider_installed(l.host));
    assert!(l.sim.hosting_provider_installed(l.controller));
    apply(&mut l.sim, HostingCommand::SetProviderSiteAddress(None));
    assert!(l.sim.hosting_provider_site_status().is_err());
    assert!(
        l.sim
            .execute_console(l.controller, "systemctl is-active hosting-provider")
            .success
    );
    // Internal registration can precede connecting the storefront address.
    l.prepare();
    assert!(!l.sim.hosting_node_eligible(l.host));
    apply(
        &mut l.sim,
        HostingCommand::SetProviderSiteAddress(Some(ip("203.0.113.18"))),
    );
    assert!(l.sim.hosting_node_eligible(l.host));
    assert!(
        !l.sim
            .execute_console(l.controller, "./vm-prepare.sh 10.0.0.10")
            .success
    );
    let unit = l.sim.server_os(l.controller).unwrap().services["hosting-provider"].clone();
    assert!(
        l.sim
            .execute_console(l.controller, "./setup-main-hoster-server.sh")
            .success
    );
    assert_eq!(
        l.sim.server_os(l.controller).unwrap().services["hosting-provider"].log,
        unit.log
    );
}

#[test]
fn node_registration_requires_the_exact_controller_and_script_argument() {
    let mut l = Lab::new();
    assert!(l.sim.ping(l.mgmt, ip("10.0.0.1")).reachable);
    for command in [
        "./vm-prepare.sh",
        "./vm-prepare.sh invalid",
        "./vm-prepare.sh 10.0.0.1",
        "./vm-prepare.sh 10.0.0.99",
    ] {
        assert!(!l.sim.execute_console(l.host, command).success);
        assert!(
            l.sim
                .hosting
                .nodes
                .get(&l.host)
                .is_none_or(|n| n.prepared_at_ms.is_none())
        );
    }
    l.prepare();
    assert_eq!(
        l.sim.hosting.nodes[&l.host].manager,
        Some(HostingManagerRegistration {
            server: l.controller,
            address: ip("10.0.0.10")
        })
    );
    // Script parameters do not leak into the interactive guest environment.
    assert!(
        !l.sim
            .server_os(l.host)
            .unwrap()
            .environment
            .contains_key("1")
    );
    let service = l.buy(1);
    let vm = l.vm(service);
    l.prepare();
    assert_eq!(l.sim.hosting.vms.len(), 1);
    assert_eq!(l.sim.hosting.services[&service].vm, Some(vm));
}

#[test]
fn provider_public_outage_pauses_sales_but_internal_outage_only_blocks_vm_management() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    let vm = l.vm(service);
    l.demand(
        vm,
        WorkloadDemand {
            cpu_mhz: 100.0,
            memory_mb: 256.0,
            network_mbps: 1.0,
            ..Default::default()
        },
    );
    let link = l.sim.link_for_port(l.controller_public).unwrap();
    let peer = if link.a == l.controller_public {
        link.b
    } else {
        link.a
    };
    l.sim
        .execute(Command::Disconnect { link: link.id })
        .unwrap();
    assert!(l.sim.hosting_provider_site_status().is_err());
    assert!(l.sim.hosting_node_eligible(l.host));
    assert!(
        l.sim
            .purchase_hosting(2, l.plan, CustomerCategory::Website)
            .is_err()
    );
    assert_eq!(l.sim.hosting.payments.len(), 1);
    apply(&mut l.sim, HostingCommand::RebootVm(vm));
    l.tick();
    l.tick();
    assert_eq!(l.sim.hosting.vms[&vm].power, VmPower::Running);
    assert!(l.sim.hosting.vms[&vm].metrics.reachable);
    l.sim
        .execute(Command::Connect {
            a: l.controller_public,
            b: peer,
        })
        .unwrap();
    assert!(l.sim.hosting_provider_site_status().is_ok());
    let link = l.sim.link_for_port(l.controller_management).unwrap().id;
    l.sim.execute(Command::Disconnect { link }).unwrap();
    assert!(l.sim.hosting_provider_site_status().is_ok());
    assert!(!l.sim.hosting_node_eligible(l.host));
    assert!(
        l.sim
            .configure_hosting(HostingCommand::RebootVm(vm))
            .is_err()
    );
    assert!(
        l.sim
            .purchase_hosting(2, l.plan, CustomerCategory::Website)
            .is_err()
    );
    l.tick();
    assert_eq!(l.sim.hosting.vms[&vm].power, VmPower::Running);
    assert!(l.sim.hosting.vms[&vm].metrics.reachable);
}

#[test]
fn controller_service_power_and_return_routes_change_live_availability() {
    let mut l = Lab::new();
    l.prepare();
    assert!(
        l.sim
            .execute_console(l.controller, "systemctl stop hosting-provider")
            .success
    );
    assert!(l.sim.hosting_provider_site_status().is_err());
    assert!(!l.sim.hosting_node_eligible(l.host));
    assert!(
        l.sim
            .execute_console(l.controller, "systemctl start hosting-provider")
            .success
    );
    assert!(l.sim.hosting_provider_site_status().is_ok());
    assert!(l.sim.hosting_node_eligible(l.host));
    l.sim
        .execute(Command::SetPower {
            device: l.controller,
            powered: false,
        })
        .unwrap();
    assert!(l.sim.hosting_provider_site_status().is_err());
    assert!(!l.sim.hosting_node_eligible(l.host));
    l.sim
        .execute(Command::SetPower {
            device: l.controller,
            powered: true,
        })
        .unwrap();
    let mut circuit = l.sim.provider().circuit(l.transit).unwrap().clone();
    circuit.routes.clear();
    l.sim
        .execute(Command::Provider(ProviderCommand::SetTransit(circuit)))
        .unwrap();
    assert!(l.sim.hosting_provider_site_status().is_err());
    assert!(l.sim.hosting_node_eligible(l.host));
    assert!(l.sim.hosting_availability(l.plan).is_err());
}

#[test]
fn old_preparation_scripts_upgrade_without_replacing_player_scripts_or_existing_vms() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    assert!(
        l.sim
            .execute_console(l.host, "echo 'vm-host prepare' > /root/vm-prepare.sh")
            .success
    );
    let saved = ron::to_string(&l.sim).unwrap();
    // Recreate the original distributed script through the guest shell.
    assert!(
        l.sim
            .execute_console(
                l.host,
                "printf '#!/bin/sh\\nvm-host prepare\\n' > /root/vm-prepare.sh"
            )
            .success
    );
    let legacy = ron::to_string(&l.sim).unwrap();
    let mut loaded: NetworkSim = ron::from_str(&legacy).unwrap();
    loaded.rebuild_indexes();
    assert!(
        loaded
            .server_os(l.host)
            .unwrap()
            .filesystem
            .read("/root/vm-prepare.sh")
            .unwrap()
            .contains("$1")
    );
    assert_eq!(loaded.hosting.services[&service].vm, Some(l.vm(service)));
    // A customized file is retained.
    let mut customized: NetworkSim = ron::from_str(&saved).unwrap();
    customized.rebuild_indexes();
    assert_eq!(
        customized
            .server_os(l.host)
            .unwrap()
            .filesystem
            .read("/root/vm-prepare.sh")
            .unwrap(),
        "vm-host prepare\n"
    );
}

#[test]
fn changing_the_internal_controller_address_requires_reregistration_and_preserves_vms() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    let vm = l.vm(service);
    l.sim
        .execute(Command::SetIpv4 {
            port: l.controller_management,
            config: Ipv4InterfaceConfig::new(ip("10.0.0.11"), 24, None, VlanId(1)),
        })
        .unwrap();
    assert!(!l.sim.hosting_node_eligible(l.host));
    assert!(
        !l.sim
            .execute_console(l.host, "./vm-prepare.sh 10.0.0.10")
            .success
    );
    assert!(
        l.sim
            .execute_console(l.host, "./vm-prepare.sh 10.0.0.11")
            .success
    );
    assert!(l.sim.hosting_node_eligible(l.host));
    assert_eq!(l.sim.hosting.services[&service].vm, Some(vm));
    assert_eq!(l.sim.hosting.vms.len(), 1);
    let mut loaded: NetworkSim = ron::from_str(&ron::to_string(&l.sim).unwrap()).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.hosting.provider_site_ipv4, Some(ip("203.0.113.18")));
    assert_eq!(loaded.hosting.provider_server, None);
    assert_eq!(
        loaded.hosting.nodes[&l.host].manager.unwrap().address,
        ip("10.0.0.11")
    );
    assert!(loaded.hosting_node_eligible(l.host));
}

#[test]
fn older_hosting_records_load_without_controller_fields() {
    let state = ron::to_string(&HostingState::default()).unwrap().replacen(
        "provider_site_ipv4:None,",
        "",
        1,
    );
    let loaded: HostingState = ron::from_str(&state).unwrap();
    assert_eq!(loaded.provider_site_ipv4, None);
    assert_eq!(loaded.provider_server, None);
    let node = HostingNode {
        prepared_at_ms: Some(123),
        ..Default::default()
    };
    let saved = ron::to_string(&node)
        .unwrap()
        .replacen("manager:None,", "", 1);
    let loaded: HostingNode = ron::from_str(&saved).unwrap();
    assert_eq!(loaded.prepared_at_ms, Some(123));
    assert_eq!(loaded.manager, None);
}

#[test]
fn setup_main_hoster_requires_operational_hardware_and_preserves_customer_vms() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    let before = ron::to_string(&l.sim).unwrap();
    assert!(
        !l.sim
            .execute_console(l.host, "./setup-main-hoster-server.sh")
            .success
    );
    assert_eq!(l.sim.hosting.services[&service].vm, Some(l.vm(service)));
    assert!(!l.sim.hosting_provider_installed(l.host));
    // Failed installation does not reserve the node or modify hosting state.
    let old: NetworkSim = ron::from_str(&before).unwrap();
    assert_eq!(
        ron::to_string(&old.hosting).unwrap(),
        ron::to_string(&l.sim.hosting).unwrap()
    );
    let SimEvent::DeviceAdded(new_server) = l.sim.execute(Command::BuyServerFullPack).unwrap()[0]
    else {
        panic!()
    };
    assert!(
        !l.sim
            .execute_console(new_server, "./setup-main-hoster-server.sh")
            .success
    );
    assert!(!l.sim.hosting_provider_installed(new_server));
    power(&mut l.sim, new_server, 6);
    // Installation takes no IP arguments and needs no address registered in HOSTING.
    assert!(
        l.sim
            .execute_console(new_server, "./setup-main-hoster-server.sh")
            .success
    );
    assert!(l.sim.hosting_provider_installed(new_server));
    assert!(!l.sim.hosting_node_eligible(new_server));
    assert_eq!(l.sim.hosting_provider_server(), Some(l.controller));
}

#[test]
fn storefront_traffic_uses_the_resolved_machine_and_shared_transit_queues() {
    let mut l = Lab::new();
    l.prepare();
    let mut plan = l.sim.hosting.products[&l.plan].clone();
    plan.port_mbps = 1000;
    apply(&mut l.sim, HostingCommand::SaveProduct(plan));
    l.sim.hosting.market.prospects_per_day = 100;
    l.tick();
    let site = l.sim.hosting.site_traffic;
    assert!(site.bytes > 0);
    assert!(site.delivered_mbps > 0.0);
    assert!(l.sim.port_telemetry(l.controller_public).tx_bytes >= site.bytes);
    assert!(l.sim.port_telemetry(l.transit).tx_bytes >= site.bytes);
    let baseline_demand = l.sim.hosting.market_stats[&l.plan].expected_orders_per_day;
    let service = l.buy(100);
    let vm = l.vm(service);
    l.demand(
        vm,
        WorkloadDemand {
            cpu_mhz: 100.0,
            memory_mb: 256.0,
            network_mbps: 1000.0,
            ..Default::default()
        },
    );
    l.tick();
    assert!(l.sim.hosting.site_traffic.loss > 0.0);
    assert!(l.sim.hosting.site_traffic.latency_ms > 0.0);
    assert!(l.sim.hosting.site_traffic.delivered_mbps < l.sim.hosting.site_traffic.demand_mbps);
    assert!(l.sim.hosting.market_stats[&l.plan].expected_orders_per_day < baseline_demand);
    let bytes = l.sim.hosting.site_traffic.bytes;
    let link = l.sim.link_for_port(l.controller_public).unwrap().id;
    l.sim.execute(Command::Disconnect { link }).unwrap();
    l.tick();
    assert_eq!(l.sim.hosting.site_traffic.bytes, bytes);
    assert_eq!(l.sim.hosting.site_traffic.delivered_mbps, 0.0);
    assert_eq!(
        l.sim.hosting.market_stats[&l.plan].expected_orders_per_day,
        0.0
    );
    assert_eq!(l.sim.hosting.vms[&vm].power, VmPower::Running);
}

#[test]
fn public_secondary_ip_connects_without_reselecting_the_server_or_node_manager() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    let name = l.sim.port(l.controller_public).unwrap().name.clone();
    let result = l.sim.execute_console(
        l.controller,
        &format!("ip addr add 203.0.113.19/24 dev {name}"),
    );
    assert!(result.success, "{:?}", result.lines);
    apply(
        &mut l.sim,
        HostingCommand::SetProviderSiteAddress(Some(ip("203.0.113.19"))),
    );
    assert_eq!(l.sim.hosting_provider_server(), Some(l.controller));
    assert!(l.sim.hosting_node_eligible(l.host));
    assert!(l.sim.hosting_provider_site_status().is_ok());
    // Only the exact published secondary address has a public return route.
    let mut circuit = l.sim.provider().circuit(l.transit).unwrap().clone();
    circuit.routes = vec![UpstreamRoute {
        prefix: prefix("203.0.113.19/32"),
        next_hop: ip("192.0.2.2"),
    }];
    l.sim
        .execute(Command::Provider(ProviderCommand::SetTransit(circuit)))
        .unwrap();
    assert!(
        !l.sim
            .ping(l.controller_public, ip("198.51.100.1"))
            .reachable
    );
    assert!(l.sim.hosting_provider_site_status().is_ok());
    l.sim.hosting.market.prospects_per_day = 500;
    l.tick();
    assert!(l.sim.hosting.site_traffic.bytes > 0);
    assert_eq!(l.sim.hosting.services[&service].vm, Some(l.vm(service)));
    let mut loaded: NetworkSim = ron::from_str(&ron::to_string(&l.sim).unwrap()).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.hosting.provider_site_ipv4, Some(ip("203.0.113.19")));
    assert!(loaded.hosting_provider_site_status().is_ok());
    assert!(loaded.hosting_node_eligible(l.host));
    assert_eq!(
        loaded.hosting.site_traffic.bytes,
        l.sim.hosting.site_traffic.bytes
    );
}

#[test]
fn legacy_provider_configuration_migrates_to_external_address_and_keeps_services() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    l.sim.hosting.provider_server = Some(HostingProviderServer {
        server: l.controller,
        public_ipv4: ip("203.0.113.18"),
        management_ipv4: ip("10.0.0.10"),
    });
    l.sim.hosting.provider_site_ipv4 = None;
    let saved = ron::to_string(&l.sim)
        .unwrap()
        .replace("provider_site_ipv4:None,", "");
    let mut loaded: NetworkSim = ron::from_str(&saved).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.hosting.provider_server, None);
    assert_eq!(loaded.hosting.provider_site_ipv4, Some(ip("203.0.113.18")));
    assert!(loaded.hosting_provider_site_status().is_ok());
    assert!(loaded.hosting_node_eligible(l.host));
    assert_eq!(loaded.hosting.services[&service].vm, Some(l.vm(service)));
    assert_eq!(loaded.hosting.payments.len(), 1);
    assert!(
        loaded
            .server_os(l.controller)
            .unwrap()
            .filesystem
            .read("/root/setup-main-hoster-server.sh")
            .is_ok()
    );
    assert!(
        !ron::to_string(&loaded.hosting)
            .unwrap()
            .contains("provider_server:")
    );
}

#[test]
fn changing_public_destination_resolves_the_new_installed_server_and_requires_node_registration() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    let vm = l.vm(service);
    let SimEvent::DeviceAdded(new_server) = l.sim.execute(Command::BuyServerFullPack).unwrap()[0]
    else {
        panic!()
    };
    power(&mut l.sim, new_server, 6);
    l.sim
        .execute(Command::BuyCableSupply {
            supply: CableSupply::Rj45Pack20,
        })
        .unwrap();
    let new_data = l.sim.device(new_server).unwrap().ports()[0];
    let new_mgmt = *l
        .sim
        .device(new_server)
        .unwrap()
        .ports()
        .iter()
        .find(|p| l.sim.port(**p).unwrap().name == "mgmt0")
        .unwrap();
    for (old, new, address, gateway) in [
        (
            l.controller_public,
            new_data,
            "203.0.113.20",
            Some(ip("203.0.113.1")),
        ),
        (l.controller_management, new_mgmt, "10.0.0.12", None),
    ] {
        let link = l.sim.link_for_port(old).unwrap();
        let peer = if link.a == old { link.b } else { link.a };
        let switch = l.sim.port(peer).unwrap().device;
        let free = l.sim.device(switch).unwrap().ports()[3];
        l.sim.execute(Command::Connect { a: new, b: free }).unwrap();
        l.sim
            .execute(Command::SetIpv4 {
                port: new,
                config: Ipv4InterfaceConfig::new(ip(address), 24, gateway, VlanId(1)),
            })
            .unwrap();
    }
    apply(
        &mut l.sim,
        HostingCommand::SetProviderSiteAddress(Some(ip("203.0.113.20"))),
    );
    assert_eq!(l.sim.hosting_provider_server(), Some(new_server));
    assert!(l.sim.hosting_provider_site_status().is_err());
    assert!(!l.sim.hosting_node_eligible(l.host));
    assert!(
        l.sim
            .execute_console(new_server, "./setup-main-hoster-server.sh")
            .success
    );
    assert!(l.sim.hosting_provider_site_status().is_ok());
    assert!(!l.sim.hosting_node_eligible(l.host));
    assert!(
        l.sim
            .execute_console(l.host, "./vm-prepare.sh 10.0.0.12")
            .success
    );
    assert!(l.sim.hosting_node_eligible(l.host));
    assert_eq!(
        l.sim.hosting.nodes[&l.host].manager.unwrap().server,
        new_server
    );
    assert_eq!(l.sim.hosting.services[&service].vm, Some(vm));
    assert_eq!(l.sim.hosting.vms.len(), 1);
    let old_bytes = l.sim.port_telemetry(l.controller_public).tx_bytes;
    l.sim.hosting.market.prospects_per_day = 100;
    l.tick();
    assert!(l.sim.hosting.site_traffic.bytes > 0);
    assert!(l.sim.port_telemetry(new_data).tx_bytes >= l.sim.hosting.site_traffic.bytes);
    assert_eq!(
        l.sim.port_telemetry(l.controller_public).tx_bytes,
        old_bytes
    );
}

#[test]
fn destroying_a_vm_releases_compute_but_keeps_its_service_and_ipv4_lease() {
    let mut l = Lab::new();
    l.prepare();
    l.policy(|p| {
        p.cpu_overcommit_percent = 1000;
        p.ram_overcommit_percent = 400;
    });
    let service = l.buy(1);
    let vm = l.vm(service);
    let port = l.sim.hosting.vms[&vm].interfaces[0];
    let address = l.sim.hosting.vms[&vm].ips[0];
    let expiry = l.sim.hosting.services[&service].expires_at_ms;
    let before_usage = l.sim.hosting_range_usage(prefix("203.0.113.0/29"));
    apply(&mut l.sim, HostingCommand::DestroyServiceVm(service));
    apply(&mut l.sim, HostingCommand::DestroyServiceVm(service));
    assert!(!l.sim.hosting.vms.contains_key(&vm));
    assert!(l.sim.port(port).is_none());
    assert_eq!(l.sim.hosting.nodes[&l.host].usage.sold_vcpu, 0);
    assert_eq!(l.sim.hosting.nodes[&l.host].usage.physical_disk_gb, 0.0);
    assert_eq!(l.sim.hosting.services[&service].expires_at_ms, expiry);
    assert!(l.sim.hosting.services[&service].terminated_at_ms.is_none());
    assert_eq!(
        l.sim.hosting_range_usage(before_usage.prefix).allocated,
        before_usage.allocated
    );
    let assignments = l.sim.hosting_ipv4_assignments();
    let row = assignments.iter().find(|r| r.address == address).unwrap();
    assert_eq!(row.service, Some(service));
    assert_eq!(
        row.customer,
        Some(l.sim.hosting.services[&service].customer)
    );
    assert_eq!(row.node, Some(l.host));
    assert!(row.vm.is_none() && row.port.is_none());
    for order in 2..=10 {
        let s = l.buy(order);
        assert!(!l.sim.hosting.vms[&l.vm(s)].ips.contains(&address));
    }
    assert!(
        l.sim
            .purchase_hosting(11, l.plan, CustomerCategory::Website)
            .is_err()
    );
    // The ordinary owned-address allocator must also honor the retained lease.
    assert!(
        l.sim
            .allocate_ipv4(
                l.data,
                prefix("203.0.113.0/29"),
                Some(ip("203.0.113.1")),
                Some(VlanId(1))
            )
            .is_err()
    );
    assert_eq!(l.sim.hosting.payments.len(), 10);
}

#[test]
fn destroyed_vm_recreation_preserves_ips_contract_expiration_and_payment_identity() {
    let mut l = Lab::new();
    l.prepare();
    let mut product = l.sim.hosting.products[&l.plan].clone();
    product.ipv4_count = 2;
    apply(&mut l.sim, HostingCommand::SaveProduct(product));
    let service = l.buy(1);
    let old_id = l.vm(service);
    l.sim
        .hosting
        .vms
        .get_mut(&old_id)
        .unwrap()
        .metrics
        .traffic_bytes = 500_000_000;
    let old = l.sim.hosting.vms[&old_id].clone();
    apply(&mut l.sim, HostingCommand::DestroyServiceVm(service));
    let mut product = l.sim.hosting.products[&l.plan].clone();
    product.vcpu = 1;
    product.ram_mb = 128;
    product.monthly_price_cents = 9999;
    product.published = false;
    apply(&mut l.sim, HostingCommand::SaveProduct(product));
    apply(&mut l.sim, HostingCommand::DeleteProduct(l.plan));
    l.sim
        .execute(Command::SetPortEnabled {
            port: l.controller_public,
            enabled: false,
        })
        .unwrap();
    assert!(l.sim.hosting_provider_site_status().is_err());
    let money = l.sim.money;
    apply(&mut l.sim, HostingCommand::RecreateServiceVm(service));
    let vm = &l.sim.hosting.vms[&l.vm(service)];
    assert_ne!(vm.id, old_id);
    assert_eq!(vm.plan, old.plan);
    assert_eq!(vm.host, old.host);
    assert_eq!(vm.ips, old.ips);
    assert_eq!(vm.ip_ranges, old.ip_ranges);
    assert_eq!(vm.expires_at_ms, old.expires_at_ms);
    assert_eq!(vm.disk_used_gb, 1.0);
    assert_eq!(vm.metrics.traffic_bytes, old.metrics.traffic_bytes);
    assert_eq!(vm.power, VmPower::Running);
    let new_id = vm.id;
    let port = vm.interfaces[0];
    assert!(l.sim.ping(port, ip("198.51.100.1")).reachable);
    apply(&mut l.sim, HostingCommand::RecreateServiceVm(service));
    assert_eq!(l.vm(service), new_id);
    assert_eq!(l.sim.hosting.vms.len(), 1);
    assert_eq!(l.sim.hosting.payments.len(), 1);
    assert_eq!(l.sim.money, money);
    assert_eq!(l.buy(1), service);
    let output = l.sim.execute_console(l.host, "ip -br a");
    assert!(output.success && output.lines.join("\n").contains(&format!("vnet{new_id}")));
}

#[test]
fn failed_recreation_keeps_reserved_ips_and_changes_no_resources_ids_or_payments() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    apply(&mut l.sim, HostingCommand::DestroyServiceVm(service));
    let restored = ron::to_string(&l.sim).unwrap();
    let mut saved: NetworkSim = ron::from_str(&restored).unwrap();
    saved.rebuild_indexes();
    assert_eq!(
        ron::to_string(&saved.hosting).unwrap(),
        ron::to_string(&l.sim.hosting).unwrap()
    );
    l.sim
        .execute(Command::SetPortEnabled {
            port: l.mgmt,
            enabled: false,
        })
        .unwrap();
    let before = ron::to_string(&l.sim).unwrap();
    assert!(
        l.sim
            .execute(Command::Hosting(HostingCommand::RecreateServiceVm(service)))
            .is_err()
    );
    assert_eq!(ron::to_string(&l.sim).unwrap(), before);
    l.sim
        .execute(Command::SetPortEnabled {
            port: l.mgmt,
            enabled: true,
        })
        .unwrap();
    l.policy(|p| p.reserved_ram_mb = 1_000_000);
    let before = ron::to_string(&l.sim).unwrap();
    assert!(l.sim.recreate_hosting_vm(service).is_err());
    assert_eq!(ron::to_string(&l.sim).unwrap(), before);
    assert!(
        l.sim
            .hosting_ipv4_assignments()
            .iter()
            .any(|r| r.service == Some(service) && r.vm.is_none())
    );
    assert_eq!(
        saved.recreate_hosting_vm(service).unwrap(),
        saved.hosting.services[&service].vm.unwrap()
    );
    assert_eq!(saved.hosting.payments.len(), 1);
}

#[test]
fn refund_prorates_unused_prepaid_time_releases_ips_and_is_idempotent_after_loading() {
    let mut l = Lab::new();
    l.prepare();
    l.sim.hosting.market.periods = vec![SubscriptionOption {
        months: 3,
        weight: 1,
    }];
    let mut product = l.sim.hosting.products[&l.plan].clone();
    product.monthly_price_cents = 501;
    apply(&mut l.sim, HostingCommand::SaveProduct(product));
    l.sim.money = 12345;
    l.sim.hosting.revenue_remainder_cents = 10;
    let service = l.buy(1);
    let vm = l.vm(service);
    let address = l.sim.hosting.vms[&vm].ips[0];
    let port = l.sim.hosting.vms[&vm].interfaces[0];
    l.sim.hosting.now_ms = MONTH_MS + MONTH_MS / 2;
    assert_eq!(l.sim.hosting_refund_cents(service).unwrap(), 751);
    // A provider cancellation can settle a service even when the node is offline.
    l.sim
        .execute(Command::SetPortEnabled {
            port: l.mgmt,
            enabled: false,
        })
        .unwrap();
    let before_cents = l.sim.money * 100 + l.sim.hosting.revenue_remainder_cents as i64;
    apply(&mut l.sim, HostingCommand::RefundService(service));
    assert_eq!(
        l.sim.money * 100 + l.sim.hosting.revenue_remainder_cents as i64,
        before_cents - 751
    );
    assert!(l.sim.hosting.services[&service].terminated_at_ms.is_some());
    assert!(l.sim.port(port).is_none());
    assert!(!l.sim.hosting.vms.contains_key(&vm));
    assert_eq!(l.sim.hosting.refunds[0].amount_cents, 751);
    assert!(
        !l.sim
            .hosting_ipv4_assignments()
            .iter()
            .any(|r| r.address == address)
    );
    assert_eq!(l.sim.hosting.nodes[&l.host].usage.sold_ram_mb, 0);
    let customer = l.sim.hosting.services[&service].customer;
    assert!(
        !l.sim.hosting.customers[&customer]
            .services
            .contains(&service)
    );
    let mut loaded: NetworkSim = ron::from_str(&ron::to_string(&l.sim).unwrap()).unwrap();
    loaded.rebuild_indexes();
    let before = ron::to_string(&loaded).unwrap();
    assert_eq!(loaded.refund_hosting_service(service).unwrap(), 751);
    assert_eq!(ron::to_string(&loaded).unwrap(), before);
    assert_eq!(
        loaded
            .purchase_hosting(1, l.plan, CustomerCategory::Website)
            .unwrap(),
        service
    );
    assert!(loaded.recreate_hosting_vm(service).is_err());
    assert_eq!(loaded.hosting.refunds.len(), 1);
    assert_eq!(loaded.hosting.payments.len(), 1);
    loaded
        .execute(Command::SetPortEnabled {
            port: l.mgmt,
            enabled: true,
        })
        .unwrap();
    let new_service = loaded
        .purchase_hosting(2, l.plan, CustomerCategory::Website)
        .unwrap();
    assert_eq!(
        loaded.hosting.vms[&loaded.hosting.services[&new_service].vm.unwrap()].ips[0],
        address
    );
}

#[test]
fn destroyed_service_does_not_auto_recreate_or_renew_and_releases_ips_at_expiration() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    let address = l.sim.hosting.vms[&l.vm(service)].ips[0];
    apply(&mut l.sim, HostingCommand::DestroyServiceVm(service));
    let reputation = l.sim.hosting.reputation;
    l.tick();
    let s = &l.sim.hosting.services[&service];
    assert!(s.satisfaction < 1.0 && s.availability < 1.0);
    assert!(l.sim.hosting.reputation < reputation);
    assert!(s.vm.is_none());
    let before_usage = l.sim.hosting_range_usage(prefix("203.0.113.0/29"));
    l.sim.hosting.now_ms = s.expires_at_ms - HOSTING_TICK_MS;
    l.tick();
    assert!(l.sim.hosting.services[&service].terminated_at_ms.is_some());
    assert_eq!(l.sim.hosting.payments.len(), 1);
    assert!(l.sim.hosting.refunds.is_empty());
    assert_eq!(
        l.sim.hosting_range_usage(before_usage.prefix).available,
        before_usage.available + 1
    );
    assert!(
        !l.sim
            .hosting_ipv4_assignments()
            .iter()
            .any(|r| r.address == address)
    );
}

#[test]
fn older_service_saves_capture_contract_before_destroying_vm_and_can_recreate() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    let vm = l.vm(service);
    let old = l.sim.hosting.vms[&vm].clone();
    l.sim
        .hosting
        .services
        .get_mut(&service)
        .unwrap()
        .provisioning = None;
    let legacy = ron::to_string(&l.sim)
        .unwrap()
        .replace("provisioning:None,", "")
        .replace("refunds:[],", "")
        .replace("retained_traffic_bytes:0,", "");
    let mut loaded: NetworkSim = ron::from_str(&legacy).unwrap();
    loaded.rebuild_indexes();
    loaded.destroy_hosting_vm(service).unwrap();
    assert_eq!(
        loaded.hosting.services[&service]
            .provisioning
            .as_ref()
            .unwrap()
            .ips,
        old.ips
    );
    let saved = ron::to_string(&loaded).unwrap();
    let mut loaded: NetworkSim = ron::from_str(&saved).unwrap();
    loaded.rebuild_indexes();
    let id = loaded.recreate_hosting_vm(service).unwrap();
    assert_eq!(loaded.hosting.vms[&id].plan, old.plan);
    assert_eq!(loaded.hosting.vms[&id].ips, old.ips);
    assert_eq!(loaded.hosting.vms[&id].expires_at_ms, old.expires_at_ms);
    assert_eq!(loaded.hosting.payments.len(), 1);
}

#[test]
fn destruction_requires_management_and_retained_service_prevents_pool_removal() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    l.sim
        .execute(Command::SetPortEnabled {
            port: l.mgmt,
            enabled: false,
        })
        .unwrap();
    let before = ron::to_string(&l.sim).unwrap();
    assert!(l.sim.destroy_hosting_vm(service).is_err());
    assert_eq!(ron::to_string(&l.sim).unwrap(), before);
    l.sim
        .execute(Command::SetPortEnabled {
            port: l.mgmt,
            enabled: true,
        })
        .unwrap();
    l.sim.destroy_hosting_vm(service).unwrap();
    apply(&mut l.sim, HostingCommand::DeleteProduct(l.plan));
    let before = ron::to_string(&l.sim).unwrap();
    let err = l
        .sim
        .execute(Command::Provider(ProviderCommand::RemovePool(prefix(
            "203.0.113.0/29",
        ))))
        .unwrap_err();
    assert!(err.to_string().contains("terminate hosting services"));
    assert_eq!(ron::to_string(&l.sim).unwrap(), before);
    let refund = l.sim.refund_hosting_service(service).unwrap();
    assert_eq!(refund, 500);
    assert!(
        l.sim
            .hosting_ipv4_assignments()
            .iter()
            .all(|r| r.service != Some(service))
    );
}

#[test]
fn refund_after_renewal_only_returns_unused_time_in_the_latest_paid_period() {
    let mut l = Lab::new();
    l.prepare();
    let service = l.buy(1);
    let vm = l.vm(service);
    l.demand(
        vm,
        WorkloadDemand {
            cpu_mhz: 100.0,
            memory_mb: 128.0,
            ..Default::default()
        },
    );
    l.sim.hosting.market.periods = vec![SubscriptionOption {
        months: 3,
        weight: 1,
    }];
    l.sim.hosting.now_ms = MONTH_MS - HOSTING_TICK_MS;
    l.tick();
    assert_eq!(l.sim.hosting.payments.len(), 2);
    assert_eq!(l.sim.hosting.payments[1].amount_cents, 1500);
    l.sim.hosting.now_ms = MONTH_MS * 2 + MONTH_MS / 2;
    assert_eq!(l.sim.hosting_refund_cents(service).unwrap(), 750);
    l.sim.destroy_hosting_vm(service).unwrap();
    assert_eq!(l.sim.refund_hosting_service(service).unwrap(), 750);
    assert_eq!(
        l.sim
            .hosting
            .payments
            .iter()
            .map(|p| p.amount_cents)
            .sum::<u64>(),
        2000
    );
    assert_eq!(l.sim.hosting.refunds.len(), 1);
    assert_eq!(l.sim.hosting.services[&service].renewals, 1);
}

fn vlan_hosting_lab() -> Lab {
    let mut l = Lab::new();
    let switch_port = l.sim.link_for_port(l.data).unwrap().other(l.data).unwrap();
    let switch = l.sim.port(switch_port).unwrap().device;
    let router_port = l
        .sim
        .device(switch)
        .unwrap()
        .ports()
        .iter()
        .filter_map(|p| l.sim.link_for_port(*p).and_then(|link| link.other(*p)))
        .find(|p| {
            matches!(
                l.sim.device(l.sim.port(*p).unwrap().device).unwrap().kind,
                DeviceKind::Router(_)
            )
        })
        .unwrap();
    let router_switch_port = l
        .sim
        .link_for_port(router_port)
        .unwrap()
        .other(router_port)
        .unwrap();
    for (vlan, gateway) in [(10, "203.0.113.1"), (20, "203.0.113.9")] {
        l.sim
            .execute(Command::CreateVlan {
                switch,
                vlan: Vlan {
                    id: VlanId(vlan),
                    name: format!("Hosting {vlan}"),
                },
            })
            .unwrap();
        l.sim
            .execute(Command::ConfigureRouterInterface {
                port: router_port,
                name: format!("p{}.{}", router_port.0, vlan),
                vlan: Some(VlanId(vlan)),
                address: Some(ip(gateway)),
                prefix: 29,
                internet_connected: false,
            })
            .unwrap();
    }
    for port in [switch_port, router_switch_port] {
        l.sim
            .execute(Command::SetSwitchPortMode {
                port,
                mode: SwitchPortMode::Trunk {
                    native_vlan: Some(VlanId(1)),
                    allowed: [VlanId(1), VlanId(10), VlanId(20)].into(),
                },
            })
            .unwrap();
    }
    l
}

#[test]
fn interface_subnet_vlan_mappings_provision_real_isolated_networks_and_survive_saves() {
    let mut l = vlan_hosting_lab();
    let original_host_addresses: Vec<_> = match &l.sim.port(l.data).unwrap().config {
        PortConfig::Server(c) => c.addresses().cloned().collect(),
        _ => panic!(),
    };
    let mappings = vec![
        HostingNodeNetwork {
            port: l.data,
            subnet: prefix("203.0.113.0/29"),
            vlan: Some(VlanId(10)),
        },
        HostingNodeNetwork {
            port: l.data,
            subnet: prefix("203.0.113.8/29"),
            vlan: Some(VlanId(20)),
        },
    ];
    apply(
        &mut l.sim,
        HostingCommand::SetNodeNetworks {
            node: l.host,
            networks: Some(mappings.clone()),
        },
    );
    l.prepare();
    let PortConfig::Server(c) = &l.sim.port(l.data).unwrap().config else {
        panic!()
    };
    assert_eq!(
        c.addresses().cloned().collect::<Vec<_>>(),
        original_host_addresses
    );
    assert!(l.sim.hosting.nodes[&l.host].auto_addresses.is_empty());
    let mut plan = l.sim.hosting.products[&l.plan].clone();
    plan.ipv4_ranges = [prefix("203.0.113.0/29")].into();
    apply(&mut l.sim, HostingCommand::SaveProduct(plan.clone()));
    let first = l.buy(1);
    plan.id = 0;
    plan.name = "VLAN 20 plan".into();
    plan.ipv4_ranges = [prefix("203.0.113.8/29")].into();
    apply(&mut l.sim, HostingCommand::SaveProduct(plan));
    let second_plan = *l.sim.hosting.products.keys().next_back().unwrap();
    let second = l
        .sim
        .purchase_hosting(2, second_plan, CustomerCategory::Website)
        .unwrap();
    for (service, vlan, range) in [
        (first, 10, "203.0.113.0/29"),
        (second, 20, "203.0.113.8/29"),
    ] {
        let vm = &l.sim.hosting.vms[&l.vm(service)];
        assert!(prefix(range).usable(vm.ips[0]));
        let PortConfig::Server(c) = &l.sim.port(vm.interfaces[0]).unwrap().config else {
            panic!()
        };
        assert_eq!(c.ipv4.as_ref().unwrap().vlan, Some(VlanId(vlan)));
        assert_eq!(c.ipv4.as_ref().unwrap().prefix, 29);
        assert!(c.tagged_vlans.contains(&VlanId(vlan)));
        assert_eq!(
            l.sim.wire_vlan_for(vm.interfaces[0], VlanId(vlan)),
            Some(VlanId(vlan))
        );
        assert!(l.sim.ping_from_internet(vm.ips[0]).reachable);
    }
    let first_port = l.sim.hosting.vms[&l.vm(first)].interfaces[0];
    let second_ip = l.sim.hosting.vms[&l.vm(second)].ips[0];
    assert!(l.sim.ping(first_port, second_ip).reachable);
    l.tick();
    assert!(
        l.sim
            .hosting
            .vms
            .values()
            .all(|v| v.metrics.reachable && v.metrics.traffic_bytes > 0)
    );
    let mut loaded: NetworkSim = ron::from_str(&ron::to_string(&l.sim).unwrap()).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.hosting.nodes[&l.host].networks, Some(mappings));
    for service in [first, second] {
        let id = loaded.hosting.services[&service].vm.unwrap();
        let guest_ip = loaded.hosting.vms[&id].ips[0];
        assert!(loaded.ping_from_internet(guest_ip).reachable);
    }
}

#[test]
fn mapped_vlan_requires_actual_trunk_and_keeps_existing_guests_after_policy_edits() {
    let mut l = vlan_hosting_lab();
    let mapping = HostingNodeNetwork {
        port: l.data,
        subnet: prefix("203.0.113.0/29"),
        vlan: Some(VlanId(10)),
    };
    apply(
        &mut l.sim,
        HostingCommand::SetNodeNetworks {
            node: l.host,
            networks: Some(vec![mapping]),
        },
    );
    l.prepare();
    let service = l.buy(1);
    let guest_ip = l.sim.hosting.vms[&l.vm(service)].ips[0];
    apply(
        &mut l.sim,
        HostingCommand::SetNodeNetworks {
            node: l.host,
            networks: Some(vec![]),
        },
    );
    assert!(l.sim.hosting_availability(l.plan).is_err());
    assert!(l.sim.ping_from_internet(guest_ip).reachable);
    let switch_port = l.sim.link_for_port(l.data).unwrap().other(l.data).unwrap();
    l.sim
        .execute(Command::SetSwitchPortMode {
            port: switch_port,
            mode: SwitchPortMode::Trunk {
                native_vlan: Some(VlanId(1)),
                allowed: [VlanId(1), VlanId(20)].into(),
            },
        })
        .unwrap();
    l.tick();
    assert!(!l.sim.hosting.vms[&l.vm(service)].metrics.reachable);
    l.sim
        .execute(Command::SetSwitchPortMode {
            port: switch_port,
            mode: SwitchPortMode::Trunk {
                native_vlan: Some(VlanId(1)),
                allowed: [VlanId(1), VlanId(10), VlanId(20)].into(),
            },
        })
        .unwrap();
    assert!(l.sim.ping_from_internet(guest_ip).reachable);
    l.sim.terminate_hosting_service(service).unwrap();
    l.tick();
    let PortConfig::Server(config) = &l.sim.port(l.data).unwrap().config else {
        panic!()
    };
    assert!(!config.tagged_vlans.contains(&VlanId(10)));
    assert!(config.addresses().all(|a| a.vlan != Some(VlanId(10))));
}

#[test]
fn hosting_network_map_never_falls_back_to_an_unmapped_interface() {
    let mut l = Lab::new();
    let second = l
        .sim
        .device(l.host)
        .unwrap()
        .ports()
        .iter()
        .copied()
        .find(|p| *p != l.data && *p != l.mgmt)
        .unwrap();
    apply(
        &mut l.sim,
        HostingCommand::SetNodeNetworks {
            node: l.host,
            networks: Some(vec![HostingNodeNetwork {
                port: second,
                subnet: prefix("203.0.113.0/29"),
                vlan: None,
            }]),
        },
    );
    l.prepare();
    let before = ron::to_string(&l.sim).unwrap();
    assert!(
        l.sim
            .purchase_hosting(1, l.plan, CustomerCategory::Website)
            .is_err()
    );
    assert_eq!(ron::to_string(&l.sim).unwrap(), before);
    let switch = l
        .sim
        .port(l.sim.link_for_port(l.data).unwrap().other(l.data).unwrap())
        .unwrap()
        .device;
    let free_switch_port = l
        .sim
        .device(switch)
        .unwrap()
        .ports()
        .iter()
        .copied()
        .find(|p| l.sim.link_for_port(*p).is_none())
        .unwrap();
    l.sim
        .execute(Command::Connect {
            a: second,
            b: free_switch_port,
        })
        .unwrap();
    l.tick();
    let service = l.buy(1);
    let guest = l.sim.hosting.vms[&l.vm(service)].clone();
    assert!(prefix("203.0.113.0/29").usable(guest.ips[0]));
    let PortConfig::Server(c) = &l.sim.port(second).unwrap().config else {
        panic!()
    };
    assert!(c.addresses().next().is_none());
    assert!(l.sim.ping_from_internet(guest.ips[0]).reachable);
    // Disconnecting only the selected NIC takes the guest offline even though
    // the first NIC remains configured, connected and in the same IPv4 subnet.
    let link = l.sim.link_for_port(second).unwrap().id;
    l.sim.execute(Command::Disconnect { link }).unwrap();
    l.tick();
    assert!(!l.sim.hosting.vms[&guest.id].metrics.reachable);
}

#[test]
fn invalid_node_interface_subnet_and_vlan_maps_are_atomic_and_legacy_saves_default_to_auto() {
    let mut l = Lab::new();
    let valid = HostingNodeNetwork {
        port: l.data,
        subnet: prefix("203.0.113.0/29"),
        vlan: Some(VlanId(10)),
    };
    for mappings in [
        vec![HostingNodeNetwork {
            port: l.mgmt,
            ..valid
        }],
        vec![HostingNodeNetwork {
            port: l.controller_public,
            ..valid
        }],
        vec![HostingNodeNetwork {
            subnet: prefix("192.0.3.0/29"),
            ..valid
        }],
        vec![HostingNodeNetwork {
            vlan: Some(VlanId(0)),
            ..valid
        }],
        vec![HostingNodeNetwork {
            vlan: Some(VlanId(4095)),
            ..valid
        }],
        vec![
            valid,
            HostingNodeNetwork {
                subnet: prefix("203.0.113.0/30"),
                ..valid
            },
        ],
    ] {
        let before = ron::to_string(&l.sim).unwrap();
        assert!(
            l.sim
                .configure_hosting(HostingCommand::SetNodeNetworks {
                    node: l.host,
                    networks: Some(mappings)
                })
                .is_err()
        );
        assert_eq!(ron::to_string(&l.sim).unwrap(), before);
    }
    l.prepare();
    let legacy = ron::to_string(&l.sim)
        .unwrap()
        .replace("networks:None,", "")
        .replace("tagged_vlans:[],", "");
    let mut loaded: NetworkSim = ron::from_str(&legacy).unwrap();
    loaded.rebuild_indexes();
    assert!(loaded.hosting.nodes[&l.host].networks.is_none());
    assert!(loaded.hosting_availability(l.plan).is_ok());
}

#[test]
fn guest_rules_bridge_tagged_and_native_vms_without_assigning_host_ipv4() {
    let mut l = vlan_hosting_lab();
    let data_name = l.sim.port(l.data).unwrap().name.clone();
    let flush = l
        .sim
        .execute_console(l.host, &format!("ip addr flush dev {data_name}"));
    assert!(flush.success, "{:?}", flush.lines);
    let mut product = l.sim.hosting.products[&l.plan].clone();
    product.ipv4_ranges = [prefix("203.0.113.16/29")].into();
    apply(&mut l.sim, HostingCommand::SaveProduct(product));
    apply(
        &mut l.sim,
        HostingCommand::SetNodeNetworks {
            node: l.host,
            networks: Some(vec![
                HostingNodeNetwork {
                    port: l.data,
                    subnet: prefix("203.0.113.8/29"),
                    vlan: Some(VlanId(20)),
                },
                HostingNodeNetwork {
                    port: l.data,
                    subnet: prefix("203.0.113.16/29"),
                    vlan: None,
                },
            ]),
        },
    );
    l.prepare();
    let PortConfig::Server(config) = &l.sim.port(l.data).unwrap().config else {
        panic!()
    };
    assert!(config.ipv4.is_none());
    assert!(config.additional_ipv4.is_empty());
    assert!(l.sim.hosting.nodes[&l.host].auto_addresses.is_empty());
    assert_eq!(config.native_vlan(), VlanId(1));
    let service = l.buy(1);
    let vm = l.sim.hosting.vms[&l.vm(service)].clone();
    assert!(prefix("203.0.113.16/29").usable(vm.ips[0]));
    assert_eq!(l.sim.wire_vlan_for(vm.interfaces[0], VlanId(1)), None);
    assert!(l.sim.ping_from_internet(vm.ips[0]).reachable);
    let mut tagged_plan = l.sim.hosting.products[&l.plan].clone();
    tagged_plan.id = 0;
    tagged_plan.ipv4_ranges = [prefix("203.0.113.8/29")].into();
    apply(&mut l.sim, HostingCommand::SaveProduct(tagged_plan));
    let tagged_plan = *l.sim.hosting.products.keys().next_back().unwrap();
    let tagged_service = l
        .sim
        .purchase_hosting(2, tagged_plan, CustomerCategory::Website)
        .unwrap();
    let tagged_vm = l.sim.hosting.vms[&l.vm(tagged_service)].clone();
    assert!(prefix("203.0.113.8/29").usable(tagged_vm.ips[0]));
    assert_eq!(
        l.sim.wire_vlan_for(tagged_vm.interfaces[0], VlanId(20)),
        Some(VlanId(20))
    );
    assert!(l.sim.ping_from_internet(tagged_vm.ips[0]).reachable);
    let PortConfig::Server(c) = &l.sim.port(l.data).unwrap().config else {
        panic!()
    };
    assert!(c.addresses().next().is_none());
    let payments = l.sim.hosting.payments.len();
    l.sim.destroy_hosting_vm(tagged_service).unwrap();
    l.sim.recreate_hosting_vm(tagged_service).unwrap();
    let recreated = l.sim.hosting.vms[&l.vm(tagged_service)].clone();
    assert_eq!(recreated.ips, tagged_vm.ips);
    assert_eq!(l.sim.hosting.payments.len(), payments);
    assert!(l.sim.ping_from_internet(recreated.ips[0]).reachable);
    let mut loaded: NetworkSim = ron::from_str(&ron::to_string(&l.sim).unwrap()).unwrap();
    loaded.rebuild_indexes();
    assert!(loaded.ping_from_internet(vm.ips[0]).reachable);
    assert!(loaded.ping_from_internet(recreated.ips[0]).reachable);
    let PortConfig::Server(c) = &loaded.port(l.data).unwrap().config else {
        panic!()
    };
    assert!(c.addresses().next().is_none());
}

#[test]
fn guest_rules_share_one_bridge_without_losing_other_allowed_subnets() {
    let mut l = Lab::new();
    let name = l.sim.port(l.data).unwrap().name.clone();
    assert!(
        l.sim
            .execute_console(l.host, &format!("ip addr flush dev {name}"))
            .success
    );
    apply(
        &mut l.sim,
        HostingCommand::SetNodeNetworks {
            node: l.host,
            networks: Some(
                ["203.0.113.0/29", "203.0.113.8/29"]
                    .map(|subnet| HostingNodeNetwork {
                        subnet: prefix(subnet),
                        port: l.data,
                        vlan: None,
                    })
                    .into(),
            ),
        },
    );
    l.prepare();
    let mut plan = l.sim.hosting.products[&l.plan].clone();
    plan.ipv4_count = 6;
    plan.ipv4_ranges = [prefix("203.0.113.0/29"), prefix("203.0.113.8/29")].into();
    apply(&mut l.sim, HostingCommand::SaveProduct(plan));
    let service = l.buy(1);
    let vm = l.sim.hosting.vms[&l.vm(service)].clone();
    assert_eq!(vm.ips.len(), 6);
    for range in ["203.0.113.0/29", "203.0.113.8/29"] {
        assert!(vm.ips.iter().any(|ip| prefix(range).usable(*ip)));
    }
    for address in vm.ips {
        assert!(l.sim.ping_from_internet(address).reachable);
    }
    let PortConfig::Server(c) = &l.sim.port(l.data).unwrap().config else {
        panic!()
    };
    assert!(c.addresses().next().is_none());
}
