use serde_json::Value;

#[test]
fn requested_network_devices_have_catalog_records() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/equipment/network_devices.json");
    let source = std::fs::read_to_string(path).expect("network device catalog must exist");
    let catalog: Value = serde_json::from_str(&source).unwrap();
    let models = catalog["models"].as_array().unwrap();
    for name in [
        "C1111-8P",
        "ISR 4321",
        "ISR 4331",
        "ISR 4351",
        "C8200L-1N-4T",
        "C8200-1N-4T",
        "C8300-1N1S-6T",
        "ASR 1001-X",
        "C8500-12X4QC",
        "CBS250-24T-4G",
        "CBS350-24T-4G",
        "C1000-24T-4G-L",
        "C1300-24T-4G",
        "WS-C2960X-24TS-L",
        "C9200L-24T-4G-E",
        "C9300-24T-E",
        "C9500-24Y4C-E",
        "N9K-C93180YC-FX3",
        "N9K-C9364C-GX",
    ] {
        assert!(
            models
                .iter()
                .any(|m| m["name"].as_str().unwrap().contains(name)),
            "missing {name}"
        );
    }
}

use cloud_provider_sim::*;

fn buy(sim: &mut NetworkSim, model_id: &str) -> DeviceId {
    let events = sim
        .execute(Command::BuyNetworkDevice {
            model_id: model_id.into(),
        })
        .unwrap();
    events
        .into_iter()
        .find_map(|event| match event {
            SimEvent::DeviceAdded(id) => Some(id),
            _ => None,
        })
        .unwrap()
}
fn install(sim: &mut NetworkSim, id: DeviceId, unit: u8, outlet: u8) {
    sim.execute(Command::PlaceDevice {
        device: id,
        rack: RackId(1),
        unit,
    })
    .unwrap();
    sim.execute(Command::ConnectPower {
        outlet: OutletId {
            source: SourceId::Rack(RackId(1)),
            index: outlet,
        },
        endpoint: PowerEndpoint::Device(id),
    })
    .unwrap();
}

#[test]
fn isr4321_uses_its_six_pin_socket_and_supplied_adapter() {
    let mut sim = NetworkSim::new();
    sim.money = 1_000_000;
    let id = buy(&mut sim, "isr_4321");
    install(&mut sim, id, 1, 0);
    let device = sim.device(id).unwrap();
    let socket = device.kind.power_connector_socket(0).unwrap();
    assert_eq!(socket.connector_type().key(), "cisco-six-pin");
    assert_eq!(socket.side, RackSide::Front);
    assert!(!socket.can_mate(&ConnectorType::CiscoFourPin));
    assert!(!socket.can_mate(&ConnectorType::IecC14));
    let adapter = device
        .kind
        .network_model()
        .unwrap()
        .adapter
        .as_ref()
        .unwrap();
    assert_eq!(adapter.output_watts, 108);
    assert_eq!(adapter.output_volts, 12);
    assert_eq!(adapter.output_current_ma, 9000);
    assert!(device.powered);
    assert!(sim.power.devices[&id].psus.is_none());
    assert!(sim.power.devices[&id].adapter.is_some());
}

#[test]
fn every_model_purchases_with_its_ports_power_height_and_saved_identity() {
    for model in &network_device_catalog().models {
        let mut sim = NetworkSim::new();
        sim.money = 1_000_000;
        let id = buy(&mut sim, &model.id);
        assert_eq!(sim.money, 1_000_000 - model.price);
        install(&mut sim, id, 1, 0);
        let d = sim.device(id).unwrap();
        assert_eq!(d.kind.network_model().unwrap().id, model.id);
        assert_eq!(d.template(), model.category.template());
        assert_eq!(d.rack_units(), model.rack_units);
        assert_eq!(d.ports().len(), model.ports.len());
        assert!(d.powered, "{} should power on", model.id);
        for (id, spec) in d.ports().iter().zip(&model.ports) {
            let p = sim.port(*id).unwrap();
            assert_eq!(p.max_speed.mbps(), spec.speed_mbps);
            assert_eq!(p.connector, spec.connector);
            assert_eq!(sim.ios_interface_name(d.id, *id), spec.ios_name);
            assert_eq!(sim.cage_profile(*id), spec.cage);
        }
        let before = sim.device_consumption(id).unwrap().demand_mw;
        assert_eq!(before, model.power.idle_mw);
        let save = ron::to_string(&sim).unwrap();
        let mut loaded: NetworkSim = ron::from_str(&save).unwrap();
        loaded.rebuild_indexes();
        assert_eq!(loaded.device(id), sim.device(id));
        assert_eq!(loaded.device_consumption(id).unwrap().demand_mw, before);
        assert_eq!(loaded.power.devices[&id].adapter, model.adapter);
    }
}

