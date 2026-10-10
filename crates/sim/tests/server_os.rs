use cloud_provider_sim::*;
use std::net::Ipv4Addr;

struct GuestLab {
    sim: NetworkSim,
    guests: Vec<DeviceId>,
}

impl GuestLab {
    fn new(count: u8) -> Self {
        let mut sim = NetworkSim::new();
        let mut guests = Vec::new();
        for index in 0..count {
            let SimEvent::DeviceAdded(device) = sim
                .execute(Command::BuyDevice {
                    kind: DeviceTemplate::Server,
                })
                .unwrap()[0]
            else {
                panic!("server missing")
            };
            sim.execute(Command::PlaceDevice {
                device,
                rack: RackId(1),
                unit: index + 1,
            })
            .unwrap();
            sim.execute(Command::ConnectPower {
                outlet: OutletId {
                    source: SourceId::Rack(RackId(1)),
                    index,
                },
                endpoint: PowerEndpoint::Device(device),
            })
            .unwrap();
            sim.execute(Command::SetPower {
                device,
                powered: true,
            })
            .unwrap();
            guests.push(device);
        }
        for supply in [CableSupply::CableBox305m, CableSupply::Rj45Pack20] {
            sim.execute(Command::BuyCableSupply { supply }).unwrap();
        }
        Self { sim, guests }
    }

    fn port(&self, guest: usize, name: &str) -> PortId {
        *self
            .sim
            .device(self.guests[guest])
            .unwrap()
            .ports()
            .iter()
            .find(|id| self.sim.port(**id).unwrap().name == name)
            .unwrap()
    }

    fn run(&mut self, guest: usize, input: &str) -> Vec<String> {
        let output = self.sim.execute_console(self.guests[guest], input);
        assert!(output.success, "{input}: {:?}", output.lines);
        output.lines
    }

    fn connect(&mut self, a: PortId, b: PortId) {
        self.sim.execute(Command::Connect { a, b }).unwrap();
    }
}

#[test]
fn shell_files_quotes_pipes_variables_and_conditions_are_persistent_per_guest() {
    let mut lab = GuestLab::new(2);
    lab.run(0, "mkdir -p /root/config; cd /root/config");
    lab.run(
        0,
        "export LABEL=eth0; printf 'eth1\neth0\neth0\n' > interfaces",
    );
    assert_eq!(
        lab.run(0, "cat interfaces | sort | uniq | grep $LABEL"),
        ["eth0"]
    );
    assert_eq!(
        lab.run(0, "echo '$LABEL' \"$LABEL\" \\$LABEL"),
        ["$LABEL eth0 $LABEL"]
    );
    assert_eq!(
        lab.run(0, "false && echo bad || echo recovered; echo $?"),
        ["recovered", "0"]
    );
    assert!(
        !lab.sim
            .execute_console(lab.guests[0], "does-not-exist")
            .success
    );
    assert_eq!(lab.run(0, "echo $?"), ["127"]);
    lab.run(0, "printf 'no-newline' > exact; echo -n '-suffix' >> exact");
    assert_eq!(
        lab.sim
            .server_os(lab.guests[0])
            .unwrap()
            .filesystem
            .read("/root/config/exact")
            .unwrap(),
        "no-newline-suffix"
    );
    assert_eq!(lab.run(1, "pwd"), ["/root"]);
    assert!(
        !lab.sim
            .execute_console(lab.guests[1], "cat /root/config/interfaces")
            .success
    );
    let saved = ron::to_string(&lab.sim).unwrap();
    lab.sim = ron::from_str(&saved).unwrap();
    lab.sim.rebuild_indexes();
    assert_eq!(lab.run(0, "pwd"), ["/root/config"]);
    assert_eq!(lab.run(0, "cat exact"), ["no-newline-suffix"]);
    assert_eq!(lab.run(0, "printf 'x' | wc -l"), ["0"]);
    assert_eq!(lab.run(0, "printf 'x' | wc -c"), ["1"]);
    assert_eq!(lab.run(0, "echo missing | grep absent | wc -l"), ["0"]);
    assert_eq!(lab.run(0, "sudo echo permissions"), ["permissions"]);
    assert!(
        lab.sim
            .terminal_prompt(lab.guests[0])
            .contains("/root/config#")
    );
}

