use cloud_provider_sim::*;

fn buy(sim: &mut NetworkSim, command: Command, unit: u8) -> DeviceId {
    let SimEvent::DeviceAdded(id) = sim.execute(command).unwrap()[0] else {
        panic!()
    };
    sim.execute(Command::PlaceDevice {
        device: id,
        rack: RackId(1),
        unit,
    })
    .unwrap();
    let index = (0..4)
        .find(|index| {
            !sim.power.connections.contains_key(&OutletId {
                source: SourceId::Rack(RackId(1)),
                index: *index,
            })
        })
        .unwrap();
    let endpoint = match &sim.device(id).unwrap().kind {
        DeviceKind::Ups(ups) => PowerEndpoint::Source(ups.source.unwrap()),
        DeviceKind::Pdu(pdu) => PowerEndpoint::Source(pdu.source.unwrap()),
        _ => PowerEndpoint::Device(id),
    };
    sim.execute(Command::ConnectPower {
        outlet: OutletId {
            source: SourceId::Rack(RackId(1)),
            index,
        },
        endpoint,
    })
    .unwrap();
    sim.execute(Command::SetPower {
        device: id,
        powered: true,
    })
    .unwrap();
    id
}
fn server(sim: &mut NetworkSim) -> DeviceId {
    sim.money = 100_000;
    buy(sim, Command::BuyServerFullPack, 1)
}
fn workload(sim: &mut NetworkSim, device: DeviceId, cpu: u16, memory: u16, storage: u16) {
    sim.execute(Command::SetDeviceWorkload {
        device,
        workload: DeviceWorkload {
            cpu,
            memory,
            storage,
        },
    })
    .unwrap();
}
fn part(sim: &NetworkSim, device: DeviceId, kind: PowerComponentKind) -> u32 {
    sim.device_consumption(device)
        .unwrap()
        .components
        .iter()
        .find(|part| part.kind == kind)
        .unwrap()
        .milliwatts
}

#[test]
fn server_consumption_follows_component_load_without_using_psu_capacity_as_draw() {
    let mut sim = NetworkSim::new();
    let device = server(&mut sim);
    let idle = sim.device_consumption(device).unwrap();
    assert!((35..65).contains(&idle.current.watts));
    assert!(idle.demand_mw < idle.peak_mw / 2);
    assert!(idle.peak_mw < 600_000);
    let idle_cpu = part(&sim, device, PowerComponentKind::Cpu);
    workload(&mut sim, device, 500, 0, 0);
    let half_cpu = part(&sim, device, PowerComponentKind::Cpu);
    workload(&mut sim, device, 1000, 1000, 1000);
    let full = sim.device_consumption(device).unwrap();
    assert!(idle_cpu < half_cpu && half_cpu < 55_000);
    assert_eq!(part(&sim, device, PowerComponentKind::Cpu), 55_000);
    assert_eq!(part(&sim, device, PowerComponentKind::Memory), 4000);
    assert_eq!(part(&sim, device, PowerComponentKind::Storage), 4000);
    assert!(full.current.watts > idle.current.watts);
    assert!(part(&sim, device, PowerComponentKind::Conversion) > 0);
    assert!(full.demand_mw <= full.peak_mw);
}

#[test]
fn extra_ram_and_spinning_drives_add_idle_draw_without_charging_their_active_rating() {
    let mut sim = NetworkSim::new();
    let device = server(&mut sim);
    sim.execute(Command::BuyServerPart {
        part_id: "ddr5_ecc_16gb".into(),
    })
    .unwrap();
    sim.execute(Command::InstallServerPart {
        device,
        part_id: "ddr5_ecc_16gb".into(),
        slot: None,
    })
    .unwrap();
    assert_eq!(part(&sim, device, PowerComponentKind::Memory), 3200);
    sim.execute(Command::BuyDrive {
        drive_id: "enterprise_hdd_2tb".into(),
    })
    .unwrap();
    sim.execute(Command::InstallDrive {
        device,
        drive_id: "enterprise_hdd_2tb".into(),
        bay: Some(1),
    })
    .unwrap();
    assert_eq!(part(&sim, device, PowerComponentKind::Storage), 6000);
    workload(&mut sim, device, 0, 1000, 1000);
    assert_eq!(part(&sim, device, PowerComponentKind::Memory), 8000);
    assert_eq!(part(&sim, device, PowerComponentKind::Storage), 13000);
}

