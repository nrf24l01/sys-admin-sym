use crate::{DeviceId, DeviceKind, IosMode, NetworkSim, PortConfig};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ConsoleCompletion {
    pub start: usize,
    pub candidates: Vec<String>,
}

const LINUX_COMMANDS: &[&str] = &[
    "help",
    "ip",
    "route",
    "arp",
    "net",
    "hostname",
    "uname",
    "uname -a",
    "lscpu",
    "free",
    "free -h",
    "lsblk",
    "lsblk -d",
    "netstat -i",
    "ethtool <interface>",
    "ethtool -i <interface>",
    "smartctl -a <drive>",
    "smartctl -i <drive>",
    "ping <address>",
    "traceroute <address>",
    "ssh <address>",
    "ip -s link",
    "ip addr",
    "ip address",
    "ip a",
    "ip addr show",
    "ip address show",
    "ip addr show dev <interface>",
    "ip address show dev <interface>",
    "ip addr add <cidr> dev <interface>",
    "ip address add <cidr> dev <interface>",
    "ip addr del <cidr> dev <interface>",
    "ip address del <cidr> dev <interface>",
    "ip addr flush dev <interface>",
    "ip address flush dev <interface>",
    "ip link",
    "ip link show",
    "ip link show dev <interface>",
    "ip link set dev <interface> up",
    "ip link set dev <interface> down",
    "ip route",
    "ip route show",
    "ip r",
    "ip route add default via <address> dev <interface>",
    "ip route replace default via <address> dev <interface>",
    "ip route del default",
    "ip route del default dev <interface>",
    "ip route del default via <address> dev <interface>",
];

impl NetworkSim {
    /// Complete the token before the cursor without executing a command or changing mode.
    pub fn console_completions(&self, device: DeviceId, input: &str) -> ConsoleCompletion {
        let mut device = device;
        let mut visited = std::collections::HashSet::new();
        while let Some(target) = self.ssh_sessions.get(&device) {
            if !visited.insert(device) {
                return ConsoleCompletion::default();
            }
            device = *target;
        }
        let start = input
            .char_indices()
            .rfind(|(_, c)| c.is_whitespace())
            .map_or(0, |(index, c)| index + c.len_utf8());
        let mut result = ConsoleCompletion {
            start,
            candidates: Vec::new(),
        };
        let Some(dev) = self.device(device) else {
            return result;
        };
        let server = matches!(dev.kind, DeviceKind::Server(_));
        let mut templates = if server {
            LINUX_COMMANDS
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
        } else {
            self.console_help(device, "")
        };
        let previous: Vec<_> = input[..start].split_whitespace().collect();
        let partial = &input[start..];
        if previous.first() == Some(&"do")
            && templates.iter().any(|template| template.starts_with("do "))
        {
            let mut exec = self.clone();
            exec.console_modes.insert(device, IosMode::Privileged);
            templates.extend(
                exec.console_help(device, "")
                    .into_iter()
                    .map(|template| format!("do {template}")),
            );
        }
        for template in templates {
            let tokens: Vec<_> = template.split_whitespace().collect();
            if !previous.iter().enumerate().all(|(index, word)| {
                tokens.get(index).is_some_and(|token| {
                    token.starts_with('<')
                        || if server {
                            token == word
                        } else {
                            token.starts_with(&word.to_ascii_lowercase())
                        }
                })
            }) {
                continue;
            }
            let Some(token) = tokens.get(previous.len()) else {
                continue;
            };
            let values = if token.starts_with('<') {
                self.completion_values(device, token)
            } else {
                vec![token.to_string()]
            };
            result.candidates.extend(values.into_iter().filter(|value| {
                if server {
                    value.starts_with(partial)
                } else {
                    value
                        .to_ascii_lowercase()
                        .starts_with(&partial.to_ascii_lowercase())
                }
            }));
        }
        result.candidates.sort();
        result.candidates.dedup();
        result
    }

