use cloud_provider_sim::*;
use std::net::Ipv4Addr;

fn ip(s: &str) -> Ipv4Addr {
    s.parse().unwrap()
}
fn prefix(s: &str) -> Ipv4Prefix {
    s.parse().unwrap()
}
fn execute(sim: &mut NetworkSim, c: ProviderCommand) {
    sim.execute(Command::Provider(c)).unwrap();
}
fn device(sim: &mut NetworkSim, kind: DeviceTemplate, unit: u8) -> DeviceId {
    let SimEvent::DeviceAdded(id) = sim.execute(Command::BuyDevice { kind }).unwrap()[0] else {
        panic!()
    };
    let rack = RackId(u64::from((unit - 1) / 4 + 1));
    sim.execute(Command::PlaceDevice {
        device: id,
        rack,
        unit: (unit - 1) % 4 + 1,
    })
    .unwrap();
    if matches!(
        kind,
        DeviceTemplate::PatchPanel | DeviceTemplate::CableManager
    ) {
        return id;
    }
    sim.execute(Command::ConnectPower {
        outlet: OutletId {
            source: SourceId::Rack(rack),
            index: (unit - 1) % 4,
        },
        endpoint: PowerEndpoint::Device(id),
    })
    .unwrap();
    sim.execute(Command::SetPower {
        device: id,
        powered: true,
    })
    .unwrap();
    id
}
fn cable(sim: &mut NetworkSim, a: PortId, b: PortId) {
    sim.execute(Command::Connect { a, b }).unwrap();
}
fn iface(sim: &mut NetworkSim, port: PortId, address: &str, prefix: u8) {
    sim.execute(Command::ConfigureRouterInterface {
        port,
        name: format!("p{}", port.0),
        vlan: None,
        address: Some(ip(address)),
        prefix,
        internet_connected: false,
    })
    .unwrap();
}
fn route(sim: &mut NetworkSim, router: DeviceId, network: &str, via: &str, egress: PortId) {
    let p = prefix(network);
    sim.execute(Command::SetStaticRoute {
        router,
        route: Route {
            network: p.network(),
            prefix: p.length(),
            via: Some(ip(via)),
            egress,
        },
    })
    .unwrap();
}
struct Lab {
    sim: NetworkSim,
    router: DeviceId,
    server: DeviceId,
    host: PortId,
    lan: PortId,
    wan: PortId,
    circuit: PortId,
}
impl Lab {
    fn new() -> Self {
        let mut sim = NetworkSim::new();
        sim.money = 100_000;
        for supply in [CableSupply::CableBox305m, CableSupply::Rj45Pack20] {
            sim.execute(Command::BuyCableSupply { supply }).unwrap();
        }
        let router = device(&mut sim, DeviceTemplate::Router, 1);
        let server = device(&mut sim, DeviceTemplate::Server, 2);
        let ports = sim.device(router).unwrap().ports().to_vec();
        let (wan, lan) = (ports[0], ports[2]);
        let host = sim.device(server).unwrap().ports()[0];
        let circuit = sim
            .network_outlets()
            .filter(|o| matches!(o.kind, NetworkOutletKind::Uplink { .. }))
            .min_by_key(|o| o.port)
            .unwrap()
            .port;
        iface(&mut sim, wan, "192.0.2.2", 30);
        iface(&mut sim, lan, "203.0.113.1", 24);
        sim.execute(Command::SetIpv4 {
            port: host,
            config: Ipv4InterfaceConfig::new(
                ip("203.0.113.10"),
                24,
                Some(ip("203.0.113.1")),
                VlanId(1),
            ),
        })
        .unwrap();
        cable(&mut sim, host, lan);
        cable(&mut sim, wan, circuit);
        execute(
            &mut sim,
            ProviderCommand::SetTransit(TransitCircuit {
                port: circuit,
                name: "Transit A".into(),
                address: ip("192.0.2.1"),
                prefix: 30,
                asn: 64501,
                capacity_mbps: 100,
                enabled: true,
                routes: vec![UpstreamRoute {
                    prefix: prefix("203.0.113.0/24"),
                    next_hop: ip("192.0.2.2"),
                }],
                offered_routes: vec![prefix("0.0.0.0/0")],
                authorizations: vec![PrefixAuthorization {
                    prefix: prefix("203.0.113.0/24"),
                    max_length: 24,
                    origin_asn: 64500,
                }],
            }),
        );
        route(&mut sim, router, "0.0.0.0/0", "192.0.2.1", wan);
        Self {
            sim,
            router,
            server,
            host,
            lan,
            wan,
            circuit,
        }
    }
    fn session(&self) -> BgpSession {
        BgpSession {
            port: self.wan,
            vlan: VlanId(1),
            circuit: self.circuit,
            local_asn: 64500,
            peer_asn: 64501,
            enabled: true,
            import_prefixes: vec![prefix("0.0.0.0/0")],
            export_prefixes: vec![prefix("203.0.113.0/24")],
            max_prefixes: 10,
            preference: 20,
        }
    }
}

#[test]
fn transit_requires_real_forward_and_return_routes() {
    let mut l = Lab::new();
    assert!(l.sim.ping(l.host, ip("8.8.8.8")).reachable);
    assert!(l.sim.ping_from_internet(ip("203.0.113.10")).reachable);
    let mut c = l.sim.provider().circuit(l.circuit).unwrap().clone();
    c.routes.clear();
    execute(&mut l.sim, ProviderCommand::SetTransit(c));
    assert!(!l.sim.ping(l.host, ip("8.8.8.8")).reachable);
    assert!(!l.sim.ping_from_internet(ip("203.0.113.10")).reachable);
}

#[test]
fn private_sources_are_not_implicitly_translated() {
    let mut l = Lab::new();
    iface(&mut l.sim, l.lan, "10.0.0.1", 24);
    l.sim
        .execute(Command::SetIpv4 {
            port: l.host,
            config: Ipv4InterfaceConfig::new(ip("10.0.0.10"), 24, Some(ip("10.0.0.1")), VlanId(1)),
        })
        .unwrap();
    assert!(l.sim.ping(l.host, ip("10.0.0.1")).reachable);
    assert!(!l.sim.ping(l.host, ip("8.8.8.8")).reachable);
}

#[test]
fn ownership_does_not_install_a_route_and_bgp_enforces_exports() {
    let mut l = Lab::new();
    execute(
        &mut l.sim,
        ProviderCommand::AddPool(AddressPool {
            prefix: prefix("203.0.113.0/24"),
            description: "Provider aggregate".into(),
        }),
    );
    let mut circuit = l.sim.provider().circuit(l.circuit).unwrap().clone();
    circuit.routes.clear();
    execute(&mut l.sim, ProviderCommand::SetTransit(circuit));
    assert!(!l.sim.ping_from_internet(ip("203.0.113.10")).reachable);
    l.sim
        .execute(Command::RemoveStaticRoute {
            router: l.router,
            route: Route {
                network: ip("0.0.0.0"),
                prefix: 0,
                via: Some(ip("192.0.2.1")),
                egress: l.wan,
            },
        })
        .unwrap();
    let session = l.session();
    execute(&mut l.sim, ProviderCommand::SetBgp(session.clone()));
    assert_eq!(l.sim.bgp_state(&session), BgpState::Established);
    assert!(
        l.sim
            .bgp_export_accepted(&session, prefix("203.0.113.0/24"))
    );
    assert!(l.sim.ping_from_internet(ip("203.0.113.10")).reachable);
    assert!(l.sim.ping(l.host, ip("8.8.8.8")).reachable);
    let mut bad = session.clone();
    bad.peer_asn = 64599;
    execute(&mut l.sim, ProviderCommand::SetBgp(bad.clone()));
    assert_eq!(l.sim.bgp_state(&bad), BgpState::PeerMismatch);
    assert!(!l.sim.ping_from_internet(ip("203.0.113.10")).reachable);
    bad = session;
    bad.export_prefixes = vec![prefix("203.0.113.0/29")];
    execute(&mut l.sim, ProviderCommand::SetBgp(bad.clone()));
    assert!(!l.sim.bgp_export_accepted(&bad, prefix("203.0.113.0/29")));
}