#[test]
fn unknown_model_purchases_and_saved_ids_fail_without_spending_money() {
    let mut sim = NetworkSim::new();
    let money = sim.money;
    assert!(
        sim.execute(Command::BuyNetworkDevice {
            model_id: "nonexistent".into()
        })
        .is_err()
    );
    assert_eq!(sim.money, money);
    assert!(serde_json::from_str::<NetworkModelId>("\"nonexistent\"").is_err());
    assert!(serde_json::from_str::<SwitchModel>("\"nonexistent\"").is_err());
    assert!(serde_json::from_str::<SwitchModel>("\"cisco_isr_c1111\"").is_err());
}

#[test]
fn legacy_missing_model_ids_resolve_without_changing_port_identity() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let router = buy(&mut sim, "cisco_isr_c1111");
    let switch = buy(&mut sim, "c1000_24t_4g_l");
    let mut value = serde_json::to_value(&sim).unwrap();
    for device in value["devices"].as_object_mut().unwrap().values_mut() {
        let kind = device["kind"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        kind.as_object_mut().unwrap().remove("model");
    }
    let mut loaded: NetworkSim = serde_json::from_value(value).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(
        loaded.device(router).unwrap().ports(),
        sim.device(router).unwrap().ports()
    );
    assert_eq!(
        loaded.device(switch).unwrap().ports(),
        sim.device(switch).unwrap().ports()
    );
    assert_eq!(
        loaded
            .device(router)
            .unwrap()
            .kind
            .network_model()
            .unwrap()
            .id,
        "cisco_isr_c1111"
    );
    assert_eq!(
        loaded
            .device(switch)
            .unwrap()
            .kind
            .network_model()
            .unwrap()
            .id,
        "c1000_24t_4g_l"
    );
}

#[test]
fn two_unit_switches_block_overlapping_placement_and_use_redundant_feeds() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let large = buy(&mut sim, "n9k_c9364c_gx");
    install(&mut sim, large, 1, 0);
    let small = buy(&mut sim, "cisco_isr_c1111");
    assert!(
        sim.execute(Command::PlaceDevice {
            device: small,
            rack: RackId(1),
            unit: 2
        })
        .is_err()
    );
    let endpoint = PowerEndpoint::DevicePsu {
        device: large,
        inlet: 1,
    };
    sim.execute(Command::ConnectPower {
        outlet: OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 1,
        },
        endpoint,
    })
    .unwrap();
    let first = OutletId {
        source: SourceId::Rack(RackId(1)),
        index: 0,
    };
    sim.execute(Command::DisconnectPower { outlet: first })
        .unwrap();
    assert!(sim.device(large).unwrap().powered);
    assert!(sim.power.psu_telemetry(large, 1).unwrap().output_mw > 0);
}

#[test]
fn layer_three_switches_support_svis_routed_ports_and_switch_services() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let id = buy(&mut sim, "c9300_24t_e");
    install(&mut sim, id, 1, 0);
    for command in [
        "enable",
        "configure terminal",
        "interface Vlan1",
        "ip address 10.20.0.1 255.255.255.0",
        "no shutdown",
        "exit",
        "interface GigabitEthernet1/0/1",
        "no switchport",
        "ip address 192.0.2.1 255.255.255.0",
        "no shutdown",
        "exit",
        "snmp-server community public ro",
    ] {
        let result = sim.execute_console(id, command);
        assert!(result.success, "{command}: {:?}", result.lines);
    }
    sim.execute(Command::Provider(ProviderCommand::SetSpanningTree {
        switch: id,
        enabled: false,
    }))
    .unwrap();
    sim.execute(Command::Provider(ProviderCommand::SetSwitchManagement(
        SwitchManagement {
            switch: id,
            address: "10.30.0.1".parse().unwrap(),
            prefix: 24,
            vlan: VlanId(1),
            gateway: None,
        },
    )))
    .unwrap();
    let DeviceKind::Router(r) = &sim.device(id).unwrap().kind else {
        panic!()
    };
    assert_eq!(r.svi_ports.len(), 1);
    assert!(
        r.interfaces
            .iter()
            .any(|i| i.address == Some("192.0.2.1".parse().unwrap()))
    );
}

