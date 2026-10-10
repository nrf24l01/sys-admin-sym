use cloud_provider_sim::*;

fn buy(sim: &mut NetworkSim, kind: DeviceTemplate, unit: u8) -> DeviceId {
    let id = match sim.execute(Command::BuyDevice { kind }).unwrap()[0] {
        SimEvent::DeviceAdded(id) => id,
        _ => unreachable!(),
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
        .find(|outlet| !sim.power.connections.contains_key(outlet))
        .expect("rack has a free C13 outlet");
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
fn run(sim: &mut NetworkSim, device: DeviceId, script: &str) -> Vec<String> {
    let mut lines = vec![];
    for input in script.lines() {
        let output = sim.execute_console(device, input);
        assert!(output.success, "{input}: {:?}", output.lines);
        lines.extend(output.lines);
    }
    lines
}
fn port(sim: &NetworkSim, device: DeviceId, index: usize) -> PortId {
    sim.device(device).unwrap().ports()[index]
}

#[test]
fn running_interface_config_supports_switch_svi_subinterface_and_rejects_missing_stanzas() {
    let mut sim = NetworkSim::new();
    let router = buy(&mut sim, DeviceTemplate::Router, 1);
    let switch = buy(&mut sim, DeviceTemplate::Switch, 2);
    let denied = sim.execute_console(router, "show running-config interface GigabitEthernet0/0/0");
    assert!(!denied.success && denied.lines[0].contains("enable"));
    run(
        &mut sim,
        router,
        "enable\nconf t\ninterface Gi0/0/0\ndescription WAN0-UPSTREAM\nip address 192.0.2.2 255.255.255.252\nexit\nvlan 8\nexit\ninterface Vlan8\nip address 10.0.8.1 255.255.255.0\nno shutdown\ninterface Gi0/0/0.20\nencapsulation dot1q 20\nip address 10.0.20.1 255.255.255.0\nend",
    );
    assert!(
        sim.console_help(router, "show run int")
            .iter()
            .any(|pattern| pattern == "show running-config interface <interface>")
    );
    let wan = run(
        &mut sim,
        router,
        "show running-config interface GigabitEthernet0/0/0",
    );
    assert_eq!(wan[0], "interface GigabitEthernet0/0/0");
    assert!(wan.contains(&" description WAN0-UPSTREAM".into()));
    assert!(wan.contains(&" ip address 192.0.2.2 255.255.255.252".into()));
    assert_eq!(
        wan.iter()
            .filter(|line| line.starts_with("interface "))
            .count(),
        1
    );
    assert!(!wan.iter().any(|line| line.contains("10.0.20.1")));
    let svi = run(&mut sim, router, "show running-config interface Vlan8");
    assert_eq!(svi[0], "interface Vlan8");
    assert!(svi.contains(&" ip address 10.0.8.1 255.255.255.0".into()));
    let sub = run(&mut sim, router, "show run int Gi0/0/0.20");
    assert_eq!(sub[0], "interface GigabitEthernet0/0/0.20");
    assert!(sub.contains(&" encapsulation dot1q 20".into()));
    assert!(
        !sim.execute_console(router, "show run int Gi0/0/0.99")
            .success
    );
    assert!(!sim.execute_console(router, "show run int Gi9/0/0").success);
    run(
        &mut sim,
        switch,
        "enable\nconf t\ninterface Gi1/0/1\ndescription Server\nspeed 100\nchannel-group 1 mode active\nend",
    );
    let config = run(&mut sim, switch, "show run int Gi1/0/1");
    assert!(config.contains(&" description Server".into()));
    assert!(config.contains(&" speed 100".into()));
    assert!(config.contains(&" channel-group 1 mode active".into()));
    assert_eq!(
        run(&mut sim, switch, "show run int Po1")[0],
        "interface Port-channel1"
    );
}

#[test]
fn router_lan_vlans_switch_and_route_through_svis_and_obey_ip_routing() {
    let mut sim = NetworkSim::new();
    for supply in [CableSupply::CableBox305m, CableSupply::Rj45Pack20] {
        sim.execute(Command::BuyCableSupply { supply }).unwrap();
    }
    let router = buy(&mut sim, DeviceTemplate::Router, 1);
    let a = buy(&mut sim, DeviceTemplate::Server, 2);
    let b = buy(&mut sim, DeviceTemplate::Server, 3);
    let c = buy(&mut sim, DeviceTemplate::Server, 4);
    let [ap, bp, cp] = [a, b, c].map(|d| port(&sim, d, 0));
    address(&mut sim, ap, "10.0.8.2", "10.0.8.1", 8);
    address(&mut sim, bp, "10.0.8.3", "10.0.8.1", 8);
    address(&mut sim, cp, "10.0.9.2", "10.0.9.1", 9);
    for (index, p) in [ap, bp, cp].into_iter().enumerate() {
        let rp = port(&sim, router, index + 2);
        link(&mut sim, p, rp);
    }
    run(
        &mut sim,
        router,
        "enable\nconf t\nvlan 8\nname SERVERS\nexit\nvlan 9\nexit\ninterface Gi0/1/0\nswitchport mode access\nswitchport access vlan 8\nexit\ninterface Gi0/1/1\nswitchport access vlan 8\nexit\ninterface Gi0/1/2\nswitchport access vlan 9\nexit\ninterface vlan 8\nip address 10.0.8.1 255.255.255.0\nno shutdown\nexit\ninterface Vlan9\nip address 10.0.9.1 255.255.255.0\nno shutdown\nexit\nip routing\nend\nwrite memory",
    );
    assert!(sim.ping(ap, "10.0.8.3".parse().unwrap()).reachable);
    assert!(sim.ping(ap, "10.0.8.1".parse().unwrap()).reachable);
    assert!(sim.ping(ap, "10.0.9.2".parse().unwrap()).reachable);
    let DeviceKind::Router(r) = &sim.device(router).unwrap().kind else {
        panic!()
    };
    assert_eq!((r.ports.len(), r.svi_ports.len()), (10, 2));
    assert!(
        sim.execute_console(router, "show ip interface brief")
            .lines
            .iter()
            .any(|l| l.contains("Vlan8") && l.contains("10.0.8.1") && l.contains("up"))
    );
    assert!(
        sim.execute_console(router, "show vlan brief")
            .lines
            .iter()
            .any(|l| l.contains("SERVERS") && l.contains("GigabitEthernet0/1/0"))
    );
    run(&mut sim, router, "conf t\nno ip routing\nend");
    assert!(!sim.ping(ap, "10.0.9.2".parse().unwrap()).reachable);
    assert!(sim.ping(ap, "10.0.8.3".parse().unwrap()).reachable);
    assert!(sim.ping(ap, "10.0.8.1".parse().unwrap()).reachable);
    run(&mut sim, router, "reload\n");
    assert!(sim.execute_console(router, "").success);
    assert!(sim.ping(ap, "10.0.9.2".parse().unwrap()).reachable);
    run(
        &mut sim,
        router,
        "enable\nconf t\ninterface vlan 8\nshutdown\nend",
    );
    assert!(!sim.ping(ap, "10.0.8.1".parse().unwrap()).reachable);
    assert!(sim.ping(ap, "10.0.8.3".parse().unwrap()).reachable);
    let mut loaded: NetworkSim = ron::from_str(&ron::to_string(&sim).unwrap()).unwrap();
    loaded.rebuild_indexes();
    assert!(!loaded.ping(ap, "10.0.9.2".parse().unwrap()).reachable);
    run(
        &mut loaded,
        router,
        "enable\nconf t\ninterface vlan 8\nno shutdown\nend",
    );
    assert!(loaded.ping(ap, "10.0.9.2".parse().unwrap()).reachable);
}

#[test]
fn no_switchport_honors_router_flex_ports_and_conversion_clears_layer_three_state() {
    let mut sim = NetworkSim::new();
    let router = buy(&mut sim, DeviceTemplate::Router, 1);
    run(&mut sim, router, "enable\nconf t");
    assert!(
        sim.console_help(router, "ip rout")
            .iter()
            .any(|s| s == "ip routing")
    );
    assert!(
        sim.console_help(router, "vlan")
            .iter()
            .any(|s| s == "vlan <id>")
    );
    run(&mut sim, router, "interface Gi0/1/0");
    let error = sim.execute_console(router, "no switchport");
    assert!(!error.success);
    assert!(
        error.lines[0].contains("fixed Layer 2") && error.lines[0].contains("GigabitEthernet0/1/6")
    );
    run(
        &mut sim,
        router,
        "exit\ninterface Gi0/1/6\nno switchport\nip address 192.0.2.1 255.255.255.0\nend",
    );
    assert!(matches!(
        sim.port(port(&sim, router, 8)).unwrap().config,
        PortConfig::Router(_)
    ));
    let config = sim
        .execute_console(router, "show running-config")
        .lines
        .join("\n");
    assert!(config.contains(" no switchport") && config.contains(" ip address 192.0.2.1"));
    run(
        &mut sim,
        router,
        "conf t\ninterface Gi0/1/6\nswitchport\nswitchport access vlan 8\nend",
    );
    let DeviceKind::Router(r) = &sim.device(router).unwrap().kind else {
        panic!()
    };
    assert!(!r.interfaces.iter().any(|i| i.port == r.ports[8]));
    assert!(
        sim.execute_console(router, "show interfaces Gi0/1/6 switchport")
            .lines
            .join("\n")
            .contains("access VLAN 8")
    );
}

#[test]
fn legacy_routed_lan_save_keeps_its_address_and_accepts_idempotent_no_switchport() {
    let mut sim = NetworkSim::new();
    let router = buy(&mut sim, DeviceTemplate::Router, 1);
    let lan = port(&sim, router, 2);
    // Build a legacy fixture in RON, the actual save format, without exposing
    // mutable world internals or serializing structured power keys as JSON.
    let mut legacy_device = sim.device(router).unwrap().clone();
    let interface = RouterInterface::lan("LAN1", lan, VlanId(1), "10.0.8.1".parse().unwrap(), 24);
    let DeviceKind::Router(r) = &mut legacy_device.kind else {
        panic!()
    };
    r.interfaces.push(interface.clone());
    let omitted_fields = [
        format!(",vlans:{}", ron::to_string(&r.vlans).unwrap()),
        ",svi_ports:[]".into(),
        ",routing_enabled:true".into(),
    ];
    let mut legacy_device = ron::to_string(&legacy_device).unwrap();
    for field in omitted_fields {
        assert!(legacy_device.contains(&field));
        legacy_device = legacy_device.replacen(&field, "", 1);
    }
    let mut legacy_port = sim.port(lan).unwrap().clone();
    legacy_port.config = PortConfig::Router(RouterPortConfig {
        interfaces: vec![interface],
    });
    let saved = ron::to_string(&sim).unwrap();
    let original_device = ron::to_string(sim.device(router).unwrap()).unwrap();
    let original_port = ron::to_string(sim.port(lan).unwrap()).unwrap();
    assert!(saved.contains(&original_device) && saved.contains(&original_port));
    let saved = saved
        .replacen(&original_device, &legacy_device, 1)
        .replacen(&original_port, &ron::to_string(&legacy_port).unwrap(), 1);
    let mut loaded: NetworkSim = ron::from_str(&saved).unwrap();
    loaded.rebuild_indexes();
    run(
        &mut loaded,
        router,
        "enable\nconf t\ninterface Gi0/1/0\nno switchport\nend",
    );
    assert_eq!(
        loaded.interface_ipv4(lan, VlanId(1)),
        Some("10.0.8.1".parse().unwrap())
    );
}
fn address(sim: &mut NetworkSim, port: PortId, ip: &str, gateway: &str, vlan: u16) {
    sim.execute(Command::SetIpv4 {
        port,
        config: Ipv4InterfaceConfig::new(
            ip.parse().unwrap(),
            24,
            Some(gateway.parse().unwrap()),
            VlanId(vlan),
        ),
    })
    .unwrap();
}
fn link(sim: &mut NetworkSim, a: PortId, b: PortId) {
    sim.execute(Command::Connect { a, b }).unwrap();
}

#[test]
fn interface_speed_is_shown_and_restored_from_startup_config() {
    let mut sim = NetworkSim::new();
    let switch = buy(&mut sim, DeviceTemplate::Switch, 1);

    run(
        &mut sim,
        switch,
        "enable\nconfigure terminal\ninterface Gi1/0/1\nspeed 100\nend",
    );
    let running = run(&mut sim, switch, "show running-config");
    assert!(running.iter().any(|line| line == " speed 100"));

    run(
        &mut sim,
        switch,
        "write memory\nconfigure terminal\ninterface Gi1/0/1\nspeed 10\nend",
    );
    assert_eq!(sim.port_link_speed(port(&sim, switch, 0)), None);
    run(&mut sim, switch, "reload");
    let output = sim.execute_console(switch, "");
    assert!(output.success, "reload confirmation: {:?}", output.lines);
    assert_eq!(
        sim.port(port(&sim, switch, 0)).unwrap().advertised_speed,
        LinkSpeed::Mbps100
    );
}

#[test]
fn modes_abbreviations_validation_and_sessions_are_independent() {
    let mut sim = NetworkSim::new();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::CableBox305m,
    })
    .unwrap();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::Rj45Pack20,
    })
    .unwrap();
    let a = buy(&mut sim, DeviceTemplate::Switch, 1);
    let b = buy(&mut sim, DeviceTemplate::Router, 2);
    assert_eq!(sim.terminal_prompt(a), "Switch>");
    assert!(!sim.execute_console(a, "configure terminal").success);
    assert!(!sim.execute_console(a, "show running-config").success);
    run(
        &mut sim,
        a,
        "EN\nconf t\nhostname Core\nvlan 20\nname Servers",
    );
    assert_eq!(sim.terminal_prompt(a), "Core(config-vlan)#");
    assert_eq!(sim.terminal_prompt(b), "Router>");
    run(
        &mut sim,
        a,
        "exit\nint gi1/0/1\ndescription Server uplink\nno shut",
    );
    assert_eq!(sim.terminal_prompt(a), "Core(config-if)#");
    assert!(
        !sim.execute_console(a, "switchport access vlan 4095")
            .success
    );
    run(&mut sim, a, "end");
    assert!(sim.execute_console(a, "sh i").lines[0].contains("Ambiguous"));
    assert!(sim.execute_console(a, "configure").lines[0].contains("Incomplete"));
    assert!(!sim.execute_console(a, "show version garbage").success);
    assert!(
        run(&mut sim, a, "show running-config")
            .join("\n")
            .contains("description Server uplink")
    );
    assert!(!sim.execute_console(a, "router ospf 1").success);
    assert!(
        sim.console_help(a, "sh ip")
            .iter()
            .any(|s| s == "show ip interface brief")
    );
    sim.execute(Command::SetPower {
        device: b,
        powered: false,
    })
    .unwrap();
    assert!(!sim.execute_console(b, "enable").success);
}