#[test]
fn bgp_prefix_limit_withdraws_imports_and_exports() {
    let mut l = Lab::new();
    execute(
        &mut l.sim,
        ProviderCommand::AddPool(AddressPool {
            prefix: prefix("203.0.113.0/24"),
            description: "aggregate".into(),
        }),
    );
    let mut c = l.sim.provider().circuit(l.circuit).unwrap().clone();
    c.offered_routes.push(prefix("192.0.2.0/24"));
    execute(&mut l.sim, ProviderCommand::SetTransit(c));
    let mut session = l.session();
    session.max_prefixes = 1;
    execute(&mut l.sim, ProviderCommand::SetBgp(session.clone()));
    assert_eq!(l.sim.bgp_state(&session), BgpState::PrefixLimit);
}

#[test]
fn routing_domains_and_ordered_filtering_are_enforced() {
    let mut l = Lab::new();
    execute(
        &mut l.sim,
        ProviderCommand::BindInterface(InterfaceBinding {
            port: l.lan,
            vlan: VlanId(1),
            domain: RoutingDomain(10),
            role: NetworkRole::Private,
        }),
    );
    assert!(!l.sim.ping(l.host, ip("8.8.8.8")).reachable);
    execute(
        &mut l.sim,
        ProviderCommand::BindInterface(InterfaceBinding {
            port: l.wan,
            vlan: VlanId(1),
            domain: RoutingDomain(10),
            role: NetworkRole::Public,
        }),
    );
    assert!(!l.sim.ping(l.host, ip("8.8.8.8")).reachable);
    execute(
        &mut l.sim,
        ProviderCommand::SetDomainRoute(DomainRoute {
            router: l.router,
            domain: RoutingDomain(10),
            prefix: prefix("0.0.0.0/0"),
            port: l.wan,
            vlan: VlanId(1),
            next_hop: Some(ip("192.0.2.1")),
            preference: 1,
            track_neighbor: false,
        }),
    );
    assert!(l.sim.ping(l.host, ip("8.8.8.8")).reachable);
    execute(
        &mut l.sim,
        ProviderCommand::SetPolicy(PolicyAttachment {
            ingress: true,
            policy: PacketPolicy {
                port: l.lan,
                vlan: VlanId(1),
                allowed_sources: vec![prefix("203.0.113.10/32")],
                rules: vec![PacketRule {
                    source: prefix("0.0.0.0/0"),
                    destination: prefix("8.8.8.8/32"),
                    protocol: Some(1),
                    action: PolicyAction::Deny,
                }],
                default_action: PolicyAction::Permit,
            },
        }),
    );
    assert!(!l.sim.ping(l.host, ip("8.8.8.8")).reachable);
    assert!(l.sim.ping(l.host, ip("1.1.1.1")).reachable);
    l.sim
        .execute(Command::SetIpv4 {
            port: l.host,
            config: Ipv4InterfaceConfig::new(
                ip("203.0.113.11"),
                24,
                Some(ip("203.0.113.1")),
                VlanId(1),
            ),
        })
        .unwrap();
    assert!(!l.sim.ping(l.host, ip("1.1.1.1")).reachable);
}

#[test]
fn failed_physical_link_selects_the_backup_static_route() {
    let mut l = Lab::new();
    let other = l
        .sim
        .network_outlets()
        .find(|o| matches!(o.kind, NetworkOutletKind::Uplink { .. }) && o.port != l.circuit)
        .unwrap()
        .port;
    let wan2 = l.sim.device(l.router).unwrap().ports()[3];
    iface(&mut l.sim, wan2, "192.0.2.6", 30);
    cable(&mut l.sim, wan2, other);
    let mut c = l.sim.provider().circuit(l.circuit).unwrap().clone();
    c.port = other;
    c.address = ip("192.0.2.5");
    c.routes[0].next_hop = ip("192.0.2.6");
    execute(&mut l.sim, ProviderCommand::SetTransit(c));
    route(&mut l.sim, l.router, "0.0.0.0/0", "192.0.2.5", wan2);
    execute(
        &mut l.sim,
        ProviderCommand::SetRoutePreference(RoutePreference {
            router: l.router,
            prefix: prefix("0.0.0.0/0"),
            egress: wan2,
            via: Some(ip("192.0.2.5")),
            preference: 100,
        }),
    );
    assert!(l.sim.ping(l.host, ip("8.8.8.8")).reachable);
    l.sim
        .execute(Command::SetPortEnabled {
            port: l.wan,
            enabled: false,
        })
        .unwrap();
    assert!(l.sim.ping(l.host, ip("8.8.8.8")).reachable);
    assert!(l.sim.ping_from_internet(ip("203.0.113.10")).reachable);
    assert_eq!(l.sim.network_capacity().active_transit_mbps, 100);
    l.sim
        .execute(Command::SetPortEnabled {
            port: wan2,
            enabled: false,
        })
        .unwrap();
    assert!(!l.sim.ping(l.host, ip("8.8.8.8")).reachable);
}

#[test]
fn provider_configuration_roundtrips_without_runtime_state() {
    let mut l = Lab::new();
    assert!(l.sim.ping_from_internet(ip("203.0.113.10")).reachable);
    let encoded = ron::to_string(&l.sim).unwrap();
    let mut loaded: NetworkSim = ron::from_str(&encoded).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.provider().circuits(), l.sim.provider().circuits());
    assert_eq!(loaded.port_telemetry(l.wan).tx_frames, 0);
    assert!(loaded.ping_from_internet(ip("203.0.113.10")).reachable);
    assert_eq!(
        loaded.path_capacity_mbps(&loaded.ping(l.host, ip("8.8.8.8"))),
        100
    );
}

#[test]
fn invalid_provider_commands_are_atomic() {
    let mut l = Lab::new();
    let before = ron::to_string(&l.sim).unwrap();
    let mut c = l.sim.provider().circuit(l.circuit).unwrap().clone();
    c.routes[0].next_hop = ip("10.1.1.1");
    assert!(
        l.sim
            .execute(Command::Provider(ProviderCommand::SetTransit(c)))
            .is_err()
    );
    assert_eq!(ron::to_string(&l.sim).unwrap(), before);
    assert!("1.2.3.4/33".parse::<Ipv4Prefix>().is_err());
    assert!(prefix("192.0.2.0/31").usable(ip("192.0.2.0")));
    assert!(!prefix("192.0.2.0/30").usable(ip("192.0.2.0")));
}

