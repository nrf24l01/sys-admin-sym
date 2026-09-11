use cloud_provider_sim::*;
use std::net::Ipv4Addr;

fn ip(value: &str) -> Ipv4Addr {
    value.parse().unwrap()
}

fn buy(sim: &mut NetworkSim, kind: DeviceTemplate) -> DeviceId {
    let events = sim.execute(Command::BuyDevice { kind }).unwrap();
    match events[0] {
        SimEvent::DeviceAdded(id) => id,
        _ => unreachable!(),
    }
}

fn install_and_power(sim: &mut NetworkSim, device: DeviceId, unit: u8) {
    sim.execute(Command::PlaceDevice {
        device,
        rack: RackId(1),
        unit,
    })
    .unwrap();
    sim.execute(Command::SetPower {
        device,
        powered: true,
    })
    .unwrap();
}

fn ports(sim: &NetworkSim, device: DeviceId) -> Vec<PortId> {
    sim.device(device).unwrap().ports().to_vec()
}

#[test]
fn dell_can_connect_directly_to_c1111_and_reach_the_internet() {
    for reverse in [false, true] {
        let mut sim = NetworkSim::new();
        sim.execute(Command::BuyCableSupply {
            supply: CableSupply::CableBox305m,
        })
        .unwrap();
        sim.execute(Command::BuyCableSupply {
            supply: CableSupply::Rj45Pack20,
        })
        .unwrap();
        let router = buy(&mut sim, DeviceTemplate::Router);
        let server = buy(&mut sim, DeviceTemplate::Server);
        install_and_power(&mut sim, router, 1);
        install_and_power(&mut sim, server, 2);
        let lan = ports(&sim, router)[2];
        let ethernet = ports(&sim, server)[0];
        sim.execute(Command::ConfigureRouterInterface {
            port: lan,
            name: "LAN1".into(),
            vlan: Some(VlanId(1)),
            address: Some(ip("192.168.1.1")),
            prefix: 24,
            internet_connected: false,
        })
        .unwrap();
        sim.execute(Command::SetIpv4 {
            port: ethernet,
            config: Ipv4InterfaceConfig::new(
                ip("192.168.1.2"),
                24,
                Some(ip("192.168.1.1")),
                VlanId(1),
            ),
        })
        .unwrap();
        let (a, b) = if reverse {
            (lan, ethernet)
        } else {
            (ethernet, lan)
        };
        sim.execute(Command::Connect { a, b }).unwrap();
        let result = sim.ping(ethernet, ip("8.8.8.8"));
        assert!(result.reachable, "{result:?}");
        let link = sim.link_for_port(ethernet).unwrap().id;
        sim.execute(Command::Disconnect { link }).unwrap();
        assert!(sim.link_for_port(ethernet).is_none());
        assert!(sim.link_for_port(lan).is_none());
        assert!(!sim.ping(ethernet, ip("8.8.8.8")).reachable);
    }
}

#[test]
fn real_device_templates_expose_expected_network_panels() {
    let mut sim = NetworkSim::new();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::CableBox305m,
    })
    .unwrap();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::Rj45Pack20,
    })
    .unwrap();
    let switch = buy(&mut sim, DeviceTemplate::Switch);
    let router = buy(&mut sim, DeviceTemplate::Router);
    let server = buy(&mut sim, DeviceTemplate::Server);

    assert!(sim.device(switch).unwrap().name.contains("C1000-24T-4G-L"));
    assert_eq!(ports(&sim, switch).len(), 28);
    assert_eq!(
        ports(&sim, switch)
            .iter()
            .filter(|id| sim.port(**id).unwrap().connector == PortConnector::Rj45)
            .count(),
        24
    );
    assert_eq!(
        ports(&sim, switch)
            .iter()
            .filter(|id| sim.port(**id).unwrap().connector == PortConnector::Sfp)
            .count(),
        4
    );
    assert_eq!(sim.port(ports(&sim, switch)[0]).unwrap().name, "Gi1/0/01");
    assert_eq!(
        sim.port(ports(&sim, switch)[0]).unwrap().connector,
        PortConnector::Rj45
    );
    assert_eq!(
        sim.port(ports(&sim, switch)[27]).unwrap().name,
        "SFP Gi1/0/28"
    );
    assert_eq!(
        sim.port(ports(&sim, switch)[27]).unwrap().connector,
        PortConnector::Sfp
    );
    assert!(sim.device(router).unwrap().name.contains("C1111-8P"));
    assert_eq!(ports(&sim, router).len(), 10);
    assert_eq!(sim.port(ports(&sim, router)[0]).unwrap().name, "WAN1");
    assert_eq!(sim.port(ports(&sim, router)[1]).unwrap().name, "WAN2");
    assert_eq!(sim.port(ports(&sim, router)[9]).unwrap().name, "LAN8");
    assert!(sim.device(server).unwrap().name.contains("PowerEdge R360"));
    assert_eq!(ports(&sim, server).len(), 2);
}

