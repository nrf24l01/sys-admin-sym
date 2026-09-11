//! Acceptance tests for the packet driven backend.
//!
//! These intentionally assert wire activity, rather than only reachability:
//! a successful diagnostic must account for the Ethernet frame at every hop.
use cloud_provider_sim::*;
use std::net::Ipv4Addr;

fn ip(value: &str) -> Ipv4Addr {
    value.parse().unwrap()
}

fn buy(sim: &mut NetworkSim, kind: DeviceTemplate) -> DeviceId {
    match sim.execute(Command::BuyDevice { kind }).unwrap()[0] {
        SimEvent::DeviceAdded(id) => id,
        _ => unreachable!(),
    }
}

fn install_power(sim: &mut NetworkSim, device: DeviceId, unit: u8) {
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

fn supplies(sim: &mut NetworkSim) {
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::CableBox305m,
    })
    .unwrap();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::Rj45Pack20,
    })
    .unwrap();
}

fn same_subnet() -> (NetworkSim, PortId, PortId, PortId, PortId) {
    let mut sim = NetworkSim::new();
    supplies(&mut sim);
    let switch = buy(&mut sim, DeviceTemplate::Switch);
    let left = buy(&mut sim, DeviceTemplate::Server);
    let right = buy(&mut sim, DeviceTemplate::Server);
    install_power(&mut sim, switch, 1);
    install_power(&mut sim, left, 2);
    install_power(&mut sim, right, 3);
    let lp = sim.device(left).unwrap().ports()[0];
    let rp = sim.device(right).unwrap().ports()[0];
    let sl = sim.device(switch).unwrap().ports()[0];
    let sr = sim.device(switch).unwrap().ports()[1];
    for port in [sl, sr] {
        sim.execute(Command::SetSwitchPortMode {
            port,
            mode: SwitchPortMode::Access {
                vlan: Some(VlanId(1)),
            },
        })
        .unwrap();
    }
    for (port, address) in [(lp, "192.0.2.2"), (rp, "192.0.2.3")] {
        sim.execute(Command::SetIpv4 {
            port,
            config: Ipv4InterfaceConfig::new(ip(address), 24, None, VlanId(1)),
        })
        .unwrap();
    }
    sim.execute(Command::Connect { a: lp, b: sl }).unwrap();
    sim.execute(Command::Connect { a: rp, b: sr }).unwrap();
    (sim, lp, rp, sl, sr)
}

fn add_isolated_host(sim: &mut NetworkSim, address: &str) -> (PortId, PortId) {
    let server = buy(sim, DeviceTemplate::Server);
    install_power(sim, server, 4);
    let host = sim.device(server).unwrap().ports()[0];
    let switch_port = sim
        .devices()
        .find(|device| matches!(device.kind, DeviceKind::Switch(_)))
        .unwrap();
    let switch_port = switch_port.ports()[2];
    let switch_id = sim
        .devices()
        .find(|device| matches!(device.kind, DeviceKind::Switch(_)))
        .unwrap()
        .id;
    sim.execute(Command::CreateVlan {
        switch: switch_id,
        vlan: Vlan {
            id: VlanId(2),
            name: "Isolated".into(),
        },
    })
    .unwrap();
    sim.execute(Command::SetSwitchPortMode {
        port: switch_port,
        mode: SwitchPortMode::Access {
            vlan: Some(VlanId(2)),
        },
    })
    .unwrap();
    sim.execute(Command::SetIpv4 {
        port: host,
        config: Ipv4InterfaceConfig::new(ip(address), 24, None, VlanId(2)),
    })
    .unwrap();
    sim.execute(Command::Connect {
        a: host,
        b: switch_port,
    })
    .unwrap();
    (host, switch_port)
}

#[test]
fn same_subnet_ping_updates_tx_rx_on_every_wire_endpoint() {
    let (mut sim, source, destination, source_switch, destination_switch) = same_subnet();
    assert!(sim.ping_mut(source, ip("192.0.2.3")).reachable);
    for port in [source, destination, source_switch, destination_switch] {
        let telemetry = sim.port_telemetry(port);
        assert!(
            telemetry.tx_frames > 0,
            "no transmit activity on {port:?}: {telemetry:?}"
        );
        assert!(
            telemetry.rx_frames > 0,
            "no receive activity on {port:?}: {telemetry:?}"
        );
    }
}

