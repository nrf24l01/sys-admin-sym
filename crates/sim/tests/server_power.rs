use cloud_provider_sim::*;

fn outlet(rack: RackId, index: u8) -> OutletId {
    OutletId {
        source: SourceId::Rack(rack),
        index,
    }
}
fn powered_server() -> (NetworkSim, DeviceId, RackId) {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let SimEvent::DeviceAdded(device) = sim.execute(Command::BuyServerFullPack).unwrap()[0] else {
        panic!()
    };
    sim.execute(Command::PlaceDevice {
        device,
        rack: RackId(1),
        unit: 1,
    })
    .unwrap();
    let other = sim.add_rack("B feed", 42);
    sim.execute(Command::ConnectPower {
        outlet: outlet(RackId(1), 0),
        endpoint: PowerEndpoint::Device(device),
    })
    .unwrap();
    sim.execute(Command::ConnectPower {
        outlet: outlet(other, 0),
        endpoint: PowerEndpoint::device_inlet(device, 1),
    })
    .unwrap();
    (sim, device, other)
}

#[test]
fn two_psus_share_dc_load_and_report_each_feeds_ac_conversion_losses() {
    let (sim, device, _) = powered_server();
    let a = sim.power.psu_telemetry(device, 0).unwrap();
    let b = sim.power.psu_telemetry(device, 1).unwrap();
    assert!(a.input_available && b.input_available);
    assert!(a.output_mw.abs_diff(b.output_mw) <= 1);
    let psus = sim.power.devices[&device].psus.as_ref().unwrap();
    assert_eq!(a.output_mw + b.output_mw, psus.dc_demand_mw);
    assert_eq!(
        a.input.watts + b.input.watts,
        sim.device_consumption(device).unwrap().current.watts
    );
    assert_eq!(
        a.input.watts,
        psus.profile.input_mw(a.output_mw).div_ceil(1000)
    );
    assert!(a.input.watts * 1000 > a.output_mw);
    assert!(a.input.watts < psus.profile.capacity_watts / 2);
}

#[test]
fn a_feed_outage_transfers_load_without_reboot_and_both_feeds_lost_stop_guest() {
    let (mut sim, device, other) = powered_server();
    assert!(
        sim.execute_console(
            device,
            "ip addr add 192.0.2.2/24 dev eth0; systemctl disable ssh"
        )
        .success
    );
    let port = sim.device(device).unwrap().ports()[0];
    sim.execute(Command::SetRackMains {
        rack: RackId(1),
        on: false,
    })
    .unwrap();
    assert!(sim.device(device).unwrap().powered);
    assert!(sim.server_os(device).unwrap().services["ssh"].active);
    assert!(matches!(&sim.port(port).unwrap().config, PortConfig::Server(c) if c.ipv4.is_some()));
    let psus = sim.power.devices[&device].psus.as_ref().unwrap();
    assert_eq!(
        sim.power.psu_telemetry(device, 1).unwrap().output_mw,
        psus.dc_demand_mw
    );
    assert_eq!(sim.power.psu_telemetry(device, 0).unwrap().input.watts, 0);
    sim.execute(Command::SetRackMains {
        rack: other,
        on: false,
    })
    .unwrap();
    assert!(!sim.device(device).unwrap().powered);
    assert!(sim.power.devices[&device].requested);
    assert!(
        sim.server_os(device)
            .unwrap()
            .services
            .values()
            .all(|s| !s.active)
    );
    assert!(!sim.execute_console(device, "echo offline").success);
    sim.execute(Command::SetRackMains {
        rack: other,
        on: true,
    })
    .unwrap();
    assert!(sim.device(device).unwrap().powered);
    assert!(!sim.server_os(device).unwrap().services["ssh"].active);
    assert!(sim.server_os(device).unwrap().services["systemd-resolved"].active);
    assert!(matches!(&sim.port(port).unwrap().config, PortConfig::Server(c) if c.ipv4.is_none()));
}

#[test]
fn second_psu_connections_survive_save_and_remove_without_dangling_cords() {
    let (sim, device, other) = powered_server();
    let serialized = ron::to_string(&sim).unwrap();
    let mut loaded: NetworkSim = ron::from_str(&serialized).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.power.connections, sim.power.connections);
    assert!(loaded.device(device).unwrap().powered);
    assert_eq!(
        loaded
            .power
            .outlet_for_endpoint(PowerEndpoint::device_inlet(device, 1)),
        Some(outlet(other, 0))
    );
    loaded.execute(Command::RemoveDevice { device }).unwrap();
    assert!(
        loaded
            .power
            .connections
            .values()
            .all(|e| e.device_id() != Some(device))
    );
    assert!(
        loaded
            .power
            .cord_routes
            .keys()
            .all(|o| loaded.power.connections.contains_key(o))
    );
}

#[test]
fn inlet_aliases_and_nonexistent_psus_cannot_connect_twice_or_spend_state() {
    let (mut sim, device, other) = powered_server();
    let original = sim.power.connections.clone();
    for inlet in [0, 1, 2, u8::MAX] {
        assert!(
            sim.execute(Command::ConnectPower {
                outlet: outlet(other, 1),
                endpoint: PowerEndpoint::DevicePsu { device, inlet }
            })
            .is_err()
        );
        assert_eq!(sim.power.connections, original);
    }
}

#[test]
fn cords_on_the_same_source_do_not_protect_against_its_outage() {
    let (mut sim, device, other) = powered_server();
    sim.execute(Command::DisconnectPower {
        outlet: outlet(other, 0),
    })
    .unwrap();
    sim.execute(Command::ConnectPower {
        outlet: outlet(RackId(1), 1),
        endpoint: PowerEndpoint::device_inlet(device, 1),
    })
    .unwrap();
    sim.execute(Command::SetRackMains {
        rack: RackId(1),
        on: false,
    })
    .unwrap();
    assert!(!sim.device(device).unwrap().powered);
}