#[test]
fn switch_cli_changes_real_traffic_and_shutdown_reload_restore_it() {
    let mut sim = NetworkSim::new();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::CableBox305m,
    })
    .unwrap();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::Rj45Pack20,
    })
    .unwrap();
    let switch = buy(&mut sim, DeviceTemplate::Switch, 1);
    let a = buy(&mut sim, DeviceTemplate::Server, 2);
    let b = buy(&mut sim, DeviceTemplate::Server, 3);
    let (ap, bp, s1, s2) = (
        port(&sim, a, 0),
        port(&sim, b, 0),
        port(&sim, switch, 0),
        port(&sim, switch, 1),
    );
    address(&mut sim, ap, "10.0.20.2", "10.0.20.1", 20);
    address(&mut sim, bp, "10.0.20.3", "10.0.20.1", 20);
    link(&mut sim, ap, s1);
    link(&mut sim, bp, s2);
    let target = "10.0.20.3".parse().unwrap();
    assert!(!sim.ping(ap, target).reachable);
    run(
        &mut sim,
        switch,
        "enable\nconf t\nvlan 20\nname Servers\nexit\ninterface range gigabitethernet 1/0/1 - 2\nswitchport access vlan 20\nswitchport mode access\nend\nwrite memory",
    );
    assert!(sim.ping(ap, target).reachable);
    run(&mut sim, switch, "conf t\nint gi1/0/1\nshutdown\nend");
    assert!(!sim.ping(ap, target).reachable);
    assert!(
        run(&mut sim, switch, "show interfaces status")
            .join("\n")
            .contains("administratively down")
    );
    run(&mut sim, switch, "reload\ncancel");
    assert!(!sim.ping(ap, target).reachable);
    run(&mut sim, switch, "reload");
    assert!(sim.execute_console(switch, "").success);
    assert_eq!(sim.terminal_prompt(switch), "Switch>");
    assert!(sim.ping(ap, target).reachable);
    run(&mut sim, switch, "enable\nconf t\nno vlan 20\nend");
    assert!(!sim.ping(ap, target).reachable);
}