#[test]
fn powered_off_source_reports_source_down() {
    let (mut sim, source, ..) = same_subnet();
    let device = sim.port(source).unwrap().device;
    sim.execute(Command::SetPower {
        device,
        powered: false,
    })
    .unwrap();
    let result = sim.ping_mut(source, ip("192.0.2.3"));
    assert_eq!(result.failure, Some(ReachabilityFailure::SourceDown));
}

#[test]
fn terminal_ip_reports_no_carrier_until_server_port_is_cabled() {
    let mut sim = NetworkSim::new();
    let server = buy(&mut sim, DeviceTemplate::Server);
    install_power(&mut sim, server, 1);
    let port = sim.device(server).unwrap().ports()[0];
    sim.execute(Command::SetIpv4 {
        port,
        config: Ipv4InterfaceConfig::new(ip("192.0.2.10"), 24, None, VlanId(1)),
    })
    .unwrap();
    let output = sim.execute_terminal(server, TerminalCommand::Ip);
    assert!(
        output.lines.iter().any(|line| line.contains("NO-CARRIER")),
        "{output:?}"
    );
}

#[test]
fn reset_port_config_keeps_configuration_commands_applicable() {
    let mut sim = NetworkSim::new();
    let switch = buy(&mut sim, DeviceTemplate::Switch);
    let router = buy(&mut sim, DeviceTemplate::Router);
    let server = buy(&mut sim, DeviceTemplate::Server);
    let switch_port = sim.device(switch).unwrap().ports()[0];
    let router_port = sim.device(router).unwrap().ports()[2];
    let server_port = sim.device(server).unwrap().ports()[0];
    sim.execute(Command::SetSwitchPortMode {
        port: switch_port,
        mode: SwitchPortMode::Trunk {
            native_vlan: Some(VlanId(1)),
            allowed: vec![VlanId(1)],
        },
    })
    .unwrap();
    sim.execute(Command::ResetPortConfig { port: switch_port })
        .unwrap();
    sim.execute(Command::SetIpv4 {
        port: server_port,
        config: Ipv4InterfaceConfig::new(ip("192.0.2.2"), 24, None, VlanId(1)),
    })
    .unwrap();
    sim.execute(Command::ConfigureRouterInterface {
        port: router_port,
        name: "LAN".into(),
        vlan: Some(VlanId(1)),
        address: Some(ip("192.0.2.1")),
        prefix: 24,
        internet_connected: false,
    })
    .unwrap();
    sim.execute(Command::ResetPortConfig { port: router_port })
        .unwrap();
    sim.execute(Command::ConfigureRouterInterface {
        port: router_port,
        name: "LAN".into(),
        vlan: None,
        address: Some(ip("192.0.2.1")),
        prefix: 24,
        internet_connected: false,
    })
    .unwrap();
    assert!(
        sim.execute(Command::ResetPortConfig { port: server_port })
            .is_ok()
    );
}

#[test]
fn router_originated_ping_updates_both_wire_endpoints() {
    let mut sim = NetworkSim::new();
    supplies(&mut sim);
    let router = buy(&mut sim, DeviceTemplate::Router);
    let server = buy(&mut sim, DeviceTemplate::Server);
    install_power(&mut sim, router, 1);
    install_power(&mut sim, server, 2);
    let router_port = sim.device(router).unwrap().ports()[2];
    let server_port = sim.device(server).unwrap().ports()[0];
    sim.execute(Command::ConfigureRouterInterface {
        port: router_port,
        name: "LAN".into(),
        vlan: Some(VlanId(1)),
        address: Some(ip("192.0.2.1")),
        prefix: 24,
        internet_connected: false,
    })
    .unwrap();
    sim.execute(Command::SetIpv4 {
        port: server_port,
        config: Ipv4InterfaceConfig::new(ip("192.0.2.2"), 24, Some(ip("192.0.2.1")), VlanId(1)),
    })
    .unwrap();
    sim.execute(Command::Connect {
        a: router_port,
        b: server_port,
    })
    .unwrap();
    assert!(sim.ping_router_mut(router, ip("192.0.2.2")).reachable);
    for port in [router_port, server_port] {
        let telemetry = sim.port_telemetry(port);
        assert!(
            telemetry.tx_frames > 0 && telemetry.rx_frames > 0,
            "incomplete activity on {port:?}: {telemetry:?}"
        );
    }
}

