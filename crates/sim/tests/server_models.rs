use cloud_provider_sim::*;

const REQUESTED: [&str; 10] = [
    "dell_r740",
    "dell_r750",
    "dell_r760",
    "hpe_dl380_gen10",
    "hpe_dl380_gen11",
    "hpe_dl360_gen11",
    "lenovo_sr650_v2",
    "lenovo_sr650_v3",
    "cisco_c240_m6",
    "supermicro_sys_221h_tnr",
];

#[test]
fn requested_models_have_distinct_platforms_and_complete_recipes() {
    let catalog = server_catalog();
    catalog.validate().unwrap();
    for id in REQUESTED {
        let model = catalog.model(id).expect("requested server is available");
        assert_eq!(model.chassis.cpu_sockets, 2, "{id}");
        assert!(model.full_pack_price(catalog).unwrap() > model.price);
        assert!(!model.artwork.front.path.is_empty());
        assert!(!model.artwork.rear.path.is_empty());
    }
    assert_eq!(catalog.model("hpe_dl360_gen11").unwrap().rack_units, 1);
    assert_eq!(
        catalog.model("dell_r740").unwrap().chassis.cpu_socket,
        "LGA3647"
    );
    assert_eq!(
        catalog.model("dell_r750").unwrap().chassis.cpu_socket,
        "LGA4189"
    );
    assert_eq!(
        catalog.model("dell_r760").unwrap().chassis.cpu_socket,
        "LGA4677"
    );
}

#[test]
fn every_full_pack_uses_its_own_model_and_can_boot() {
    let mut sim = NetworkSim::new();
    sim.money = 1_000_000;
    for model in &server_catalog().models {
        let price = model.full_pack_price(server_catalog()).unwrap();
        let before = sim.money;
        let events = sim
            .execute(Command::BuyServer {
                model_id: model.id.clone(),
                configuration: ServerConfiguration::FullPack,
            })
            .unwrap();
        let SimEvent::DeviceAdded(id) = events[0] else {
            panic!()
        };
        let device = sim.device(id).unwrap();
        let DeviceKind::Server(server) = &device.kind else {
            panic!()
        };
        assert_eq!(server.model.as_str(), model.id);
        assert!(
            server.hardware.as_ref().unwrap().ready_for_model(model),
            "{}",
            model.id
        );
        assert_eq!(device.rack_units(), model.rack_units);
        assert_eq!(before - sim.money, price);
    }
}

#[test]
fn unknown_model_purchase_does_not_change_the_world() {
    let mut sim = NetworkSim::new();
    let before = serde_json::to_string(&sim).unwrap();
    assert!(
        sim.execute(Command::BuyServer {
            model_id: "unknown".into(),
            configuration: ServerConfiguration::FullPack,
        })
        .is_err()
    );
    assert_eq!(serde_json::to_string(&sim).unwrap(), before);
}

fn buy(sim: &mut NetworkSim, model: &str, configuration: ServerConfiguration) -> DeviceId {
    let SimEvent::DeviceAdded(id) = sim
        .execute(Command::BuyServer {
            model_id: model.into(),
            configuration,
        })
        .unwrap()[0]
    else {
        panic!()
    };
    id
}
fn install(sim: &mut NetworkSim, device: DeviceId, part: &str, slot: Option<usize>) {
    sim.execute(Command::BuyServerPart {
        part_id: part.into(),
    })
    .unwrap();
    sim.execute(Command::InstallServerPart {
        device,
        part_id: part.into(),
        slot,
    })
    .unwrap();
}

#[test]
fn cpu_two_dimm_requires_cpu_two_and_prevents_its_removal() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let device = buy(&mut sim, "dell_r740", ServerConfiguration::Chassis);
    install(&mut sim, device, "xeon_silver_4210r", None);
    assert!(
        sim.server_part_installation_slot(device, "ddr4_rdimm_16gb_2933", Some(12))
            .is_err()
    );
    install(&mut sim, device, "xeon_silver_4210r", None);
    install(&mut sim, device, "ddr4_rdimm_16gb_2933", Some(12));
    let before = serde_json::to_string(&sim).unwrap();
    assert!(
        sim.execute(Command::RemoveServerPart {
            device,
            part_id: "xeon_silver_4210r".into(),
            slot: None
        })
        .is_err()
    );
    assert_eq!(serde_json::to_string(&sim).unwrap(), before);
}

