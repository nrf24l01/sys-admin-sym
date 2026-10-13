use super::*;

#[test]
fn first_vm_guide_empty_world_has_no_completed_tasks() {
    let report = NetworkSim::new().first_vm_guide(None, None);
    assert!(report.checks.iter().all(|c| !c.complete));
    assert!(!report.sold);
    assert_eq!(report.controller, None);
    assert_eq!(report.node, None);
}

#[test]
fn first_vm_guide_checks_existing_setup_and_regresses_when_management_breaks() {
    let mut lab = Lab::new();
    lab.prepare();
    let report = lab.sim.first_vm_guide(Some(lab.controller), Some(lab.host));
    for step in [
        FirstVmStep::Switch,
        FirstVmStep::Servers,
        FirstVmStep::Power,
        FirstVmStep::ManagementCables,
        FirstVmStep::ManagementAddresses,
        FirstVmStep::ManagementPing,
        FirstVmStep::ControllerService,
        FirstVmStep::NodePrepared,
        FirstVmStep::PublicRange,
        FirstVmStep::DataNetwork,
        FirstVmStep::Storefront,
        FirstVmStep::Product,
    ] {
        assert!(
            report.check(step).complete,
            "{step:?}: {:?}",
            report.check(step)
        );
    }
    // The lab uses VLAN 1; the explicit management VLAN lesson remains pending.
    assert!(!report.check(FirstVmStep::ManagementVlan).complete);
    let cable = lab.sim.link_for_port(lab.mgmt).unwrap().id;
    lab.sim
        .execute(Command::Disconnect { link: cable })
        .unwrap();
    let report = lab.sim.first_vm_guide(Some(lab.controller), Some(lab.host));
    for step in [
        FirstVmStep::ManagementCables,
        FirstVmStep::ManagementPing,
        FirstVmStep::NodePrepared,
    ] {
        assert!(!report.check(step).complete, "{step:?}");
    }
    assert!(!report.sold);
}

#[test]
fn first_vm_guide_does_not_mix_an_unrelated_controller_with_a_registered_node() {
    let mut lab = Lab::new();
    lab.prepare();
    let report = lab.sim.first_vm_guide(Some(lab.host), Some(lab.controller));
    assert!(!report.check(FirstVmStep::ControllerService).complete);
    assert!(!report.check(FirstVmStep::NodePrepared).complete);
    assert!(!report.check(FirstVmStep::Storefront).complete);
    assert!(!report.check(FirstVmStep::Product).complete);
    let report = lab
        .sim
        .first_vm_guide(Some(lab.controller), Some(lab.controller));
    assert_ne!(report.controller, report.node);
}

#[test]
fn first_vm_guide_paid_sale_survives_termination_product_removal_and_save() {
    let mut lab = Lab::new();
    lab.prepare();
    let service = lab.buy(777);
    assert!(lab.sim.first_vm_guide(None, None).sold);
    apply(&mut lab.sim, HostingCommand::TerminateService(service));
    apply(&mut lab.sim, HostingCommand::DeleteProduct(lab.plan));
    let saved = ron::to_string(&lab.sim).unwrap();
    let restored: NetworkSim = ron::from_str(&saved).unwrap();
    assert!(restored.first_vm_guide(None, None).sold);
    assert!(
        restored
            .first_vm_guide(None, None)
            .check(FirstVmStep::FirstSale)
            .complete
    );
}

#[test]
fn first_vm_guide_free_and_dedicated_sales_do_not_complete_the_vm_milestone() {
    let mut lab = Lab::new();
    lab.prepare();
    lab.buy(1);
    lab.sim.hosting.payments[0].amount_cents = 0;
    assert!(!lab.sim.first_vm_guide(None, None).sold);
    let mut lab = Lab::new();
    lab.prepare();
    lab.sim
        .hosting
        .products
        .get_mut(&lab.plan)
        .unwrap()
        .category = HostingCategory::DedicatedServer;
    lab.buy(2);
    assert!(!lab.sim.first_vm_guide(None, None).sold);
}

