use cloud_provider_sim::*;

fn assembled(module: &str, count: usize) -> ServerHardware {
    ServerHardware {
        cpus: vec![ServerFullPack::CPU.into()],
        ram: vec![module.into(); count],
        ..Default::default()
    }
}

#[test]
fn r360_capacity_channels_and_population_change_operating_speed() {
    let single = assembled("ddr5_ecc_16gb", 1).memory_status().unwrap();
    assert_eq!(
        (
            single.capacity_gb,
            single.max_capacity_gb,
            single.populated_channels,
            single.available_channels,
            single.speed_mt_s
        ),
        (16, 128, 1, 2, 4400)
    );
    assert!(!single.balanced);
    let pair = assembled("ddr5_ecc_16gb", 2).memory_status().unwrap();
    assert_eq!((pair.capacity_gb, pair.speed_mt_s), (32, 4400));
    assert!(pair.balanced);
    assert_eq!(
        assembled("ddr5_ecc_16gb", 4)
            .memory_status()
            .unwrap()
            .speed_mt_s,
        4000
    );
    let full = assembled("ddr5_ecc_32gb", 4);
    assert!(full.ready());
    let memory = full.memory_status().unwrap();
    assert_eq!((memory.capacity_gb, memory.speed_mt_s), (128, 3600));
    assert!(memory.balanced);
}

#[test]
fn cpu_and_chassis_limits_both_apply_and_are_catalog_driven() {
    let mut catalog = server_catalog().clone();
    let hardware = assembled("ddr5_ecc_32gb", 2);
    catalog.chassis.memory.max_capacity_gb = 32;
    assert!(
        hardware
            .validate_limits(&catalog)
            .unwrap_err()
            .contains("capacity")
    );
    catalog.chassis.memory.max_capacity_gb = 128;
    let ServerPartKind::Cpu { memory, .. } = &mut catalog.parts[0].kind else {
        panic!()
    };
    memory.max_capacity_gb = 32;
    assert!(
        hardware
            .validate_limits(&catalog)
            .unwrap_err()
            .contains("capacity")
    );
    let ServerPartKind::Cpu { memory, .. } = &mut catalog.parts[0].kind else {
        panic!()
    };
    memory.max_capacity_gb = 128;
    memory.channels = 1;
    assert!(
        hardware
            .validate_limits(&catalog)
            .unwrap_err()
            .contains("channel")
    );
    let ServerPartKind::Cpu { memory, .. } = &mut catalog.parts[0].kind else {
        panic!()
    };
    memory.channels = 2;
    memory.dimms_per_channel = 1;
    assert!(
        assembled("ddr5_ecc_32gb", 3)
            .validate_limits(&catalog)
            .unwrap_err()
            .contains("DIMM limits")
    );
    let ServerPartKind::Cpu { memory, .. } = &mut catalog.parts[0].kind else {
        panic!()
    };
    memory.max_speed_mt_s = 3200;
    assert_eq!(
        hardware
            .memory_status_with_catalog(&catalog)
            .unwrap()
            .speed_mt_s,
        3200
    );
    catalog.chassis.max_cpu_tdp_w = 35;
    assert!(
        hardware
            .validate_limits(&catalog)
            .unwrap_err()
            .contains("CPU")
    );
}

#[test]
fn dimm_type_rank_voltage_and_mixing_rules_are_enforced() {
    let mut hardware = assembled("ddr5_ecc_16gb", 1);
    hardware.ram.push("ddr5_ecc_32gb".into());
    assert!(
        hardware
            .validate_limits(server_catalog())
            .unwrap_err()
            .contains("identical")
    );
    for field in ["type", "rank", "voltage", "capacity"] {
        let mut catalog = server_catalog().clone();
        let ServerPartKind::Ram {
            memory_type,
            ranks,
            voltage_mv,
            capacity_gb,
            ..
        } = &mut catalog.parts[1].kind
        else {
            panic!()
        };
        match field {
            "type" => *memory_type = "DDR5 ECC RDIMM".into(),
            "rank" => *ranks = 4,
            "voltage" => *voltage_mv = 1200,
            _ => *capacity_gb = 64,
        }
        assert!(
            assembled("ddr5_ecc_16gb", 1)
                .validate_limits(&catalog)
                .is_err(),
            "{field}"
        );
    }
    let mut no_cpu = assembled("ddr5_ecc_16gb", 1);
    no_cpu.cpus.clear();
    assert!(!no_cpu.ready());
}