#[test]
fn supplied_parts_can_reach_every_represented_memory_capacity() {
    let catalog = server_catalog();
    for model in &catalog.models {
        let cpu = model
            .chassis
            .supported_cpu_ids
            .iter()
            .max_by_key(|id| match &catalog.part(id).unwrap().kind {
                ServerPartKind::Cpu { memory, .. } => memory.max_capacity_gb,
                _ => 0,
            })
            .unwrap();
        let module = model
            .chassis
            .memory
            .supported_modules
            .iter()
            .max_by_key(|m| m.capacity_gb)
            .unwrap();
        let part=catalog.parts.iter().find(|p| matches!(&p.kind,ServerPartKind::Ram{capacity_gb,ranks,voltage_mv,speed_mt_s,memory_type} if *capacity_gb==module.capacity_gb && *ranks==module.ranks && *voltage_mv==module.voltage_mv && module.rated_speeds_mt_s.contains(speed_mt_s) && (module.memory_type.is_empty() || *memory_type==module.memory_type))).unwrap();
        let h = ServerHardware {
            cpus: vec![cpu.clone(); model.chassis.cpu_sockets],
            ram: vec![part.id.clone(); model.chassis.dimm_slots],
            ram_slot_indices: model.chassis.memory.population_order.clone(),
            pcie: vec![None; model.chassis.pcie_slots.len()],
            drives: vec![None; model.chassis.drive_bays.len()],
            ..Default::default()
        };
        h.validate_for_model(model, catalog)
            .unwrap_or_else(|e| panic!("{}: {e}", model.id));
        assert_eq!(
            h.memory_status_for_model(model).unwrap().capacity_gb,
            model.chassis.memory.max_capacity_gb,
            "{}",
            model.id
        );
        assert!(h.ready_for_model(model), "{}", model.id);
    }
}

#[test]
fn incompatible_memory_and_cpu_are_rejected_without_consuming_inventory() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let device = buy(&mut sim, "dell_r760", ServerConfiguration::FullPack);
    for part in ["xeon_silver_4310", "ddr5_ecc_16gb", "ddr4_rdimm_16gb_3200"] {
        sim.execute(Command::BuyServerPart {
            part_id: part.into(),
        })
        .unwrap();
        let before = serde_json::to_string(&sim).unwrap();
        assert!(
            sim.execute(Command::InstallServerPart {
                device,
                part_id: part.into(),
                slot: None
            })
            .is_err(),
            "{part}"
        );
        assert_eq!(serde_json::to_string(&sim).unwrap(), before);
    }
}

#[test]
fn model_identity_round_trips_and_unknown_explicit_identity_is_a_load_error() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let device = buy(&mut sim, "lenovo_sr650_v3", ServerConfiguration::FullPack);
    let mut json = serde_json::to_value(&sim).unwrap();
    let mut loaded: NetworkSim = serde_json::from_value(json.clone()).unwrap();
    loaded.rebuild_indexes();
    let DeviceKind::Server(server) = &loaded.device(device).unwrap().kind else {
        panic!()
    };
    assert_eq!(server.model.as_str(), "lenovo_sr650_v3");
    json["devices"][device.0.to_string()]["kind"]["Server"]["model"] = "missing-model".into();
    assert!(
        serde_json::from_value::<NetworkSim>(json)
            .unwrap_err()
            .to_string()
            .contains("missing-model")
    );
}

#[test]
fn two_unit_server_blocks_both_rack_units() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let a = buy(&mut sim, "dell_r740", ServerConfiguration::Chassis);
    let b = buy(&mut sim, "hpe_dl360_gen11", ServerConfiguration::Chassis);
    sim.execute(Command::PlaceDevice {
        device: a,
        rack: RackId(1),
        unit: 1,
    })
    .unwrap();
    assert!(
        sim.execute(Command::PlaceDevice {
            device: b,
            rack: RackId(1),
            unit: 2
        })
        .is_err()
    );
    sim.execute(Command::PlaceDevice {
        device: b,
        rack: RackId(1),
        unit: 3,
    })
    .unwrap();
    assert!(
        sim.execute(Command::PlaceDevice {
            device: a,
            rack: RackId(1),
            unit: 42
        })
        .is_err()
    );
}

#[test]
fn auto_nic_installation_skips_slots_owned_by_absent_second_cpu() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let device = buy(
        &mut sim,
        "supermicro_sys_221h_tnr",
        ServerConfiguration::Chassis,
    );
    assert!(sim.device(device).unwrap().ports().is_empty());
    install(&mut sim, device, "xeon_silver_4410y", None);
    assert_eq!(
        sim.server_part_installation_slot(device, "intel_i350_t4", None)
            .unwrap(),
        Some(4)
    );
    install(&mut sim, device, "intel_i350_t4", None);
    assert_eq!(sim.device(device).unwrap().ports().len(), 4);
}

#[test]
fn registry_rejects_duplicate_ids_invalid_geometry_and_incompatible_recipes() {
    for scenario in ["duplicate", "geometry", "recipe"] {
        let mut catalog = server_catalog().clone();
        match scenario {
            "duplicate" => catalog.models.push(catalog.models[0].clone()),
            "geometry" => catalog.models[0].power_inlets[0].rect[0] = -0.1,
            _ => catalog.models[0].full_pack.parts[0].part_id = "xeon_silver_4310".into(),
        }
        assert!(catalog.validate_models().is_err(), "{scenario}");
    }
}

