use cloud_provider_sim::*;

fn device(sim: &mut NetworkSim, command: Command, unit: u8) -> DeviceId {
    let SimEvent::DeviceAdded(id) = sim.execute(command).unwrap()[0] else {
        panic!()
    };
    sim.execute(Command::PlaceDevice {
        device: id,
        rack: RackId(1),
        unit,
    })
    .unwrap();
    let outlet = (0..RACK_C13_OUTLETS as u8)
        .map(|index| OutletId {
            source: SourceId::Rack(RackId(1)),
            index,
        })
        .find(|id| !sim.power.connections.contains_key(id))
        .unwrap();
    sim.execute(Command::ConnectPower {
        outlet,
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
fn buy(sim: &mut NetworkSim, kind: DeviceTemplate, unit: u8) -> DeviceId {
    device(sim, Command::BuyDevice { kind }, unit)
}
fn port(sim: &NetworkSim, device: DeviceId, index: usize) -> PortId {
    sim.device(device).unwrap().ports()[index]
}
fn script(sim: &mut NetworkSim, device: DeviceId, script: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for input in script.lines() {
        let result = sim.execute_console(device, input);
        assert!(result.success, "{input}: {:?}", result.lines);
        lines.extend(result.lines);
    }
    lines
}
fn stock(sim: &mut NetworkSim) {
    sim.money = 100_000;
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::CableBox305m,
    })
    .unwrap();
    for _ in 0..3 {
        sim.execute(Command::BuyCableSupply {
            supply: CableSupply::Rj45Pack20,
        })
        .unwrap();
    }
}
fn connect(sim: &mut NetworkSim, a: PortId, b: PortId) {
    sim.execute(Command::Connect { a, b }).unwrap();
}
fn frame(source: PortId, target: PortId, dscp: u8) -> EthernetFrame {
    EthernetFrame {
        qos: FrameQos {
            dscp,
            cos: 0,
            length_bytes: 64,
        },
        source: MacAddress::for_port(source),
        destination: MacAddress::for_port(target),
        vlan: None,
        payload: EthernetPayload::Ipv4 {
            packet: Ipv4Packet {
                dscp,
                source: "10.0.0.1".parse().unwrap(),
                destination: "10.0.0.2".parse().unwrap(),
                ttl: 64,
                protocol: 17,
            },
            icmp: IcmpMessage::EchoRequest {
                identifier: 1,
                sequence: 1,
            },
        },
    }
}

#[test]
fn catalyst_models_have_distinct_hardware_numbering_and_power() {
    let mut sim = NetworkSim::new();
    stock(&mut sim);
    let g = buy(&mut sim, DeviceTemplate::Switch, 1);
    let x = device(
        &mut sim,
        Command::Optics(OpticsCommand::BuyHardware {
            model: "switch_10g".into(),
        }),
        2,
    );
    for (device, model, prefix, first, watts) in [
        (g, SwitchModel::Catalyst24T4G, "GigabitEthernet", 25, 16),
        (x, SwitchModel::Catalyst24T4X, "TenGigabitEthernet", 1, 18),
    ] {
        let DeviceKind::Switch(switch) = &sim.device(device).unwrap().kind else {
            panic!()
        };
        assert_eq!(switch.model, model);
        assert_eq!(switch.ports.len(), 28);
        for index in 0..24 {
            assert_eq!(
                sim.port(switch.ports[index]).unwrap().max_speed,
                LinkSpeed::Gbps1
            );
        }
        for index in 24..28 {
            assert_eq!(
                sim.ios_interface_name(device, switch.ports[index]),
                format!("{prefix}1/0/{}", first + index - 24)
            );
        }
        assert_eq!(sim.power.devices[&device].load.watts, watts);
    }
    assert!(
        script(&mut sim, x, "show version")
            .iter()
            .any(|line| line.contains("128000"))
    );
    assert!(
        script(&mut sim, g, "show inventory")
            .iter()
            .any(|line| line.contains("C1000-24T-4G-L"))
    );
    let server = buy(&mut sim, DeviceTemplate::Server, 3);
    let source = port(&sim, server, 0);
    let ingress = port(&sim, g, 0);
    connect(&mut sim, source, ingress);
    sim.transmit_frame(source, frame(source, ingress, 0));
    assert_eq!(sim.power.devices[&g].load.watts, 16);
    sim.advance_time(1001);
    assert_eq!(sim.power.devices[&g].load.watts, 16);
    let names = sim
        .console_completions(x, "show interfaces Te1/0/")
        .candidates;
    assert!(names.contains(&"Te1/0/1".into()));
    assert!(!names.contains(&"Te1/0/25".into()));
}

#[test]
fn old_10g_saves_and_startup_descriptions_migrate_without_changing_ports_or_links() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let x = device(
        &mut sim,
        Command::Optics(OpticsCommand::BuyHardware {
            model: "switch_10g".into(),
        }),
        1,
    );
    script(
        &mut sim,
        x,
        "enable\nconfigure terminal\ninterface Te1/0/1\ndescription Fiber uplink\nend\nwrite memory",
    );
    let ports = sim.device(x).unwrap().ports().to_vec();
    let encoded = ron::to_string(&sim).unwrap();
    assert!(encoded.contains("model:c1000_24t_4x_l,"));
    let legacy = encoded
        .replace("model:c1000_24t_4x_l,", "")
        .replace("TenGigabitEthernet1/0/1", "TenGigabitEthernet1/0/25")
        .replace("Te1/0/01", "Te1/0/25");
    assert!(
        !legacy.contains("model:c1000_24t_4x_l,"),
        "model must be omitted from the legacy fixture"
    );
    let mut loaded: NetworkSim = ron::from_str(&legacy).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.device(x).unwrap().ports(), ports);
    assert_eq!(
        loaded.ios_interface_name(x, ports[24]),
        "TenGigabitEthernet1/0/1"
    );
    assert_eq!(loaded.power.devices[&x].load.watts, 18);
    assert!(
        script(&mut loaded, x, "enable\nshow running-config")
            .iter()
            .any(|line| line.contains("Fiber uplink"))
    );
    script(&mut loaded, x, "reload\n\n");
    assert_eq!(
        loaded.ios_interface_name(x, ports[24]),
        "TenGigabitEthernet1/0/1"
    );
    assert!(
        script(&mut loaded, x, "enable\nshow running-config")
            .iter()
            .any(|line| line.contains("Fiber uplink"))
    );
}