#[test]
fn router_subinterfaces_trunks_and_gateway_ping_work_from_cli() {
    let mut sim = NetworkSim::new();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::CableBox305m,
    })
    .unwrap();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::Rj45Pack20,
    })
    .unwrap();
    let switch = buy(&mut sim, DeviceTemplate::Switch, 1);
    let router = buy(&mut sim, DeviceTemplate::Router, 2);
    let a = buy(&mut sim, DeviceTemplate::Server, 3);
    let b = buy(&mut sim, DeviceTemplate::Server, 4);
    let (ap, bp, s1, s2, s3, rp) = (
        port(&sim, a, 0),
        port(&sim, b, 0),
        port(&sim, switch, 0),
        port(&sim, switch, 1),
        port(&sim, switch, 2),
        port(&sim, router, 8),
    );
    address(&mut sim, ap, "10.0.20.2", "10.0.20.1", 20);
    address(&mut sim, bp, "10.0.30.2", "10.0.30.1", 30);
    link(&mut sim, ap, s1);
    link(&mut sim, bp, s2);
    link(&mut sim, rp, s3);
    run(
        &mut sim,
        switch,
        "enable\nconf t\nint gi1/0/1\nswitchport access vlan 20\nexit\nint gi1/0/2\nswitchport access vlan 30\nexit\nint gi1/0/3\nswitchport mode trunk\nswitchport trunk allowed vlan 20,30\nend",
    );
    run(
        &mut sim,
        router,
        "enable\nconf t\nhostname Edge\ninterface Gi0/1/6\nno switchport\nexit\ninterface Gi0/1/6.20\nencapsulation dot1Q 20\nip address 10.0.20.1 255.255.255.0\nexit\ninterface GigabitEthernet0/1/6.30\nencapsulation dot1q 30\nip address 10.0.30.1 255.255.255.0\nend\ncopy run start",
    );
    assert!(sim.ping(ap, "10.0.30.2".parse().unwrap()).reachable);
    assert!(sim.ping(ap, "10.0.20.1".parse().unwrap()).reachable);
    let uplink = sim
        .network_outlets()
        .find(|outlet| matches!(outlet.kind, NetworkOutletKind::Uplink { .. }))
        .unwrap()
        .port;
    let wan = port(&sim, router, 0);
    link(&mut sim, wan, uplink);
    assert!(!sim.ping(ap, "8.8.8.8".parse().unwrap()).reachable);
    run(&mut sim, router, "ping 10.0.20.2\ntraceroute 10.0.30.2");
    run(
        &mut sim,
        switch,
        "conf t\nint gi1/0/3\nswitchport trunk allowed vlan remove 30\nend",
    );
    assert!(!sim.ping(ap, "10.0.30.2".parse().unwrap()).reachable);
    run(
        &mut sim,
        switch,
        "conf t\nint gi1/0/3\nswitchport trunk allowed vlan add 30\nend",
    );
    assert!(sim.ping(ap, "10.0.30.2".parse().unwrap()).reachable);
    run(&mut sim, router, "conf t\nint gi0/0/0\nshutdown\nend");
    assert!(!sim.ping(ap, "8.8.8.8".parse().unwrap()).reachable);
    assert!(
        run(&mut sim, router, "show ip route")
            .join("\n")
            .contains("10.0.30.0/24")
    );
}

