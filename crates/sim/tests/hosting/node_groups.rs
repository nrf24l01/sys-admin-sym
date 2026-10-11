use super::*;
use std::collections::BTreeSet;

fn spare_node(sim: &mut NetworkSim) -> DeviceId {
    let SimEvent::DeviceAdded(node) = sim.execute(Command::BuyServerFullPack).unwrap()[0] else {
        panic!()
    };
    node
}

fn save_group(sim: &mut NetworkSim, group: HostingNodeGroup) -> u64 {
    apply(sim, HostingCommand::SaveNodeGroup(group));
    *sim.hosting.node_groups.keys().next_back().unwrap()
}

fn network() -> HostingGroupNetwork {
    HostingGroupNetwork {
        interface: "eth0".into(),
        subnet: prefix("203.0.113.0/29"),
        vlan: Some(VlanId(10)),
    }
}

#[test]
fn shared_settings_resolve_local_interfaces_and_synchronize_new_members() {
    let mut l = Lab::new();
    l.prepare();
    let second = spare_node(&mut l.sim);
    let manager = l.sim.hosting.nodes[&l.host].manager;
    let prepared = l.sim.hosting.nodes[&l.host].prepared_at_ms;
    let id = save_group(
        &mut l.sim,
        HostingNodeGroup {
            name: "Shared fleet".into(),
            nodes: [l.host].into(),
            policy: HypervisorPolicy {
                cpu_overcommit_percent: 700,
                swap_mb: 4096,
                thin_provisioning: false,
                ..Default::default()
            },
            ipv4_ranges: Some([prefix("203.0.113.0/29")].into()),
            networks: Some(vec![network()]),
            ..Default::default()
        },
    );
    apply(
        &mut l.sim,
        HostingCommand::SetNodeGroup {
            node: second,
            group: Some(id),
        },
    );
    for node in [l.host, second] {
        let config = &l.sim.hosting.nodes[&node];
        assert_eq!(config.policy.cpu_overcommit_percent, 700);
        assert_eq!(config.policy.swap_mb, 4096);
        assert!(!config.policy.thin_provisioning);
        assert_eq!(config.ipv4_ranges, Some([prefix("203.0.113.0/29")].into()));
        let mapped = &config.networks.as_ref().unwrap()[0];
        assert_eq!(l.sim.port(mapped.port).unwrap().device, node);
        assert_eq!(l.sim.port(mapped.port).unwrap().name, "eth0");
        assert_eq!(mapped.vlan, Some(VlanId(10)));
    }
    assert_ne!(
        l.sim.hosting.nodes[&l.host].networks,
        l.sim.hosting.nodes[&second].networks
    );
    assert_eq!(l.sim.hosting.nodes[&l.host].manager, manager);
    assert_eq!(l.sim.hosting.nodes[&l.host].prepared_at_ms, prepared);

    let mut group = l.sim.hosting.node_groups[&id].clone();
    group.policy.cpu_overcommit_percent = 200;
    group.policy.swap_mb = 1024;
    group.networks = None;
    group.ipv4_ranges = None;
    apply(&mut l.sim, HostingCommand::SaveNodeGroup(group));
    for node in [l.host, second] {
        assert_eq!(
            l.sim.hosting.nodes[&node].policy.cpu_overcommit_percent,
            200
        );
        assert_eq!(l.sim.hosting.nodes[&node].policy.swap_mb, 1024);
        assert!(l.sim.hosting.nodes[&node].networks.is_none());
        assert!(l.sim.hosting.nodes[&node].ipv4_ranges.is_none());
    }
}