#[test]
fn explicit_dhcp_server_and_console_configuration_work() {
    let mut l = Lab::new();
    let command = format!(
        "netctl dhcp add {} 1 203.0.113.0/24 203.0.113.20 203.0.113.30 203.0.113.1",
        l.lan
    );
    assert!(!l.sim.execute_console(l.server, &command).success);
    let args: Vec<_> = command
        .split_whitespace()
        .skip(1)
        .map(str::to_owned)
        .collect();
    provider::console::ProviderConsole::run(&mut l.sim, &args).unwrap();
    assert_eq!(l.sim.request_dhcp(l.host).unwrap(), ip("203.0.113.20"));
    assert_eq!(l.sim.request_dhcp(l.host).unwrap(), ip("203.0.113.20"));
    assert!(l.sim.ping(l.host, ip("8.8.8.8")).reachable);
    l.sim
        .execute(Command::SetPower {
            device: l.router,
            powered: false,
        })
        .unwrap();
    assert!(l.sim.request_dhcp(l.host).is_err());
}

#[test]
fn spanning_tree_blocks_a_cycle_and_reconverges_after_link_failure() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    for supply in [CableSupply::CableBox305m, CableSupply::Rj45Pack20] {
        sim.execute(Command::BuyCableSupply { supply }).unwrap();
    }
    let mut switches = Vec::new();
    for unit in 1..=3 {
        let id = device(&mut sim, DeviceTemplate::Switch, unit);
        switches.push(sim.device(id).unwrap().ports().to_vec());
    }
    for (a, b) in [
        (switches[0][0], switches[1][0]),
        (switches[1][1], switches[2][0]),
        (switches[2][1], switches[0][1]),
    ] {
        cable(&mut sim, a, b);
    }
    let blocked = sim.spanning_tree_blocked_ports(VlanId(1));
    assert_eq!(blocked.len(), 1);
    let link = sim.link_for_port(switches[0][0]).unwrap().id;
    sim.execute(Command::Disconnect { link }).unwrap();
    assert!(sim.spanning_tree_blocked_ports(VlanId(1)).is_empty());
}

#[test]
fn rack_sockets_are_not_an_implicit_switch_or_a_dhcp_service() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    for supply in [CableSupply::CableBox305m, CableSupply::Rj45Pack20] {
        sim.execute(Command::BuyCableSupply { supply }).unwrap();
    }
    let mut clients = Vec::new();
    for (unit, address) in [(1, "10.0.0.10"), (5, "10.0.0.11")] {
        let server = device(&mut sim, DeviceTemplate::Server, unit);
        let host = sim.device(server).unwrap().ports()[0];
        let rack = sim.device(server).unwrap().rack.unwrap().rack;
        let socket = sim
            .network_outlets()
            .find(|o| o.kind == NetworkOutletKind::Lan { rack })
            .unwrap()
            .port;
        sim.execute(Command::SetIpv4 {
            port: host,
            config: Ipv4InterfaceConfig::new(ip(address), 24, None, VlanId(1)),
        })
        .unwrap();
        cable(&mut sim, host, socket);
        clients.push(host);
    }
    assert!(!sim.ping(clients[0], ip("10.0.0.11")).reachable);
    assert!(sim.request_dhcp(clients[0]).is_err());
    assert_eq!(sim.datacenter_resources().lan.compute_mhz, 0);
    assert_eq!(sim.datacenter_resources().global.compute_mhz, 0);
}

#[test]
fn switch_management_requires_the_correct_vlan_and_policy() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    for supply in [CableSupply::CableBox305m, CableSupply::Rj45Pack20] {
        sim.execute(Command::BuyCableSupply { supply }).unwrap();
    }
    let server = device(&mut sim, DeviceTemplate::Server, 1);
    let switch = device(&mut sim, DeviceTemplate::Switch, 2);
    let mgmt = sim.device(server).unwrap().ports()[2];
    let access = sim.device(switch).unwrap().ports()[0];
    cable(&mut sim, mgmt, access);
    sim.execute(Command::SetIpv4 {
        port: mgmt,
        config: Ipv4InterfaceConfig {
            vlan: None,
            ..Ipv4InterfaceConfig::new(ip("10.10.0.10"), 24, None, VlanId(1))
        },
    })
    .unwrap();
    sim.execute(Command::CreateVlan {
        switch,
        vlan: Vlan {
            id: VlanId(99),
            name: "Management".into(),
        },
    })
    .unwrap();
    execute(
        &mut sim,
        ProviderCommand::SetSwitchManagement(SwitchManagement {
            switch,
            address: ip("10.10.0.1"),
            prefix: 24,
            vlan: VlanId(99),
            gateway: None,
        }),
    );
    assert!(!sim.management_path_reaches(mgmt, switch));
    sim.execute(Command::SetSwitchPortMode {
        port: access,
        mode: SwitchPortMode::Access {
            vlan: Some(VlanId(99)),
        },
    })
    .unwrap();
    assert!(sim.management_path_reaches(mgmt, switch));
    assert!(sim.execute_console(server, "ssh 10.10.0.1").success);
    assert!(sim.execute_console(server, "exit").success);
    execute(
        &mut sim,
        ProviderCommand::SetPolicy(PolicyAttachment {
            ingress: true,
            policy: PacketPolicy {
                port: access,
                vlan: VlanId(99),
                allowed_sources: vec![],
                rules: vec![],
                default_action: PolicyAction::Deny,
            },
        }),
    );
    assert!(!sim.execute_console(server, "ssh 10.10.0.1").success);
}

#[test]
fn tracked_static_route_can_withdraw_but_untracked_more_specific_stays() {
    let mut l = Lab::new();
    // Use a /29 handoff so .3 is a usable but absent next hop, not a broadcast.
    iface(&mut l.sim, l.wan, "192.0.2.2", 29);
    let mut circuit = l.sim.provider().circuit(l.circuit).unwrap().clone();
    circuit.prefix = 29;
    execute(&mut l.sim, ProviderCommand::SetTransit(circuit));

    let r = DomainRoute {
        router: l.router,
        domain: RoutingDomain(0),
        prefix: prefix("8.8.8.8/32"),
        port: l.wan,
        vlan: VlanId(1),
        next_hop: Some(ip("192.0.2.3")),
        preference: 1,
        track_neighbor: false,
    };
    execute(&mut l.sim, ProviderCommand::SetDomainRoute(r));
    assert!(!l.sim.ping(l.host, ip("8.8.8.8")).reachable);
    execute(
        &mut l.sim,
        ProviderCommand::SetDomainRoute(DomainRoute {
            track_neighbor: true,
            ..r
        }),
    );
    assert!(l.sim.ping(l.host, ip("8.8.8.8")).reachable);
}

#[test]
fn policy_drops_are_counted_at_switch_ingress() {
    let mut l = Lab::new();
    let switch = device(&mut l.sim, DeviceTemplate::Switch, 3);
    let ports = l.sim.device(switch).unwrap().ports().to_vec();
    let link = l.sim.link_for_port(l.host).unwrap().id;
    l.sim.execute(Command::Disconnect { link }).unwrap();
    cable(&mut l.sim, l.host, ports[0]);
    cable(&mut l.sim, ports[1], l.lan);
    execute(
        &mut l.sim,
        ProviderCommand::SetPolicy(PolicyAttachment {
            ingress: true,
            policy: PacketPolicy {
                port: ports[0],
                vlan: VlanId(1),
                allowed_sources: vec![prefix("203.0.113.99/32")],
                rules: vec![],
                default_action: PolicyAction::Permit,
            },
        }),
    );
    assert!(!l.sim.ping_mut(l.host, ip("8.8.8.8")).reachable);
    execute(
        &mut l.sim,
        ProviderCommand::RemovePolicy {
            port: ports[0],
            vlan: VlanId(1),
            ingress: true,
        },
    );
    assert!(l.sim.ping_mut(l.host, ip("8.8.8.8")).reachable);
}

