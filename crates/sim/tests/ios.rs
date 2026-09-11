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
        port(&sim, router, 2),
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
        "enable\nconf t\nhostname Edge\ninterface Gi0/1/0.20\nencapsulation dot1Q 20\nip address 10.0.20.1 255.255.255.0\nexit\ninterface GigabitEthernet0/1/0.30\nencapsulation dot1q 30\nip address 10.0.30.1 255.255.255.0\nend\ncopy run start",
    );
    assert!(sim.ping(ap, "10.0.30.2".parse().unwrap()).reachable);
    assert!(sim.ping(ap, "10.0.20.1".parse().unwrap()).reachable);
    assert!(sim.ping(ap, "8.8.8.8".parse().unwrap()).reachable);
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
    assert!(!sim.execute_console(sw, "switchport access vlan 20").success);
    assert!(
        matches!(&sim.port(last_rj45).unwrap().config, PortConfig::Switch(c) if c.mode == SwitchPortMode::Access { vlan: None })
    );
    assert!(
        matches!(&sim.device(sw).unwrap().kind, DeviceKind::Switch(c) if !c.vlans.iter().any(|v| v.id == VlanId(20)))
    );
    run(
        &mut sim,
        router,
        "enable\nconf t\nint gi0/1/0\nip address 10.0.0.1 255.255.255.0",
    );
    assert!(
        !sim.execute_console(router, "ip address 10.1.0.1 255.0.255.0")
            .success
    );
    let lines = run(&mut sim, router, "do show ip interface brief").join("\n");
    assert!(lines.contains("10.0.0.1"));
    assert!(!lines.contains("10.1.0.1"));
    run(&mut sim, router, "exit\nint gi0/1/0.20");
    assert!(
        !sim.execute_console(router, "ip address 10.0.20.1 255.255.255.0")
            .success
    );
    run(
        &mut sim,
        router,
        "encapsulation dot1q 20\nip address 10.0.20.1 255.255.255.0\nexit\nint gi0/1/0.30",
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