#[test]
fn simulated_wan_reply_updates_source_rx_and_tx() {
    let mut sim = NetworkSim::new();
    supplies(&mut sim);
    let router = buy(&mut sim, DeviceTemplate::Router);
    let server = buy(&mut sim, DeviceTemplate::Server);
    install_power(&mut sim, router, 1);
    install_power(&mut sim, server, 2);
    let router_port = sim.device(router).unwrap().ports()[2];
    let source = sim.device(server).unwrap().ports()[0];
    sim.execute(Command::ConfigureRouterInterface {
        port: router_port,
        name: "LAN".into(),
        vlan: Some(VlanId(1)),
        address: Some(ip("192.0.2.1")),
        prefix: 24,
        internet_connected: true,
    })
    .unwrap();
    sim.execute(Command::SetIpv4 {
        port: source,
        config: Ipv4InterfaceConfig::new(ip("192.0.2.2"), 24, Some(ip("192.0.2.1")), VlanId(1)),
    })
    .unwrap();
    sim.execute(Command::Connect {
        a: router_port,
        b: source,
    })
    .unwrap();
    assert!(sim.ping_mut(source, ip("8.8.8.8")).reachable);
    let telemetry = sim.port_telemetry(source);
    assert!(
        telemetry.tx_frames > 0 && telemetry.rx_frames > 0,
        "WAN reply did not return to source: {telemetry:?}"
    );
}

#[test]
fn untagged_router_interface_reaches_access_vlan_gateway() {
    let mut sim = NetworkSim::new();
    supplies(&mut sim);
    let switch = buy(&mut sim, DeviceTemplate::Switch);
    let router = buy(&mut sim, DeviceTemplate::Router);
    let server = buy(&mut sim, DeviceTemplate::Server);
    for (device, unit) in [(switch, 1), (router, 2), (server, 3)] {
        install_power(&mut sim, device, unit);
    }
    let switch_port_router = sim.device(switch).unwrap().ports()[0];
    let switch_port_server = sim.device(switch).unwrap().ports()[1];
    let router_port = sim.device(router).unwrap().ports()[2];
    let server_port = sim.device(server).unwrap().ports()[0];
    for port in [switch_port_router, switch_port_server] {
        sim.execute(Command::SetSwitchPortMode {
            port,
            mode: SwitchPortMode::Access {
                vlan: Some(VlanId(1)),
            },
        })
        .unwrap();
    }
    sim.execute(Command::ConfigureRouterInterface {
        port: router_port,
        name: "LAN".into(),
        vlan: None,
        address: Some(ip("10.0.0.1")),
        prefix: 24,
        internet_connected: false,
    })
    .unwrap();
    sim.execute(Command::SetIpv4 {
        port: server_port,
        config: Ipv4InterfaceConfig::new(ip("10.0.0.2"), 24, Some(ip("10.0.0.1")), VlanId(1)),
    })
    .unwrap();
    sim.execute(Command::Connect {
        a: router_port,
        b: switch_port_router,
    })
    .unwrap();
    sim.execute(Command::Connect {
        a: server_port,
        b: switch_port_server,
    })
    .unwrap();
    assert!(sim.ping_mut(server_port, ip("10.0.0.1")).reachable);
}