    fn completion_values(&self, device: DeviceId, token: &str) -> Vec<String> {
        let dev = self.device(device).expect("completion device exists");
        match token {
            "<interface>" | "<interface...>" | "<range...>" => {
                let mut names = Vec::new();
                for id in dev.ports() {
                    if let Some(port) = self.port(*id) {
                        if matches!(dev.kind, DeviceKind::Server(_)) {
                            names.push(port.name.clone());
                        } else {
                            let name = self.ios_interface_name(device, *id);
                            names.push(name.replace("GigabitEthernet", "Gi"));
                            names.push(name);
                            if let PortConfig::Router(config) = &port.config {
                                names.extend(
                                    config
                                        .interfaces
                                        .iter()
                                        .map(|interface| interface.name.clone()),
                                );
                            }
                        }
                    }
                }
                names
            }
            "<drive>" => {
                let DeviceKind::Server(server) = &dev.kind else {
                    return Vec::new();
                };
                server
                    .hardware
                    .as_ref()
                    .into_iter()
                    .flat_map(|hardware| &hardware.drives)
                    .flatten()
                    .enumerate()
                    .map(|(index, _)| format!("/dev/{}", crate::linux_drive_name(index)))
                    .collect()
            }
            "<address>" => self
                .ports()
                .flat_map(|port| match &port.config {
                    PortConfig::Server(config) => config
                        .ipv4
                        .iter()
                        .flat_map(|ip| {
                            std::iter::once(ip.address.to_string())
                                .chain(ip.gateway.map(|gateway| gateway.to_string()))
                        })
                        .collect::<Vec<_>>(),
                    PortConfig::Router(config) => config
                        .interfaces
                        .iter()
                        .filter_map(|interface| interface.address.map(|ip| ip.to_string()))
                        .collect(),
                    _ => Vec::new(),
                })
                .chain(
                    self.ios_configs
                        .values()
                        .filter_map(|config| config.management_ip.map(|ip| ip.to_string())),
                )
                .collect(),
            "<cidr>" => dev
                .ports()
                .iter()
                .filter_map(|id| self.port(*id))
                .filter_map(|port| match &port.config {
                    PortConfig::Server(config) => config
                        .ipv4
                        .as_ref()
                        .map(|ip| format!("{}/{}", ip.address, ip.prefix)),
                    _ => None,
                })
                .collect(),
            "<id>" | "<list>" => {
                let mut values: Vec<_> = match &dev.kind {
                    DeviceKind::Switch(switch) => switch
                        .vlans
                        .iter()
                        .map(|vlan| vlan.id.to_string())
                        .collect(),
                    DeviceKind::Router(router) => router
                        .interfaces
                        .iter()
                        .filter_map(|interface| interface.vlan.map(|vlan| vlan.to_string()))
                        .collect(),
                    _ => Vec::new(),
                };
                if token == "<list>" {
                    values.extend(["all".into(), "none".into()]);
                }
                values
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, DeviceTemplate, Ipv4InterfaceConfig, SimEvent};

    #[test]
    fn completion_follows_in_game_ssh_and_includes_management_ips() {
        let mut sim = NetworkSim::new();
        let SimEvent::DeviceAdded(server) = sim.execute(Command::BuyServerChassis).unwrap()[0]
        else {
            panic!()
        };
        let SimEvent::DeviceAdded(switch) = sim
            .execute(Command::BuyDevice {
                kind: DeviceTemplate::Switch,
            })
            .unwrap()[0]
        else {
            panic!()
        };
        sim.ios_configs.entry(switch).or_default().management_ip =
            Some("192.0.2.3".parse().unwrap());
        assert_eq!(
            sim.console_completions(server, "ssh 192.").candidates,
            ["192.0.2.3"]
        );
        sim.ssh_sessions.insert(server, switch);
        sim.console_modes.insert(switch, IosMode::Privileged);
        assert_eq!(
            sim.console_completions(server, "conf").candidates,
            ["configure"]
        );
        assert!(sim.console_completions(server, "lsc").candidates.is_empty());
        assert_eq!(sim.ssh_sessions.get(&server), Some(&switch));
        assert_eq!(sim.console_modes.get(&switch), Some(&IosMode::Privileged));
        sim.ssh_sessions.insert(switch, server);
        assert!(
            sim.console_completions(server, "conf")
                .candidates
                .is_empty()
        );
    }

    #[test]
    fn ios_completion_tracks_mode_and_preserves_configuration() {
        let mut sim = NetworkSim::new();
        let SimEvent::DeviceAdded(device) = sim
            .execute(Command::BuyDevice {
                kind: DeviceTemplate::Switch,
            })
            .unwrap()[0]
        else {
            panic!()
        };
        assert_eq!(sim.console_completions(device, "en").candidates, ["enable"]);
        assert!(
            sim.console_completions(device, "conf")
                .candidates
                .is_empty()
        );
        sim.console_modes.insert(device, IosMode::Privileged);
        assert_eq!(
            sim.console_completions(device, "conf").candidates,
            ["configure"]
        );
        assert_eq!(
            sim.console_completions(device, "conf t").candidates,
            ["terminal"]
        );
        sim.console_modes.insert(device, IosMode::Global);
        let before = serde_json::to_string(&sim).unwrap();
        let interfaces = sim.console_completions(device, "interface Gi1/0/");
        assert_eq!(interfaces.start, 10);
        assert!(interfaces.candidates.contains(&"Gi1/0/1".into()));
        assert_eq!(
            sim.console_completions(device, "do sh ver").candidates,
            ["version"]
        );
        assert_eq!(sim.terminal_prompt(device), "Switch(config)#");
        assert_eq!(serde_json::to_string(&sim).unwrap(), before);
    }

    #[test]
    fn linux_completion_uses_interfaces_drives_and_known_addresses() {
        let mut sim = NetworkSim::new();
        let SimEvent::DeviceAdded(device) = sim.execute(Command::BuyServerChassis).unwrap()[0]
        else {
            panic!()
        };
        let port = sim.device(device).unwrap().ports()[0];
        sim.execute(Command::SetIpv4 {
            port,
            config: Ipv4InterfaceConfig {
                address: "192.168.10.2".parse().unwrap(),
                prefix: 24,
                gateway: None,
                vlan: None,
            },
        })
        .unwrap();
        assert_eq!(sim.console_completions(device, "lsc").candidates, ["lscpu"]);
        assert_eq!(
            sim.console_completions(device, "ip addr show d").candidates,
            ["dev"]
        );
        let interfaces = sim.console_completions(device, "ip link set dev et");
        assert!(interfaces.candidates.contains(&"eth0".into()));
        assert_eq!(interfaces.start, 16);
        assert_eq!(
            sim.console_completions(device, "ping 192.").candidates,
            ["192.168.10.2"]
        );
        assert_eq!(
            sim.console_completions(device, "ip addr del ").candidates,
            ["192.168.10.2/24"]
        );
        assert!(
            sim.console_completions(device, "smartctl -a /dev/")
                .candidates
                .is_empty()
        );
        sim.execute(Command::BuyDrive {
            drive_id: "enterprise_ssd_960gb".into(),
        })
        .unwrap();
        sim.execute(Command::InstallDrive {
            device,
            drive_id: "enterprise_ssd_960gb".into(),
            bay: Some(2),
        })
        .unwrap();
        assert_eq!(
            sim.console_completions(device, "smartctl -a /dev/s")
                .candidates,
            ["/dev/sda"]
        );
        assert!(
            sim.console_completions(device, "unknown ")
                .candidates
                .is_empty()
        );
    }
}