#[test]
fn population_slots_are_preserved_and_primary_socket_must_be_present() {
    let mut hardware = assembled("ddr5_ecc_16gb", 2);
    hardware.ram_slot_indices = vec![0, 2];
    let status = hardware.memory_status().unwrap();
    assert_eq!((status.populated_channels, status.speed_mt_s), (1, 4000));
    hardware.ram_slot_indices = vec![2, 3];
    assert!(
        hardware
            .validate_limits(server_catalog())
            .unwrap_err()
            .contains("primary")
    );
    hardware.ram_slot_indices = vec![0, 0];
    assert!(
        hardware
            .validate_limits(server_catalog())
            .unwrap_err()
            .contains("duplicated")
    );
}

#[test]
fn failed_dimm_installation_keeps_inventory_and_successful_removal_keeps_physical_slots() {
    let mut sim = NetworkSim::new();
    let SimEvent::DeviceAdded(device) = sim.execute(Command::BuyServerFullPack).unwrap()[0] else {
        panic!()
    };
    sim.execute(Command::BuyServerPart {
        part_id: "ddr5_ecc_32gb".into(),
    })
    .unwrap();
    let before = serde_json::to_string(&sim).unwrap();
    assert!(
        sim.execute(Command::InstallServerPart {
            device,
            part_id: "ddr5_ecc_32gb".into(),
            slot: None
        })
        .is_err()
    );
    assert_eq!(serde_json::to_string(&sim).unwrap(), before);
    sim.execute(Command::BuyServerPart {
        part_id: "ddr5_ecc_16gb".into(),
    })
    .unwrap();
    sim.execute(Command::InstallServerPart {
        device,
        part_id: "ddr5_ecc_16gb".into(),
        slot: Some(1),
    })
    .unwrap();
    sim.execute(Command::RemoveServerPart {
        device,
        part_id: "ddr5_ecc_16gb".into(),
        slot: Some(0),
    })
    .unwrap();
    let DeviceKind::Server(server) = &sim.device(device).unwrap().kind else {
        panic!()
    };
    assert_eq!(server.hardware.as_ref().unwrap().ram_slot_indices, [1]);
    let mut saved = serde_json::to_value(&sim).unwrap();
    saved["devices"][device.0.to_string()]["kind"]["Server"]["hardware"]
        .as_object_mut()
        .unwrap()
        .remove("ram_slot_indices");
    let mut loaded: NetworkSim = serde_json::from_value(saved).unwrap();
    loaded.rebuild_indexes();
    let DeviceKind::Server(server) = &loaded.device(device).unwrap().kind else {
        panic!()
    };
    assert_eq!(server.hardware.as_ref().unwrap().ram_slot_indices, [0]);
}

#[test]
fn maximum_configuration_rejects_a_fifth_dimm_without_consuming_it() {
    let mut sim = NetworkSim::new();
    let SimEvent::DeviceAdded(device) = sim.execute(Command::BuyServerChassis).unwrap()[0] else {
        panic!()
    };
    for part in [
        ServerFullPack::CPU,
        "ddr5_ecc_32gb",
        "ddr5_ecc_32gb",
        "ddr5_ecc_32gb",
        "ddr5_ecc_32gb",
    ] {
        sim.execute(Command::BuyServerPart {
            part_id: part.into(),
        })
        .unwrap();
        sim.execute(Command::InstallServerPart {
            device,
            part_id: part.into(),
            slot: None,
        })
        .unwrap();
    }
    sim.execute(Command::BuyServerPart {
        part_id: "ddr5_ecc_32gb".into(),
    })
    .unwrap();
    assert!(
        sim.server_part_installation_slot(device, "ddr5_ecc_32gb", None)
            .is_err()
    );
    assert!(
        sim.execute(Command::InstallServerPart {
            device,
            part_id: "ddr5_ecc_32gb".into(),
            slot: None
        })
        .is_err()
    );
    assert_eq!(sim.server_parts["ddr5_ecc_32gb"], 1);
}