#[test]
fn off_and_incomplete_devices_do_not_create_phantom_circuit_load() {
    let mut sim = NetworkSim::new();
    let device = server(&mut sim);
    sim.execute(Command::SetPower {
        device,
        powered: false,
    })
    .unwrap();
    assert_eq!(sim.device_consumption(device).unwrap().current.watts, 0);
    assert_eq!(
        sim.power
            .source_telemetry(SourceId::Rack(RackId(1)))
            .unwrap()
            .output
            .watts,
        0
    );
    let SimEvent::DeviceAdded(empty) = sim.execute(Command::BuyServerChassis).unwrap()[0] else {
        panic!()
    };
    assert_eq!(sim.power.devices[&empty].load.watts, 0);
    assert_eq!(sim.device_consumption(empty).unwrap().current.watts, 0);
}

#[test]
fn workload_validation_is_atomic_and_load_controls_persist_across_saves() {
    let mut sim = NetworkSim::new();
    let device = server(&mut sim);
    workload(&mut sim, device, 500, 300, 200);
    let before = sim.device_consumption(device).unwrap().demand_mw;
    assert!(
        sim.execute(Command::SetDeviceWorkload {
            device,
            workload: DeviceWorkload {
                cpu: 1001,
                memory: 0,
                storage: 0
            }
        })
        .is_err()
    );
    assert_eq!(sim.device_workload(device).cpu, 500);
    assert_eq!(sim.device_consumption(device).unwrap().demand_mw, before);
    let saved = ron::to_string(&sim).unwrap();
    let mut loaded: NetworkSim = ron::from_str(&saved).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.device_workload(device), sim.device_workload(device));
    assert_eq!(loaded.device_consumption(device).unwrap().demand_mw, before);
    workload(&mut loaded, device, 0, 0, 0);
    assert!(loaded.device_consumption(device).unwrap().demand_mw < before);
}

#[test]
fn guest_file_io_changes_drive_load_but_tmpfs_and_power_queries_do_not() {
    let mut sim = NetworkSim::new();
    let device = server(&mut sim);
    let command = format!("echo '{}' > /root/data", "x".repeat(1_100_000));
    assert!(sim.execute_console(device, &command).success);
    assert!(part(&sim, device, PowerComponentKind::Storage) > 1000);
    sim.advance_time(1000);
    assert_eq!(part(&sim, device, PowerComponentKind::Storage), 1000);
    let idle = sim.device_consumption(device).unwrap().demand_mw;
    assert!(sim.execute_console(device, "power").success);
    assert_eq!(sim.device_consumption(device).unwrap().demand_mw, idle);
    assert!(
        sim.execute_console(device, &command.replace("/root/data", "/tmp/data"))
            .success
    );
    assert_eq!(part(&sim, device, PowerComponentKind::Storage), 1000);
    assert!(
        sim.execute_console(device, "power workload 75 50 25")
            .success
    );
    assert_eq!(
        sim.device_workload(device),
        DeviceWorkload {
            cpu: 750,
            memory: 500,
            storage: 250
        }
    );
    assert!(
        !sim.execute_console(device, "power workload 101 0 0")
            .success
    );
}