#[test]
fn lacp_bundle_forwards_once_fails_over_and_obeys_minimum_links() {
    let mut sim = NetworkSim::new();
    stock(&mut sim);
    let a = buy(&mut sim, DeviceTemplate::Switch, 1);
    let b = buy(&mut sim, DeviceTemplate::Switch, 2);
    let sa = buy(&mut sim, DeviceTemplate::Server, 3);
    let sb = buy(&mut sim, DeviceTemplate::Server, 4);
    for index in 0..2 {
        let ap = port(&sim, a, index);
        let bp = port(&sim, b, index);
        connect(&mut sim, ap, bp);
    }
    let source = port(&sim, sa, 0);
    let destination = port(&sim, sb, 0);
    let ap = port(&sim, a, 2);
    let bp = port(&sim, b, 2);
    connect(&mut sim, source, ap);
    connect(&mut sim, destination, bp);
    script(
        &mut sim,
        a,
        "enable\nconfigure terminal\ninterface range Gi1/0/1 - 2\nchannel-group 1 mode active\nend",
    );
    script(
        &mut sim,
        b,
        "enable\nconfigure terminal\ninterface range Gi1/0/1 - 2\nchannel-group 2 mode passive\nend",
    );
    assert_eq!(sim.channel_capacity_mbps(a, 1), 2000);
    assert!(
        script(&mut sim, a, "show interfaces Po1")
            .iter()
            .any(|line| line.contains("BW 2000 Mbps"))
    );
    assert_eq!(sim.channel_capacity_mbps(b, 2), 2000);
    assert!(sim.spanning_tree_blocked_ports(VlanId(1)).is_empty());
    let delivered = sim.transmit_frame(source, frame(source, destination, 0));
    assert_eq!(
        delivered.iter().filter(|d| d.port == destination).count(),
        1
    );
    let members = sim.channel_ports(a, 1);
    let used = *members
        .iter()
        .find(|port| sim.port_telemetry(**port).tx_frames > 0)
        .unwrap();
    sim.execute(Command::SetPortEnabled {
        port: used,
        enabled: false,
    })
    .unwrap();
    assert_eq!(sim.channel_capacity_mbps(a, 1), 1000);
    assert_eq!(
        sim.transmit_frame(source, frame(source, destination, 0))
            .iter()
            .filter(|d| d.port == destination)
            .count(),
        1
    );
    script(
        &mut sim,
        a,
        "configure terminal\ninterface Port-channel1\nport-channel min-links 2\nend",
    );
    assert_eq!(sim.channel_capacity_mbps(a, 1), 0);
    assert_eq!(sim.channel_capacity_mbps(b, 2), 0);
    assert!(
        !sim.transmit_frame(source, frame(source, destination, 0))
            .iter()
            .any(|d| d.port == destination)
    );
}