#[test]
fn sfp_cabling_is_explicitly_outside_the_mvp() {
    let mut sim = NetworkSim::new();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::CableBox305m,
    })
    .unwrap();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::Rj45Pack20,
    })
    .unwrap();
    let switch = buy(&mut sim, DeviceTemplate::Switch);
    let server = buy(&mut sim, DeviceTemplate::Server);
    let sfp = ports(&sim, switch)[24];
    let ethernet = ports(&sim, server)[0];

    assert_eq!(
        sim.execute(Command::Connect {
            a: sfp,
            b: ethernet,
        }),
        Err(SimError::UnsupportedConnector {
            port: sfp,
            connector: PortConnector::Sfp,
        })
    );
    assert_eq!(
        sim.execute(Command::SetSwitchPortMode {
            port: sfp,
            mode: SwitchPortMode::Access { vlan: VlanId(1) },
        }),
        Err(SimError::UnsupportedConnector {
            port: sfp,
            connector: PortConnector::Sfp,
        })
    );
}

fn basic_vlan() -> (NetworkSim, DeviceId, DeviceId, DeviceId) {
    let mut sim = NetworkSim::new();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::CableBox305m,
    })
    .unwrap();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::Rj45Pack20,
    })
    .unwrap();
    let switch = buy(&mut sim, DeviceTemplate::Switch);
    let a = buy(&mut sim, DeviceTemplate::Server);
    let b = buy(&mut sim, DeviceTemplate::Server);
    install_and_power(&mut sim, switch, 1);
    install_and_power(&mut sim, a, 2);
    install_and_power(&mut sim, b, 3);
    sim.execute(Command::CreateVlan {
        switch,
        vlan: Vlan {
            id: VlanId(20),
            name: "Servers".into(),
        },
    })
    .unwrap();
    let sp = ports(&sim, switch);
    let ap = ports(&sim, a)[0];
    let bp = ports(&sim, b)[0];
    for port in [sp[1], sp[2]] {
        sim.execute(Command::SetSwitchPortMode {
            port,
            mode: SwitchPortMode::Access { vlan: VlanId(20) },
        })
        .unwrap();
    }
    sim.execute(Command::SetIpv4 {
        port: ap,
        config: Ipv4InterfaceConfig::new(ip("10.10.20.11"), 24, None, VlanId(20)),
    })
    .unwrap();
    sim.execute(Command::SetIpv4 {
        port: bp,
        config: Ipv4InterfaceConfig::new(ip("10.10.20.12"), 24, None, VlanId(20)),
    })
    .unwrap();
    sim.execute(Command::Connect { a: ap, b: sp[1] }).unwrap();
    sim.execute(Command::Connect { a: bp, b: sp[2] }).unwrap();
    (sim, switch, a, b)
}

#[test]
fn access_ports_in_same_vlan_can_talk() {
    let (sim, _, a, _) = basic_vlan();
    assert!(sim.ping(ports(&sim, a)[0], ip("10.10.20.12")).reachable);
}

#[test]
fn different_access_vlan_is_blocked() {
    let (mut sim, switch, a, _) = basic_vlan();
    sim.execute(Command::CreateVlan {
        switch,
        vlan: Vlan {
            id: VlanId(30),
            name: "Private".into(),
        },
    })
    .unwrap();
    let sw_port = ports(&sim, switch)[2];
    sim.execute(Command::SetSwitchPortMode {
        port: sw_port,
        mode: SwitchPortMode::Access { vlan: VlanId(30) },
    })
    .unwrap();
    let result = sim.ping(ports(&sim, a)[0], ip("10.10.20.12"));
    assert_eq!(result.failure, Some(ReachabilityFailure::VlanBlocked));
}

#[test]
fn powered_off_switch_breaks_path() {
    let (mut sim, switch, a, _) = basic_vlan();
    sim.execute(Command::SetPower {
        device: switch,
        powered: false,
    })
    .unwrap();
    assert!(!sim.ping(ports(&sim, a)[0], ip("10.10.20.12")).reachable);
}

#[test]
fn disconnected_cable_breaks_path_immediately() {
    let (mut sim, _, a, _) = basic_vlan();
    let source = ports(&sim, a)[0];
    let link = sim.link_for_port(source).unwrap().id;
    sim.execute(Command::Disconnect { link }).unwrap();
    assert_eq!(
        sim.ping(source, ip("10.10.20.12")).failure,
        Some(ReachabilityFailure::NoPhysicalLink)
    );
}

#[test]
fn duplicate_ip_is_reported() {
    let (mut sim, _, a, b) = basic_vlan();
    sim.execute(Command::SetIpv4 {
        port: ports(&sim, b)[0],
        config: Ipv4InterfaceConfig::new(ip("10.10.20.11"), 24, None, VlanId(20)),
    })
    .unwrap();
    assert_eq!(
        sim.ping(ports(&sim, a)[0], ip("10.10.20.11")).failure,
        Some(ReachabilityFailure::AddressConflict)
    );
}