#[test]
fn upstream_does_not_invent_replies_from_unknown_or_silent_hosts() {
    let mut l = Lab::new();
    assert!(!l.sim.ping(l.host, ip("9.9.9.9")).reachable);
    execute(
        &mut l.sim,
        ProviderCommand::SetExternalHost(ExternalHost {
            address: ip("9.9.9.9"),
            available: true,
            echo_enabled: true,
        }),
    );
    assert!(l.sim.ping(l.host, ip("9.9.9.9")).reachable);
    execute(
        &mut l.sim,
        ProviderCommand::SetExternalHost(ExternalHost {
            address: ip("9.9.9.9"),
            available: true,
            echo_enabled: false,
        }),
    );
    assert!(!l.sim.ping(l.host, ip("9.9.9.9")).reachable);
}

#[test]
fn address_purchase_is_independent_of_transit_and_does_not_change_routing() {
    let mut sim = NetworkSim::new();
    let before = sim.money;
    sim.execute(Command::BuyPublicIpv4Pool).unwrap();
    let block = sim.public_ipv4_blocks()[0];
    assert_eq!(block.uplink, PortId(0));
    assert_eq!(sim.money, before - PublicIpv4Block::PRICE);
    assert!(sim.provider().owns(prefix("203.0.113.0/29")));
    assert!(sim.provider().circuits().is_empty());
    assert!(!sim.ping_from_internet(ip("203.0.113.2")).reachable);
}

#[test]
fn owned_address_allocation_supports_host_and_point_to_point_prefixes() {
    let mut l = Lab::new();
    let spare = l.sim.device(l.server).unwrap().ports()[1];
    let host_prefix = prefix("198.51.100.20/32");
    let before = ron::to_string(&l.sim).unwrap();
    assert!(l.sim.allocate_ipv4(spare, host_prefix, None, None).is_err());
    assert_eq!(before, ron::to_string(&l.sim).unwrap());
    execute(
        &mut l.sim,
        ProviderCommand::AddPool(AddressPool {
            prefix: host_prefix,
            description: "routed host".into(),
        }),
    );
    assert_eq!(
        l.sim.allocate_ipv4(spare, host_prefix, None, None).unwrap(),
        ip("198.51.100.20")
    );
    assert_eq!(
        l.sim.allocate_ipv4(spare, host_prefix, None, None).unwrap(),
        ip("198.51.100.20")
    );
    let point_to_point = prefix("198.51.100.30/31");
    execute(
        &mut l.sim,
        ProviderCommand::AddPool(AddressPool {
            prefix: point_to_point,
            description: "point-to-point".into(),
        }),
    );
    assert_eq!(
        l.sim
            .allocate_ipv4(spare, point_to_point, Some(ip("198.51.100.31")), None)
            .unwrap(),
        ip("198.51.100.30")
    );
}

#[test]
fn path_capacity_includes_intermediate_switch_ports() {
    let mut l = Lab::new();
    let switch = device(&mut l.sim, DeviceTemplate::Switch, 3);
    let ports = l.sim.device(switch).unwrap().ports().to_vec();
    let old = l.sim.link_for_port(l.host).unwrap().id;
    l.sim.execute(Command::Disconnect { link: old }).unwrap();
    cable(&mut l.sim, l.host, ports[0]);
    cable(&mut l.sim, ports[1], l.lan);
    l.sim
        .execute(Command::SetPortSpeed {
            port: ports[1],
            speed: LinkSpeed::Mbps10,
        })
        .unwrap();
    let result = l.sim.ping(l.host, ip("8.8.8.8"));
    assert!(result.reachable);
    assert_eq!(l.sim.path_capacity_mbps(&result), 10);
}

#[test]
fn bgp_session_loss_withdraws_the_default_and_selects_another_peer() {
    let mut l = Lab::new();
    execute(
        &mut l.sim,
        ProviderCommand::AddPool(AddressPool {
            prefix: prefix("203.0.113.0/24"),
            description: "aggregate".into(),
        }),
    );
    l.sim
        .execute(Command::RemoveStaticRoute {
            router: l.router,
            route: Route {
                network: ip("0.0.0.0"),
                prefix: 0,
                via: Some(ip("192.0.2.1")),
                egress: l.wan,
            },
        })
        .unwrap();
    let mut circuit_a = l.sim.provider().circuit(l.circuit).unwrap().clone();
    circuit_a.routes.clear();
    execute(&mut l.sim, ProviderCommand::SetTransit(circuit_a.clone()));
    let session_a = l.session();
    execute(&mut l.sim, ProviderCommand::SetBgp(session_a.clone()));
    let circuit_b_port = l
        .sim
        .network_outlets()
        .find(|o| matches!(o.kind, NetworkOutletKind::Uplink { .. }) && o.port != l.circuit)
        .unwrap()
        .port;
    let wan_b = l.sim.device(l.router).unwrap().ports()[3];
    iface(&mut l.sim, wan_b, "192.0.2.6", 30);
    cable(&mut l.sim, wan_b, circuit_b_port);
    let circuit_b = TransitCircuit {
        port: circuit_b_port,
        address: ip("192.0.2.5"),
        asn: 64502,
        ..circuit_a
    };
    execute(&mut l.sim, ProviderCommand::SetTransit(circuit_b));
    let session_b = BgpSession {
        port: wan_b,
        circuit: circuit_b_port,
        peer_asn: 64502,
        preference: 30,
        ..session_a.clone()
    };
    execute(&mut l.sim, ProviderCommand::SetBgp(session_b.clone()));
    assert!(l.sim.ping(l.host, ip("8.8.8.8")).reachable);
    execute(
        &mut l.sim,
        ProviderCommand::SetBgp(BgpSession {
            enabled: false,
            ..session_a
        }),
    );
    assert!(l.sim.ping(l.host, ip("8.8.8.8")).reachable);
    assert!(l.sim.ping_from_internet(ip("203.0.113.10")).reachable);
    execute(
        &mut l.sim,
        ProviderCommand::SetBgp(BgpSession {
            enabled: false,
            ..session_b
        }),
    );
    assert!(!l.sim.ping(l.host, ip("8.8.8.8")).reachable);
    assert!(!l.sim.ping_from_internet(ip("203.0.113.10")).reachable);
}

fn configured_routes(sim: &NetworkSim, id: DeviceId) -> &[DomainRoute] {
    let DeviceKind::Router(router) = &sim.device(id).unwrap().kind else {
        panic!("expected router")
    };
    &router.domain_routes
}