#[test]
fn routed_echo_reply_fails_when_destination_has_no_gateway() {
    let mut sim = NetworkSim::new();
    supplies(&mut sim);
    let ra = buy(&mut sim, DeviceTemplate::Router);
    let rb = buy(&mut sim, DeviceTemplate::Router);
    let left = buy(&mut sim, DeviceTemplate::Server);
    let right = buy(&mut sim, DeviceTemplate::Server);
    for (device, unit) in [(ra, 1), (rb, 2), (left, 3), (right, 4)] {
        install_power(&mut sim, device, unit);
    }
    let rap = sim.device(ra).unwrap().ports().to_vec();
    let rbp = sim.device(rb).unwrap().ports().to_vec();
    let source = sim.device(left).unwrap().ports()[0];
    let destination = sim.device(right).unwrap().ports()[0];
    let (ra_lan, ra_transit) = (rap[2], rap[3]);
    let (rb_lan, rb_transit) = (rbp[2], rbp[3]);
    for (port, name, address) in [
        (ra_lan, "LAN-A", "10.0.1.1"),
        (ra_transit, "TRANSIT-A", "10.0.12.1"),
        (rb_lan, "LAN-B", "10.0.2.1"),
        (rb_transit, "TRANSIT-B", "10.0.12.2"),
    ] {
        sim.execute(Command::ConfigureRouterInterface {
            port,
            name: name.into(),
            vlan: Some(VlanId(1)),
            address: Some(ip(address)),
            prefix: 24,
            internet_connected: false,
        })
        .unwrap();
    }
    sim.execute(Command::SetIpv4 {
        port: source,
        config: Ipv4InterfaceConfig::new(ip("10.0.1.10"), 24, Some(ip("10.0.1.1")), VlanId(1)),
    })
    .unwrap();
    // The destination has no gateway, so its echo reply cannot return.
    sim.execute(Command::SetIpv4 {
        port: destination,
        config: Ipv4InterfaceConfig::new(ip("10.0.2.10"), 24, None, VlanId(1)),
    })
    .unwrap();
    for (a, b) in [
        (source, ra_lan),
        (ra_transit, rb_transit),
        (rb_lan, destination),
    ] {
        sim.execute(Command::Connect { a, b }).unwrap();
    }
    sim.execute(Command::SetStaticRoute {
        router: ra,
        route: Route {
            network: ip("10.0.2.0"),
            prefix: 24,
            via: Some(ip("10.0.12.2")),
            egress: ra_transit,
        },
    })
    .unwrap();
    sim.execute(Command::SetStaticRoute {
        router: rb,
        route: Route {
            network: ip("10.0.1.0"),
            prefix: 24,
            via: Some(ip("10.0.12.1")),
            egress: rb_transit,
        },
    })
    .unwrap();
    let result = sim.ping_mut(source, ip("10.0.2.10"));
    assert!(!result.reachable);
    assert_eq!(result.failure, Some(ReachabilityFailure::NoGateway));
}

#[test]
fn unrelated_vlan_does_not_receive_same_subnet_frames() {
    let (mut sim, source, destination, _, _) = same_subnet();
    let (_, isolated_switch_port) = add_isolated_host(&mut sim, "198.51.100.2");
    assert!(sim.ping_mut(source, ip("192.0.2.3")).reachable);
    let telemetry = sim.port_telemetry(isolated_switch_port);
    assert_eq!(telemetry.tx_frames, 0);
    assert_eq!(telemetry.rx_frames, 0);
    assert!(sim.port_telemetry(destination).rx_frames > 0);
}

#[test]
fn duplicate_address_in_disconnected_vlan_does_not_block_valid_peer() {
    let (mut sim, source, _, _, _) = same_subnet();
    let (_, isolated_switch_port) = add_isolated_host(&mut sim, "192.0.2.2");
    let result = sim.ping_mut(source, ip("192.0.2.3"));
    assert!(
        result.reachable,
        "duplicate in another VLAN blocked ping: {result:?}"
    );
    let telemetry = sim.port_telemetry(isolated_switch_port);
    assert_eq!(telemetry.tx_frames, 0);
    assert_eq!(telemetry.rx_frames, 0);
}