#[test]
fn invalid_interface_range_and_mask_do_not_partially_mutate_state() {
    let mut sim = NetworkSim::new();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::CableBox305m,
    })
    .unwrap();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::Rj45Pack20,
    })
    .unwrap();
    let sw = buy(&mut sim, DeviceTemplate::Switch, 1);
    let router = buy(&mut sim, DeviceTemplate::Router, 2);
    let last_rj45 = port(&sim, sw, 23);
    run(&mut sim, sw, "enable\nconf t\nint range gi1/0/24 - 25");
    assert!(
        !sim.execute_console(sw, "switchport access vlan 4095")
            .success
    );
    assert!(
        matches!(&sim.port(last_rj45).unwrap().config, PortConfig::Switch(c) if c.mode == SwitchPortMode::Access { vlan: None })
    );
    assert!(
        matches!(&sim.device(sw).unwrap().kind, DeviceKind::Switch(c) if !c.vlans.iter().any(|v| v.id == VlanId(4095)))
    );
    run(
        &mut sim,
        router,
        "enable\nconf t\nint gi0/1/6\nno switchport\nip address 10.0.0.1 255.255.255.0",
    );
    assert!(
        !sim.execute_console(router, "ip address 10.1.0.1 255.0.255.0")
            .success
    );
    let lines = run(&mut sim, router, "do show ip interface brief").join("\n");
    assert!(lines.contains("10.0.0.1"));
    assert!(!lines.contains("10.1.0.1"));
    run(&mut sim, router, "exit\nint gi0/1/6.20");
    assert!(
        !sim.execute_console(router, "ip address 10.0.20.1 255.255.255.0")
            .success
    );
    run(
        &mut sim,
        router,
        "encapsulation dot1q 20\nip address 10.0.20.1 255.255.255.0\nexit\nint gi0/1/6.30",
    );
    assert!(
        !sim.execute_console(router, "encapsulation dot1q 20")
            .success
    );
}