#[test]
fn malformed_shell_syntax_does_not_execute_a_partial_configuration() {
    let mut lab = GuestLab::new(1);
    for input in [
        "ip addr add 192.0.2.2/24 dev eth0 &&",
        "ip addr add 192.0.2.2/24 dev eth0 |",
        "echo 'unfinished",
        "echo hello >",
    ] {
        assert!(!lab.sim.execute_console(lab.guests[0], input).success);
    }
    assert!(
        matches!(&lab.sim.port(lab.port(0, "eth0")).unwrap().config, PortConfig::Server(config) if config.ipv4.is_none())
    );
}

#[test]
fn secondary_addresses_respond_to_arp_and_ping_and_deleting_primary_promotes_secondary() {
    let mut lab = GuestLab::new(2);
    lab.connect(lab.port(0, "eth0"), lab.port(1, "eth0"));
    lab.run(0, "ip addr add 192.0.2.2/24 dev eth0");
    lab.run(
        1,
        "ip addr add 10.0.0.3/24 dev eth0; ip addr add 192.0.2.3/24 dev eth0",
    );
    lab.run(0, "ping -c 2 192.0.2.3");
    assert!(lab.sim.port_telemetry(lab.port(1, "eth0")).tx_frames > 0);
    lab.run(1, "ip addr del 10.0.0.3/24 dev eth0");
    lab.run(0, "ping 192.0.2.3");
    assert!(lab.run(1, "ip -br addr show dev eth0")[0].contains("192.0.2.3/24"));
    lab.run(1, "ip addr flush dev eth0");
    assert!(
        !lab.sim
            .execute_console(lab.guests[0], "ping 192.0.2.3")
            .success
    );
}

#[test]
fn longest_prefix_and_metric_choose_the_actual_egress_interface() {
    let mut lab = GuestLab::new(2);
    lab.run(
        0,
        "ip addr add 10.1.0.2/24 dev eth0; ip addr add 10.2.0.2/24 dev eth1",
    );
    lab.run(1, "ip addr add 10.2.0.3/24 dev eth0");
    lab.connect(lab.port(0, "eth1"), lab.port(1, "eth0"));
    lab.run(0, "ping 10.2.0.3");
    assert_eq!(lab.sim.port_telemetry(lab.port(0, "eth0")).tx_frames, 0);
    lab.run(0, "ip route add default via 10.1.0.1 dev eth0 metric 200");
    lab.run(0, "ip route add 0.0.0.0/0 via 10.2.0.1 dev eth1 metric 50");
    let internet: Ipv4Addr = "8.8.8.8".parse().unwrap();
    assert_eq!(
        lab.sim
            .server_route_selection(lab.guests[0], internet, None)
            .unwrap()
            .port,
        lab.port(0, "eth1")
    );
    lab.run(
        0,
        "ip route add 8.8.8.0/24 via 10.1.0.1 dev eth0 metric 500",
    );
    assert!(lab.run(0, "ip route get 8.8.8.8")[0].contains("dev eth0 src 10.1.0.2"));
    lab.run(0, "ip route del 8.8.8.0/24");
    lab.run(0, "ip route flush");
    assert!(
        lab.sim
            .server_route_selection(lab.guests[0], internet, None)
            .is_none()
    );
}

#[test]
fn network_configuration_file_applies_atomically_and_survives_reboot() {
    let mut lab = GuestLab::new(1);
    lab.run(0, "printf 'auto lo eth0\niface lo inet loopback\niface eth0 inet static\n address 192.0.2.10\n netmask 255.255.255.0\n gateway 192.0.2.1\n dns-nameservers 192.0.2.53\n post-up ip route add 198.51.100.0/24 via 192.0.2.254 dev eth0\n' > /etc/network/interfaces");
    lab.run(0, "systemctl restart networking");
    assert!(
        lab.run(0, "ip a show dev eth0")
            .iter()
            .any(|line| line.contains("192.0.2.10/24"))
    );
    assert_eq!(
        lab.run(0, "cat /etc/resolv.conf"),
        ["nameserver 192.0.2.53"]
    );
    lab.run(0, "ip addr add 10.9.0.1/24 dev eth1");
    lab.run(0, "reboot");
    assert!(!lab.run(0, "ip -br a show dev eth1")[0].contains("10.9.0.1"));
    assert!(lab.run(0, "ip route get 198.51.100.1")[0].contains("via 192.0.2.254"));
    lab.run(0, "printf 'auto eth0 eth1\niface eth0 inet static\n address 192.0.2.99/24\niface eth1 inet static\n address broken\n' > /etc/network/interfaces");
    assert!(
        !lab.sim
            .execute_console(lab.guests[0], "systemctl restart networking")
            .success
    );
    assert!(lab.run(0, "ip -br a show dev eth0")[0].contains("192.0.2.10/24"));
    assert!(!lab.run(0, "ip -br a show dev eth0")[0].contains("192.0.2.99"));
}