#[test]
fn each_player_router_needs_its_own_forward_and_return_routes() {
    let mut lab = Lab::new();
    let edge = device(&mut lab.sim, DeviceTemplate::Router, 3);
    let ports = lab.sim.device(edge).unwrap().ports().to_vec();
    let (outside, inside) = (ports[0], ports[2]);
    let link = lab.sim.link_for_port(lab.wan).unwrap().id;
    lab.sim.execute(Command::Disconnect { link }).unwrap();
    cable(&mut lab.sim, lab.wan, inside);
    cable(&mut lab.sim, outside, lab.circuit);
    iface(&mut lab.sim, inside, "192.0.2.1", 30);
    iface(&mut lab.sim, outside, "198.18.0.2", 30);
    let mut carrier = lab.sim.provider().circuit(lab.circuit).unwrap().clone();
    carrier.address = ip("198.18.0.1");
    carrier.routes[0].next_hop = ip("198.18.0.2");
    execute(&mut lab.sim, ProviderCommand::SetTransit(carrier));
    lab.sim
        .execute(Command::RemoveStaticRoute {
            router: lab.router,
            route: Route {
                network: ip("0.0.0.0"),
                prefix: 0,
                via: Some(ip("192.0.2.1")),
                egress: lab.wan,
            },
        })
        .unwrap();
    let internal = DomainRoute {
        router: lab.router,
        domain: RoutingDomain(0),
        prefix: prefix("0.0.0.0/0"),
        port: lab.wan,
        vlan: VlanId(1),
        next_hop: Some(ip("192.0.2.1")),
        preference: 1,
        track_neighbor: false,
    };
    assert!(!lab.sim.ping(lab.host, ip("8.8.8.8")).reachable);
    execute(&mut lab.sim, ProviderCommand::SetDomainRoute(internal));
    assert!(configured_routes(&lab.sim, edge).is_empty());
    assert!(!lab.sim.ping(lab.host, ip("8.8.8.8")).reachable);
    let external = DomainRoute {
        router: edge,
        port: outside,
        next_hop: Some(ip("198.18.0.1")),
        ..internal
    };
    execute(&mut lab.sim, ProviderCommand::SetDomainRoute(external));
    assert!(!lab.sim.ping(lab.host, ip("8.8.8.8")).reachable);
    let return_route = DomainRoute {
        router: edge,
        prefix: prefix("203.0.113.0/24"),
        port: inside,
        next_hop: Some(ip("192.0.2.2")),
        ..internal
    };
    execute(&mut lab.sim, ProviderCommand::SetDomainRoute(return_route));
    let path = lab.sim.ping(lab.host, ip("8.8.8.8"));
    assert!(path.reachable);
    assert!(path.hops.iter().any(|hop| hop.device == lab.router));
    assert!(path.hops.iter().any(|hop| hop.device == edge));
    assert!(lab.sim.ping_from_internet(ip("203.0.113.10")).reachable);
    let before = ron::to_string(&lab.sim).unwrap();
    assert!(
        lab.sim
            .execute(Command::Provider(ProviderCommand::SetDomainRoute(
                DomainRoute {
                    router: lab.router,
                    ..external
                }
            )))
            .is_err()
    );
    assert_eq!(ron::to_string(&lab.sim).unwrap(), before);
    assert!(
        lab.sim
            .execute(Command::Provider(ProviderCommand::ReplaceDomainRoute {
                previous: return_route,
                route: DomainRoute {
                    next_hop: Some(ip("10.99.0.1")),
                    ..return_route
                },
            }))
            .is_err()
    );
    assert_eq!(ron::to_string(&lab.sim).unwrap(), before);
    let edited = DomainRoute {
        preference: 20,
        ..return_route
    };
    execute(
        &mut lab.sim,
        ProviderCommand::ReplaceDomainRoute {
            previous: return_route,
            route: edited,
        },
    );
    assert_eq!(configured_routes(&lab.sim, edge).len(), 2);
    assert!(configured_routes(&lab.sim, edge).contains(&edited));
    assert!(lab.sim.ping_from_internet(ip("203.0.113.10")).reachable);
    execute(&mut lab.sim, ProviderCommand::RemoveDomainRoute(internal));
    assert!(!lab.sim.ping(lab.host, ip("8.8.8.8")).reachable);
    assert_eq!(configured_routes(&lab.sim, edge).len(), 2);
}

#[test]
fn saved_global_routes_migrate_once_into_their_owning_router() {
    let mut lab = Lab::new();
    let route = DomainRoute {
        router: lab.router,
        domain: RoutingDomain(0),
        prefix: prefix("9.9.9.9/32"),
        port: lab.wan,
        vlan: VlanId(1),
        next_hop: Some(ip("192.0.2.1")),
        preference: 8,
        track_neighbor: true,
    };
    let saved = ron::to_string(&lab.sim).unwrap();
    let legacy = saved.replacen(
        "provider:(",
        &format!("provider:(routes:[{}],", ron::to_string(&route).unwrap()),
        1,
    );
    assert_ne!(saved, legacy);
    lab.sim = ron::from_str(&legacy).unwrap();
    lab.sim.rebuild_indexes();
    assert_eq!(configured_routes(&lab.sim, lab.router), &[route]);
    lab.sim.rebuild_indexes();
    assert_eq!(configured_routes(&lab.sim, lab.router), &[route]);
    let saved = ron::to_string(&lab.sim).unwrap();
    assert!(!saved.contains("provider:(routes:"));
    let mut restored: NetworkSim = ron::from_str(&saved).unwrap();
    restored.rebuild_indexes();
    assert_eq!(configured_routes(&restored, lab.router), &[route]);
    assert!(restored.execute_console(lab.router, "enable").success);
    let output = restored.execute_console(lab.router, "show ip route");
    assert!(output.success);
    assert!(output.lines.join("\n").contains("9.9.9.9/32"));
}

#[test]
fn guest_network_commands_cannot_configure_other_devices_or_the_carrier() {
    let mut lab = Lab::new();
    let before = ron::to_string(lab.sim.provider()).unwrap();
    for command in [
        format!("netctl bind {} 1 42 private", lab.lan),
        format!("netctl transit state {} down", lab.circuit),
        format!(
            "netctl route add {} 0 9.9.9.9/32 {} 1 192.0.2.1 1 keep",
            lab.router, lab.wan
        ),
    ] {
        assert!(!lab.sim.execute_console(lab.server, &command).success);
    }
    assert_eq!(ron::to_string(lab.sim.provider()).unwrap(), before);
    assert!(configured_routes(&lab.sim, lab.router).is_empty());
    assert!(
        lab.sim
            .execute_console(
                lab.server,
                &format!("netctl bind {} 1 42 private", lab.host)
            )
            .success
    );
    assert_eq!(
        lab.sim.provider().domain(lab.host, VlanId(1)),
        RoutingDomain(42)
    );
}