#[test]
fn hundred_gigabit_optics_link_through_authoritative_evaluator_without_copper_stock() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let a = buy(&mut sim, "c9500_24y4c_e");
    let b = buy(&mut sim, "n9k_c93180yc_fx3");
    install(&mut sim, a, 1, 0);
    install(&mut sim, b, 2, 1);
    let ap = sim.device(a).unwrap().ports()[24];
    let bp = sim.device(b).unwrap().ports()[48];
    for port in [ap, bp] {
        sim.execute(Command::Optics(OpticsCommand::BuyTransceiver {
            model: "qsfp100_lr4".into(),
        }))
        .unwrap();
        let module = sim
            .optics
            .transceivers
            .values()
            .find(|m| m.port.is_none())
            .unwrap()
            .id;
        sim.execute(Command::Optics(OpticsCommand::InstallTransceiver {
            port,
            module,
        }))
        .unwrap();
    }
    let cable = optics_catalog()
        .cables
        .iter()
        .find(|c| {
            matches!(
                c.medium,
                AssemblyMedium::Fiber {
                    fiber: FiberClass::Os2,
                    strands: 2,
                    ..
                }
            ) && c.length_cm >= 300
        })
        .unwrap();
    let stock = sim.cable_inventory().clone();
    sim.execute(Command::Optics(OpticsCommand::BuyAssembly {
        model: cable.id.clone(),
    }))
    .unwrap();
    let assembly = sim.optics.assemblies.values().next().unwrap().id;
    sim.execute(Command::Optics(OpticsCommand::ConnectAssembly {
        assembly,
        a: ap,
        b: bp,
        route: vec![],
    }))
    .unwrap();
    assert_eq!(sim.link_status(ap).speed, Some(LinkSpeed::Gbps100));
    assert_eq!(sim.cable_inventory(), &stock);
    let save = ron::to_string(&sim).unwrap();
    let mut loaded: NetworkSim = ron::from_str(&save).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.link_status(ap).speed, Some(LinkSpeed::Gbps100));
}

#[test]
fn combo_ports_use_one_interface_and_switch_back_to_copper_after_optic_removal() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let id = buy(&mut sim, "isr_4321");
    let port = sim.device(id).unwrap().ports()[0];
    assert!(sim.port_is_copper(port));
    assert!(sim.cage_profile(port).is_some());
    sim.execute(Command::Optics(OpticsCommand::BuyTransceiver {
        model: "sfp_1g_sx".into(),
    }))
    .unwrap();
    let module = sim.optics.transceivers.values().next().unwrap().id;
    sim.execute(Command::Optics(OpticsCommand::InstallTransceiver {
        port,
        module,
    }))
    .unwrap();
    assert!(!sim.port_is_copper(port));
    assert_eq!(sim.device(id).unwrap().ports().len(), 2);
    sim.execute(Command::Optics(OpticsCommand::RemoveTransceiver { port }))
        .unwrap();
    assert!(sim.port_is_copper(port));
}

#[test]
fn legacy_ron_switch_model_variants_still_load() {
    for (encoded, expected) in [
        ("c1000_24t_4g_l", "c1000_24t_4g_l"),
        ("c1000_24t_4x_l", "c1000_24t_4x_l"),
    ] {
        let model: SwitchModel = ron::from_str(encoded).unwrap();
        assert_eq!(model.as_str(), expected);
    }
}