#[test]
fn ssh_service_controls_management_connections_and_commands_run_on_remote_guest() {
    let mut lab = GuestLab::new(2);
    lab.connect(lab.port(0, "eth1"), lab.port(1, "eth1"));
    lab.run(0, "ip addr add 192.0.2.1/24 dev eth1");
    lab.run(
        1,
        "ip addr add 192.0.2.2/24 dev eth1; systemctl disable --now ssh",
    );
    assert!(
        !lab.sim
            .execute_console(lab.guests[0], "ssh root@192.0.2.2")
            .success
    );
    assert!(
        !lab.run(1, "ss -lntp")
            .iter()
            .any(|line| line.contains("sshd"))
    );
    lab.run(1, "systemctl enable --now ssh.service");
    lab.run(0, "ssh root@192.0.2.2");
    lab.run(
        0,
        "echo remote > /root/remote-file; hostnamectl set-hostname worker",
    );
    assert!(
        lab.sim
            .terminal_prompt(lab.guests[0])
            .contains("root@worker")
    );
    lab.run(0, "exit");
    assert!(
        !lab.sim
            .execute_console(lab.guests[0], "cat /root/remote-file")
            .success
    );
    assert_eq!(lab.run(1, "cat /root/remote-file"), ["remote"]);
}

#[test]
fn public_secondary_address_uses_owned_uplink_for_both_directions() {
    let mut lab = GuestLab::new(1);
    let uplink = lab
        .sim
        .network_outlets()
        .find(|outlet| matches!(outlet.kind, NetworkOutletKind::Uplink { .. }))
        .unwrap()
        .port;
    let block = lab.sim.buy_public_ipv4_block(uplink).unwrap();
    lab.connect(lab.port(0, "eth0"), uplink);
    lab.run(0, "ip addr add 10.0.0.2/16 dev eth0");
    let public = block.host_addresses().next().unwrap();
    lab.sim
        .execute(Command::Provider(ProviderCommand::SetTransit(
            TransitCircuit {
                port: uplink,
                name: "On-link handoff".into(),
                address: block.gateway(),
                prefix: 29,
                asn: 64501,
                capacity_mbps: 1000,
                enabled: true,
                routes: vec![UpstreamRoute {
                    prefix: Ipv4Prefix::new(block.network, 29).unwrap(),
                    next_hop: public,
                }],
                offered_routes: vec![],
                authorizations: vec![],
            },
        )))
        .unwrap();

    lab.run(
        0,
        &format!(
            "ip addr add {public}/29 dev eth0; ip route add default via {} dev eth0",
            block.gateway()
        ),
    );
    lab.run(0, "ping 8.8.8.8");
    assert!(lab.sim.ping_from_internet(public).reachable);
    assert_eq!(
        lab.sim.public_assignments(block),
        [(public, lab.port(0, "eth0"))]
    );
    lab.run(0, "ip route del default");
    assert!(!lab.sim.ping_from_internet(public).reachable);
}

#[test]
fn guest_scripts_apply_commands_and_hosts_files_resolve_names() {
    let mut lab = GuestLab::new(1);
    lab.run(0, "printf '#!/bin/sh\nip addr add 10.0.0.5/16 dev eth0\nhostnamectl set-hostname database\n' > /root/setup.sh; chmod 755 /root/setup.sh; sh /root/setup.sh");
    lab.run(0, "echo '10.0.0.5 database.internal' >> /etc/hosts");
    assert_eq!(
        lab.run(0, "getent hosts database.internal"),
        ["10.0.0.5\tdatabase.internal"]
    );
    lab.run(0, "ping database.internal");
    lab.run(0, "ip link set lo down");
    assert!(
        !lab.sim
            .execute_console(lab.guests[0], "ping localhost")
            .success
    );
    assert!(lab.run(0, "ip link show dev lo")[0].contains("state DOWN"));
}