#[test]
fn passive_peers_do_not_bundle_and_vlan_mismatches_are_suspended() {
    let mut sim = NetworkSim::new();
    stock(&mut sim);
    let a = buy(&mut sim, DeviceTemplate::Switch, 1);
    let b = buy(&mut sim, DeviceTemplate::Switch, 2);
    let ap = port(&sim, a, 0);
    let bp = port(&sim, b, 0);
    connect(&mut sim, ap, bp);
    sim.set_channel_group(ap, Some((1, ChannelMode::Passive)))
        .unwrap();
    sim.set_channel_group(bp, Some((1, ChannelMode::Passive)))
        .unwrap();
    assert_eq!(sim.channel_state(ap), ChannelState::Suspended);
    sim.set_channel_group(ap, Some((1, ChannelMode::Active)))
        .unwrap();
    assert_eq!(sim.channel_state(ap), ChannelState::Bundled);
    script(
        &mut sim,
        b,
        "enable\nconfigure terminal\ninterface Gi1/0/1\nswitchport mode trunk\nend",
    );
    assert_eq!(sim.channel_state(ap), ChannelState::Suspended);
    let second = port(&sim, a, 1);
    script(
        &mut sim,
        a,
        "enable\nconfigure terminal\ninterface Gi1/0/2\nspeed 100\nend",
    );
    assert!(
        sim.set_channel_group(second, Some((1, ChannelMode::Active)))
            .is_err()
    );
    assert!(sim.channel_member(second).is_none());
}

#[test]
fn lacp_has_eight_active_links_with_hot_standby_takeover() {
    let mut sim = NetworkSim::new();
    stock(&mut sim);
    let a = buy(&mut sim, DeviceTemplate::Switch, 1);
    let b = buy(&mut sim, DeviceTemplate::Switch, 2);
    for index in 0..16 {
        let ap = port(&sim, a, index);
        let bp = port(&sim, b, index);
        connect(&mut sim, ap, bp);
        sim.set_channel_group(ap, Some((1, ChannelMode::Active)))
            .unwrap();
        sim.set_channel_group(bp, Some((1, ChannelMode::Passive)))
            .unwrap();
    }
    let members = sim.channel_ports(a, 1);
    assert_eq!(sim.channel_capacity_mbps(a, 1), 8000);
    assert_eq!(
        members
            .iter()
            .filter(|p| sim.channel_state(**p) == ChannelState::Bundled)
            .count(),
        8
    );
    assert_eq!(
        members
            .iter()
            .filter(|p| sim.channel_state(**p) == ChannelState::Standby)
            .count(),
        8
    );
    sim.execute(Command::SetPortEnabled {
        port: members[0],
        enabled: false,
    })
    .unwrap();
    assert_eq!(sim.channel_capacity_mbps(a, 1), 8000);
    assert_eq!(sim.channel_state(members[8]), ChannelState::Bundled);
}