#[test]
fn range_delivery_requires_the_wan_and_local_gateway_configured_by_the_player() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    for supply in [CableSupply::CableBox305m, CableSupply::Rj45Pack20] {
        sim.execute(Command::BuyCableSupply { supply }).unwrap();
    }
    let router = device(&mut sim, DeviceTemplate::Router, 1);
    let server = device(&mut sim, DeviceTemplate::Server, 2);
    let ports = sim.device(router).unwrap().ports().to_vec();
    let (wan, lan) = (ports[0], ports[2]);
    let host = sim.device(server).unwrap().ports()[0];
    let block = sim.buy_public_ipv4_pool().unwrap();
    let range = Ipv4Prefix::new(block.network, PublicIpv4Block::PREFIX).unwrap();
    let mut uplinks: Vec<_> = sim
        .network_outlets()
        .filter(|o| matches!(o.kind, NetworkOutletKind::Uplink { .. }))
        .map(|o| o.port)
        .collect();
    uplinks.sort();
    let before = sim.device(router).unwrap().kind.clone();
    let handoff = sim.range_handoff(range, uplinks[0]).unwrap();
    assert_eq!(handoff.upstream_gateway, ip("192.0.2.1"));
    assert_eq!(handoff.router_address, ip("192.0.2.2"));
    assert_eq!(handoff.local_gateway, block.gateway());
    assert_eq!(sim.range_uplink(range), None);
    execute(
        &mut sim,
        ProviderCommand::RouteOwnedRange {
            prefix: range,
            uplink: Some(uplinks[0]),
        },
    );
    assert_eq!(sim.device(router).unwrap().kind, before);
    assert_eq!(sim.range_uplink(range), Some(uplinks[0]));
    assert_eq!(sim.range_handoff(range, uplinks[0]).unwrap(), handoff);
    cable(&mut sim, wan, uplinks[0]);
    cable(&mut sim, lan, host);
    let address = sim.assign_public_ipv4(host, block.network).unwrap();
    assert!(!sim.ping(host, ip("8.8.8.8")).reachable);
    iface(
        &mut sim,
        wan,
        &handoff.router_address.to_string(),
        handoff.upstream_subnet.length(),
    );
    iface(
        &mut sim,
        lan,
        &handoff.local_gateway.to_string(),
        range.length(),
    );
    assert!(!sim.ping(host, ip("8.8.8.8")).reachable);
    let default = DomainRoute {
        router,
        domain: RoutingDomain(0),
        prefix: prefix("0.0.0.0/0"),
        port: wan,
        vlan: VlanId(1),
        next_hop: Some(handoff.upstream_gateway),
        preference: 1,
        track_neighbor: false,
    };
    execute(&mut sim, ProviderCommand::SetDomainRoute(default));
    assert!(sim.ping(host, ip("8.8.8.8")).reachable);
    assert!(sim.ping_from_internet(address).reachable);
    let router_before = sim.device(router).unwrap().kind.clone();
    execute(
        &mut sim,
        ProviderCommand::RouteOwnedRange {
            prefix: range,
            uplink: Some(uplinks[1]),
        },
    );
    assert_eq!(sim.device(router).unwrap().kind, router_before);
    assert_eq!(sim.range_uplink(range), Some(uplinks[1]));
    assert!(
        !sim.provider()
            .circuit(uplinks[0])
            .unwrap()
            .routes
            .iter()
            .any(|r| r.prefix == range)
    );
    assert!(!sim.ping_from_internet(address).reachable);
    assert!(!sim.ping(host, ip("8.8.8.8")).reachable);
    let next = sim.range_handoff(range, uplinks[1]).unwrap();
    assert_eq!(next.upstream_gateway, ip("192.0.2.5"));
    assert_eq!(next.router_address, ip("192.0.2.6"));
    let link = sim.link_for_port(wan).unwrap().id;
    sim.execute(Command::Disconnect { link }).unwrap();
    cable(&mut sim, wan, uplinks[1]);
    iface(
        &mut sim,
        wan,
        &next.router_address.to_string(),
        next.upstream_subnet.length(),
    );
    execute(
        &mut sim,
        ProviderCommand::ReplaceDomainRoute {
            previous: default,
            route: DomainRoute {
                next_hop: Some(next.upstream_gateway),
                ..default
            },
        },
    );
    assert!(sim.ping(host, ip("8.8.8.8")).reachable);
    assert!(sim.ping_from_internet(address).reachable);
    let another = sim.buy_public_ipv4_pool().unwrap();
    let another_range = Ipv4Prefix::new(another.network, PublicIpv4Block::PREFIX).unwrap();
    execute(
        &mut sim,
        ProviderCommand::RouteOwnedRange {
            prefix: another_range,
            uplink: Some(uplinks[1]),
        },
    );
    assert_eq!(
        sim.range_handoff(another_range, uplinks[1])
            .unwrap()
            .router_address,
        next.router_address
    );
    let saved = ron::to_string(&sim).unwrap();
    let mut loaded: NetworkSim = ron::from_str(&saved).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.range_uplink(range), Some(uplinks[1]));
    assert_eq!(loaded.range_handoff(range, uplinks[1]).unwrap(), next);
    assert!(loaded.ping_from_internet(address).reachable);
    execute(
        &mut loaded,
        ProviderCommand::RouteOwnedRange {
            prefix: range,
            uplink: None,
        },
    );
    assert_eq!(loaded.range_uplink(range), None);
    assert_eq!(loaded.range_uplink(another_range), Some(uplinks[1]));
    assert!(!loaded.ping_from_internet(address).reachable);
}

#[test]
fn invalid_range_delivery_changes_are_atomic() {
    let mut lab = Lab::new();
    let block = lab.sim.buy_public_ipv4_pool().unwrap();
    let owned = Ipv4Prefix::new(block.network, PublicIpv4Block::PREFIX).unwrap();
    let before = ron::to_string(&lab.sim).unwrap();
    for (prefix, uplink) in [(owned, lab.host), (prefix("198.51.100.0/24"), lab.circuit)] {
        assert!(
            lab.sim
                .execute(Command::Provider(ProviderCommand::RouteOwnedRange {
                    prefix,
                    uplink: Some(uplink)
                }))
                .is_err()
        );
        assert_eq!(ron::to_string(&lab.sim).unwrap(), before);
    }
}

#[test]
fn ios_default_route_for_the_range_page_forwards_and_survives_reload() {
    let mut lab = Lab::new();
    let block = lab.sim.buy_public_ipv4_pool().unwrap();
    let range = Ipv4Prefix::new(block.network, PublicIpv4Block::PREFIX).unwrap();
    let mut uplinks: Vec<_> = lab
        .sim
        .network_outlets()
        .filter(|o| matches!(o.kind, NetworkOutletKind::Uplink { .. }))
        .map(|o| o.port)
        .collect();
    uplinks.sort();
    execute(
        &mut lab.sim,
        ProviderCommand::RouteOwnedRange {
            prefix: range,
            uplink: Some(uplinks[1]),
        },
    );
    let link = lab.sim.link_for_port(lab.wan).unwrap().id;
    lab.sim.execute(Command::Disconnect { link }).unwrap();
    cable(&mut lab.sim, lab.wan, uplinks[1]);
    lab.sim
        .execute(Command::RemoveStaticRoute {
            router: lab.router,
            route: Route {
                network: ip("0.0.0.0"),
                prefix: 0,
                via: Some(ip("192.0.2.1")),
                egress: lab.wan,
            },
        })
        .unwrap();
    let host = lab.sim.assign_public_ipv4(lab.host, block.network).unwrap();
    let lan_name = lab.sim.ios_interface_name(lab.router, lab.lan);
    let script = format!(
        "enable\nconfigure terminal\ninterface GigabitEthernet0/0/0\ndescription Uplink_to_Higher_Network\nip address 192.0.2.6 255.255.255.252\nno shutdown\ninterface {lan_name}\ndescription LAN_Server_Network\nip address 203.0.113.1 255.255.255.248\nno shutdown\nexit\nip route 0.0.0.0 0.0.0.0 192.0.2.5\nend\nwrite memory"
    );
    for line in script.lines() {
        let output = lab.sim.execute_console(lab.router, line);
        assert!(output.success, "{line}: {:?}", output.lines);
    }
    assert_eq!(configured_routes(&lab.sim, lab.router)[0].port, lab.wan);
    assert!(lab.sim.ping(lab.host, ip("8.8.8.8")).reachable);
    assert!(lab.sim.ping_from_internet(host).reachable);
    for target in ["192.0.2.5", "8.8.8.8"] {
        let output = lab
            .sim
            .execute_console(lab.router, &format!("ping {target}"));
        assert!(output.success, "{:?}", output.lines);
    }
    let PortConfig::Router(wan) = &lab.sim.port(lab.wan).unwrap().config else {
        panic!()
    };
    assert_eq!(wan.interfaces.len(), 1);
    let output = lab.sim.execute_console(lab.router, "show ip route");
    assert!(
        output
            .lines
            .join("\n")
            .contains("S 0.0.0.0/0 [1] via 192.0.2.5")
    );
    let output = lab.sim.execute_console(lab.router, "show running-config");
    assert!(
        output
            .lines
            .join("\n")
            .contains("ip route 0.0.0.0 0.0.0.0 GigabitEthernet0/0/0 192.0.2.5")
    );
    for line in [
        "configure terminal",
        "no ip route 0.0.0.0 0.0.0.0 192.0.2.5",
        "end",
    ] {
        assert!(lab.sim.execute_console(lab.router, line).success);
    }
    assert!(!lab.sim.ping(lab.host, ip("8.8.8.8")).reachable);
    assert!(lab.sim.execute_console(lab.router, "reload").success);
    assert!(lab.sim.execute_console(lab.router, "").success);
    assert!(lab.sim.ping(lab.host, ip("8.8.8.8")).reachable);
    assert!(lab.sim.ping_from_internet(host).reachable);
}

