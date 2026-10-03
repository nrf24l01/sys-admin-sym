use cloud_provider_sim::{
    Command, DeviceKind, DeviceTemplate, NetworkSim, ServerHardware, ServerPartKind, server_catalog,
};

#[test]
fn memory_score_accounts_for_ddr_generation_and_module_type() {
    let score = |memory_type: &str| {
        ServerPartKind::Ram {
            memory_type: memory_type.into(),
            capacity_gb: 30,
        }
        .memory_score_gb()
        .unwrap()
    };
    assert_eq!(score("DDR3 UDIMM"), 30);
    assert_eq!(score("DDR4 RDIMM"), 48);
    assert_eq!(score("DDR5 RDIMM"), 60);
    assert_eq!(score("DDR5 DIMM"), 45);
}

#[test]
fn integrated_power_supply_allows_assembled_server_to_be_ready() {
    let hardware = ServerHardware {
        cpus: vec!["xeon_e_2434".into()],
        ram: vec!["ddr5_ecc_16gb".into()],
        ..Default::default()
    };
    assert!(hardware.ready());
    assert_eq!(hardware.compute_mhz(), 4 * 3400);
    assert_eq!(hardware.memory_score_gb(), 16 * 5 * 10 / 30);
    assert_eq!(server_catalog().chassis.integrated_psu_watts, 600);
}

#[test]
fn server_has_dedicated_management_nic_and_no_network_credit_when_disconnected() {
    let mut sim = NetworkSim::new();
    let id = match sim
        .execute(Command::BuyDevice {
            kind: DeviceTemplate::Server,
        })
        .unwrap()
        .as_slice()
    {
        [cloud_provider_sim::SimEvent::DeviceAdded(id)] => *id,
        _ => panic!("server purchase event missing"),
    };
    let device = sim.device(id).unwrap();
    let DeviceKind::Server(server) = &device.kind else {
        panic!("expected server")
    };
    assert!(
        server
            .ports
            .iter()
            .any(|id| sim.port(*id).unwrap().name == "mgmt0")
    );
    assert_eq!(sim.datacenter_resources().lan.compute_mhz, 0);
    assert_eq!(sim.datacenter_resources().global.compute_mhz, 0);
    let serialized = ron::to_string(&sim).unwrap();
    let mut reloaded: NetworkSim = ron::from_str(&serialized).unwrap();
    reloaded.rebuild_indexes();
    assert_eq!(
        reloaded.network_outlets().count(),
        sim.network_outlets().count()
    );
}