#[test]
fn qos_rewrites_untrusted_packets_prioritizes_voice_and_enforces_rate_budgets() {
    let mut sim = NetworkSim::new();
    stock(&mut sim);
    let sw = buy(&mut sim, DeviceTemplate::Switch, 1);
    let a = buy(&mut sim, DeviceTemplate::Server, 2);
    let b = buy(&mut sim, DeviceTemplate::Server, 3);
    let source = port(&sim, a, 0);
    let target = port(&sim, b, 0);
    let ingress = port(&sim, sw, 0);
    let egress = port(&sim, sw, 1);
    connect(&mut sim, source, ingress);
    connect(&mut sim, target, egress);
    let voice = frame(source, target, 46);
    assert_eq!(
        sim.transmit_frame(source, voice)
            .into_iter()
            .find(|d| d.port == target)
            .unwrap()
            .frame
            .qos
            .dscp,
        46
    );
    script(&mut sim, sw, "enable\nconfigure terminal\nmls qos\nend");
    assert_eq!(
        sim.transmit_frame(source, voice)
            .into_iter()
            .find(|d| d.port == target)
            .unwrap()
            .frame
            .qos
            .dscp,
        0
    );
    script(
        &mut sim,
        sw,
        "configure terminal\ninterface Gi1/0/1\nmls qos trust dscp\ninterface Gi1/0/2\npriority-queue out\nend",
    );
    let delivered = sim.transmit_frames(source, vec![frame(source, target, 0), voice]);
    assert_eq!(
        delivered
            .iter()
            .filter(|d| d.port == target)
            .map(|d| d.frame.qos.dscp)
            .collect::<Vec<_>>(),
        vec![46, 0]
    );
    for delivery in &delivered {
        if let EthernetPayload::Ipv4 { packet, .. } = delivery.frame.payload {
            assert_eq!(packet.dscp, delivery.frame.qos.dscp);
        }
    }
    assert!(sim.qos_counters(egress).transmitted[0] > 0);
    script(
        &mut sim,
        sw,
        "configure terminal\ninterface Gi1/0/2\nspeed 10\nsrr-queue bandwidth limit 10\nend",
    );
    let mut large = voice;
    large.qos.length_bytes = 1500;
    let delivered = sim.transmit_frames(source, vec![large; 12]);
    assert_eq!(delivered.iter().filter(|d| d.port == target).count(), 1);
    assert_eq!(sim.qos_counters(egress).dropped[0], 11);
    sim.advance_time(20);
    assert!(
        sim.transmit_frame(source, large)
            .iter()
            .any(|d| d.port == target)
    );
    script(&mut sim, sw, "configure terminal\nno mls qos\nend");
    assert_eq!(
        sim.transmit_frames(source, vec![large; 12])
            .iter()
            .filter(|d| d.port == target)
            .count(),
        12
    );
}

#[test]
fn snmp_guest_queries_require_reachability_and_community_permissions_and_use_udp_policy() {
    let mut sim = NetworkSim::new();
    stock(&mut sim);
    let sw = buy(&mut sim, DeviceTemplate::Switch, 1);
    let server = buy(&mut sim, DeviceTemplate::Server, 2);
    let source = port(&sim, server, 0);
    let ingress = port(&sim, sw, 0);
    connect(&mut sim, source, ingress);
    sim.execute(Command::SetIpv4 {
        port: source,
        config: Ipv4InterfaceConfig::new("10.0.0.1".parse().unwrap(), 24, None, VlanId(1)),
    })
    .unwrap();
    script(
        &mut sim,
        sw,
        "enable\nconfigure terminal\nmanagement ip 10.0.0.2\nsnmp-server community monitor ro\nsnmp-server community operator rw\nsnmp-server contact Ops team\nend",
    );
    let result = sim.execute_console(server, "snmpget -v 2c -c monitor 10.0.0.2 sysDescr.0");
    assert!(result.success, "{:?}", result.lines);
    assert!(result.lines.join(" ").contains("C1000-24T-4G-L"));
    assert!(
        !sim.execute_console(
            server,
            "snmpset -v 2c -c monitor 10.0.0.2 sysLocation.0 s rack1"
        )
        .success
    );
    assert!(
        sim.execute_console(
            server,
            "snmpset -v 2c -c operator 10.0.0.2 sysLocation.0 s rack1"
        )
        .success
    );
    assert_eq!(
        sim.snmp_get(sw, "monitor", "sysLocation.0").unwrap(),
        SnmpValue::Text("rack1".into())
    );
    assert!(
        !sim.execute_console(server, "snmpget -v 2c -c wrong 10.0.0.2 sysName.0")
            .success
    );
    let all = Ipv4Prefix::new("0.0.0.0".parse().unwrap(), 0).unwrap();
    let policy = |protocol| {
        ProviderCommand::SetPolicy(PolicyAttachment {
            ingress: true,
            policy: PacketPolicy {
                port: ingress,
                vlan: VlanId(1),
                allowed_sources: vec![],
                rules: vec![PacketRule {
                    source: all,
                    destination: all,
                    protocol: Some(protocol),
                    action: PolicyAction::Deny,
                }],
                default_action: PolicyAction::Permit,
            },
        })
    };
    sim.execute(Command::Provider(policy(1))).unwrap();
    assert!(!sim.ping(source, "10.0.0.2".parse().unwrap()).reachable);
    assert!(
        sim.execute_console(server, "snmpget -v 2c -c monitor 10.0.0.2 ifInOctets.1")
            .success
    );
    sim.execute(Command::Provider(policy(17))).unwrap();
    assert!(
        !sim.execute_console(server, "snmpget -v 2c -c monitor 10.0.0.2 sysName.0")
            .success
    );
    assert!(
        sim.snmp_walk(sw, "monitor", "1.3.6.1.2.1.2.2.1.2")
            .unwrap()
            .len()
            == 28
    );
}