#[test]
fn first_vm_guide_management_vlan_requires_both_switch_ports_and_valid_addresses() {
    let mut lab = Lab::new();
    lab.prepare();
    let switch_port = |sim: &NetworkSim, port| *sim.link_status(port).path.last().unwrap();
    let a = switch_port(&lab.sim, lab.mgmt);
    let b = switch_port(&lab.sim, lab.controller_management);
    let switch = lab.sim.port(a).unwrap().device;
    lab.sim
        .execute(Command::CreateVlan {
            switch,
            vlan: Vlan {
                id: VlanId(10),
                name: "Management".into(),
            },
        })
        .unwrap();
    for port in [a, b] {
        lab.sim
            .execute(Command::SetSwitchPortMode {
                port,
                mode: SwitchPortMode::Access {
                    vlan: Some(VlanId(10)),
                },
            })
            .unwrap();
    }
    for (port, address) in [
        (lab.mgmt, "10.0.0.2"),
        (lab.controller_management, "10.0.0.10"),
    ] {
        lab.sim
            .execute(Command::SetIpv4 {
                port,
                config: Ipv4InterfaceConfig::new(ip(address), 24, None, VlanId(10)),
            })
            .unwrap();
    }
    let report = lab.sim.first_vm_guide(Some(lab.controller), Some(lab.host));
    assert!(report.check(FirstVmStep::ManagementVlan).complete);
    assert!(report.check(FirstVmStep::ManagementPing).complete);
    lab.sim
        .execute(Command::SetSwitchPortMode {
            port: b,
            mode: SwitchPortMode::Access {
                vlan: Some(VlanId(1)),
            },
        })
        .unwrap();
    let report = lab.sim.first_vm_guide(Some(lab.controller), Some(lab.host));
    assert!(!report.check(FirstVmStep::ManagementVlan).complete);
    assert!(!report.check(FirstVmStep::ManagementPing).complete);
    lab.sim
        .execute(Command::SetIpv4 {
            port: lab.mgmt,
            config: Ipv4InterfaceConfig::new(ip("10.9.0.2"), 24, None, VlanId(1)),
        })
        .unwrap();
    assert!(
        !lab.sim
            .first_vm_guide(Some(lab.controller), Some(lab.host))
            .check(FirstVmStep::ManagementAddresses)
            .complete
    );
}

#[test]
fn first_vm_guide_preparation_can_complete_before_the_public_storefront_is_connected() {
    let mut lab = Lab::new();
    lab.prepare();
    apply(&mut lab.sim, HostingCommand::SetProviderSiteAddress(None));
    let report = lab.sim.first_vm_guide(Some(lab.controller), Some(lab.host));
    assert!(report.check(FirstVmStep::NodePrepared).complete);
    assert!(!report.check(FirstVmStep::Storefront).complete);
}