#[test]
fn traffic_increases_network_draw_proportionally_and_expires_on_the_packet_clock() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let a = buy(
        &mut sim,
        Command::BuyDevice {
            kind: DeviceTemplate::Switch,
        },
        1,
    );
    let b = buy(
        &mut sim,
        Command::BuyDevice {
            kind: DeviceTemplate::Switch,
        },
        2,
    );
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::CableBox305m,
    })
    .unwrap();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::Rj45Pack20,
    })
    .unwrap();
    let source = sim.device(a).unwrap().ports()[0];
    let target = sim.device(b).unwrap().ports()[0];
    sim.execute(Command::Connect {
        a: source,
        b: target,
    })
    .unwrap();
    for line in [
        "enable",
        "configure terminal",
        "management ip 10.0.0.2",
        "end",
    ] {
        assert!(sim.execute_console(b, line).success);
    }
    let idle = sim.device_consumption(b).unwrap().demand_mw;
    let frame = EthernetFrame {
        source: MacAddress::for_port(source),
        destination: MacAddress::for_port(target),
        vlan: None,
        qos: FrameQos {
            length_bytes: 65_000,
            ..Default::default()
        },
        payload: EthernetPayload::DhcpDiscover,
    };
    sim.transmit_frame(source, frame);
    assert_eq!(sim.power.devices[&b].load.watts, 16);
    // A synthetic byte-count batch exercises the rate estimator independently
    // from protocol payload size; it is not a claim of jumbo-frame support.
    sim.transmit_frames(source, vec![frame; 256]);
    assert!(sim.device_consumption(b).unwrap().demand_mw > idle);
    assert!(sim.device_consumption(b).unwrap().demand_mw < 22_800);
    sim.advance_time(1000);
    assert_eq!(sim.device_consumption(b).unwrap().demand_mw, idle);
}

#[test]
fn cpu_load_shortens_ups_runtime_and_dynamic_ticks_are_partition_invariant() {
    let mut sim = NetworkSim::new();
    let device = server(&mut sim);
    let ups = buy(
        &mut sim,
        Command::BuyDevice {
            kind: DeviceTemplate::Ups,
        },
        3,
    );
    let DeviceKind::Ups(ups_device) = &sim.device(ups).unwrap().kind else {
        panic!()
    };
    let source = ups_device.source.unwrap();
    sim.execute(Command::DisconnectPower {
        outlet: OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 0,
        },
    })
    .unwrap();
    sim.execute(Command::ConnectPower {
        outlet: OutletId { source, index: 0 },
        endpoint: PowerEndpoint::Device(device),
    })
    .unwrap();
    let idle_runtime = sim
        .power
        .source_telemetry(source)
        .unwrap()
        .runtime_seconds
        .unwrap();
    workload(&mut sim, device, 1000, 1000, 1000);
    assert!(
        sim.power
            .source_telemetry(source)
            .unwrap()
            .runtime_seconds
            .unwrap()
            < idle_runtime
    );
    sim.execute(Command::SetRackMains {
        rack: RackId(1),
        on: false,
    })
    .unwrap();
    assert!(
        sim.execute_console(device, "echo test > /root/data")
            .success
    );
    let mut split = sim.clone();
    sim.advance_time(2000);
    for _ in 0..20 {
        split.advance_time(100);
    }
    assert_eq!(
        sim.power.source_telemetry(source).unwrap().battery_mwh,
        split.power.source_telemetry(source).unwrap().battery_mwh
    );
}

#[test]
fn idle_ups_consumes_electronics_power_on_battery_and_disabled_ups_does_not() {
    let mut power = PowerSystem::new();
    power.add_rack(RackId(1));
    let id = power.add_ups(UpsSpec::default());
    let source = SourceId::Ups(id);
    power
        .connect(
            OutletId {
                source: SourceId::Rack(RackId(1)),
                index: 0,
            },
            PowerEndpoint::Source(source),
        )
        .unwrap();
    assert_eq!(power.source_telemetry(source).unwrap().output.watts, 0);
    assert_eq!(
        power
            .source_telemetry(source)
            .unwrap()
            .self_consumption_watts,
        9
    );
    power.set_rack_mains(RackId(1), false);
    let initial = power.ups[&id].battery_mwh;
    power.tick_ms(3_600_000);
    assert!(power.ups[&id].battery_mwh < initial);
    power.set_source_enabled(source, false);
    let stopped = power.ups[&id].battery_mwh;
    power.tick_ms(3_600_000);
    assert_eq!(power.ups[&id].battery_mwh, stopped);
}

