use cloud_provider_sim::{
    CableSupply, Command, DeviceKind, DeviceTemplate, NetworkSim, RackId, ServerPartKind, SimError,
    SimEvent, server_catalog,
};

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
    install(&mut sim, id, "r360_psu_600w", None);
    let DeviceKind::Server(server) = &sim.device(id).unwrap().kind else {
        panic!()
    };
    assert!(server.hardware.as_ref().unwrap().ready());
    assert_eq!(sim.power.device_status(id).unwrap().load.watts, 160);
    install(&mut sim, id, "intel_i350_t4", Some(1));
    let DeviceKind::Server(server) = &sim.device(id).unwrap().kind else {
        panic!()
    };
    assert_eq!(server.ports.len(), onboard + 4);
    assert_eq!(sim.power.device_status(id).unwrap().load.watts, 165);
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
    assert_eq!(loaded.power.device_status(id).unwrap().load.watts, 160);
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