#[test]
fn physical_room_sockets_gate_resource_credit() {
    use cloud_provider_sim::{
        CableSupply, NetworkOutletKind, OutletId, PowerEndpoint, RackId, SimEvent, SourceId,
    };
    let mut sim = NetworkSim::new();
    let SimEvent::DeviceAdded(id) = sim.execute(Command::BuyServerChassis).unwrap()[0] else {
        panic!("server purchase failed")
    };
    for part_id in ["xeon_e_2434", "ddr5_ecc_16gb"] {
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
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::CableBox305m,
    })
    .unwrap();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::Rj45Pack20,
    })
    .unwrap();
    let DeviceKind::Server(server) = &sim.device(id).unwrap().kind else {
        panic!("server missing")
    };
    let data_port = server.ports[0];
    let lan = sim
        .network_outlets()
        .find(|outlet| outlet.kind == NetworkOutletKind::Lan { rack: RackId(1) })
        .unwrap()
        .port;
    let uplink = sim
        .network_outlets()
        .find(|outlet| matches!(outlet.kind, NetworkOutletKind::Uplink { .. }))
        .unwrap()
        .port;
    assert_eq!(sim.datacenter_resources().lan.compute_mhz, 0);
    let cable = sim
        .execute(Command::ConnectCable {
            a: data_port,
            b: lan,
            length_cm: 1000,
        })
        .unwrap();
    assert_eq!(sim.datacenter_resources().lan.compute_mhz, 13600);
    assert_eq!(sim.datacenter_resources().global.compute_mhz, 0);
    let SimEvent::LinkCreated(link) = cable[0] else {
        panic!("link missing")
    };
    sim.execute(Command::Disconnect { link }).unwrap();
    assert_eq!(sim.datacenter_resources().lan.compute_mhz, 0);
    let DeviceKind::Server(server) = &sim.device(id).unwrap().kind else {
        panic!("server missing")
    };
    let management = server
        .ports
        .iter()
        .find(|port| sim.port(**port).unwrap().name == "mgmt0")
        .copied()
        .unwrap();
    let cable = sim
        .execute(Command::ConnectCable {
            a: management,
            b: lan,
            length_cm: 1000,
        })
        .unwrap();
    assert_eq!(sim.datacenter_resources().lan.compute_mhz, 0);
    let SimEvent::LinkCreated(link) = cable[0] else {
        panic!("link missing")
    };
    sim.execute(Command::Disconnect { link }).unwrap();
    let cable = sim
        .execute(Command::ConnectCable {
            a: data_port,
            b: uplink,
            length_cm: 1000,
        })
        .unwrap();
    assert_eq!(sim.datacenter_resources().global.compute_mhz, 13600);
    let SimEvent::LinkCreated(link) = cable[0] else {
        panic!("link missing")
    };
    sim.execute(Command::Disconnect { link }).unwrap();
    let SimEvent::DeviceAdded(switch) = sim
        .execute(Command::BuyDevice {
            kind: DeviceTemplate::Switch,
        })
        .unwrap()[0]
    else {
        panic!("switch missing")
    };
    sim.execute(Command::PlaceDevice {
        device: switch,
        rack: RackId(1),
        unit: 2,
    })
    .unwrap();
    sim.execute(Command::ConnectPower {
        outlet: OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 1,
        },
        endpoint: PowerEndpoint::Device(switch),
    })
    .unwrap();
    sim.execute(Command::SetPower {
        device: switch,
        powered: true,
    })
    .unwrap();
    let ports = sim.device(switch).unwrap().ports().to_vec();
    sim.execute(Command::ConnectCable {
        a: data_port,
        b: ports[0],
        length_cm: 1000,
    })
    .unwrap();
    sim.execute(Command::ConnectCable {
        a: ports[1],
        b: uplink,
        length_cm: 1000,
    })
    .unwrap();
    assert_eq!(sim.datacenter_resources().global.compute_mhz, 13600);
}

