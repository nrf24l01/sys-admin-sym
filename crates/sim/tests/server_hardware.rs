use cloud_provider_sim::{
    CableSupply, Command, DeviceKind, DeviceTemplate, NetworkSim, OutletId, PowerEndpoint, RackId,
    ServerFullPack, ServerPartKind, SimError, SimEvent, SourceId, server_catalog,
};

#[test]
fn full_pack_order_installs_every_component_and_charges_once() {
    let mut sim = NetworkSim::new();
    let before = sim.money;
    let SimEvent::DeviceAdded(id) = sim.execute(Command::BuyServerFullPack).unwrap()[0] else {
        panic!("full pack did not create a server");
    };
    assert_eq!(sim.money, before - ServerFullPack::price());
    let DeviceKind::Server(server) = &sim.device(id).unwrap().kind else {
        panic!("full pack did not create a server");
    };
    let hardware = server.hardware.as_ref().unwrap();
    assert!(hardware.ready());
    assert_eq!(hardware.cpus, [ServerFullPack::CPU]);
    assert_eq!(hardware.ram, [ServerFullPack::RAM]);
    assert_eq!(
        hardware.pcie.iter().flatten().next().unwrap(),
        ServerFullPack::NIC
    );
    assert_eq!(
        hardware.drives.iter().flatten().next().unwrap(),
        ServerFullPack::DRIVE
    );
    assert!(hardware.power_supplies.is_empty());
    assert_eq!(server.ports.len(), 6);
}

#[test]
fn unaffordable_full_pack_does_not_change_inventory_or_money() {
    let mut sim = NetworkSim::new();
    sim.money = ServerFullPack::price() - 1;
    assert!(matches!(
        sim.execute(Command::BuyServerFullPack),
        Err(SimError::InsufficientFunds { .. })
    ));
    assert_eq!(sim.money, ServerFullPack::price() - 1);
    assert_eq!(sim.devices().count(), 0);
    assert!(sim.server_parts.is_empty());
}

#[test]
fn power_supply_is_included_and_not_sold_separately() {
    let mut sim = NetworkSim::new();
    assert!(
        server_catalog()
            .parts
            .iter()
            .all(|part| !matches!(part.kind, ServerPartKind::PowerSupply { .. }))
    );
    assert!(matches!(
        sim.execute(Command::BuyServerPart {
            part_id: "r360_psu_600w".into()
        }),
        Err(SimError::UnknownServerPart(_))
    ));
}

fn chassis(sim: &mut NetworkSim) -> cloud_provider_sim::DeviceId {
    let SimEvent::DeviceAdded(id) = sim.execute(Command::BuyServerChassis).unwrap()[0] else {
        panic!("server not bought")
    };
    id
}

fn install(
    sim: &mut NetworkSim,
    device: cloud_provider_sim::DeviceId,
    part_id: &str,
    slot: Option<usize>,
) {
    sim.execute(Command::BuyServerPart {
        part_id: part_id.into(),
    })
    .unwrap();
    sim.execute(Command::InstallServerPart {
        device,
        part_id: part_id.into(),
        slot,
    })
    .unwrap();
}