#[test]
fn router_trunk_keeps_vlan_tags_through_a_patch_panel() {
    let mut l = Lab::new();
    let old = l.sim.link_for_port(l.lan).unwrap().id;
    l.sim.execute(Command::Disconnect { link: old }).unwrap();
    let switch = device(&mut l.sim, DeviceTemplate::Switch, 3);
    let panel = device(&mut l.sim, DeviceTemplate::PatchPanel, 4);
    let sw = l.sim.device(switch).unwrap().ports().to_vec();
    let pp = l.sim.device(panel).unwrap().ports().to_vec();
    cable(&mut l.sim, l.lan, pp[0]);
    cable(&mut l.sim, pp[1], sw[0]);
    cable(&mut l.sim, sw[1], l.host);
    l.sim
        .execute(Command::CreateVlan {
            switch,
            vlan: Vlan {
                id: VlanId(99),
                name: "Tagged LAN".into(),
            },
        })
        .unwrap();
    l.sim
        .execute(Command::SetSwitchPortMode {
            port: sw[0],
            mode: SwitchPortMode::Trunk {
                native_vlan: Some(VlanId(1)),
                allowed: vec![VlanId(1), VlanId(99)],
            },
        })
        .unwrap();
    l.sim
        .execute(Command::SetSwitchPortMode {
            port: sw[1],
            mode: SwitchPortMode::Access {
                vlan: Some(VlanId(99)),
            },
        })
        .unwrap();
    l.sim
        .execute(Command::ConfigureRouterInterface {
            port: l.lan,
            name: "Gi0/1/0.99".into(),
            vlan: Some(VlanId(99)),
            address: Some(ip("10.99.0.1")),
            prefix: 24,
            internet_connected: false,
        })
        .unwrap();
    l.sim
        .execute(Command::SetIpv4 {
            port: l.host,
            config: Ipv4InterfaceConfig {
                address: ip("10.99.0.2"),
                prefix: 24,
                gateway: Some(ip("10.99.0.1")),
                vlan: None,
            },
        })
        .unwrap();
    assert_eq!(l.sim.wire_vlan_for(l.lan, VlanId(99)), Some(VlanId(99)));
    assert!(l.sim.ping(l.host, ip("10.99.0.1")).reachable);
    assert!(l.sim.ping_router_mut(l.router, ip("10.99.0.2")).reachable);
    let output = l.sim.execute_console(l.router, "enable");
    assert!(output.success);
    assert!(
        l.sim
            .execute_console(l.router, "configure terminal")
            .success
    );
    assert!(l.sim.execute_console(l.router, "do ping 10.99.0.2").success);
    // WAN is connected to a carrier socket; it has no player device owner.
    let neighbors = l.sim.execute_console(l.router, "do show cdp neighbors");
    assert!(neighbors.success);
    assert_eq!(neighbors.lines.len(), 2);
    assert!(neighbors.lines[1].contains(&l.sim.device(switch).unwrap().name));

    assert_eq!(l.sim.terminal_prompt(l.router), "Router(config)#");
    let arp = l.sim.execute_console(l.router, "do show ip arp");
    assert!(arp.success);
    assert!(
        arp.lines
            .iter()
            .any(|line| line.contains("10.99.0.2") && line.ends_with("99"))
    );
    l.sim
        .execute(Command::SetPortEnabled {
            port: l.lan,
            enabled: false,
        })
        .unwrap();
    let arp = l.sim.execute_console(l.router, "do show arp");
    assert!(arp.success);
    assert!(!arp.lines.iter().any(|line| line.contains("10.99.0.2")));
}

#[test]
fn dhcp_reserves_management_and_transit_endpoints() {
    let mut l = Lab::new();
    let switch = device(&mut l.sim, DeviceTemplate::Switch, 3);
    execute(
        &mut l.sim,
        ProviderCommand::SetSwitchManagement(SwitchManagement {
            switch,
            address: ip("203.0.113.20"),
            prefix: 24,
            vlan: VlanId(1),
            gateway: None,
        }),
    );
    let mut circuit = l.sim.provider().circuit(l.circuit).unwrap().clone();
    circuit.address = ip("203.0.113.21");
    circuit.prefix = 24;
    circuit.routes.clear();
    execute(&mut l.sim, ProviderCommand::SetTransit(circuit));
    execute(
        &mut l.sim,
        ProviderCommand::SetDhcp(DhcpPool {
            server: l.lan,
            vlan: VlanId(1),
            prefix: prefix("203.0.113.0/24"),
            first: ip("203.0.113.20"),
            last: ip("203.0.113.22"),
            gateway: Some(ip("203.0.113.1")),
        }),
    );
    assert_eq!(l.sim.request_dhcp(l.host).unwrap(), ip("203.0.113.22"));
}