#[test]
fn rack_lan_sockets_form_a_shared_management_network() {
    use cloud_provider_sim::{
        CableSupply, Ipv4InterfaceConfig, NetworkOutletKind, OutletId, PowerEndpoint, RackId,
        SimEvent, SourceId, VlanId,
    };
    let mut sim = NetworkSim::new();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::CableBox305m,
    })
    .unwrap();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::Rj45Pack20,
    })
    .unwrap();
    let mut ports = Vec::new();
    for (rack, address) in [(RackId(1), "192.0.2.1"), (RackId(2), "192.0.2.2")] {
        let SimEvent::DeviceAdded(id) = sim
            .execute(Command::BuyDevice {
                kind: DeviceTemplate::Server,
            })
            .unwrap()[0]
        else {
            panic!("server missing")
        };
        sim.execute(Command::PlaceDevice {
            device: id,
            rack,
            unit: 1,
        })
        .unwrap();
        sim.execute(Command::ConnectPower {
            outlet: OutletId {
                source: SourceId::Rack(rack),
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
            panic!("server missing")
        };
        let management = server
            .ports
            .iter()
            .find(|id| sim.port(**id).unwrap().name == "mgmt0")
            .copied()
            .unwrap();
        let lan = sim
            .network_outlets()
            .find(|outlet| outlet.kind == NetworkOutletKind::Lan { rack })
            .unwrap()
            .port;
        sim.execute(Command::SetIpv4 {
            port: management,
            config: Ipv4InterfaceConfig::new(address.parse().unwrap(), 24, None, VlanId(1)),
        })
        .unwrap();
        sim.execute(Command::ConnectCable {
            a: management,
            b: lan,
            length_cm: 1000,
        })
        .unwrap();
        ports.push((id, management));
    }
    assert!(sim.ping(ports[0].1, "192.0.2.2".parse().unwrap()).reachable);
    let ssh = sim.execute_console(ports[0].0, "ssh 192.0.2.2");
    assert!(ssh.success, "{:?}", ssh.lines);
    assert!(sim.terminal_prompt(ports[0].0).starts_with("ssh:"));
    assert!(sim.execute_console(ports[0].0, "hostname").success);
    assert!(sim.execute_console(ports[0].0, "exit").success);
    let SimEvent::DeviceAdded(switch) = sim
        .execute(Command::BuyDevice {
            kind: DeviceTemplate::Switch,
        })
        .unwrap()[0]
    else {
        panic!("switch missing")
    };
    sim.execute(Command::PlaceDevice {
        device: switch,
        rack: RackId(3),
        unit: 1,
    })
    .unwrap();
    sim.execute(Command::ConnectPower {
        outlet: OutletId {
            source: SourceId::Rack(RackId(3)),
            index: 0,
        },
        endpoint: PowerEndpoint::Device(switch),
    })
    .unwrap();
    sim.execute(Command::SetPower {
        device: switch,
        powered: true,
    })
    .unwrap();
    let switch_port = sim.device(switch).unwrap().ports()[0];
    let rack_lan = sim
        .network_outlets()
        .find(|outlet| outlet.kind == NetworkOutletKind::Lan { rack: RackId(3) })
        .unwrap()
        .port;
    sim.execute(Command::ConnectCable {
        a: switch_port,
        b: rack_lan,
        length_cm: 1000,
    })
    .unwrap();
    for command in ["enable", "configure terminal", "management ip 192.0.2.3"] {
        let output = sim.execute_console(switch, command);
        assert!(output.success, "{command}: {:?}", output.lines);
    }
    let ssh = sim.execute_console(ports[0].0, "ssh 192.0.2.3");
    assert!(ssh.success, "{:?}", ssh.lines);
    assert!(sim.execute_console(ports[0].0, "end").success);
    assert!(
        sim.execute_console(ports[0].0, "show interfaces status")
            .success
    );
    assert!(sim.execute_console(ports[0].0, "exit").success);
    let SimEvent::DeviceAdded(router) = sim
        .execute(Command::BuyDevice {
            kind: DeviceTemplate::Router,
        })
        .unwrap()[0]
    else {
        panic!("router missing")
    };
    sim.execute(Command::PlaceDevice {
        device: router,
        rack: RackId(4),
        unit: 1,
    })
    .unwrap();
    sim.execute(Command::ConnectPower {
        outlet: OutletId {
            source: SourceId::Rack(RackId(4)),
            index: 0,
        },
        endpoint: PowerEndpoint::Device(router),
    })
    .unwrap();
    sim.execute(Command::SetPower {
        device: router,
        powered: true,
    })
    .unwrap();
    let router_port = sim.device(router).unwrap().ports()[2];
    let rack_lan = sim
        .network_outlets()
        .find(|outlet| outlet.kind == NetworkOutletKind::Lan { rack: RackId(4) })
        .unwrap()
        .port;
    sim.execute(Command::ConfigureRouterInterface {
        port: router_port,
        name: "MGMT".into(),
        vlan: Some(VlanId(1)),
        address: Some("192.0.2.4".parse().unwrap()),
        prefix: 24,
        internet_connected: false,
    })
    .unwrap();
    sim.execute(Command::ConnectCable {
        a: router_port,
        b: rack_lan,
        length_cm: 1000,
    })
    .unwrap();
    let ssh = sim.execute_console(ports[0].0, "ssh 192.0.2.4");
    assert!(ssh.success, "{:?}", ssh.lines);
}