#[test]
fn cpu_and_memory_cannot_be_changed_while_powered_on() {
    let mut sim = NetworkSim::new();
    let SimEvent::DeviceAdded(device) = sim.execute(Command::BuyServerFullPack).unwrap()[0] else {
        panic!()
    };
    sim.execute(Command::PlaceDevice {
        device,
        rack: RackId(1),
        unit: 1,
    })
    .unwrap();
    sim.execute(Command::ConnectPower {
        outlet: OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 0,
        },
        endpoint: PowerEndpoint::Device(device),
    })
    .unwrap();
    sim.execute(Command::SetPower {
        device,
        powered: true,
    })
    .unwrap();
    sim.execute(Command::BuyServerPart {
        part_id: ServerFullPack::RAM.into(),
    })
    .unwrap();
    assert!(
        sim.server_part_installation_slot(device, ServerFullPack::RAM, None)
            .is_ok()
    );
    let before = ron::to_string(&sim).unwrap();
    assert!(
        sim.execute(Command::InstallServerPart {
            device,
            part_id: ServerFullPack::RAM.into(),
            slot: None
        })
        .unwrap_err()
        .to_string()
        .contains("power off")
    );
    assert!(
        sim.execute(Command::RemoveServerPart {
            device,
            part_id: ServerFullPack::RAM.into(),
            slot: Some(0)
        })
        .is_err()
    );
    assert_eq!(ron::to_string(&sim).unwrap(), before);
}

#[test]
fn catalog_validation_rejects_invalid_topology_and_missing_speed_matrix() {
    let mut catalog = server_catalog().clone();
    catalog.chassis.memory.slots[1].channel = 7;
    assert!(catalog.validate().is_err());
    let mut catalog = server_catalog().clone();
    catalog.chassis.memory.supported_modules[0]
        .operating_speeds_mt_s
        .clear();
    assert!(catalog.validate().is_err());
}

#[test]
fn installed_json_controls_cpu_capacity_channels_and_automatic_placement() {
    let root = std::env::temp_dir().join(format!("sim-server-limits-{}", std::process::id()));
    let equipment = root.join("assets/equipment");
    std::fs::create_dir_all(&equipment).unwrap();
    let mut json: serde_json::Value =
        serde_json::from_str(include_str!("../../../assets/equipment/server_parts.json")).unwrap();
    json["chassis"]["memory"]["max_capacity_gb"] = serde_json::json!(96);
    json["parts"][0]["memory"]["max_capacity_gb"] = serde_json::json!(32);
    json["parts"][0]["memory"]["channels"] = serde_json::json!(1);
    json["parts"][0]["memory"]["max_dimms"] = serde_json::json!(2);
    std::fs::write(
        equipment.join("server_parts.json"),
        serde_json::to_vec(&json).unwrap(),
    )
    .unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "installed_server_limits_child", "--nocapture"])
        .current_dir(&root)
        .env("SIM_SERVER_LIMITS_CHILD", "1")
        .output()
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn installed_server_limits_child() {
    if std::env::var_os("SIM_SERVER_LIMITS_CHILD").is_none() {
        return;
    }
    let mut sim = NetworkSim::new();
    let SimEvent::DeviceAdded(device) = sim.execute(Command::BuyServerFullPack).unwrap()[0] else {
        panic!()
    };
    sim.execute(Command::BuyServerPart {
        part_id: ServerFullPack::RAM.into(),
    })
    .unwrap();
    assert_eq!(
        sim.server_part_installation_slot(device, ServerFullPack::RAM, None)
            .unwrap(),
        Some(2)
    );
    sim.execute(Command::InstallServerPart {
        device,
        part_id: ServerFullPack::RAM.into(),
        slot: None,
    })
    .unwrap();
    let DeviceKind::Server(server) = &sim.device(device).unwrap().kind else {
        panic!()
    };
    let status = server.hardware.as_ref().unwrap().memory_status().unwrap();
    assert_eq!(
        (
            status.capacity_gb,
            status.max_capacity_gb,
            status.available_channels
        ),
        (32, 32, 1)
    );
    sim.execute(Command::BuyServerPart {
        part_id: ServerFullPack::RAM.into(),
    })
    .unwrap();
    assert!(
        sim.execute(Command::InstallServerPart {
            device,
            part_id: ServerFullPack::RAM.into(),
            slot: None
        })
        .is_err()
    );
    assert_eq!(sim.server_parts[ServerFullPack::RAM], 1);
}
