use crate::{Command, Ipv4InterfaceConfig, PortId};
use crate::{DeviceId, DeviceKind, NetworkSim, PortConfig, ReachabilityFailure};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalCommand {
    Ip,
    IpAddrShow(Option<String>),
    IpAddrAdd {
        address: Ipv4Addr,
        prefix: u8,
        interface: String,
    },
    IpAddrDel {
        address: Ipv4Addr,
        prefix: u8,
        interface: String,
    },
    IpAddrFlush {
        interface: String,
    },
    IpLinkShow(Option<String>),
    IpLinkSet {
        interface: String,
        enabled: bool,
    },
    Route,
    IpRouteShow,
    IpRouteDefault {
        interface: Option<String>,
        gateway: Option<Ipv4Addr>,
        delete: bool,
        replace: bool,
    },
    Arp,
    Ping(Ipv4Addr),
    Traceroute(Ipv4Addr),
    Ethtool {
        interface: String,
        driver: bool,
    },
    NetstatInterfaces,
    Lsblk,
    Smartctl {
        device: String,
        all: bool,
    },
    Free,
    Lscpu,
    Uname,
    Hostname,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalOutput {
    pub lines: Vec<String>,
    pub success: bool,
}

pub fn parse_terminal_command(input: &str) -> Result<TerminalCommand, String> {
    let words: Vec<_> = input.split_whitespace().collect();
    match words.as_slice() {
        ["ip"] => Ok(TerminalCommand::Ip),
        ["route"] => Ok(TerminalCommand::Route),
        ["arp"] => Ok(TerminalCommand::Arp),
        ["help"] => Ok(TerminalCommand::Help),
        ["ethtool", iface] => Ok(TerminalCommand::Ethtool {
            interface: (*iface).into(),
            driver: false,
        }),
        ["ethtool", "-i", iface] => Ok(TerminalCommand::Ethtool {
            interface: (*iface).into(),
            driver: true,
        }),
        ["net"] | ["netstat", "-i"] | ["ip", "-s", "link"] => {
            Ok(TerminalCommand::NetstatInterfaces)
        }
        ["lsblk"] | ["lsblk", "-d"] => Ok(TerminalCommand::Lsblk),
        ["smartctl", "-a", drive] => Ok(TerminalCommand::Smartctl {
            device: (*drive).into(),
            all: true,
        }),
        ["smartctl", "-i", drive] => Ok(TerminalCommand::Smartctl {
            device: (*drive).into(),
            all: false,
        }),
        ["free"] | ["free", "-h"] => Ok(TerminalCommand::Free),
        ["lscpu"] => Ok(TerminalCommand::Lscpu),
        ["uname"] | ["uname", "-a"] => Ok(TerminalCommand::Uname),
        ["hostname"] => Ok(TerminalCommand::Hostname),
        ["ping", ip] => ip
            .parse()
            .map(TerminalCommand::Ping)
            .map_err(|_| "invalid IPv4 address".into()),
        ["traceroute", ip] => ip
            .parse()
            .map(TerminalCommand::Traceroute)
            .map_err(|_| "invalid IPv4 address".into()),
        ["ip", "addr"] | ["ip", "address"] | ["ip", "a"] => Ok(TerminalCommand::IpAddrShow(None)),
        ["ip", "addr", "show"] | ["ip", "address", "show"] => Ok(TerminalCommand::IpAddrShow(None)),
        ["ip", "addr", "show", "dev", iface] | ["ip", "address", "show", "dev", iface] => {
            Ok(TerminalCommand::IpAddrShow(Some((*iface).into())))
        }
        ["ip", "addr", action, cidr, "dev", iface]
        | ["ip", "address", action, cidr, "dev", iface]
            if *action == "add" || *action == "del" =>
        {
            let (address, prefix) = parse_cidr(cidr)?;
            Ok(if *action == "add" {
                TerminalCommand::IpAddrAdd {
                    address,
                    prefix,
                    interface: (*iface).into(),
                }
            } else {
                TerminalCommand::IpAddrDel {
                    address,
                    prefix,
                    interface: (*iface).into(),
                }
            })
        }
        ["ip", "addr", "flush", "dev", iface] | ["ip", "address", "flush", "dev", iface] => {
            Ok(TerminalCommand::IpAddrFlush {
                interface: (*iface).into(),
            })
        }
        ["ip", "link"] | ["ip", "link", "show"] => Ok(TerminalCommand::IpLinkShow(None)),
        ["ip", "link", "show", "dev", iface] => {
            Ok(TerminalCommand::IpLinkShow(Some((*iface).into())))
        }
        ["ip", "link", "set", "dev", iface, state] if *state == "up" || *state == "down" => {
            Ok(TerminalCommand::IpLinkSet {
                interface: (*iface).into(),
                enabled: *state == "up",
            })
        }
        ["ip", "route"] | ["ip", "route", "show"] | ["ip", "r"] => Ok(TerminalCommand::IpRouteShow),
        ["ip", "route", action, "default", rest @ ..]
            if *action == "add" || *action == "replace" || *action == "del" =>
        {
            parse_default_route(action, rest)
        }
        _ => Err("unknown command; type help".into()),
    }
}

fn parse_cidr(value: &str) -> Result<(Ipv4Addr, u8), String> {
    let (address, prefix) = value
        .split_once('/')
        .ok_or("address must use CIDR notation, e.g. 192.0.2.2/24")?;
    let address = address
        .parse()
        .map_err(|_| "invalid IPv4 address".to_owned())?;
    let prefix = prefix
        .parse()
        .map_err(|_| "invalid IPv4 prefix".to_owned())?;
    if prefix > 32 {
        return Err("invalid IPv4 prefix".into());
    }
    Ok((address, prefix))
}

fn parse_default_route(action: &str, words: &[&str]) -> Result<TerminalCommand, String> {
    if action == "del" && words.is_empty() {
        return Ok(TerminalCommand::IpRouteDefault {
            interface: None,
            gateway: None,
            delete: true,
            replace: false,
        });
    }
    let mut interface = None;
    let mut gateway = None;
    let mut i = 0;
    while i < words.len() {
        match words[i] {
            "via" if i + 1 < words.len() => {
                gateway = Some(
                    words[i + 1]
                        .parse()
                        .map_err(|_| "invalid IPv4 gateway".to_owned())?,
                );
                i += 2;
            }
            "dev" if i + 1 < words.len() => {
                interface = Some(words[i + 1].to_owned());
                i += 2;
            }
            _ => return Err("usage: ip route replace default via GATEWAY dev IFACE".into()),
        }
    }
    if action != "del" && (interface.is_none() || gateway.is_none()) {
        return Err("usage: ip route replace default via GATEWAY dev IFACE".into());
    }
    Ok(TerminalCommand::IpRouteDefault {
        interface,
        gateway,
        delete: action == "del",
        replace: action == "replace",
    })
}

impl NetworkSim {
    pub fn execute_terminal(
        &mut self,
        device: DeviceId,
        command: TerminalCommand,
    ) -> TerminalOutput {
        let Some(dev) = self.device(device) else {
            return output(false, "device not found");
        };
        let DeviceKind::Server(server) = &dev.kind else {
            return output(
                false,
                "terminal is only available on servers; Cisco IOS console is available on routers and switches",
            );
        };
        let server = server.clone();
        if !matches!(
            command,
            TerminalCommand::Ip
                | TerminalCommand::Route
                | TerminalCommand::Arp
                | TerminalCommand::Ping(_)
                | TerminalCommand::Traceroute(_)
                | TerminalCommand::Help
        ) {
            return self.execute_linux_command(device, &server, command);
        }
        let source = server.ports.iter().copied().find(|id| {
            self.port_up(*id)
                && self
                    .port(*id)
                    .is_some_and(|p| matches!(&p.config, PortConfig::Server(c) if c.ipv4.is_some()))
        });
        match command {
            TerminalCommand::Help => output(
                true,
                "commands: ip addr|link|route, ethtool [-i] <iface>, netstat -i, lsblk, smartctl -a /dev/sdX, free -h, lscpu, uname -a, hostname, arp, ping <ip>, traceroute <ip>, ssh <management-ip>, exit",
            ),
            TerminalCommand::Ip => {
                let lines = server
                    .ports
                    .iter()
                    .map(|id| {
                        let p = &self.ports[id];
                        let addr = match &p.config {
                            PortConfig::Server(c) => c
                                .ipv4
                                .as_ref()
                                .map(|v| {
                                    format!(
                                        "{}/{}  vlan {}",
                                        v.address,
                                        v.prefix,
                                        v.vlan
                                            .map_or_else(|| "untagged".into(), |v| v.0.to_string())
                                    )
                                })
                                .unwrap_or_else(|| "unconfigured".into()),
                            _ => "n/a".into(),
                        };
                        let state = if !dev.powered || dev.rack.is_none() || !p.enabled {
                            "DOWN"
                        } else if !self.port_link_up(*id) {
                            "NO-CARRIER"
                        } else {
                            "UP"
                        };
                        format!("{}  {}  {}", p.name, addr, state)
                    })
                    .collect();
                TerminalOutput {
                    lines,
                    success: true,
                }
            }
            TerminalCommand::Route => {
                let text = source
                    .and_then(|id| self.port(id))
                    .and_then(|p| match &p.config {
                        PortConfig::Server(v) => v.ipv4.as_ref(),
                        _ => None,
                    })
                    .and_then(|v| v.gateway)
                    .map(|v| format!("default via {v}"))
                    .unwrap_or_else(|| "no default route".into());
                output(true, text)
            }
            TerminalCommand::Arp => {
                let mut lines: Vec<_> = source
                    .into_iter()
                    .filter(|port| self.port_up(*port))
                    .flat_map(|port| self.arp_entries(port).into_iter())
                    .map(|(ip, mac)| format!("{ip}  {mac}"))
                    .collect();
                if lines.is_empty() {
                    lines.push("ARP table is empty".into());
                }
                TerminalOutput {
                    lines,
                    success: true,
                }
            }
            TerminalCommand::Ping(ip) | TerminalCommand::Traceroute(ip) => {
                let tracing = matches!(command, TerminalCommand::Traceroute(_));
                let Some(source) = source else {
                    return output(false, "no interface");
                };
                let result = self.transmit_icmp(source, ip);
                if result.reachable {
                    let mut lines: Vec<_> = result
                        .hops
                        .iter()
                        .enumerate()
                        .map(|(i, h)| {
                            format!(
                                "{}  {}  {}",
                                i + 1,
                                self.device(h.device)
                                    .map(|d| d.name.as_str())
                                    .unwrap_or("?"),
                                h.note
                            )
                        })
                        .collect();
                    if tracing {
                        lines.push(format!("trace complete: {ip}"));
                    } else {
                        lines.push(format!("Reply from {ip}: ICMP echo reply"));
                    }
                    TerminalOutput {
                        lines,
                        success: true,
                    }
                } else {
                    let failure = result.failure.unwrap_or(ReachabilityFailure::NoRoute);
                    let gateway = Some(source).and_then(|id| self.port(id)).and_then(|port| {
                        match &port.config {
                            PortConfig::Server(config) => {
                                config.ipv4.as_ref().and_then(|ip| ip.gateway)
                            }
                            _ => None,
                        }
                    });
                    let reason = match failure {
                        ReachabilityFailure::VlanBlocked => "VLAN path blocked".to_owned(),
                        ReachabilityFailure::NoGateway => gateway
                            .map(|value| format!("no route to gateway {value}"))
                            .unwrap_or_else(|| "no route to gateway".into()),
                        ReachabilityFailure::NoRoute => "router has no route to destination".into(),
                        ReachabilityFailure::AddressConflict => "IP address conflict".into(),
                        ReachabilityFailure::NoPhysicalLink => {
                            "interface or network link is down".into()
                        }
                        ReachabilityFailure::SourceDown => "source interface is down".into(),
                        ReachabilityFailure::NoAddress => {
                            "source interface has no IPv4 address".into()
                        }
                        ReachabilityFailure::DestinationDown => "destination is down".into(),
                        ReachabilityFailure::DestinationNotFound => {
                            "destination address is not reachable or routed".into()
                        }
                        ReachabilityFailure::TtlExpired => "TTL expired in transit".into(),
                    };
                    let mut lines = if tracing {
                        result
                            .hops
                            .iter()
                            .enumerate()
                            .map(|(i, h)| {
                                format!(
                                    "{}  {}  {}",
                                    i + 1,
                                    self.device(h.device)
                                        .map(|d| d.name.as_str())
                                        .unwrap_or("?"),
                                    h.note
                                )
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };
                    lines.push(format!("{ip}: {reason}"));
                    TerminalOutput {
                        lines,
                        success: false,
                    }
                }
            }
            other => self.execute_linux_command(device, &server, other),
        }
    }
}

impl NetworkSim {
    fn execute_linux_command(
        &mut self,
        _device: DeviceId,
        server: &crate::Server,
        command: TerminalCommand,
    ) -> TerminalOutput {
        let interface_ids: std::collections::HashMap<_, _> = server
            .ports
            .iter()
            .filter_map(|id| self.port(*id).map(|p| (p.name.clone(), *id)))
            .collect();
        let interface = |name: &str| interface_ids.get(name).copied();
        let link_status = |sim: &NetworkSim, p: &crate::Port| {
            if !p.enabled {
                ("DOWN", "DOWN")
            } else if sim.port_link_up(p.id) {
                ("UP,LOWER_UP", "UP")
            } else {
                ("UP,NO-CARRIER", "DOWN")
            }
        };
        let show_addr = |sim: &NetworkSim, ids: &[PortId]| -> Vec<String> {
            ids.iter()
                .filter_map(|id| sim.port(*id))
                .map(|p| {
                    let addr = match &p.config {
                        PortConfig::Server(c) => c
                            .ipv4
                            .as_ref()
                            .map(|v| format!("{}/{}", v.address, v.prefix))
                            .unwrap_or_default(),
                        _ => String::new(),
                    };
                    let (flags, state) = link_status(sim, p);
                    let address = if addr.is_empty() {
                        String::new()
                    } else {
                        format!(" inet {addr}")
                    };
                    format!("{}: <{}> state {}{}", p.name, flags, state, address)
                })
                .collect()
        };
        match command {
            TerminalCommand::Hostname => output(true, server.hostname.clone()),
            TerminalCommand::Uname => output(
                true,
                format!(
                    "Linux {} 6.6.0-sim #1 SMP x86_64 GNU/Linux",
                    server.hostname
                ),
            ),
            TerminalCommand::Lscpu => {
                let cpus: Vec<_> = server
                    .hardware
                    .as_ref()
                    .into_iter()
                    .flat_map(|h| &h.cpus)
                    .filter_map(|id| crate::server_catalog().parts.iter().find(|p| &p.id == id))
                    .collect();
                let model = cpus.first().map_or(
                    if server.hardware.is_some() {
                        "no processor installed"
                    } else {
                        "legacy processor"
                    },
                    |cpu| cpu.name.as_str(),
                );
                TerminalOutput {
                    success: true,
                    lines: vec![
                        "Architecture: x86_64".into(),
                        format!(
                            "Socket(s): {}",
                            if server.hardware.is_some() {
                                cpus.len()
                            } else {
                                1
                            }
                        ),
                        format!("Model name: {model}"),
                    ],
                }
            }
            TerminalCommand::Free => {
                let gib = server.hardware.as_ref().map_or(16, |h| {
                    h.ram
                        .iter()
                        .filter_map(|id| crate::server_catalog().parts.iter().find(|p| &p.id == id))
                        .filter_map(|p| match p.kind {
                            crate::ServerPartKind::Ram { capacity_gb, .. } => {
                                Some(u32::from(capacity_gb))
                            }
                            _ => None,
                        })
                        .sum()
                });
                TerminalOutput {
                    success: true,
                    lines: vec![
                        "               total        used        free".into(),
                        format!("Mem:           {gib}Gi         0Gi        {gib}Gi"),
                        "Swap:            0B          0B          0B".into(),
                    ],
                }
            }
            TerminalCommand::Ethtool {
                interface: name,
                driver,
            } => {
                let Some(port_id) = interface(&name) else {
                    return output(false, format!("Cannot find device \"{name}\""));
                };
                let Some(port) = self.port(port_id) else {
                    return output(false, "interface not found");
                };
                let card = server
                    .hardware
                    .as_ref()
                    .and_then(|h| {
                        h.card_ports
                            .iter()
                            .enumerate()
                            .find(|(_, ports)| ports.contains(&port_id))
                            .map(|(slot, _)| slot)
                    })
                    .and_then(|slot| server.hardware.as_ref()?.pcie.get(slot)?.as_deref())
                    .and_then(|id| crate::server_catalog().parts.iter().find(|p| p.id == id));
                if driver {
                    return TerminalOutput {
                        success: true,
                        lines: vec![
                            format!(
                                "driver: {}",
                                if card.is_some() {
                                    "igb-sim"
                                } else {
                                    "onboard-sim"
                                }
                            ),
                            format!(
                                "bus-info: {}",
                                card.map_or("onboard".into(), |p| p.id.clone())
                            ),
                        ],
                    };
                }
                let detected = self.port_link_up(port_id);
                TerminalOutput {
                    success: true,
                    lines: vec![
                        format!("Settings for {name}:"),
                        format!(
                            "    Supported ports: [{}]",
                            if port.connector.supports_cabling() {
                                "TP"
                            } else {
                                "FIBRE"
                            }
                        ),
                        format!(
                            "    Supported link modes: up to {}Mb/s",
                            port.max_speed.mbps()
                        ),
                        format!(
                            "    Advertised link modes: up to {}Mb/s",
                            port.advertised_speed.mbps()
                        ),
                        format!(
                            "    Speed: {}",
                            self.port_link_speed(port_id)
                                .map_or("Unknown!".into(), |speed| format!("{}Mb/s", speed.mbps()))
                        ),
                        "    Duplex: Full".into(),
                        "    Auto-negotiation: on".into(),
                        format!("    Link detected: {}", if detected { "yes" } else { "no" }),
                    ],
                }
            }
            TerminalCommand::NetstatInterfaces => {
                let mut lines = vec!["Iface          RX-OK   TX-OK   Link".into()];
                for id in &server.ports {
                    if let Some(port) = self.port(*id) {
                        let stats = self.port_telemetry(*id);
                        lines.push(format!(
                            "{:<12} {:>5}   {:>5}   {}",
                            port.name,
                            stats.rx_frames,
                            stats.tx_frames,
                            if self.port_link_up(*id) { "up" } else { "down" }
                        ));
                    }
                }
                TerminalOutput {
                    lines,
                    success: true,
                }
            }
            TerminalCommand::Lsblk => {
                let mut lines = vec!["NAME   SIZE  ROTA  TYPE  MODEL".into()];
                let mut ordinal = 0;
                for id in server
                    .hardware
                    .as_ref()
                    .into_iter()
                    .flat_map(|h| &h.drives)
                    .flatten()
                {
                    if let Some(drive) = crate::drive_catalog().drives.iter().find(|d| &d.id == id)
                    {
                        let name = crate::linux_drive_name(ordinal);
                        lines.push(format!(
                            "{name:<5} {:>4}G  {}     disk  {}",
                            drive.capacity_gb,
                            if drive.kind == crate::DriveKind::Hdd {
                                1
                            } else {
                                0
                            },
                            drive.name
                        ));
                        ordinal += 1;
                    }
                }
                TerminalOutput {
                    lines,
                    success: true,
                }
            }
            TerminalCommand::Smartctl { device: path, all } => {
                let name = path.strip_prefix("/dev/").unwrap_or(&path);
                let mut ordinal = 0;
                for id in server
                    .hardware
                    .as_ref()
                    .into_iter()
                    .flat_map(|h| &h.drives)
                    .flatten()
                {
                    if let Some(drive) = crate::drive_catalog().drives.iter().find(|d| &d.id == id)
                    {
                        if crate::linux_drive_name(ordinal) == name {
                            let mut lines = vec![
                                format!("Device Model: {}", drive.name),
                                format!("User Capacity: {} GB", drive.capacity_gb),
                                format!("Transport protocol: {}", drive.interface),
                                format!(
                                    "Rotation: {}",
                                    if drive.kind == crate::DriveKind::Hdd {
                                        "HDD"
                                    } else {
                                        "Solid State Device"
                                    }
                                ),
                            ];
                            if all {
                                lines.extend([
                                    format!(
                                        "Rated read: {} MB/s, {} IOPS",
                                        drive.read_mb_s, drive.read_iops
                                    ),
                                    format!(
                                        "Rated write: {} MB/s, {} IOPS",
                                        drive.write_mb_s, drive.write_iops
                                    ),
                                ]);
                            }
                            return TerminalOutput {
                                success: true,
                                lines,
                            };
                        }
                        ordinal += 1;
                    }
                }
                output(
                    false,
                    format!("smartctl: cannot open {path}: No such device"),
                )
            }
            TerminalCommand::IpAddrShow(which) => {
                let ids: Vec<_> = which
                    .as_deref()
                    .map(interface)
                    .into_iter()
                    .flatten()
                    .collect();
                if which.is_some() && ids.is_empty() {
                    return output(
                        false,
                        format!("Cannot find device \"{}\"", which.as_deref().unwrap_or("")),
                    );
                }
                let ids = if which.is_some() {
                    ids
                } else {
                    server.ports.clone()
                };
                TerminalOutput {
                    lines: show_addr(self, &ids),
                    success: true,
                }
            }
            TerminalCommand::IpLinkShow(which) => {
                let ids: Vec<_> = which
                    .as_deref()
                    .map(interface)
                    .into_iter()
                    .flatten()
                    .collect();
                if which.is_some() && ids.is_empty() {
                    return output(
                        false,
                        format!("Cannot find device \"{}\"", which.as_deref().unwrap_or("")),
                    );
                }
                let ids = if which.is_some() {
                    ids
                } else {
                    server.ports.clone()
                };
                TerminalOutput {
                    lines: ids
                        .iter()
                        .filter_map(|id| self.port(*id))
                        .map(|p| {
                            let (flags, state) = link_status(self, p);
                            format!("{}: <{}> state {}", p.name, flags, state)
                        })
                        .collect(),
                    success: true,
                }
            }
            TerminalCommand::IpLinkSet {
                interface: name,
                enabled,
            } => {
                let Some(port) = interface(&name) else {
                    return output(false, format!("Cannot find device \"{name}\""));
                };
                match self.execute(Command::SetPortEnabled { port, enabled }) {
                    Ok(_) => output(
                        true,
                        format!("{}: set {}", name, if enabled { "UP" } else { "DOWN" }),
                    ),
                    Err(e) => output(false, e.to_string()),
                }
            }
            TerminalCommand::IpAddrAdd {
                address,
                prefix,
                interface: name,
            } => self.change_address(server, &interface, &name, Some((address, prefix)), false),
            TerminalCommand::IpAddrDel {
                address,
                prefix,
                interface: name,
            } => self.change_address(server, &interface, &name, Some((address, prefix)), true),
            TerminalCommand::IpAddrFlush { interface: name } => {
                self.change_address(server, &interface, &name, None, true)
            }
            TerminalCommand::IpRouteShow => {
                let lines = server
                    .ports
                    .iter()
                    .filter_map(|id| self.port(*id))
                    .filter_map(|p| match &p.config {
                        PortConfig::Server(c) => c.ipv4.as_ref().and_then(|v| {
                            v.gateway.map(|g| format!("default via {g} dev {}", p.name))
                        }),
                        _ => None,
                    })
                    .collect();
                TerminalOutput {
                    lines,
                    success: true,
                }
            }
            TerminalCommand::IpRouteDefault {
                interface: name,
                gateway,
                delete,
                replace,
            } => {
                if delete {
                    return self.change_gateway(
                        server,
                        &interface,
                        name.as_deref(),
                        gateway,
                        true,
                        true,
                    );
                }
                if !replace && name.as_deref().and_then(interface).and_then(|p| self.port(p)).is_some_and(|p| matches!(&p.config, PortConfig::Server(c) if c.ipv4.as_ref().is_some_and(|v| v.gateway.is_some()))) {
                    return output(false, "File exists: default route already configured; use replace");
                }
                self.change_gateway(server, &interface, name.as_deref(), gateway, false, false)
            }
            _ => output(false, "unsupported Linux command"),
        }
    }

    fn change_address(
        &mut self,
        _server: &crate::Server,
        resolve: &impl Fn(&str) -> Option<PortId>,
        name: &str,
        address: Option<(Ipv4Addr, u8)>,
        delete: bool,
    ) -> TerminalOutput {
        let Some(port) = resolve(name) else {
            return output(false, format!("Cannot find device \"{name}\""));
        };
        let old = self.port(port).and_then(|p| match &p.config {
            PortConfig::Server(c) => c.ipv4.clone(),
            _ => None,
        });
        if !delete
            && old
                .as_ref()
                .is_some_and(|v| address.is_some_and(|a| v.address != a.0 || v.prefix != a.1))
        {
            return output(
                false,
                "RTNETLINK answers: address already configured; flush or delete it first",
            );
        }
        if delete && address.is_some() && old.as_ref().map(|v| (v.address, v.prefix)) != address {
            return output(false, "address not found");
        }
        let Some((ip, prefix)) = (if delete { None } else { address }) else {
            return self
                .execute(Command::ResetPortConfig { port })
                .map(|_| output(true, format!("flushed {}", name)))
                .unwrap_or_else(|e| output(false, e.to_string()));
        };
        let config = Ipv4InterfaceConfig {
            address: ip,
            prefix,
            gateway: old.as_ref().and_then(|v| v.gateway),
            vlan: old.as_ref().and_then(|v| v.vlan),
        };
        self.execute(Command::SetIpv4 { port, config })
            .map(|_| output(true, format!("added {ip}/{prefix} dev {name}")))
            .unwrap_or_else(|e| output(false, e.to_string()))
    }

    fn change_gateway(
        &mut self,
        server: &crate::Server,
        resolve: &impl Fn(&str) -> Option<PortId>,
        name: Option<&str>,
        gateway: Option<Ipv4Addr>,
        check_existing: bool,
        clear: bool,
    ) -> TerminalOutput {
        let port = if let Some(name) = name {
            resolve(name)
        } else {
            server.ports.iter().copied().find(|id| {
                self.port(*id)
                    .and_then(|p| match &p.config {
                        PortConfig::Server(c) => c.ipv4.as_ref().filter(|v| {
                            (!clear || v.gateway.is_some())
                                && gateway.is_none_or(|g| v.gateway == Some(g))
                        }),
                        _ => None,
                    })
                    .is_some()
            })
        };
        let Some(port) = port else {
            if let Some(name) = name {
                return output(false, format!("cannot find device: {name}"));
            }
            if clear {
                return output(false, "default route not found");
            }
            return output(false, "no interface with an IPv4 address");
        };
        let Some(old) = self.port(port).and_then(|p| match &p.config {
            PortConfig::Server(c) => c.ipv4.clone(),
            _ => None,
        }) else {
            return output(false, "interface has no IPv4 address");
        };
        if check_existing && gateway.is_some() && old.gateway != gateway {
            return output(false, "default route not found");
        }
        if clear && old.gateway.is_none() {
            return output(false, "default route not found");
        }
        let config = Ipv4InterfaceConfig {
            address: old.address,
            prefix: old.prefix,
            gateway: if clear { None } else { gateway },
            vlan: old.vlan,
        };
        self.execute(Command::SetIpv4 { port, config })
            .map(|_| {
                output(
                    true,
                    if !clear && let Some(gateway) = gateway {
                        format!("default via {gateway}")
                    } else {
                        "default route removed".into()
                    },
                )
            })
            .unwrap_or_else(|e| output(false, e.to_string()))
    }
}

fn output(success: bool, line: impl Into<String>) -> TerminalOutput {
    TerminalOutput {
        lines: vec![line.into()],
        success,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CableSupply, DeviceTemplate, OutletId, PowerEndpoint, RackId, SimEvent, SourceId};

    fn server() -> (NetworkSim, DeviceId) {
        let mut sim = NetworkSim::new();
        let id = match sim
            .execute(Command::BuyDevice {
                kind: DeviceTemplate::Server,
            })
            .unwrap()[0]
        {
            SimEvent::DeviceAdded(id) => id,
            _ => unreachable!(),
        };
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
        (sim, id)
    }

    #[test]
    fn linux_ip_config_preserves_untagged_and_rejects_wrong_interface_without_mutation() {
        let (mut sim, id) = server();
        assert!(
            sim.execute_console(id, "ip addr add 192.0.2.2/24 dev eth0")
                .success
        );
        assert!(
            sim.execute_console(id, "ip route add default via 192.0.2.1 dev eth0")
                .success
        );
        assert!(
            !sim.execute_console(id, "ip route add default via 192.0.2.254 dev eth0")
                .success
        );
        assert!(
            sim.execute_console(id, "ip route replace default via 192.0.2.254 dev eth0")
                .success
        );
        assert!(
            !sim.execute_console(id, "ip route replace default via 192.0.2.1 dev missing")
                .success
        );
        let port = sim.device(id).unwrap().ports()[0];
        assert!(
            matches!(&sim.port(port).unwrap().config, PortConfig::Server(c) if c.ipv4.as_ref().is_some_and(|v| v.vlan.is_none() && v.gateway == Some("192.0.2.254".parse().unwrap())))
        );
        assert!(
            sim.execute_console(id, "ip route del default via 192.0.2.254 dev eth0")
                .success
        );
        let routes = sim.execute_console(id, "ip route show");
        assert!(
            !routes.lines.iter().any(|line| line.contains("default")),
            "{routes:?}"
        );
        assert!(
            !sim.execute_console(id, "ip addr add 198.51.100.2/24 dev eth0")
                .success
        );
        assert!(
            !sim.execute_console(id, "ip addr del 198.51.100.2/24 dev eth0")
                .success
        );
    }

    #[test]
    fn linux_link_state_reports_admin_down_and_no_carrier() {
        let (mut sim, id) = server();
        assert!(sim.execute_console(id, "ip link set dev eth0 down").success);
        let admin_down = sim.execute_console(id, "ip link show dev eth0").lines[0].clone();
        assert!(admin_down.contains("<DOWN>"));
        assert!(sim.execute_console(id, "ip link set dev eth0 up").success);
        let no_carrier = sim.execute_console(id, "ip link show dev eth0").lines[0].clone();
        assert!(no_carrier.contains("<UP,NO-CARRIER>"));
        assert_ne!(admin_down, no_carrier);
    }

    #[test]
    fn passive_devices_have_no_console_even_when_powered_off() {
        let mut sim = NetworkSim::new();
        let id = match sim
            .execute(Command::BuyDevice {
                kind: DeviceTemplate::PatchPanel,
            })
            .unwrap()[0]
        {
            SimEvent::DeviceAdded(id) => id,
            _ => unreachable!(),
        };
        assert_eq!(sim.terminal_prompt(id), "no-console");
        assert!(!sim.execute_console(id, "show version").success);
        assert!(sim.console_help(id, "").is_empty());
    }

    #[test]
    fn linux_network_configuration_drives_ping_and_flush() {
        let (mut sim, a) = server();
        let (_, b) = (
            (),
            match sim
                .execute(Command::BuyDevice {
                    kind: DeviceTemplate::Server,
                })
                .unwrap()[0]
            {
                SimEvent::DeviceAdded(id) => id,
                _ => unreachable!(),
            },
        );
        sim.execute(Command::PlaceDevice {
            device: b,
            rack: RackId(1),
            unit: 2,
        })
        .unwrap();
        sim.execute(Command::ConnectPower {
            outlet: OutletId {
                source: SourceId::Rack(RackId(1)),
                index: 1,
            },
            endpoint: PowerEndpoint::Device(b),
        })
        .unwrap();
        sim.execute(Command::SetPower {
            device: b,
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
        let ap = sim.device(a).unwrap().ports()[0];
        let bp = sim.device(b).unwrap().ports()[0];
        sim.execute(Command::Connect { a: ap, b: bp }).unwrap();
        assert!(
            sim.execute_console(a, "ip addr add 10.0.0.1/24 dev eth0")
                .success
        );
        assert!(
            sim.execute_console(b, "ip addr add 10.0.0.2/24 dev eth0")
                .success
        );
        assert!(sim.ping(ap, "10.0.0.2".parse().unwrap()).reachable);
        assert!(sim.execute_console(a, "ip link set dev eth0 down").success);
        assert!(!sim.ping(ap, "10.0.0.2".parse().unwrap()).reachable);
        assert!(sim.execute_console(a, "ip link set dev eth0 up").success);
        assert!(sim.ping(ap, "10.0.0.2".parse().unwrap()).reachable);
        assert!(sim.execute_console(a, "ip addr flush dev eth0").success);
        assert!(!sim.ping(ap, "10.0.0.2".parse().unwrap()).reachable);
    }

    #[test]
    fn deleting_default_route_without_dev_targets_configured_gateway() {
        let (mut sim, id) = server();
        assert!(
            sim.execute_console(id, "ip addr add 10.0.0.2/24 dev eth0")
                .success
        );
        assert!(
            sim.execute_console(id, "ip addr add 10.0.1.2/24 dev eth1")
                .success
        );
        assert!(
            sim.execute_console(id, "ip route replace default via 10.0.1.1 dev eth1")
                .success
        );
        assert!(sim.execute_console(id, "ip route del default").success);
        let missing = sim.execute_console(id, "ip route del default");
        assert!(!missing.success);
        assert!(missing.lines[0].contains("default route not found"));
        assert!(sim.execute_console(id, "ip addr show dev eth0").lines[0].contains("10.0.0.2/24"));
    }
}