#[test]
fn catalog_validation_rejects_duplicate_ids_bad_geometry_and_incompatible_modes() {
    let original: Value = serde_json::from_str(include_str!(
        "../../../assets/equipment/network_devices.json"
    ))
    .unwrap();
    for mutation in 0..4 {
        let mut source = original.clone();
        match mutation {
            0 => source["models"][1]["id"] = source["models"][0]["id"].clone(),
            1 => {
                source["models"][0]["ports"][0]["socket"]["rect"] =
                    serde_json::json!([0.5, 0.5, 0.2, 0.8])
            }
            2 => {
                let cage = source["models"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .flat_map(|model| model["ports"].as_array_mut().unwrap())
                    .filter_map(|port| port.get_mut("cage"))
                    .find(|cage| cage["kind"] == "sfp")
                    .expect("catalog must include an SFP cage");
                cage["modes"][0]["speed"] = serde_json::json!("Gbps100")
            }
            _ => {
                source["models"][0]["artwork"]["front"]["path"] =
                    serde_json::json!("../outside.png")
            }
        }
        let catalog: NetworkDeviceCatalog = serde_json::from_value(source).unwrap();
        assert!(catalog.validate().is_err());
    }
}

#[test]
fn installed_json_can_add_a_model_and_change_every_hardware_property_without_rebuilding() {
    use std::{
        fs, process,
        time::{SystemTime, UNIX_EPOCH},
    };
    let root = std::env::temp_dir().join(format!(
        "sim-network-json-{}-{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let dir = root.join("assets/equipment");
    fs::create_dir_all(&dir).unwrap();
    let mut catalog: Value = serde_json::from_str(include_str!(
        "../../../assets/equipment/network_devices.json"
    ))
    .unwrap();
    let mut model = catalog["models"][0].clone();
    model["id"] = serde_json::json!("json_only_router");
    model["shop_id"] = serde_json::json!("json_only_router");
    model["name"] = serde_json::json!("JSON test router");
    model["price"] = serde_json::json!(1234);
    model["rack_units"] = serde_json::json!(3);
    model["power"]["idle_mw"] = serde_json::json!(12000);
    model["power"]["peak_mw"] = serde_json::json!(30000);
    model["adapter"]["output_watts"] = serde_json::json!(36);
    model["adapter"]["output_current_ma"] = serde_json::json!(3000);
    model["adapter"]["efficiency_percent"] = serde_json::json!(80);
    model["ports"][0]["name"] = serde_json::json!("NewWAN");
    model["ports"][0]["ios_name"] = serde_json::json!("GigabitEthernet2/0/0");
    model["ports"][0]["flex"] = serde_json::json!(true);
    model["ports"][0]["side"] = serde_json::json!("Rear");
    model["ports"][0]["socket"]["rect"] = serde_json::json!([0.1, 0.2, 0.2, 0.4]);
    model["ports"].as_array_mut().unwrap().truncate(2);
    let mut bridge = model.clone();
    bridge["id"] = serde_json::json!("json_bridge_router");
    bridge["shop_id"] = serde_json::json!("json_bridge_router");
    bridge["routing_enabled"] = serde_json::json!(false);
    for port in bridge["ports"].as_array_mut().unwrap() {
        port["role"] = serde_json::json!("switch");
        port["flex"] = serde_json::json!(false);
    }
    catalog["models"].as_array_mut().unwrap().push(bridge);
    catalog["models"].as_array_mut().unwrap().push(model);
    fs::write(
        dir.join("network_devices.json"),
        serde_json::to_vec(&catalog).unwrap(),
    )
    .unwrap();
    let output = process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "installed_json_model_child", "--nocapture"])
        .current_dir(&root)
        .env("SIM_NETWORK_JSON_CHILD", "1")
        .output()
        .unwrap();
    fs::remove_dir_all(&root).unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn installed_json_model_child() {
    if std::env::var_os("SIM_NETWORK_JSON_CHILD").is_none() {
        return;
    }
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let bridge = buy(&mut sim, "json_bridge_router");
    assert_eq!(
        sim.device(bridge).unwrap().template(),
        DeviceTemplate::Router
    );
    sim.money = 100_000;
    let id = buy(&mut sim, "json_only_router");
    install(&mut sim, id, 1, 0);
    assert_eq!(sim.money, 100_000 - 1234);
    assert_eq!(sim.device(id).unwrap().rack_units(), 3);
    let port = sim.device(id).unwrap().ports()[0];
    assert_eq!(sim.port(port).unwrap().name, "NewWAN");
    assert_eq!(sim.port(port).unwrap().side, RackSide::Rear);
    assert_eq!(sim.ios_interface_name(id, port), "GigabitEthernet2/0/0");
    assert_eq!(sim.device_consumption(id).unwrap().demand_mw, 12000);
    assert_eq!(sim.power.device_input_load(id).watts, 15);
    sim.execute(Command::SetRouterSwitchport {
        port,
        switchport: true,
    })
    .unwrap();
    assert!(matches!(
        sim.port(port).unwrap().config,
        PortConfig::Switch(_)
    ));
    let mut loaded: NetworkSim = ron::from_str(&ron::to_string(&sim).unwrap()).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.port(port).unwrap().side, RackSide::Rear);
}

#[test]
fn shared_bay_modes_disable_conflicting_interfaces_from_json_rules() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let router = buy(&mut sim, "c8500_12x4qc");
    let switch = buy(&mut sim, "c1000_24t_4g_l");
    install(&mut sim, router, 1, 0);
    install(&mut sim, switch, 2, 1);
    let a = sim.device(router).unwrap().ports()[0];
    let b = sim.device(switch).unwrap().ports()[24];
    for port in [a, b] {
        sim.execute(Command::Optics(OpticsCommand::BuyTransceiver {
            model: "sfp_1g_lx10".into(),
        }))
        .unwrap();
        let module = sim
            .optics
            .transceivers
            .values()
            .find(|m| m.port.is_none())
            .unwrap()
            .id;
        sim.execute(Command::Optics(OpticsCommand::InstallTransceiver {
            port,
            module,
        }))
        .unwrap();
    }
    let cable = optics_catalog()
        .cables
        .iter()
        .find(|c| {
            matches!(
                c.medium,
                AssemblyMedium::Fiber {
                    fiber: FiberClass::Os2,
                    strands: 2,
                    ..
                }
            ) && c.length_cm >= 300
        })
        .unwrap();
    sim.execute(Command::Optics(OpticsCommand::BuyAssembly {
        model: cable.id.clone(),
    }))
    .unwrap();
    let assembly = sim.optics.assemblies.values().next().unwrap().id;
    sim.execute(Command::Optics(OpticsCommand::ConnectAssembly {
        assembly,
        a,
        b,
        route: vec![],
    }))
    .unwrap();
    assert_eq!(sim.link_status(a).speed, Some(LinkSpeed::Gbps1));
    let trigger = sim.device(router).unwrap().ports()[14];
    sim.execute(Command::Optics(OpticsCommand::BuyTransceiver {
        model: "qsfp100_lr4".into(),
    }))
    .unwrap();
    let module = sim
        .optics
        .transceivers
        .values()
        .find(|m| m.port.is_none())
        .unwrap()
        .id;
    sim.execute(Command::Optics(OpticsCommand::InstallTransceiver {
        port: trigger,
        module,
    }))
    .unwrap();
    assert_eq!(sim.link_status(a).speed, None);
    sim.execute(Command::Optics(OpticsCommand::RemoveTransceiver {
        port: trigger,
    }))
    .unwrap();
    assert_eq!(sim.link_status(a).speed, Some(LinkSpeed::Gbps1));
}