#[test]
fn power_profiles_reject_invalid_curves_factors_and_efficiency_tables() {
    for json in [
        r#"{"idle_mw":2000,"peak_mw":1000}"#,
        r#"{"idle_mw":1000,"peak_mw":2000,"link_share_permille":1001}"#,
        r#"{"idle_mw":1000,"peak_mw":2000,"power_factor_percent":0}"#,
        r#"{"idle_mw":1000,"peak_mw":2000,"power_factor_percent":101}"#,
        r#"{"idle_mw":1000,"peak_mw":2000,"peakk_mw":3000}"#,
    ] {
        assert!(
            serde_json::from_str::<PowerProfile>(json).is_err(),
            "{json}"
        );
    }
    for points in [
        "[]",
        "[[0,900]]",
        "[[1,900],[1000,950]]",
        "[[0,0],[1000,900]]",
        "[[0,900],[500,950],[500,940],[1000,920]]",
        "[[0,900],[1000,1001]]",
    ] {
        let json = format!(r#"{{"capacity_watts":600,"efficiency_permille":{points}}}"#);
        assert!(
            serde_json::from_str::<PsuPowerProfile>(&json).is_err(),
            "{json}"
        );
    }
    let profile: PowerProfile = serde_json::from_str(r#"{"idle_mw":1234,"peak_mw":9876}"#).unwrap();
    assert_eq!(profile.draw_mw(500), 5555);
}

#[test]
fn installed_power_json_changes_all_equipment_without_rebuilding() {
    use serde_json::{Value, json};
    use std::{
        fs, process,
        time::{SystemTime, UNIX_EPOCH},
    };
    let root = std::env::temp_dir().join(format!(
        "sim-power-json-{}-{}",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let equipment = root.join("assets/equipment");
    fs::create_dir_all(&equipment).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/equipment");
    for filename in [
        "server_config.json",
        "server_parts.json",
        "router_config.json",
        "switches.json",
        "drives.json",
        "ups_config.json",
        "pdu_config.json",
        "optics.json",
    ] {
        let mut config: Value =
            serde_json::from_slice(&fs::read(source.join(filename)).unwrap()).unwrap();
        match filename {
            "server_config.json" => {
                config["power"]["board"] = json!({"idle_mw":30000,"peak_mw":45000});
                config["power"]["fans"] = json!({"idle_mw":4000,"peak_mw":12000});
                config["power"]["psu"] =
                    json!({"capacity_watts":500,"efficiency_permille":[[0,1000],[1000,1000]]});
            }
            "server_parts.json" => {
                config["parts"][0]["power"] = json!({"idle_mw":10000,"peak_mw":70000});
                config["parts"][1]["power"] = json!({"idle_mw":3000,"peak_mw":5000});
                config["parts"][2]["power"] =
                    json!({"idle_mw":4000,"peak_mw":8000,"link_share_permille":500});
            }
            "drives.json" => config["drives"][1]["power"] = json!({"idle_mw":2000,"peak_mw":6000}),
            "router_config.json" => {
                config["power"]["idle_mw"] = json!(12000);
                config["power"]["peak_mw"] = json!(48000);
                config["power"]["adapter"]["efficiency_percent"] = json!(80);
                config["power"]["adapter"]["output_watts"] = json!(20);
            }
            "switches.json" => {
                config[0]["power"] = json!({"idle_mw":20000,"peak_mw":30000,"link_share_permille":500,"power_factor_percent":95})
            }
            "ups_config.json" => {
                config["power"]["capacity_watts"] = json!(800);
                config["power"]["battery_wh"] = json!(300);
                config["power"]["self_watts"] = json!(12);
                config["power"]["efficiency_percent"] = json!(80);
            }
            "pdu_config.json" => {
                config["power"]["capacity_watts"] = json!(800);
                config["power"]["self_watts"] = json!(3);
            }
            "optics.json" => config["modules"][0]["power"] = json!({"idle_mw":500,"peak_mw":1700}),
            _ => unreachable!(),
        }
        fs::write(
            equipment.join(filename),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
    }
    let output = process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "installed_power_json_child", "--nocapture"])
        .current_dir(&root)
        .env("SIM_POWER_JSON_TEST_CHILD", "1")
        .output()
        .unwrap();
    fs::remove_dir_all(root).unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn installed_power_json_child() {
    if std::env::var_os("SIM_POWER_JSON_TEST_CHILD").is_none() {
        return;
    }
    let mut sim = NetworkSim::new();
    let device = server(&mut sim);
    assert_eq!(part(&sim, device, PowerComponentKind::Cpu), 10360);
    assert_eq!(part(&sim, device, PowerComponentKind::Memory), 3000);
    assert_eq!(part(&sim, device, PowerComponentKind::Storage), 2000);
    assert_eq!(part(&sim, device, PowerComponentKind::Network), 4000);
    assert_eq!(part(&sim, device, PowerComponentKind::Conversion), 0);
    assert_eq!(server_power_profile().psu.capacity_watts, 500);
    workload(&mut sim, device, 1000, 1000, 1000);
    assert_eq!(part(&sim, device, PowerComponentKind::Cpu), 70000);
    assert_eq!(part(&sim, device, PowerComponentKind::Memory), 5000);
    assert_eq!(part(&sim, device, PowerComponentKind::Storage), 6000);
    let router = buy(
        &mut sim,
        Command::BuyDevice {
            kind: DeviceTemplate::Router,
        },
        3,
    );
    assert_eq!(sim.device_consumption(router).unwrap().current.watts, 12);
    assert_eq!(
        PowerCordKind::Cisco66WAdapter
            .input_load(sim.device_consumption(router).unwrap().current)
            .watts,
        15
    );
    workload(&mut sim, router, 1000, 0, 0);
    assert!(!sim.device(router).unwrap().powered);
    assert_eq!(sim.device_consumption(router).unwrap().current.watts, 0);
    workload(&mut sim, router, 0, 0, 0);
    assert!(sim.device(router).unwrap().powered);
    let switch = buy(
        &mut sim,
        Command::BuyDevice {
            kind: DeviceTemplate::Switch,
        },
        5,
    );
    assert_eq!(sim.device_consumption(switch).unwrap().current.watts, 20);
    assert_eq!(sim.device_consumption(switch).unwrap().current.va, 22);
    let ups = buy(
        &mut sim,
        Command::BuyDevice {
            kind: DeviceTemplate::Ups,
        },
        7,
    );
    let DeviceKind::Ups(model) = &sim.device(ups).unwrap().kind else {
        panic!()
    };
    assert_eq!(
        sim.power
            .source_telemetry(model.source.unwrap())
            .unwrap()
            .input
            .watts,
        15
    );
    assert_eq!(UpsSpec::default().watts, 800);
    assert_eq!(UpsSpec::default().battery_wh, 300);
    let saved = ron::to_string(&sim).unwrap();
    let mut loaded: NetworkSim = ron::from_str(&saved).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(
        loaded.device_consumption(device).unwrap().demand_mw,
        sim.device_consumption(device).unwrap().demand_mw
    );
    // A fifth device uses an isolated rack outlet after freeing the UPS input.
    sim.execute(Command::DisconnectPower {
        outlet: OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 3,
        },
    })
    .unwrap();
    let pdu = buy(
        &mut sim,
        Command::BuyDevice {
            kind: DeviceTemplate::Pdu,
        },
        9,
    );
    let DeviceKind::Pdu(model) = &sim.device(pdu).unwrap().kind else {
        panic!()
    };
    assert_eq!(
        sim.power
            .source_telemetry(model.source.unwrap())
            .unwrap()
            .input
            .watts,
        3
    );
    assert_eq!(PduState::default().watts, 800);
    let module = optics_catalog().module("sfp_1g_sx").unwrap();
    assert_eq!(module.power.draw_mw(0), 500);
    assert_eq!(module.power.draw_mw(1000), 1700);
    let SimEvent::DeviceAdded(optical) = sim
        .execute(Command::Optics(OpticsCommand::BuyHardware {
            model: "switch_10g".into(),
        }))
        .unwrap()[0]
    else {
        panic!()
    };
    let port = sim.device(optical).unwrap().ports()[24];
    let events = sim
        .execute(Command::Optics(OpticsCommand::BuyTransceiver {
            model: "sfp_1g_sx".into(),
        }))
        .unwrap();
    assert!(!events.is_empty());
    let module_id = *sim.optics.transceivers.keys().next().unwrap();
    sim.execute(Command::Optics(OpticsCommand::InstallTransceiver {
        port,
        module: module_id,
    }))
    .unwrap();
    assert_eq!(part(&sim, optical, PowerComponentKind::Optics), 500);
}