#[test]
fn ttl_one_expires_at_the_first_router() {
    let mut sim = NetworkSim::new();
    supplies(&mut sim);
    let router = buy(&mut sim, DeviceTemplate::Router);
    let server = buy(&mut sim, DeviceTemplate::Server);
    install_power(&mut sim, router, 1);
    install_power(&mut sim, server, 2);
    let rp = sim.device(router).unwrap().ports()[2];
    let sp = sim.device(server).unwrap().ports()[0];
    sim.execute(Command::ConfigureRouterInterface {
        port: rp,
        name: "LAN".into(),
        vlan: Some(VlanId(1)),
        address: Some(ip("192.0.2.1")),
        prefix: 24,
        internet_connected: true,
    })
    .unwrap();
    sim.execute(Command::SetIpv4 {
        port: sp,
        config: Ipv4InterfaceConfig::new(ip("192.0.2.2"), 24, Some(ip("192.0.2.1")), VlanId(1)),
    })
    .unwrap();
    sim.execute(Command::Connect { a: rp, b: sp }).unwrap();
    let result = sim.ping_with_ttl(sp, ip("8.8.8.8"), 1);
    assert!(!result.reachable);
    assert_eq!(result.failure, Some(ReachabilityFailure::TtlExpired));
}

#[test]
fn static_routes_forward_and_longest_prefix_wins() {
    let mut sim = NetworkSim::new();
    supplies(&mut sim);
    let ra = buy(&mut sim, DeviceTemplate::Router);
    let rb = buy(&mut sim, DeviceTemplate::Router);
    let left = buy(&mut sim, DeviceTemplate::Server);
    let right = buy(&mut sim, DeviceTemplate::Server);
    for (device, unit) in [(ra, 1), (rb, 2), (left, 3), (right, 4)] {
        install_power(&mut sim, device, unit);
    }
    let rap = sim.device(ra).unwrap().ports().to_vec();
    let rbp = sim.device(rb).unwrap().ports().to_vec();
    let lp = sim.device(left).unwrap().ports()[0];
    let rp = sim.device(right).unwrap().ports()[0];
    let (ra_lan, ra_transit) = (rap[2], rap[3]);
    let (rb_lan, rb_transit) = (rbp[2], rbp[3]);
    for (port, name, address) in [
        (ra_lan, "LAN-A", "10.0.1.1"),
        (ra_transit, "TRANSIT-A", "10.0.12.1"),
        (rb_lan, "LAN-B", "10.0.2.1"),
        (rb_transit, "TRANSIT-B", "10.0.12.2"),
    ] {
        sim.execute(Command::ConfigureRouterInterface {
            port,
            name: name.into(),
            vlan: Some(VlanId(1)),
            address: Some(ip(address)),
            prefix: 24,
            internet_connected: false,
        })
        .unwrap();
    }
    sim.execute(Command::SetIpv4 {
        port: lp,
        config: Ipv4InterfaceConfig::new(ip("10.0.1.10"), 24, Some(ip("10.0.1.1")), VlanId(1)),
    })
    .unwrap();
    sim.execute(Command::SetIpv4 {
        port: rp,
        config: Ipv4InterfaceConfig::new(ip("10.0.2.10"), 24, Some(ip("10.0.2.1")), VlanId(1)),
    })
    .unwrap();
    for (a, b) in [(lp, ra_lan), (ra_transit, rb_transit), (rb_lan, rp)] {
        sim.execute(Command::Connect { a, b }).unwrap();
    }
    sim.execute(Command::SetStaticRoute {
        router: ra,
        route: Route {
            network: ip("10.0.2.0"),
            prefix: 24,
            via: Some(ip("10.0.12.2")),
            egress: ra_transit,
        },
    })
    .unwrap();
    sim.execute(Command::SetStaticRoute {
        router: rb,
        route: Route {
            network: ip("10.0.1.0"),
            prefix: 24,
            via: Some(ip("10.0.12.1")),
            egress: rb_transit,
        },
    })
    .unwrap();
    assert!(sim.ping_mut(lp, ip("10.0.2.10")).reachable);

    // A matching /32 with an unreachable next hop must take precedence over /24.
    sim.execute(Command::SetStaticRoute {
        router: ra,
        route: Route {
            network: ip("10.0.2.10"),
            prefix: 32,
            via: Some(ip("10.0.12.99")),
            egress: ra_transit,
        },
    })
    .unwrap();
    assert!(!sim.ping_mut(lp, ip("10.0.2.10")).reachable);
}