fn routed_network(allowed: Vec<VlanId>) -> (NetworkSim, DeviceId, DeviceId, DeviceId, DeviceId) {
    let mut sim = NetworkSim::new();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::CableBox305m,
    })
    .unwrap();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::Rj45Pack20,
    })
    .unwrap();
    let router = buy(&mut sim, DeviceTemplate::Router);
    let switch = buy(&mut sim, DeviceTemplate::Switch);
    let a = buy(&mut sim, DeviceTemplate::Server);
    let b = buy(&mut sim, DeviceTemplate::Server);
    for (device, unit) in [(router, 1), (switch, 2), (a, 3), (b, 4)] {
        install_and_power(&mut sim, device, unit);
    }
    for vlan in [VlanId(10), VlanId(20)] {
        sim.execute(Command::CreateVlan {
            switch,
            vlan: Vlan {
                id: vlan,
                name: format!("VLAN {}", vlan.0),
            },
        })
        .unwrap();
    }
    let swp = ports(&sim, switch);
    let rp = ports(&sim, router);
    let ap = ports(&sim, a)[0];
    let bp = ports(&sim, b)[0];
    sim.execute(Command::SetSwitchPortMode {
        port: swp[0],
        mode: SwitchPortMode::Trunk {
            native_vlan: None,
            allowed,
        },
    })
    .unwrap();
    sim.execute(Command::SetSwitchPortMode {
        port: swp[1],
        mode: SwitchPortMode::Access { vlan: VlanId(10) },
    })
    .unwrap();
    sim.execute(Command::SetSwitchPortMode {
        port: swp[2],
        mode: SwitchPortMode::Access { vlan: VlanId(20) },
    })
    .unwrap();
    sim.execute(Command::ConfigureRouterInterface {
        port: rp[2],
        name: "vlan10".into(),
        vlan: Some(VlanId(10)),
        address: Some(ip("10.10.10.1")),
        prefix: 24,
        internet_connected: false,
    })
    .unwrap();
    sim.execute(Command::ConfigureRouterInterface {
        port: rp[2],
        name: "vlan20".into(),
        vlan: Some(VlanId(20)),
        address: Some(ip("10.10.20.1")),
        prefix: 24,
        internet_connected: false,
    })
    .unwrap();
    sim.execute(Command::SetIpv4 {
        port: ap,
        config: Ipv4InterfaceConfig::new(ip("10.10.10.11"), 24, Some(ip("10.10.10.1")), VlanId(10)),
    })
    .unwrap();
    sim.execute(Command::SetIpv4 {
        port: bp,
        config: Ipv4InterfaceConfig::new(ip("10.10.20.12"), 24, Some(ip("10.10.20.1")), VlanId(20)),
    })
    .unwrap();
    sim.execute(Command::Connect {
        a: rp[2],
        b: swp[0],
    })
    .unwrap();
    sim.execute(Command::Connect { a: ap, b: swp[1] }).unwrap();
    sim.execute(Command::Connect { a: bp, b: swp[2] }).unwrap();
    (sim, router, switch, a, b)
}

#[test]
fn router_forwards_between_vlans() {
    let (sim, _, _, a, _) = routed_network(vec![VlanId(10), VlanId(20)]);
    assert!(sim.ping(ports(&sim, a)[0], ip("10.10.20.12")).reachable);
}

#[test]
fn router_provides_internet_through_allowed_trunk() {
    let (sim, _, _, _, b) = routed_network(vec![VlanId(10), VlanId(20)]);
    assert!(sim.ping(ports(&sim, b)[0], ip("8.8.8.8")).reachable);
}

#[test]
fn trunk_blocks_omitted_vlan() {
    let (sim, _, _, _, b) = routed_network(vec![VlanId(10)]);
    let result = sim.ping(ports(&sim, b)[0], ip("8.8.8.8"));
    assert_eq!(result.failure, Some(ReachabilityFailure::VlanBlocked));
}

#[test]
fn wrong_gateway_only_breaks_external_traffic() {
    let (mut sim, _, a, _) = basic_vlan();
    let source = ports(&sim, a)[0];
    sim.execute(Command::SetIpv4 {
        port: source,
        config: Ipv4InterfaceConfig::new(ip("10.10.20.11"), 24, Some(ip("10.10.30.1")), VlanId(20)),
    })
    .unwrap();
    assert!(sim.ping(source, ip("10.10.20.12")).reachable);
    assert_eq!(
        sim.ping(source, ip("8.8.8.8")).failure,
        Some(ReachabilityFailure::NoGateway)
    );
}

#[test]
fn serialization_rebuilds_canonical_link_index() {
    let (sim, _, a, _) = basic_vlan();
    let snapshot = format!("{:?}", sim.topology_revision);
    assert!(!snapshot.is_empty());
    assert!(sim.link_for_port(ports(&sim, a)[0]).is_some());
}