#[test]
fn first_vm_guide_console_walkthrough_reaches_a_paid_customer_vm() {
    let mut sim = NetworkSim::new();
    sim.money = 100000;
    for supply in [CableSupply::CableBox305m, CableSupply::Rj45Pack20] {
        sim.execute(Command::BuyCableSupply { supply }).unwrap();
    }
    let mut buy = |command, unit| {
        let SimEvent::DeviceAdded(id) = sim.execute(command).unwrap()[0] else {
            panic!()
        };
        power(&mut sim, id, unit);
        id
    };
    let controller = buy(Command::BuyServerFullPack, 1);
    let node = buy(Command::BuyServerFullPack, 2);
    let switch = buy(
        Command::BuyDevice {
            kind: DeviceTemplate::Switch,
        },
        3,
    );
    let router = buy(
        Command::BuyDevice {
            kind: DeviceTemplate::Router,
        },
        4,
    );
    let port = |sim: &NetworkSim, device, name: &str| {
        *sim.device(device)
            .unwrap()
            .ports()
            .iter()
            .find(|p| sim.port(**p).unwrap().name == name)
            .unwrap()
    };
    let cp = port(&sim, controller, "eth0");
    let cm = port(&sim, controller, "eth1");
    let np = port(&sim, node, "eth0");
    let nm = port(&sim, node, "eth1");
    let sw = sim.device(switch).unwrap().ports().to_vec();
    let rp = sim.device(router).unwrap().ports().to_vec();
    let transit = sim
        .network_outlets()
        .filter(|o| matches!(o.kind, NetworkOutletKind::Uplink { .. }))
        .min_by_key(|o| o.port)
        .unwrap()
        .port;
    for (a, b) in [
        (cm, sw[0]),
        (nm, sw[1]),
        (cp, sw[2]),
        (np, sw[3]),
        (rp[8], sw[4]),
        (rp[0], transit),
    ] {
        sim.execute(Command::Connect { a, b }).unwrap();
    }
    let commands = |article: &str, index: usize| {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/wiki/en/tutorial")
                .join(format!("{article}.md")),
        )
        .unwrap();
        source
            .split("```")
            .skip(1)
            .step_by(2)
            .nth(index)
            .unwrap()
            .split_once('\n')
            .unwrap()
            .1
            .to_owned()
    };
    let run = |sim: &mut NetworkSim, device, commands: &str| {
        for command in commands.lines().filter(|line| !line.is_empty()) {
            let reply = sim.execute_console(device, command);
            assert!(reply.success, "{command}: {:?}", reply.lines);
        }
    };
    run(&mut sim, switch, &commands("management-vlan", 0));
    run(&mut sim, controller, &commands("management-addresses", 0));
    run(&mut sim, node, &commands("management-addresses", 1));
    run(&mut sim, controller, &commands("hosting-control", 0));
    run(&mut sim, node, &commands("management-ping", 0));
    run(&mut sim, controller, &commands("management-ping", 1));
    run(&mut sim, node, &commands("hosting-control", 1));
    assert!(
        sim.first_vm_guide(Some(controller), Some(node))
            .check(FirstVmStep::NodePrepared)
            .complete
    );
    sim.buy_public_ipv4_pool().unwrap();
    sim.execute(Command::Provider(ProviderCommand::RouteOwnedRange {
        prefix: prefix("203.0.113.0/29"),
        uplink: Some(transit),
    }))
    .unwrap();
    run(&mut sim, switch, &commands("data-network", 0));
    run(&mut sim, router, &commands("data-network", 1));
    apply(
        &mut sim,
        HostingCommand::SetNodeNetworks {
            node,
            networks: Some(vec![HostingNodeNetwork {
                port: np,
                subnet: prefix("203.0.113.0/29"),
                vlan: Some(VlanId(20)),
            }]),
        },
    );
    run(&mut sim, controller, &commands("storefront", 0));
    apply(
        &mut sim,
        HostingCommand::SetProviderSiteAddress(Some(ip("203.0.113.2"))),
    );
    apply(
        &mut sim,
        HostingCommand::SaveProduct(HostingProduct {
            vcpu: 1,
            ram_mb: 1024,
            disk_gb: 10,
            eligible_nodes: [node].into(),
            ipv4_ranges: [prefix("203.0.113.0/29")].into(),
            ..Default::default()
        }),
    );
    let product = *sim.hosting.products.keys().next().unwrap();
    apply(
        &mut sim,
        HostingCommand::Publish {
            product,
            published: true,
        },
    );
    let report = sim.first_vm_guide(Some(controller), Some(node));
    for check in report
        .checks
        .iter()
        .filter(|c| c.step != FirstVmStep::FirstSale)
    {
        assert!(check.complete, "{check:?}");
    }
    assert!(!report.sold);
    sim.hosting.market.time_scale = 1;
    sim.hosting.market.prospects_per_day = 5000;
    for _ in 0..48 {
        sim.advance_time(HOSTING_TICK_MS);
        if sim.first_vm_guide(None, None).sold {
            break;
        }
    }
    assert!(sim.first_vm_guide(None, None).sold);
    assert!(sim.hosting.services.values().any(|s| s.vm.is_some()));
}