#[test]
fn unused_second_cpu_can_be_removed_without_detaching_first_cpu_nic() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let device = buy(&mut sim, "dell_r740", ServerConfiguration::FullPack);
    install(&mut sim, device, "xeon_silver_4210r", None);
    let ports = sim.device(device).unwrap().ports().to_vec();
    sim.execute(Command::RemoveServerPart {
        device,
        part_id: "xeon_silver_4210r".into(),
        slot: None,
    })
    .unwrap();
    assert_eq!(sim.device(device).unwrap().ports(), ports);
}

#[test]
fn memory_maximum_uses_only_slots_owned_by_installed_cpus() {
    let catalog = server_catalog();
    for (model, cpu, ram, expected) in [
        (
            "dell_r740",
            "xeon_platinum_8280l",
            "ddr4_lrdimm_128gb_2666",
            1536,
        ),
        (
            "dell_r750",
            "xeon_silver_4310",
            "ddr4_rdimm_16gb_3200",
            4096,
        ),
    ] {
        let hardware = ServerHardware {
            cpus: vec![cpu.into()],
            ram: vec![ram.into()],
            ..Default::default()
        };
        assert_eq!(
            hardware
                .memory_status_for_model(catalog.model(model).unwrap())
                .unwrap()
                .max_capacity_gb,
            expected
        );
    }
}

#[test]
fn installed_registry_accepts_new_models_and_orders_their_assembly() {
    use std::{fs, process};
    let dir = std::env::temp_dir().join(format!("server-reordered-recipe-{}", process::id()));
    let equipment = dir.join("assets/equipment");
    fs::create_dir_all(&equipment).unwrap();
    let mut data: serde_json::Value =
        serde_json::from_str(include_str!("../../../assets/equipment/servers.json")).unwrap();
    data["models"][0]["full_pack"]["parts"]
        .as_array_mut()
        .unwrap()
        .reverse();
    let mut paired = data["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == "dell_r740")
        .unwrap()
        .clone();
    paired["id"] = "example_pair_server".into();
    paired["chassis"]["memory"]["supported_dimm_counts"] = serde_json::json!([2, 4, 8, 16, 24]);
    data["models"].as_array_mut().unwrap().push(paired);
    fs::write(
        equipment.join("servers.json"),
        serde_json::to_vec(&data).unwrap(),
    )
    .unwrap();
    let output = process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "reordered_recipe_child", "--nocapture"])
        .current_dir(&dir)
        .env("SERVER_REORDERED_RECIPE_CHILD", "1")
        .output()
        .unwrap();
    fs::remove_dir_all(dir).unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn reordered_recipe_child() {
    if std::env::var_os("SERVER_REORDERED_RECIPE_CHILD").is_none() {
        return;
    }
    let mut sim = NetworkSim::new();
    let id = buy(&mut sim, "dell_r360", ServerConfiguration::FullPack);
    let DeviceKind::Server(server) = &sim.device(id).unwrap().kind else {
        panic!()
    };
    assert!(
        server
            .hardware
            .as_ref()
            .unwrap()
            .ready_for_model(server.model())
    );
    // Population requirements govern boot readiness, while individual DIMMs
    // must remain installable during assembly of a supported pair.
    let id = buy(
        &mut sim,
        "example_pair_server",
        ServerConfiguration::Chassis,
    );
    install(&mut sim, id, "xeon_silver_4210r", None);
    install(&mut sim, id, "ddr4_rdimm_16gb_2933", None);
    let DeviceKind::Server(server) = &sim.device(id).unwrap().kind else {
        panic!()
    };
    assert!(
        !server
            .hardware
            .as_ref()
            .unwrap()
            .ready_for_model(server.model())
    );
    install(&mut sim, id, "ddr4_rdimm_16gb_2933", None);
    let DeviceKind::Server(server) = &sim.device(id).unwrap().kind else {
        panic!()
    };
    assert!(
        server
            .hardware
            .as_ref()
            .unwrap()
            .ready_for_model(server.model())
    );
    buy(
        &mut sim,
        "example_pair_server",
        ServerConfiguration::FullPack,
    );
}

#[test]
fn lenovo_v3_standard_and_3ds_dimms_use_their_documented_rates() {
    let catalog = server_catalog();
    let model = catalog.model("lenovo_sr650_v3").unwrap();
    for (ram, primary_speed) in [
        ("ddr5_rdimm_32gb_5600", 5600),
        ("ddr5_rdimm_128gb_5600", 5200),
        ("ddr5_rdimm_256gb_5600", 5200),
    ] {
        let mut hardware = ServerHardware {
            cpus: vec!["xeon_platinum_8592_plus".into()],
            ram: vec![ram.into()],
            ram_slot_indices: vec![0],
            ..Default::default()
        };
        assert_eq!(
            hardware.memory_status_for_model(model).unwrap().speed_mt_s,
            primary_speed
        );
        let secondary = model
            .chassis
            .memory
            .slots
            .iter()
            .position(|s| s.cpu_socket == 0 && s.channel == 0 && s.position == 1)
            .unwrap();
        hardware.ram.push(ram.into());
        hardware.ram_slot_indices.push(secondary);
        assert_eq!(
            hardware.memory_status_for_model(model).unwrap().speed_mt_s,
            4400
        );
    }
}
