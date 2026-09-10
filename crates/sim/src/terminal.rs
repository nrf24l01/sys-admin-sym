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
    pub fn execute_terminal(&self, device: DeviceId, command: TerminalCommand) -> TerminalOutput {
        let Some(dev) = self.device(device) else {
            return output(false, "device not found");
        };
        let DeviceKind::Server(server) = &dev.kind else {
            return output(false, "terminal is only available on servers");
        };
        let source = server.ports.first().copied();
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
                                .map(|v| format!("{}/{}  vlan {}", v.address, v.prefix, v.vlan.0))
                                .unwrap_or_else(|| "unconfigured".into()),
                            _ => "n/a".into(),
                        };
                        format!(
                            "{}  {}  {}",
                            p.name,
                            addr,
                            if dev.powered { "UP" } else { "DOWN" }
                        )
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
            TerminalCommand::Arp => output(true, "ARP is derived on demand; no cached entries"),
            TerminalCommand::Ping(ip) | TerminalCommand::Traceroute(ip) => {
                let Some(source) = source else {
                    return output(false, "no interface");
                };
                let result = self.ping(source, ip);
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
                    lines.push(format!("Reply from {ip}: time=1 ms"));
                    TerminalOutput {
                        lines,
                        success: true,
                    }
                } else {
                    let reason = match result.failure.unwrap_or(ReachabilityFailure::NoRoute) {
                        ReachabilityFailure::VlanBlocked => "VLAN blocked",
                        ReachabilityFailure::NoGateway => "gateway unavailable",
                        ReachabilityFailure::AddressConflict => "IP address conflict",
                        ReachabilityFailure::NoPhysicalLink => "network cable disconnected",
                        ReachabilityFailure::SourceDown => "source interface down",
                        ReachabilityFailure::DestinationDown => "destination down",
                        _ => "destination unreachable",
                    };
                    output(false, reason)
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