#[test]
fn switch_services_restore_with_startup_configuration_and_invalid_ranges_are_atomic() {
    let mut sim = NetworkSim::new();
    let sw = buy(&mut sim, DeviceTemplate::Switch, 1);
    script(
        &mut sim,
        sw,
        "enable\nconfigure terminal\nmls qos\nsnmp-server community monitor ro\ninterface range Gi1/0/1 - 2\nchannel-group 1 mode active\nmls qos trust dscp\nend\nwrite memory",
    );
    script(
        &mut sim,
        sw,
        "configure terminal\nno mls qos\nno snmp-server community monitor\ninterface Gi1/0/1\nno channel-group\nend\nreload\n\n",
    );
    let DeviceKind::Switch(switch) = &sim.device(sw).unwrap().kind else {
        panic!()
    };
    assert!(switch.services.qos.enabled);
    assert_eq!(switch.services.etherchannel.members.len(), 2);
    assert_eq!(switch.services.snmp.communities.len(), 1);
    script(
        &mut sim,
        sw,
        "enable\nconfigure terminal\ninterface range Gi1/0/3 - 4",
    );
    assert!(
        !sim.execute_console(sw, "channel-group 7 mode active")
            .success
    );
    assert_eq!(sim.channel_ports(sw, 1).len(), 2);
    let mut loaded: NetworkSim = ron::from_str(&ron::to_string(&sim).unwrap()).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.channel_ports(sw, 1).len(), 2);
    assert_eq!(
        loaded.snmp_get(sw, "monitor", "ifNumber.0").unwrap(),
        SnmpValue::Integer(28)
    );
}

#[test]
fn snmp_reports_ten_gigabit_speed_and_restarts_uptime_on_power_cycle() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    sim.advance_time(20_000);
    let sw = device(
        &mut sim,
        Command::Optics(OpticsCommand::BuyHardware {
            model: "switch_10g".into(),
        }),
        1,
    );
    script(
        &mut sim,
        sw,
        "enable\nconfigure terminal\nsnmp-server community monitor ro\nend",
    );
    assert_eq!(
        sim.snmp_get(sw, "monitor", "ifHighSpeed.25").unwrap(),
        SnmpValue::Gauge32(10_000)
    );
    assert_eq!(
        sim.snmp_get(sw, "monitor", "sysUpTime.0").unwrap(),
        SnmpValue::TimeTicks(0)
    );
    sim.advance_time(1500);
    assert_eq!(
        sim.snmp_get(sw, "monitor", "sysUpTime.0").unwrap(),
        SnmpValue::TimeTicks(150)
    );
    sim.execute(Command::SetPower {
        device: sw,
        powered: false,
    })
    .unwrap();
    assert!(sim.snmp_get(sw, "monitor", "sysUpTime.0").is_err());
    sim.execute(Command::SetPower {
        device: sw,
        powered: true,
    })
    .unwrap();
    assert_eq!(
        sim.snmp_get(sw, "monitor", "sysUpTime.0").unwrap(),
        SnmpValue::TimeTicks(0)
    );
}

#[test]
fn equivalent_default_access_vlans_bundle_and_pagp_requires_an_active_peer() {
    let mut sim = NetworkSim::new();
    stock(&mut sim);
    let a = buy(&mut sim, DeviceTemplate::Switch, 1);
    let b = buy(&mut sim, DeviceTemplate::Switch, 2);
    let ap = port(&sim, a, 0);
    let bp = port(&sim, b, 0);
    connect(&mut sim, ap, bp);
    script(
        &mut sim,
        b,
        "enable\nconfigure terminal\ninterface Gi1/0/1\nswitchport access vlan 1\nend",
    );
    sim.set_channel_group(ap, Some((1, ChannelMode::Auto)))
        .unwrap();
    sim.set_channel_group(bp, Some((1, ChannelMode::Auto)))
        .unwrap();
    assert_eq!(sim.channel_state(ap), ChannelState::Suspended);
    sim.set_channel_group(bp, Some((1, ChannelMode::Desirable)))
        .unwrap();
    assert_eq!(sim.channel_state(ap), ChannelState::Bundled);
    sim.set_channel_group(ap, Some((1, ChannelMode::On)))
        .unwrap();
    assert_eq!(sim.channel_state(ap), ChannelState::Suspended);
    sim.set_channel_group(bp, Some((1, ChannelMode::On)))
        .unwrap();
    assert_eq!(sim.channel_state(ap), ChannelState::Bundled);
}