#[test]
fn unplugged_pcie_interfaces_report_no_carrier_in_linux_commands() {
    let mut sim = NetworkSim::new();
    let id = chassis(&mut sim);
    install(&mut sim, id, "xeon_e_2434", None);
    install(&mut sim, id, "ddr5_ecc_16gb", None);
    install(&mut sim, id, "intel_i350_t4", Some(1));
    sim.execute(Command::PlaceDevice {
        device: id,
        rack: RackId(1),
        unit: 1,
    })
    .unwrap();
    sim.execute(Command::ConnectPower {
        outlet: OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 0,
        },
        endpoint: PowerEndpoint::Device(id),
    })
    .unwrap();
    sim.execute(Command::SetPower {
        device: id,
        powered: true,
    })
    .unwrap();
    let DeviceKind::Server(server) = &sim.device(id).unwrap().kind else {
        panic!()
    };
    let card_names: Vec<_> = server.hardware.as_ref().unwrap().card_ports[1]
        .iter()
        .map(|port| sim.port(*port).unwrap().name.clone())
        .collect();
    assert_eq!(card_names.len(), 4);
    for name in &card_names {
        let addr = sim.execute_console(id, "ip a");
        let line = addr
            .lines
            .iter()
            .find(|line| line.contains(&format!(": {name}:")))
            .unwrap();
        assert!(
            line.contains("UP,NO-CARRIER>") && line.contains("state DOWN"),
            "{line}"
        );
        let summary = sim.execute_console(id, "ip -br addr");
        assert!(
            summary
                .lines
                .iter()
                .any(|line| line.starts_with(name) && line.contains("DOWN"))
        );
        let link = sim.execute_console(id, &format!("ip link show dev {name}"));
        assert!(link.lines[0].contains("UP,NO-CARRIER>") && link.lines[0].contains("state DOWN"));
        let ethtool = sim.execute_console(id, &format!("ethtool {name}"));
        assert!(
            ethtool
                .lines
                .iter()
                .any(|line| line.contains("Link detected: no"))
        );
    }
    assert!(
        sim.execute_console(id, &format!("ip link set dev {} down", card_names[0]))
            .success
    );
    assert!(sim.execute_console(id, "ip a").lines.iter().any(|line| {
        line.contains(&format!(": {}: <DOWN>", card_names[0])) && line.contains("state DOWN")
    }));
}

#[test]
fn bare_chassis_requires_parts_and_installed_nic_adds_real_ports() {
    let mut sim = NetworkSim::new();
    let id = chassis(&mut sim);
    assert!(matches!(
        sim.execute(Command::SetPower {
            device: id,
            powered: true
        }),
        Err(SimError::ServerHardware(_))
    ));
    let onboard = sim.device(id).unwrap().ports().len();
    install(&mut sim, id, "xeon_e_2434", None);
    install(&mut sim, id, "ddr5_ecc_16gb", None);
    let DeviceKind::Server(server) = &sim.device(id).unwrap().kind else {
        panic!()
    };
    assert!(server.hardware.as_ref().unwrap().ready());
    let before_nic_dc = sim.power.device_status(id).unwrap().load.watts;
    install(&mut sim, id, "intel_i350_t4", Some(1));
    let DeviceKind::Server(server) = &sim.device(id).unwrap().kind else {
        panic!()
    };
    assert_eq!(server.ports.len(), onboard + 4);
    assert!(sim.power.device_status(id).unwrap().load.watts > before_nic_dc);
    assert_eq!(
        server.hardware.as_ref().unwrap().pcie[1].as_deref(),
        Some("intel_i350_t4")
    );
    for port in &server.ports[onboard..] {
        assert!(sim.port(*port).is_some());
    }
    let slot = &server_catalog().chassis.pcie_slots[1];
    for (index, port) in server.ports[onboard..].iter().enumerate() {
        let position = sim
            .device(id)
            .unwrap()
            .kind
            .port_position_normalized(onboard + index);
        assert_eq!(position, slot.port_position(index, 4));
        assert_eq!(
            server.hardware.as_ref().unwrap().card_ports[1][index],
            *port
        );
    }
    let nic_port = server.ports[onboard];
    let SimEvent::DeviceAdded(switch) = sim
        .execute(Command::BuyDevice {
            kind: DeviceTemplate::Switch,
        })
        .unwrap()[0]
    else {
        panic!()
    };
    sim.execute(Command::PlaceDevice {
        device: id,
        rack: RackId(1),
        unit: 1,
    })
    .unwrap();
    sim.execute(Command::PlaceDevice {
        device: switch,
        rack: RackId(1),
        unit: 2,
    })
    .unwrap();
    for supply in [CableSupply::CableBox305m, CableSupply::Rj45Pack20] {
        sim.execute(Command::BuyCableSupply { supply }).unwrap();
    }
    sim.execute(Command::Connect {
        a: nic_port,
        b: sim.device(switch).unwrap().ports()[0],
    })
    .unwrap();
    assert!(sim.link_for_port(nic_port).is_some());
    let saved = serde_json::to_string(&sim).unwrap();
    let mut loaded: NetworkSim = serde_json::from_str(&saved).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.device(id).unwrap().ports().len(), onboard + 4);
    loaded
        .execute(Command::RemoveServerPart {
            device: id,
            part_id: "intel_i350_t4".into(),
            slot: Some(1),
        })
        .unwrap();
    assert!(loaded.link_for_port(nic_port).is_none());
    assert!(loaded.port(nic_port).is_none());
    assert_eq!(loaded.device(id).unwrap().ports().len(), onboard);
    assert_eq!(loaded.server_parts["intel_i350_t4"], 1);
    assert_eq!(
        loaded.power.device_status(id).unwrap().load.watts,
        before_nic_dc
    );
}

