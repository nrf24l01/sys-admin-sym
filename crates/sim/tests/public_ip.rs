use cloud_provider_sim::{
    CableSupply, Command, DeviceKind, DeviceTemplate, NetworkOutletKind, NetworkSim, OutletId,
    PortId, PowerEndpoint, PublicIpv4Block, RackId, SimEvent, SourceId,
};

fn ready_network() -> (NetworkSim, PortId, PortId, PortId) {
    let mut sim = NetworkSim::new();
    let SimEvent::DeviceAdded(switch) = sim
        .execute(Command::BuyDevice {
            kind: DeviceTemplate::Switch,
        })
        .unwrap()[0]
    else {
        panic!("switch missing")
    };
    let SimEvent::DeviceAdded(server) = sim.execute(Command::BuyServerFullPack).unwrap()[0] else {
        panic!("server missing")
    };
    for (device, unit, outlet) in [(switch, 1, 0), (server, 2, 1)] {
        sim.execute(Command::PlaceDevice {
            device,
            rack: RackId(1),
            unit,
        })
        .unwrap();
        sim.execute(Command::ConnectPower {
            outlet: OutletId {
                source: SourceId::Rack(RackId(1)),
                index: outlet,
            },
            endpoint: PowerEndpoint::Device(device),
        })
        .unwrap();
        sim.execute(Command::SetPower {
            device,
            powered: true,
        })
        .unwrap();
    }
    for supply in [CableSupply::CableBox305m, CableSupply::Rj45Pack20] {
        sim.execute(Command::BuyCableSupply { supply }).unwrap();
    }
    let switch_ports = [
        sim.device(switch).unwrap().ports()[0],
        sim.device(switch).unwrap().ports()[1],
    ];
    let uplink = sim
        .network_outlets()
        .filter(|outlet| matches!(outlet.kind, NetworkOutletKind::Uplink { .. }))
        .min_by_key(|outlet| outlet.port)
        .unwrap()
        .port;
    let server_port = match &sim.device(server).unwrap().kind {
        DeviceKind::Server(server) => server.ports[0],
        _ => unreachable!(),
    };
    sim.execute(Command::Connect {
        a: switch_ports[0],
        b: uplink,
    })
    .unwrap();
    sim.execute(Command::Connect {
        a: switch_ports[1],
        b: server_port,
    })
    .unwrap();
    (sim, uplink, server_port, switch_ports[0])
}

fn explicit_handoff(sim: &mut NetworkSim, port: PortId, block: PublicIpv4Block) {
    use cloud_provider_sim::{Ipv4Prefix, ProviderCommand, TransitCircuit, UpstreamRoute};
    sim.execute(Command::Provider(ProviderCommand::SetTransit(
        TransitCircuit {
            port,
            name: "On-link upstream".into(),
            address: block.gateway(),
            prefix: 29,
            asn: 64501,
            capacity_mbps: 1000,
            enabled: true,
            routes: vec![UpstreamRoute {
                prefix: Ipv4Prefix::new(block.network, 29).unwrap(),
                next_hop: block.host_addresses().next().unwrap(),
            }],
            offered_routes: vec![],
            authorizations: vec![],
        },
    )))
    .unwrap();
}

#[test]
fn explicit_on_link_handoff_routes_allocated_addresses() {
    let (mut sim, uplink, server_port, switch_port) = ready_network();
    let before = sim.money;
    sim.execute(Command::BuyPublicIpv4Block { uplink }).unwrap();
    let block = sim.public_ipv4_blocks()[0];
    assert_eq!(sim.money, before - PublicIpv4Block::PRICE);
    assert_eq!(block.uplink, uplink);
    assert_eq!(block.gateway().to_string(), "203.0.113.1");
    sim.execute(Command::AssignPublicIpv4 {
        port: server_port,
        network: block.network,
    })
    .unwrap();
    let address = block.host_addresses().next().unwrap();
    assert_eq!(
        sim.assign_public_ipv4(server_port, block.network).unwrap(),
        address
    );
    assert!(!sim.ping_from_internet(address).reachable);
    explicit_handoff(&mut sim, uplink, block);
    assert!(sim.ping_from_internet(address).reachable);
    assert!(sim.ping(server_port, [8, 8, 8, 8].into()).reachable);
    assert!(sim.port_telemetry(switch_port).rx_frames > 0);
    let saved = ron::to_string(&sim).unwrap();
    let mut loaded: NetworkSim = ron::from_str(&saved).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.public_ipv4_blocks(), &[block]);
    assert!(loaded.ping_from_internet(address).reachable);
}

#[test]
fn routing_uses_the_selected_uplink_and_fails_after_disconnection() {
    let (mut sim, connected_uplink, server_port, switch_port) = ready_network();
    let other_uplink = sim
        .network_outlets()
        .find(|outlet| {
            matches!(outlet.kind, NetworkOutletKind::Uplink { .. })
                && outlet.port != connected_uplink
        })
        .unwrap()
        .port;
    sim.execute(Command::BuyPublicIpv4Block {
        uplink: other_uplink,
    })
    .unwrap();
    let block = sim.public_ipv4_blocks()[0];
    sim.execute(Command::AssignPublicIpv4 {
        port: server_port,
        network: block.network,
    })
    .unwrap();
    let address = block.host_addresses().next().unwrap();
    explicit_handoff(&mut sim, other_uplink, block);
    assert!(!sim.ping_from_internet(address).reachable);
    let link = sim.link_for_port(connected_uplink).unwrap().id;
    sim.execute(Command::Disconnect { link }).unwrap();
    assert!(!sim.ping_from_internet(address).reachable);
    sim.execute(Command::Connect {
        a: switch_port,
        b: other_uplink,
    })
    .unwrap();
    assert!(sim.ping_from_internet(address).reachable);
}

#[test]
fn lan_assignment_is_private_and_public_purchase_requires_uplink() {
    let (mut sim, uplink, server_port, switch_port) = ready_network();
    let address = match sim
        .execute(Command::AssignLanIpv4 { port: server_port })
        .unwrap()[0]
    {
        SimEvent::PortConfigChanged(_) => match &sim.port(server_port).unwrap().config {
            cloud_provider_sim::PortConfig::Server(config) => config.ipv4.as_ref().unwrap().address,
            _ => unreachable!(),
        },
        _ => panic!("assignment did not change the port"),
    };
    assert_eq!(address.to_string(), "10.0.0.10");
    assert_eq!(sim.assign_lan_ipv4(server_port).unwrap(), address);
    assert!(!sim.ping_from_internet(address).reachable);
    assert!(
        sim.execute(Command::BuyPublicIpv4Block {
            uplink: switch_port
        })
        .is_err()
    );
    assert_eq!(sim.public_ipv4_blocks().len(), 0);
    sim.execute(Command::BuyPublicIpv4Block { uplink }).unwrap();
    let passive = sim
        .network_outlets()
        .find(|o| matches!(o.kind, NetworkOutletKind::Lan { .. }))
        .unwrap()
        .port;
    let network = sim.public_ipv4_blocks()[0].network;
    assert!(
        sim.execute(Command::AssignPublicIpv4 {
            port: passive,
            network
        })
        .is_err()
    );
}