#[test]
fn invalid_group_edits_reject_atomically_even_without_members() {
    let mut l = Lab::new();
    let id = save_group(
        &mut l.sim,
        HostingNodeGroup {
            name: "Valid".into(),
            nodes: [l.host].into(),
            ..Default::default()
        },
    );
    let original = l.sim.hosting.node_groups[&id].clone();
    let mut invalid = Vec::new();
    let mut add = |change: fn(&mut HostingNodeGroup)| {
        let mut group = original.clone();
        group.policy.swap_mb = 9999;
        change(&mut group);
        invalid.push(group);
    };
    add(|g| g.name = " ".into());
    add(|g| g.name = "x".repeat(101));
    add(|g| g.policy.cpu_overcommit_percent = 99);
    add(|g| g.policy.network_overcommit_percent = 10001);
    add(|g| g.nodes.insert(DeviceId(u64::MAX)).then_some(()).unwrap());
    add(|g| g.ipv4_ranges = Some([prefix("198.51.100.0/24")].into()));
    add(|g| g.ipv4_ranges = Some([prefix("203.0.113.0/31")].into()));
    add(|g| {
        let mut n = network();
        n.interface = "missing".into();
        g.networks = Some(vec![n]);
    });
    add(|g| {
        let mut n = network();
        n.vlan = Some(VlanId(0));
        g.nodes.clear();
        g.networks = Some(vec![n]);
    });
    add(|g| {
        let mut n = network();
        n.vlan = Some(VlanId(4095));
        g.nodes.clear();
        g.networks = Some(vec![n]);
    });
    add(|g| {
        let mut n = network();
        n.interface = "mgmt0".into();
        g.nodes.clear();
        g.networks = Some(vec![n]);
    });
    add(|g| {
        let mut n = network();
        n.subnet = prefix("198.51.100.0/29");
        g.nodes.clear();
        g.networks = Some(vec![n]);
    });
    add(|g| {
        let mut n = network();
        n.subnet = prefix("203.0.113.0/30");
        g.nodes.clear();
        g.networks = Some(vec![network(), n]);
    });
    let mut provider_member = original.clone();
    provider_member.nodes.insert(l.controller);
    invalid.push(provider_member);
    for group in invalid {
        let before = ron::to_string(&l.sim).unwrap();
        assert!(
            l.sim
                .configure_hosting(HostingCommand::SaveNodeGroup(group))
                .is_err()
        );
        assert_eq!(
            ron::to_string(&l.sim).unwrap(),
            before,
            "Invalid group saves must have no side effects"
        );
    }
}

