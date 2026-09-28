use cloud_provider_sim::{
    Command, DeviceKind, NetworkSim, SimError, SimEvent, TerminalCommand, drive_catalog,
    parse_terminal_command,
};

fn chassis(sim: &mut NetworkSim) -> cloud_provider_sim::DeviceId {
    let SimEvent::DeviceAdded(id) = sim.execute(Command::BuyServerChassis).unwrap()[0] else {
        panic!()
    };
    id
}

#[test]
fn drives_are_purchased_installed_saved_and_returned() {
    let mut sim = NetworkSim::new();
    let id = chassis(&mut sim);
    assert_eq!(sim.power.device_status(id).unwrap().load.watts, 100);
    sim.execute(Command::BuyDrive {
        drive_id: "enterprise_ssd_960gb".into(),
    })
    .unwrap();
    assert_eq!(sim.drive_inventory["enterprise_ssd_960gb"], 1);
    sim.execute(Command::InstallDrive {
        device: id,
        drive_id: "enterprise_ssd_960gb".into(),
        bay: Some(2),
    })
    .unwrap();
    let DeviceKind::Server(server) = &sim.device(id).unwrap().kind else {
        panic!()
    };
    assert_eq!(
        server.hardware.as_ref().unwrap().drives[2].as_deref(),
        Some("enterprise_ssd_960gb")
    );
    assert_eq!(sim.power.device_status(id).unwrap().load.watts, 104);
    let saved = serde_json::to_string(&sim).unwrap();
    let mut loaded: NetworkSim = serde_json::from_str(&saved).unwrap();
    loaded.rebuild_indexes();
    let output = loaded.execute_terminal(id, TerminalCommand::Lsblk);
    assert!(
        output
            .lines
            .iter()
            .any(|line| line.contains("sda") && line.contains("960G"))
    );
    let output = loaded.execute_terminal(
        id,
        TerminalCommand::Smartctl {
            device: "/dev/sda".into(),
            all: true,
        },
    );
    assert!(output.lines.iter().any(|line| line.contains("98000 IOPS")));
    let identity =
        loaded.execute_terminal(id, parse_terminal_command("smartctl -i /dev/sda").unwrap());
    assert!(identity.success && identity.lines.iter().all(|line| !line.contains("IOPS")));
    loaded
        .execute(Command::RemoveDrive { device: id, bay: 2 })
        .unwrap();
    assert_eq!(loaded.drive_inventory["enterprise_ssd_960gb"], 1);
    assert_eq!(loaded.power.device_status(id).unwrap().load.watts, 100);
    assert!(
        !loaded
            .execute_terminal(
                id,
                TerminalCommand::Smartctl {
                    device: "/dev/sda".into(),
                    all: true
                }
            )
            .success
    );
}

#[test]
fn drive_bays_enforce_ownership_capacity_and_compatibility() {
    let mut sim = NetworkSim::new();
    let id = chassis(&mut sim);
    assert!(matches!(
        sim.execute(Command::InstallDrive {
            device: id,
            drive_id: "enterprise_hdd_2tb".into(),
            bay: None
        }),
        Err(SimError::DriveNotOwned(_))
    ));
    assert!(matches!(
        sim.execute(Command::BuyDrive {
            drive_id: "missing".into()
        }),
        Err(SimError::UnknownDrive(_))
    ));
    for bay in 0..4 {
        sim.execute(Command::BuyDrive {
            drive_id: "enterprise_hdd_2tb".into(),
        })
        .unwrap();
        sim.execute(Command::InstallDrive {
            device: id,
            drive_id: "enterprise_hdd_2tb".into(),
            bay: Some(bay),
        })
        .unwrap();
    }
    sim.execute(Command::BuyDrive {
        drive_id: "enterprise_ssd_960gb".into(),
    })
    .unwrap();
    assert!(matches!(
        sim.execute(Command::InstallDrive {
            device: id,
            drive_id: "enterprise_ssd_960gb".into(),
            bay: None
        }),
        Err(SimError::ServerHardware(_))
    ));
    assert_eq!(sim.drive_inventory["enterprise_ssd_960gb"], 1);
    assert_eq!(drive_catalog().drives.len(), 2);
}

#[test]
fn linux_parser_and_terminal_report_network_and_storage() {
    let mut sim = NetworkSim::new();
    let id = chassis(&mut sim);
    assert!(matches!(
        parse_terminal_command("ethtool eth0"),
        Ok(TerminalCommand::Ethtool { .. })
    ));
    assert!(matches!(
        parse_terminal_command("netstat -i"),
        Ok(TerminalCommand::NetstatInterfaces)
    ));
    let iface = sim.execute_terminal(id, parse_terminal_command("ethtool eth0").unwrap());
    assert!(
        iface
            .lines
            .iter()
            .any(|line| line.contains("Link detected: no"))
    );
    let net = sim.execute_terminal(id, parse_terminal_command("netstat -i").unwrap());
    assert!(net.lines.iter().any(|line| line.contains("eth0")));
    for part_id in ["xeon_e_2434", "intel_i350_t4"] {
        sim.execute(Command::BuyServerPart {
            part_id: part_id.into(),
        })
        .unwrap();
        sim.execute(Command::InstallServerPart {
            device: id,
            part_id: part_id.into(),
            slot: None,
        })
        .unwrap();
    }
    let driver = sim.execute_terminal(id, parse_terminal_command("ethtool -i eth3").unwrap());
    assert!(driver.lines.iter().any(|line| line.contains("igb-sim")));
    assert!(parse_terminal_command("show version").is_err());
    let cpu = sim.execute_terminal(id, parse_terminal_command("lscpu").unwrap());
    assert!(cpu.lines.iter().any(|line| line.contains("Xeon E-2434")));
    let disks = sim.execute_terminal(id, parse_terminal_command("lsblk").unwrap());
    assert_eq!(disks.lines.len(), 1);
    let missing = sim.execute_terminal(id, parse_terminal_command("smartctl -a /dev/sda").unwrap());
    assert!(!missing.success);
}

#[test]
fn selling_a_chassis_returns_installed_drives_to_inventory() {
    let mut sim = NetworkSim::new();
    let id = chassis(&mut sim);
    sim.execute(Command::BuyDrive {
        drive_id: "enterprise_hdd_2tb".into(),
    })
    .unwrap();
    sim.execute(Command::InstallDrive {
        device: id,
        drive_id: "enterprise_hdd_2tb".into(),
        bay: Some(0),
    })
    .unwrap();
    sim.execute(Command::SellDevice { device: id }).unwrap();
    assert_eq!(sim.drive_inventory["enterprise_hdd_2tb"], 1);
}