#[test]
fn first_vm_guide_does_not_change_saved_state_while_probing() {
    let mut lab = Lab::new();
    lab.prepare();
    let before = ron::to_string(&lab.sim).unwrap();
    for _ in 0..3 {
        let report = lab.sim.first_vm_guide(None, None);
        assert!(report.check(FirstVmStep::ManagementPing).complete);
    }
    assert_eq!(ron::to_string(&lab.sim).unwrap(), before);
}

#[test]
fn first_vm_guide_power_completes_before_management_cabling() {
    let mut sim = NetworkSim::new();
    sim.money = 100000;
    let mut ids = Vec::new();
    for (unit, command) in [
        (1, Command::BuyServerFullPack),
        (2, Command::BuyServerFullPack),
        (
            3,
            Command::BuyDevice {
                kind: DeviceTemplate::Switch,
            },
        ),
    ] {
        let SimEvent::DeviceAdded(id) = sim.execute(command).unwrap()[0] else {
            panic!()
        };
        power(&mut sim, id, unit);
        ids.push(id);
    }
    let report = sim.first_vm_guide(Some(ids[0]), Some(ids[1]));
    assert!(report.check(FirstVmStep::Power).complete);
    assert!(!report.check(FirstVmStep::ManagementCables).complete);
}

#[test]
fn first_vm_guide_registration_does_not_follow_private_ip_to_another_controller() {
    let mut lab = Lab::new();
    lab.prepare();
    let SimEvent::DeviceAdded(replacement) =
        lab.sim.execute(Command::BuyServerFullPack).unwrap()[0]
    else {
        panic!()
    };
    power(&mut lab.sim, replacement, 6);
    let replacement_management = *lab
        .sim
        .device(replacement)
        .unwrap()
        .ports()
        .iter()
        .find(|id| lab.sim.port(**id).unwrap().name == "eth1")
        .unwrap();
    let peer = *lab.sim.link_status(lab.mgmt).path.last().unwrap();
    let switch = lab.sim.port(peer).unwrap().device;
    let spare = lab.sim.device(switch).unwrap().ports()[3];
    assert!(
        lab.sim
            .execute_console(lab.controller, "ip addr flush dev eth1")
            .success
    );
    lab.sim
        .execute(Command::SetIpv4 {
            port: replacement_management,
            config: Ipv4InterfaceConfig::new(ip("10.0.0.10"), 24, None, VlanId(1)),
        })
        .unwrap();
    lab.sim
        .execute(Command::Connect {
            a: replacement_management,
            b: spare,
        })
        .unwrap();
    assert!(
        lab.sim
            .execute_console(replacement, "./setup-main-hoster-server.sh")
            .success
    );
    assert!(lab.sim.ping(lab.mgmt, ip("10.0.0.10")).reachable);
    assert!(!lab.sim.hosting_management_connected(lab.host));
    let report = lab.sim.first_vm_guide(Some(lab.controller), Some(lab.host));
    assert!(!report.check(FirstVmStep::NodePrepared).complete);
}

#[test]
fn first_vm_guide_finds_management_on_secondary_private_addresses() {
    let mut lab = Lab::new();
    lab.prepare();
    assert!(
        lab.sim
            .execute_console(lab.controller, "ip addr flush dev eth1")
            .success
    );
    assert!(
        lab.sim
            .execute_console(lab.controller, "ip addr add 10.9.0.10/24 dev eth1")
            .success
    );
    assert!(
        lab.sim
            .execute_console(lab.controller, "ip addr add 10.0.0.10/24 dev eth1")
            .success
    );
    assert!(lab.sim.hosting_management_connected(lab.host));
    let report = lab.sim.first_vm_guide(Some(lab.controller), Some(lab.host));
    assert!(report.check(FirstVmStep::ManagementAddresses).complete);
    assert!(report.check(FirstVmStep::ManagementPing).complete);
}