#[test]
fn failover_overload_trips_a_source_already_visited_in_the_same_update() {
    let mut power = PowerSystem::new();
    let device = DeviceId(1);
    power.add_device(device, 0, 100);
    let profile: PsuPowerProfile = serde_json::from_str(
        r#"{"capacity_watts":600,"efficiency_permille":[[0,1000],[1000,1000]]}"#,
    )
    .unwrap();
    power.devices.get_mut(&device).unwrap().psus = Some(DevicePsus {
        count: 2,
        profile,
        dc_demand_mw: 0,
    });
    let b = SourceId::Ups(power.add_ups(UpsSpec {
        watts: 70,
        va: 100,
        ..Default::default()
    }));
    let a = SourceId::Ups(power.add_ups(UpsSpec {
        watts: 40,
        va: 100,
        ..Default::default()
    }));
    power
        .connect(
            OutletId {
                source: b,
                index: 0,
            },
            PowerEndpoint::Device(device),
        )
        .unwrap();
    power
        .connect(
            OutletId {
                source: a,
                index: 0,
            },
            PowerEndpoint::device_inlet(device, 1),
        )
        .unwrap();
    power
        .devices
        .get_mut(&device)
        .unwrap()
        .psus
        .as_mut()
        .unwrap()
        .dc_demand_mw = 90_000;
    power.recompute_now();
    assert!(power.ups.values().all(|u| u.tripped));
    assert!(!power.devices[&device].effective);
}

#[test]
fn surviving_psu_cannot_deliver_more_than_its_configured_dc_rating() {
    let (mut sim, device, other) = powered_server();
    sim.power
        .devices
        .get_mut(&device)
        .unwrap()
        .psus
        .as_mut()
        .unwrap()
        .dc_demand_mw = 800_000;
    sim.power.recompute_now();
    assert!(sim.power.devices[&device].effective);
    sim.power.set_rack_mains(other, false);
    assert!(!sim.power.devices[&device].effective);
    assert_eq!(sim.power.psu_telemetry(device, 0).unwrap().output_mw, 0);
}

#[test]
fn full_outage_discards_ssh_sessions_even_if_power_returns_before_next_command() {
    let (mut sim, device, other) = powered_server();
    let SimEvent::DeviceAdded(remote) = sim
        .execute(Command::BuyDevice {
            kind: DeviceTemplate::Server,
        })
        .unwrap()[0]
    else {
        panic!()
    };
    sim.execute(Command::PlaceDevice {
        device: remote,
        rack: other,
        unit: 2,
    })
    .unwrap();
    sim.execute(Command::ConnectPower {
        outlet: outlet(other, 1),
        endpoint: PowerEndpoint::Device(remote),
    })
    .unwrap();
    let a = sim.device(device).unwrap().ports()[0];
    let b = sim.device(remote).unwrap().ports()[0];
    for supply in [CableSupply::CableBox305m, CableSupply::Rj45Pack20] {
        sim.execute(Command::BuyCableSupply { supply }).unwrap();
    }
    sim.execute(Command::Connect { a, b }).unwrap();
    assert!(
        sim.execute_console(device, "ip addr add 192.0.2.1/24 dev eth0")
            .success
    );
    assert!(
        sim.execute_console(
            remote,
            "ip addr add 192.0.2.2/24 dev eth0; ip addr add 192.0.2.20/24 dev eth0"
        )
        .success
    );
    assert!(sim.execute_console(device, "ssh 192.0.2.20").success);
    assert!(sim.execute_console(device, "hostname").lines[0].contains("server02"));
    sim.execute(Command::SetPower {
        device: remote,
        powered: false,
    })
    .unwrap();
    sim.execute(Command::SetPower {
        device: remote,
        powered: true,
    })
    .unwrap();
    assert!(sim.execute_console(device, "hostname").lines[0].contains("server01"));
}

#[test]
fn restored_power_reloads_persistent_network_configuration_and_disk_files() {
    let (mut sim, device, other) = powered_server();
    assert!(sim.execute_console(device, "printf 'auto eth1\niface eth1 inet static\n address 192.0.2.3/24\n' > /etc/network/interfaces; echo persistent > /root/data; ifup eth1").success);
    sim.execute(Command::SetRackMains {
        rack: RackId(1),
        on: false,
    })
    .unwrap();
    sim.execute(Command::SetRackMains {
        rack: other,
        on: false,
    })
    .unwrap();
    sim.execute(Command::SetRackMains {
        rack: other,
        on: true,
    })
    .unwrap();
    assert!(
        sim.execute_console(device, "ip -br addr show dev eth1")
            .lines
            .iter()
            .any(|line| line.contains("192.0.2.3/24"))
    );
    assert_eq!(
        sim.execute_console(device, "cat /root/data").lines,
        ["persistent"]
    );
}

#[test]
fn a_server_that_has_never_opened_a_console_still_loses_volatile_ips_on_outage() {
    let (mut sim, device, other) = powered_server();
    let port = sim.device(device).unwrap().ports()[0];
    sim.execute(Command::SetIpv4 {
        port,
        config: Ipv4InterfaceConfig::new("192.0.2.2".parse().unwrap(), 24, None, VlanId(1)),
    })
    .unwrap();
    assert!(sim.server_os(device).is_none());
    sim.execute(Command::SetRackMains {
        rack: RackId(1),
        on: false,
    })
    .unwrap();
    sim.execute(Command::SetRackMains {
        rack: other,
        on: false,
    })
    .unwrap();
    assert!(matches!(&sim.port(port).unwrap().config, PortConfig::Server(c) if c.ipv4.is_none()));
}