#[test]
fn server_static_route_forwards_packets_through_the_configured_gateway() {
    let mut lab = GuestLab::new(1);
    let SimEvent::DeviceAdded(router) = lab
        .sim
        .execute(Command::BuyDevice {
            kind: DeviceTemplate::Router,
        })
        .unwrap()[0]
    else {
        panic!("router missing")
    };
    lab.sim
        .execute(Command::PlaceDevice {
            device: router,
            rack: RackId(1),
            unit: 2,
        })
        .unwrap();
    lab.sim
        .execute(Command::ConnectPower {
            outlet: OutletId {
                source: SourceId::Rack(RackId(1)),
                index: 1,
            },
            endpoint: PowerEndpoint::Device(router),
        })
        .unwrap();
    lab.sim
        .execute(Command::SetPower {
            device: router,
            powered: true,
        })
        .unwrap();
    let ports = lab.sim.device(router).unwrap().ports().to_vec();
    for index in [8, 9] {
        lab.sim
            .execute(Command::SetRouterSwitchport {
                port: ports[index],
                switchport: false,
            })
            .unwrap();
    }
    for (port, address) in [(ports[8], "192.0.2.1"), (ports[9], "198.51.100.1")] {
        lab.sim
            .execute(Command::ConfigureRouterInterface {
                port,
                name: "LAN".into(),
                vlan: None,
                address: Some(address.parse().unwrap()),
                prefix: 24,
                internet_connected: false,
            })
            .unwrap();
    }
    lab.connect(lab.port(0, "eth0"), ports[8]);
    lab.run(0, "ip addr add 192.0.2.2/24 dev eth0");
    assert!(
        !lab.sim
            .execute_console(lab.guests[0], "ping 198.51.100.1")
            .success
    );
    lab.run(0, "ip route add 198.51.100.0/24 via 192.0.2.1 dev eth0");
    lab.run(0, "ping 198.51.100.1");
    assert!(lab.sim.port_telemetry(ports[8]).tx_frames > 0);
    lab.run(0, "ip route del 198.51.100.0/24");
    assert!(
        !lab.sim
            .execute_console(lab.guests[0], "ping 198.51.100.1")
            .success
    );
}

#[test]
fn replacing_a_legacy_default_does_not_restore_it_when_the_new_route_is_deleted() {
    let mut lab = GuestLab::new(1);
    lab.sim
        .execute(Command::SetIpv4 {
            port: lab.port(0, "eth0"),
            config: Ipv4InterfaceConfig::new(
                "192.0.2.2".parse().unwrap(),
                24,
                Some("192.0.2.1".parse().unwrap()),
                VlanId(1),
            ),
        })
        .unwrap();
    lab.run(0, "ip addr add 198.51.100.2/24 dev eth1");
    lab.run(0, "ip route replace default via 198.51.100.1 dev eth1");
    lab.run(0, "ip route del default");
    assert!(
        !lab.sim
            .execute_console(lab.guests[0], "ip route get 8.8.8.8")
            .success
    );
}

#[test]
fn an_unpatched_room_socket_does_not_supply_a_dhcp_lease() {
    let mut lab = GuestLab::new(1);
    lab.run(
        0,
        "printf 'auto eth1\niface eth1 inet dhcp\n' > /etc/network/interfaces",
    );
    assert!(!lab.sim.execute_console(lab.guests[0], "ifup eth1").success);
    let lan = lab
        .sim
        .network_outlets()
        .find(|outlet| outlet.kind == NetworkOutletKind::Lan { rack: RackId(1) })
        .unwrap()
        .port;
    lab.connect(lab.port(0, "eth1"), lan);
    assert!(!lab.sim.execute_console(lab.guests[0], "ifup eth1").success);
    assert!(!lab.run(0, "ip -br addr show dev eth1")[0].contains("10.0.0.10/16"));
}