#[test]
fn grouped_nodes_reject_individual_overrides_and_can_move_or_leave() {
    let mut l = Lab::new();
    let first = save_group(
        &mut l.sim,
        HostingNodeGroup {
            name: "First".into(),
            nodes: [l.host].into(),
            ..Default::default()
        },
    );
    let second = save_group(
        &mut l.sim,
        HostingNodeGroup {
            name: "Second".into(),
            policy: HypervisorPolicy {
                cpu_overcommit_percent: 800,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    for command in [
        HostingCommand::SetNodePolicy {
            node: l.host,
            policy: HypervisorPolicy::default(),
        },
        HostingCommand::SetNodeIpv4Ranges {
            node: l.host,
            ranges: None,
        },
        HostingCommand::SetNodeNetworks {
            node: l.host,
            networks: None,
        },
    ] {
        assert!(
            l.sim
                .configure_hosting(command)
                .unwrap_err()
                .contains("group")
        );
    }
    let mut conflicting = l.sim.hosting.node_groups[&second].clone();
    conflicting.nodes.insert(l.host);
    assert!(
        l.sim
            .configure_hosting(HostingCommand::SaveNodeGroup(conflicting))
            .is_err()
    );
    apply(
        &mut l.sim,
        HostingCommand::SetNodeGroup {
            node: l.host,
            group: Some(second),
        },
    );
    assert!(!l.sim.hosting.node_groups[&first].nodes.contains(&l.host));
    assert_eq!(l.sim.hosting_node_group(l.host).unwrap().id, second);
    assert_eq!(
        l.sim.hosting.nodes[&l.host].policy.cpu_overcommit_percent,
        800
    );
    apply(
        &mut l.sim,
        HostingCommand::SetNodeGroup {
            node: l.host,
            group: None,
        },
    );
    assert!(l.sim.hosting_node_group(l.host).is_none());
    assert_eq!(
        l.sim.hosting.nodes[&l.host].policy.cpu_overcommit_percent,
        800
    );
    apply(
        &mut l.sim,
        HostingCommand::SetNodePolicy {
            node: l.host,
            policy: HypervisorPolicy::default(),
        },
    );
}

#[test]
fn group_only_product_placement_tracks_membership_and_preserves_paid_guests() {
    let mut l = vlan_hosting_lab();
    l.prepare();
    let id = save_group(
        &mut l.sim,
        HostingNodeGroup {
            name: "Public VLAN".into(),
            nodes: [l.host].into(),
            networks: Some(vec![network()]),
            ..Default::default()
        },
    );
    let mut product = l.sim.hosting.products[&l.plan].clone();
    product.eligible_nodes.clear();
    product.eligible_groups.insert(id);
    product.ipv4_ranges = [prefix("203.0.113.0/29")].into();
    apply(&mut l.sim, HostingCommand::SaveProduct(product.clone()));
    let service = l.buy(1);
    let vm = l.vm(service);
    let addresses = l.sim.hosting.vms[&vm].ips.clone();
    assert_eq!(l.sim.hosting.vms[&vm].host, l.host);
    assert!(l.sim.hosting_product_nodes(&product).contains(&l.host));
    product.eligible_nodes.insert(l.host);
    assert_eq!(
        l.sim.hosting_product_nodes(&product).len(),
        1,
        "Group and explicit selection must deduplicate"
    );
    apply(
        &mut l.sim,
        HostingCommand::SetNodeGroup {
            node: l.host,
            group: None,
        },
    );
    assert!(l.sim.hosting_availability(l.plan).is_err());
    assert_eq!(l.sim.hosting.vms[&vm].ips, addresses);
    assert!(l.sim.ping_from_internet(addresses[0]).reachable);
    apply(
        &mut l.sim,
        HostingCommand::SetNodeGroup {
            node: l.host,
            group: Some(id),
        },
    );
    assert!(l.sim.hosting_availability(l.plan).is_ok());
    apply(&mut l.sim, HostingCommand::DestroyServiceVm(service));
    apply(&mut l.sim, HostingCommand::DeleteNodeGroup(id));
    assert!(l.sim.hosting.products[&l.plan].eligible_groups.is_empty());
    assert!(l.sim.hosting_availability(l.plan).is_err());
    apply(&mut l.sim, HostingCommand::RecreateServiceVm(service));
    assert_eq!(l.sim.hosting.vms[&l.vm(service)].ips, addresses);
}

#[test]
fn incompatible_membership_move_keeps_the_original_group_and_settings() {
    let mut l = Lab::new();
    let first = save_group(
        &mut l.sim,
        HostingNodeGroup {
            name: "Original".into(),
            nodes: [l.host].into(),
            ..Default::default()
        },
    );
    let mut missing = network();
    missing.interface = "eth99".into();
    let second = save_group(
        &mut l.sim,
        HostingNodeGroup {
            name: "Different hardware".into(),
            networks: Some(vec![missing]),
            ..Default::default()
        },
    );
    let before = ron::to_string(&l.sim).unwrap();
    assert!(
        l.sim
            .configure_hosting(HostingCommand::SetNodeGroup {
                node: l.host,
                group: Some(second)
            })
            .is_err()
    );
    assert_eq!(ron::to_string(&l.sim).unwrap(), before);
    assert_eq!(l.sim.hosting_node_group(l.host).unwrap().id, first);
}

#[test]
fn groups_survive_saves_and_legacy_saves_default_to_ungrouped_nodes() {
    let mut l = Lab::new();
    let id = save_group(
        &mut l.sim,
        HostingNodeGroup {
            name: "Saved group".into(),
            nodes: [l.host].into(),
            ..Default::default()
        },
    );
    let mut product = l.sim.hosting.products[&l.plan].clone();
    product.eligible_groups.insert(id);
    apply(&mut l.sim, HostingCommand::SaveProduct(product));
    let saved = ron::to_string(&l.sim).unwrap();
    let restored: NetworkSim = ron::from_str(&saved).unwrap();
    assert_eq!(restored.hosting.node_groups[&id].name, "Saved group");
    assert_eq!(
        restored.hosting.node_groups[&id].nodes,
        BTreeSet::from([l.host])
    );
    assert_eq!(
        restored.hosting.products[&l.plan].eligible_groups,
        BTreeSet::from([id])
    );
    let mut old = serde_json::to_value(&l.sim.hosting).unwrap();
    old.as_object_mut().unwrap().remove("node_groups");
    for product in old["products"].as_object_mut().unwrap().values_mut() {
        product.as_object_mut().unwrap().remove("eligible_groups");
    }
    let restored: HostingState = serde_json::from_value(old).unwrap();
    assert!(restored.node_groups.is_empty());
    assert!(restored.products[&l.plan].eligible_groups.is_empty());
    assert!(restored.products[&l.plan].eligible_nodes.contains(&l.host));
    assert_eq!(restored.nodes[&l.host].policy, HypervisorPolicy::default());
}

#[test]
fn selling_a_member_removes_it_from_group_placement() {
    let mut l = Lab::new();
    let spare = spare_node(&mut l.sim);
    let id = save_group(
        &mut l.sim,
        HostingNodeGroup {
            name: "Inventory".into(),
            nodes: [spare].into(),
            ..Default::default()
        },
    );
    l.sim
        .execute(Command::SellDevice { device: spare })
        .unwrap();
    assert!(l.sim.hosting.node_groups[&id].nodes.is_empty());
}

#[test]
fn adding_a_ready_member_enables_group_orders_and_automatic_public_networking() {
    let mut l = Lab::new();
    l.prepare();
    let id = save_group(
        &mut l.sim,
        HostingNodeGroup {
            name: "Growing fleet".into(),
            nodes: [l.host].into(),
            ..Default::default()
        },
    );
    let mut product = l.sim.hosting.products[&l.plan].clone();
    product.eligible_nodes.clear();
    product.eligible_groups.insert(id);
    apply(&mut l.sim, HostingCommand::SaveProduct(product.clone()));
    let first = l.buy(1);
    let first_vm = l.vm(first);
    let first_ips = l.sim.hosting.vms[&first_vm].ips.clone();
    let second = spare_node(&mut l.sim);
    power(&mut l.sim, second, 6);
    let ports = l.sim.device(second).unwrap().ports().to_vec();
    let data = ports[0];
    let mgmt = *ports
        .iter()
        .find(|p| l.sim.port(**p).unwrap().name == "eth1")
        .unwrap();
    l.sim
        .execute(Command::BuyCableSupply {
            supply: CableSupply::Rj45Pack20,
        })
        .unwrap();
    for (source, target) in [(l.data, data), (l.mgmt, mgmt)] {
        let peer = l.sim.link_for_port(source).unwrap().other(source).unwrap();
        let switch = l.sim.port(peer).unwrap().device;
        let switch_port = l.sim.device(switch).unwrap().ports()[3];
        l.sim
            .execute(Command::Connect {
                a: target,
                b: switch_port,
            })
            .unwrap();
    }
    l.sim
        .execute(Command::SetIpv4 {
            port: mgmt,
            config: Ipv4InterfaceConfig::new(ip("10.0.0.3"), 24, None, VlanId(1)),
        })
        .unwrap();
    l.sim.prepare_hosting_node(second, ip("10.0.0.10")).unwrap();
    apply(
        &mut l.sim,
        HostingCommand::SetNodeGroup {
            node: second,
            group: Some(id),
        },
    );
    let PortConfig::Server(config) = &l.sim.port(data).unwrap().config else {
        panic!()
    };
    assert!(
        config
            .addresses()
            .any(|a| a.gateway == Some(ip("203.0.113.1")))
    );
    assert_eq!(
        l.sim.hosting_product_nodes(&product),
        BTreeSet::from([l.host, second])
    );
    let order = l.buy(2);
    assert_eq!(l.sim.hosting.vms[&l.vm(order)].host, second);
    assert_eq!(l.sim.hosting.vms[&first_vm].ips, first_ips);
    assert_eq!(
        l.sim.hosting.products[&l.plan].eligible_nodes.len(),
        0,
        "Group joins must not expand the saved explicit node list"
    );
    l.tick();
    assert!(l.sim.hosting.vms[&l.vm(order)].metrics.reachable);
}

#[test]
fn editing_group_overselling_changes_admission_on_its_members() {
    let mut l = Lab::new();
    l.prepare();
    let ram = l.sim.hosting_capacity(l.host).ram_mb as u64;
    let id = save_group(
        &mut l.sim,
        HostingNodeGroup {
            name: "Admission".into(),
            nodes: [l.host].into(),
            policy: HypervisorPolicy {
                reserved_ram_mb: ram - 1024,
                ram_overcommit_percent: 100,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(
        l.sim
            .hosting_availability(l.plan)
            .unwrap_err()
            .contains("RAM")
    );
    let mut group = l.sim.hosting.node_groups[&id].clone();
    group.policy.ram_overcommit_percent = 200;
    apply(&mut l.sim, HostingCommand::SaveNodeGroup(group));
    assert_eq!(l.sim.hosting_availability(l.plan).unwrap(), l.host);
    l.buy(1);
    assert!(l.sim.hosting_availability(l.plan).is_err());
}

#[test]
fn product_groups_must_exist_and_supply_members_before_publication() {
    let mut l = Lab::new();
    let mut product = l.sim.hosting.products[&l.plan].clone();
    product.eligible_groups.insert(u64::MAX);
    assert!(l.sim.validate_hosting_product(&product, false).is_err());
    let id = save_group(
        &mut l.sim,
        HostingNodeGroup {
            name: "Empty".into(),
            ..Default::default()
        },
    );
    product.eligible_groups = [id].into();
    product.eligible_nodes.clear();
    product.published = false;
    apply(&mut l.sim, HostingCommand::SaveProduct(product));
    assert!(
        l.sim
            .configure_hosting(HostingCommand::Publish {
                product: l.plan,
                published: true
            })
            .is_err()
    );
}