#[test]
fn console_reload_derives_combo_connector_from_current_inventory() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let id = buy(&mut sim, "isr_4321");
    install(&mut sim, id, 1, 0);
    let port = sim.device(id).unwrap().ports()[0];
    for command in ["enable", "write memory"] {
        assert!(sim.execute_console(id, command).success);
    }
    sim.execute(Command::Optics(OpticsCommand::BuyTransceiver {
        model: "sfp_1g_sx".into(),
    }))
    .unwrap();
    let module = sim.optics.transceivers.values().next().unwrap().id;
    sim.execute(Command::Optics(OpticsCommand::InstallTransceiver {
        port,
        module,
    }))
    .unwrap();
    for command in ["reload", "yes"] {
        assert!(sim.execute_console(id, command).success);
    }
    assert!(sim.installed_transceiver(port).is_some());
    assert!(!sim.port_is_copper(port));
    for command in ["enable", "write memory"] {
        assert!(sim.execute_console(id, command).success);
    }
    sim.execute(Command::Optics(OpticsCommand::RemoveTransceiver { port }))
        .unwrap();
    for command in ["reload", "yes"] {
        assert!(sim.execute_console(id, command).success);
    }
    assert!(sim.installed_transceiver(port).is_none());
    assert!(sim.port_is_copper(port));
}

#[test]
fn no_network_sockets_share_the_same_side_and_rectangle() {
    for model in &network_device_catalog().models {
        let mut sockets = Vec::new();
        for port in &model.ports {
            for socket in std::iter::once(&port.socket).chain(port.optical_socket.as_ref()) {
                assert!(
                    !sockets.contains(&(port.side, socket.rect)),
                    "{} has duplicate sockets",
                    model.id
                );
                sockets.push((port.side, socket.rect));
            }
        }
    }
}
