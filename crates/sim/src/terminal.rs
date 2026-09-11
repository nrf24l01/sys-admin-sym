use crate::{DeviceId, DeviceKind, NetworkSim, PortConfig, ReachabilityFailure};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalCommand {
    Ip,
    Route,
    Arp,
    Ping(Ipv4Addr),
    Traceroute(Ipv4Addr),
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalOutput {
    pub lines: Vec<String>,
    pub success: bool,
}

pub fn parse_terminal_command(input: &str) -> Result<TerminalCommand, String> {
    let mut words = input.split_whitespace();
    match (words.next(), words.next(), words.next()) {
        (Some("ip"), None, None) => Ok(TerminalCommand::Ip),
        (Some("route"), None, None) => Ok(TerminalCommand::Route),
        (Some("arp"), None, None) => Ok(TerminalCommand::Arp),
        (Some("help"), None, None) => Ok(TerminalCommand::Help),
        (Some("ping"), Some(ip), None) => ip
            .parse()
            .map(TerminalCommand::Ping)
            .map_err(|_| "invalid IPv4 address".into()),
        (Some("traceroute"), Some(ip), None) => ip
            .parse()
            .map(TerminalCommand::Traceroute)
            .map_err(|_| "invalid IPv4 address".into()),
        _ => Err("unknown command; type help".into()),
    }
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
            return output(false, "terminal is only available on servers");
        };
        let source = server.ports.iter().copied().find(|id| {
            self.port_up(*id)
                && self
                    .port(*id)
                    .is_some_and(|p| matches!(&p.config, PortConfig::Server(c) if c.ipv4.is_some()))
        });
        match command {
            TerminalCommand::Help => {
                output(true, "commands: ip, route, arp, ping <ip>, traceroute <ip>")
            }
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
        }
    }
}

fn output(success: bool, line: impl Into<String>) -> TerminalOutput {
    TerminalOutput {
        lines: vec![line.into()],
        success,
    }
}