#[test]
fn trunk_allow_list_does_not_create_vlans_and_removed_vlans_do_not_forward() {
    let mut sim = NetworkSim::new();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::CableBox305m,
    })
    .unwrap();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::Rj45Pack20,
    })
    .unwrap();
    let sw = buy(&mut sim, DeviceTemplate::Switch, 1);
    run(
        &mut sim,
        sw,
        "enable\nconf t\nint gi1/0/1\nswitchport mode trunk\nswitchport trunk allowed vlan 10-20,30\nswitchport trunk allowed vlan remove 12-19\nend",
    );
    let output = run(&mut sim, sw, "show interfaces trunk").join("\n");
    assert!(output.contains("10-11,20,30"));
    assert!(matches!(&sim.device(sw).unwrap().kind, DeviceKind::Switch(c) if c.vlans.len() == 1));
    run(
        &mut sim,
        sw,
        "conf t\nint gi1/0/1\nswitchport trunk allowed vlan none\nend",
    );
    assert!(
        run(&mut sim, sw, "show interfaces trunk")
            .join("\n")
            .contains("allowed none")
    );
}

#[test]
fn static_route_validation_and_explicit_interface_selection_are_atomic() {
    let mut sim = NetworkSim::new();
    let router = buy(&mut sim, DeviceTemplate::Router, 1);
    run(
        &mut sim,
        router,
        "enable\nconfigure terminal\ninterface Gi0/0/0\nip address 192.0.2.2 255.255.255.252\ninterface Gi0/0/1\nip address 192.0.2.2 255.255.255.252\nexit",
    );
    let before = sim.device(router).unwrap().kind.clone();
    for command in [
        "ip route 0.0.0.0 255.0.255.0 192.0.2.1",
        "ip route 10.0.0.1 255.255.255.0 192.0.2.1",
        "ip route 0.0.0.0 0.0.0.0 192.0.2.3",
        "ip route 0.0.0.0 0.0.0.0 10.20.0.1",
        "ip route 0.0.0.0 0.0.0.0 192.0.2.1",
    ] {
        assert!(!sim.execute_console(router, command).success, "{command}");
        assert_eq!(sim.device(router).unwrap().kind, before);
        assert_eq!(sim.terminal_prompt(router), "Router(config)#");
    }
    run(
        &mut sim,
        router,
        "ip route 0.0.0.0 0.0.0.0 Gi0/0/1 192.0.2.1\nip route 0.0.0.0 0.0.0.0 Gi0/0/1 192.0.2.1",
    );
    let DeviceKind::Router(config) = &sim.device(router).unwrap().kind else {
        panic!()
    };
    assert_eq!(config.domain_routes.len(), 1);
    assert_eq!(config.domain_routes[0].port, port(&sim, router, 1));
    // A saved next-hop route can still be removed after its interface changes.
    run(
        &mut sim,
        router,
        "interface Gi0/0/1\nip address 198.51.100.2 255.255.255.252\nexit\nno ip route 0.0.0.0 0.0.0.0 Gi0/0/1 192.0.2.1",
    );
    let DeviceKind::Router(config) = &sim.device(router).unwrap().kind else {
        panic!()
    };
    assert!(config.domain_routes.is_empty());
    let switch = buy(&mut sim, DeviceTemplate::Switch, 2);
    run(&mut sim, switch, "enable\nconfigure terminal");
    assert!(
        !sim.execute_console(switch, "ip route 0.0.0.0 0.0.0.0 192.0.2.1")
            .success
    );
}