#[test]
fn reload_restores_only_the_owning_devices_advanced_configuration() {
    let mut l = Lab::new();
    let switch = device(&mut l.sim, DeviceTemplate::Switch, 3);
    let management = SwitchManagement {
        switch,
        address: ip("10.0.0.1"),
        prefix: 24,
        vlan: VlanId(1),
        gateway: None,
    };
    execute(&mut l.sim, ProviderCommand::SetSwitchManagement(management));
    execute(
        &mut l.sim,
        ProviderCommand::SetSpanningTree {
            switch,
            enabled: false,
        },
    );
    let binding = InterfaceBinding {
        port: l.wan,
        vlan: VlanId(1),
        domain: RoutingDomain(0),
        role: NetworkRole::Public,
    };
    execute(&mut l.sim, ProviderCommand::BindInterface(binding));
    execute(
        &mut l.sim,
        ProviderCommand::AddPool(AddressPool {
            prefix: prefix("203.0.113.0/24"),
            description: "Test aggregate".into(),
        }),
    );
    let session = l.session();
    execute(&mut l.sim, ProviderCommand::SetBgp(session.clone()));
    let pool = DhcpPool {
        server: l.lan,
        vlan: VlanId(1),
        prefix: prefix("203.0.113.0/24"),
        first: ip("203.0.113.20"),
        last: ip("203.0.113.30"),
        gateway: Some(ip("203.0.113.1")),
    };
    execute(&mut l.sim, ProviderCommand::SetDhcp(pool));
    for dev in [l.router, switch] {
        assert!(l.sim.execute_console(dev, "enable").success);
        assert!(l.sim.execute_console(dev, "write memory").success);
    }
    execute(
        &mut l.sim,
        ProviderCommand::RemoveBinding {
            port: l.wan,
            vlan: VlanId(1),
        },
    );
    execute(
        &mut l.sim,
        ProviderCommand::RemoveBgp {
            port: l.wan,
            vlan: VlanId(1),
        },
    );
    execute(
        &mut l.sim,
        ProviderCommand::RemoveDhcp {
            server: l.lan,
            vlan: VlanId(1),
        },
    );
    execute(
        &mut l.sim,
        ProviderCommand::SetSpanningTree {
            switch,
            enabled: true,
        },
    );
    assert!(l.sim.execute_console(switch, "configure terminal").success);
    assert!(
        l.sim
            .execute_console(switch, "management ip 10.0.0.2")
            .success
    );
    assert_eq!(
        l.sim.switch_management(switch).unwrap().address,
        ip("10.0.0.2")
    );
    assert!(l.sim.execute_console(switch, "no management ip").success);
    assert_eq!(l.sim.switch_management(switch), None);
    assert!(l.sim.execute_console(switch, "end").success);
    // Save serialization retains the per-device startup snapshot.
    let mut restored: NetworkSim = ron::from_str(&ron::to_string(&l.sim).unwrap()).unwrap();
    assert!(restored.execute_console(l.router, "enable").success);
    assert!(restored.execute_console(l.router, "reload").success);
    assert!(restored.execute_console(l.router, "").success);
    assert_eq!(restored.provider().bindings(), &[binding]);
    assert_eq!(restored.provider().dhcp_pools(), &[pool]);
    assert_eq!(restored.provider().sessions(), &[session]);
    assert_eq!(restored.switch_management(switch), None);
    assert!(restored.provider().spanning_tree_enabled(switch));
    assert!(restored.execute_console(switch, "enable").success);
    assert!(restored.execute_console(switch, "reload").success);
    assert!(restored.execute_console(switch, "").success);
    assert_eq!(restored.switch_management(switch), Some(management));
    assert!(!restored.provider().spanning_tree_enabled(switch));
    assert_eq!(restored.provider().dhcp_pools(), &[pool]);
}

#[test]
fn old_duplicate_wan_records_are_repaired_in_running_and_startup_config() {
    let mut l = Lab::new();
    l.sim
        .execute(Command::ConfigureRouterInterface {
            port: l.wan,
            name: "GigabitEthernet0/0/0".into(),
            vlan: Some(VlanId(1)),
            address: Some(ip("192.0.2.6")),
            prefix: 30,
            internet_connected: false,
        })
        .unwrap();
    let mut circuit = l.sim.provider().circuit(l.circuit).unwrap().clone();
    circuit.address = ip("192.0.2.5");
    circuit.routes[0].next_hop = ip("192.0.2.6");
    execute(&mut l.sim, ProviderCommand::SetTransit(circuit));
    let DeviceKind::Router(router) = &l.sim.device(l.router).unwrap().kind else {
        panic!()
    };
    let old_route = router.routes[0].clone();
    l.sim
        .execute(Command::RemoveStaticRoute {
            router: l.router,
            route: old_route,
        })
        .unwrap();
    route(&mut l.sim, l.router, "0.0.0.0/0", "192.0.2.5", l.wan);
    l.sim
        .execute(Command::ConfigureRouterInterface {
            port: l.wan,
            name: "GigabitEthernet0/0/0.20".into(),
            vlan: Some(VlanId(20)),
            address: Some(ip("10.20.0.1")),
            prefix: 24,
            internet_connected: false,
        })
        .unwrap();
    assert!(l.sim.execute_console(l.router, "enable").success);
    assert!(l.sim.execute_console(l.router, "write memory").success);
    let PortConfig::Router(config) = &l.sim.port(l.wan).unwrap().config else {
        panic!()
    };
    let configured = ron::to_string(
        config
            .interfaces
            .iter()
            .find(|i| i.vlan == Some(VlanId(1)))
            .unwrap(),
    )
    .unwrap();
    let placeholder = ron::to_string(&RouterInterface::wan("WAN1", l.wan)).unwrap();
    let saved = ron::to_string(&l.sim).unwrap();
    // Reproduce the old game's redundant records in both running and startup copies.
    assert_eq!(saved.matches(&configured).count(), 4);
    let legacy = saved.replace(&configured, &format!("{placeholder},{configured}"));
    let mut loaded: NetworkSim = ron::from_str(&legacy).unwrap();
    assert_eq!(
        loaded.interface_ipv4(l.wan, VlanId(1)),
        Some(ip("192.0.2.6"))
    );
    loaded.rebuild_indexes();
    for reload in [false, true] {
        if reload {
            assert!(loaded.execute_console(l.router, "reload").success);
            assert!(loaded.execute_console(l.router, "").success);
        }
        assert!(loaded.execute_console(l.router, "enable").success);
        let interfaces = loaded.execute_console(l.router, "show ip interface brief");
        assert!(interfaces.success);
        assert_eq!(
            interfaces
                .lines
                .iter()
                .filter(|line| line.starts_with("GigabitEthernet0/0/0 "))
                .count(),
            1
        );
        assert!(
            interfaces
                .lines
                .iter()
                .any(|line| line.contains("10.20.0.1"))
        );
        let PortConfig::Router(config) = &loaded.port(l.wan).unwrap().config else {
            panic!()
        };
        assert_eq!(config.interfaces.len(), 2);
        assert!(loaded.execute_console(l.router, "ping 192.0.2.5").success);
        assert!(loaded.execute_console(l.router, "ping 8.8.8.8").success);
        assert!(loaded.ping(l.host, ip("8.8.8.8")).reachable);
        assert!(loaded.ping_from_internet(ip("203.0.113.10")).reachable);
    }
    let mut again: NetworkSim = ron::from_str(&ron::to_string(&loaded).unwrap()).unwrap();
    again.rebuild_indexes();
    assert!(again.ping_router_mut(l.router, ip("192.0.2.5")).reachable);
}

#[test]
fn vlan_one_and_untagged_router_configuration_replace_the_same_record() {
    let mut l = Lab::new();
    for vlan in [Some(VlanId(1)), None, Some(VlanId(1))] {
        l.sim
            .execute(Command::ConfigureRouterInterface {
                port: l.wan,
                name: "GigabitEthernet0/0/0".into(),
                vlan,
                address: Some(ip("192.0.2.2")),
                prefix: 30,
                internet_connected: false,
            })
            .unwrap();
        let PortConfig::Router(config) = &l.sim.port(l.wan).unwrap().config else {
            panic!()
        };
        assert_eq!(config.interfaces.len(), 1);
        let DeviceKind::Router(router) = &l.sim.device(l.router).unwrap().kind else {
            panic!()
        };
        assert_eq!(
            router.interfaces.iter().filter(|i| i.port == l.wan).count(),
            1
        );
        assert!(l.sim.ping_router_mut(l.router, ip("192.0.2.1")).reachable);
    }
}