#[test]
fn pcie_slots_and_cpu_lane_budget_reject_invalid_installations_without_consuming_parts() {
    let mut sim = NetworkSim::new();
    let id = chassis(&mut sim);
    sim.execute(Command::BuyServerPart {
        part_id: "intel_i350_t4".into(),
    })
    .unwrap();
    assert!(matches!(
        sim.execute(Command::InstallServerPart {
            device: id,
            part_id: "intel_i350_t4".into(),
            slot: Some(0)
        }),
        Err(SimError::ServerHardware(_))
    ));
    assert_eq!(sim.server_parts["intel_i350_t4"], 1);
    install(&mut sim, id, "xeon_e_2434", None);
    sim.execute(Command::InstallServerPart {
        device: id,
        part_id: "intel_i350_t4".into(),
        slot: Some(0),
    })
    .unwrap();
    assert_eq!(sim.server_parts["intel_i350_t4"], 0);
    assert!(matches!(
        sim.execute(Command::RemoveServerPart {
            device: id,
            part_id: "xeon_e_2434".into(),
            slot: None
        }),
        Err(SimError::ServerHardware(_))
    ));
    assert!(
        server_catalog()
            .parts
            .iter()
            .any(|part| matches!(part.kind, ServerPartKind::PciCard { .. }))
    );
}

#[test]
fn legacy_server_purchase_keeps_existing_configuration() {
    let mut sim = NetworkSim::new();
    let SimEvent::DeviceAdded(id) = sim
        .execute(Command::BuyDevice {
            kind: DeviceTemplate::Server,
        })
        .unwrap()[0]
    else {
        panic!()
    };
    let DeviceKind::Server(server) = &sim.device(id).unwrap().kind else {
        panic!()
    };
    assert!(server.hardware.is_none());
}

#[test]
fn loading_old_fans_refunds_owned_and_installed_stock_once() {
    let mut sim = NetworkSim::new();
    let id = chassis(&mut sim);
    let before = sim.money;
    let mut saved = serde_json::to_value(&sim).unwrap();
    saved["server_parts"]["r360_fan"] = serde_json::json!(2);
    saved["devices"][id.0.to_string()]["kind"]["Server"]["hardware"]["cooling"] =
        serde_json::json!(["r360_fan"]);
    let mut loaded: NetworkSim = serde_json::from_value(saved).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.money, before + 3 * 45);
    assert!(!loaded.server_parts.contains_key("r360_fan"));
    assert!(!serde_json::to_string(&loaded).unwrap().contains("cooling"));
    loaded.rebuild_indexes();
    assert_eq!(loaded.money, before + 3 * 45);
}

#[test]
fn loading_old_power_supplies_refunds_them_once() {
    let mut sim = NetworkSim::new();
    let id = chassis(&mut sim);
    let before = sim.money;
    let mut saved = serde_json::to_value(&sim).unwrap();
    saved["server_parts"]["r360_psu_600w"] = serde_json::json!(2);
    saved["devices"][id.0.to_string()]["kind"]["Server"]["hardware"]["power_supplies"] =
        serde_json::json!(["r360_psu_600w"]);
    let mut loaded: NetworkSim = serde_json::from_value(saved).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.money, before + 3 * 180);
    assert!(!loaded.server_parts.contains_key("r360_psu_600w"));
    let DeviceKind::Server(server) = &loaded.device(id).unwrap().kind else {
        panic!("server missing");
    };
    assert!(server.hardware.as_ref().unwrap().power_supplies.is_empty());
    loaded.rebuild_indexes();
    assert_eq!(loaded.money, before + 3 * 180);
}
