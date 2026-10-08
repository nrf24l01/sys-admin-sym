//! IOS-style console for the simulator. No Cisco firmware is executed.
mod routes;
mod switching;

use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IosDeviceConfig {
    pub hostname: Option<String>,
    #[serde(default)]
    pub management_ip: Option<Ipv4Addr>,
    pub descriptions: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IosStartupConfig {
    kind: DeviceKind,
    ports: Vec<Port>,
    metadata: IosDeviceConfig,
    #[serde(default)]
    network: Option<provider::DeviceNetworkConfig>,
}

impl IosStartupConfig {
    pub(crate) fn normalize_interfaces(&mut self) {
        if let DeviceKind::Switch(switch) = &mut self.kind {
            let spec = switch.model.spec();
            if self.ports.iter().any(|p| p.max_speed == LinkSpeed::Gbps10) {
                switch.model = SwitchModel::Catalyst24T4X;
                for (index, port) in self.ports.iter_mut().enumerate().skip(spec.copper_ports) {
                    let number = index + 1 - spec.copper_ports;
                    port.name = format!("Te1/0/{number:02}");
                    if let Some(description) = self
                        .metadata
                        .descriptions
                        .remove(&format!("TenGigabitEthernet1/0/{}", index + 1))
                    {
                        self.metadata
                            .descriptions
                            .insert(format!("TenGigabitEthernet1/0/{number}"), description);
                    }
                }
            }
        }
        if let DeviceKind::Router(router) = &mut self.kind {
            crate::normalize_router_interfaces(&mut router.interfaces);
        }
        for port in &mut self.ports {
            if let PortConfig::Router(config) = &mut port.config {
                crate::normalize_router_interfaces(&mut config.interfaces);
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum IosMode {
    #[default]
    User,
    Privileged,
    Global,
    Vlan(VlanId),
    PortChannel(u8),
    Interface {
        ports: Vec<PortId>,
        subinterface: Option<u16>,
    },
    ReloadConfirm,
}

const SHOW: &[&str] = &[
    "show version",
    "show inventory",
    "show interfaces transceiver",
    "show interfaces <interface> transceiver",
    "show interfaces",
    "show interfaces status",
    "show interfaces <interface>",
    "show interfaces <interface> switchport",
    "show ip interface brief",
    "show ip route",
    "show arp",
    "show ip arp",
    "show vlan brief",
    "show interfaces trunk",
    "show cdp neighbors",
    "show running-config",
    "show startup-config",
];

fn grammar(mode: &IosMode, switch: bool) -> Vec<&'static str> {
    let mut commands = vec!["exit", "help"];
    match mode {
        IosMode::User | IosMode::Privileged => {
            commands.extend(SHOW.iter().copied());
            if switch {
                commands.extend(switching::SHOW.iter().copied());
            }
            commands.push("enable");
            if !switch {
                commands.extend(["ping <address>", "traceroute <address>"]);
            }
            if *mode == IosMode::Privileged {
                commands.extend([
                    "disable",
                    "configure terminal",
                    "write memory",
                    "copy running-config startup-config",
                    "reload",
                ]);
            } else {
                commands.retain(|c| !matches!(*c, "show running-config" | "show startup-config"));
            }
        }
        IosMode::Global
        | IosMode::Vlan(_)
        | IosMode::Interface { .. }
        | IosMode::PortChannel(_) => {
            commands.extend(["end", "do <command...>"]);
            if matches!(mode, IosMode::Global) {
                commands.extend([
                    "hostname <name>",
                    "no hostname",
                    "interface <interface...>",
                    "interface range <range...>",
                ]);
                if !switch {
                    commands.extend([
                        "ip route <network> <mask> <next-hop>",
                        "ip route <network> <mask> <interface> <next-hop>",
                        "no ip route <network> <mask> <next-hop>",
                        "no ip route <network> <mask> <interface> <next-hop>",
                    ]);
                }
                if switch {
                    commands.extend(switching::GLOBAL.iter().copied());
                    commands.extend([
                        "vlan <id>",
                        "no vlan <id>",
                        "management ip <address>",
                        "no management ip",
                    ]);
                }
            }
            if matches!(mode, IosMode::Vlan(_)) {
                commands.push("name <name>");
            }
            if matches!(mode, IosMode::Interface { .. } | IosMode::PortChannel(_)) {
                commands.extend(["interface <interface...>", "interface range <range...>"]);
                commands.extend(["description <text...>", "no description"]);
                if !matches!(
                    mode,
                    IosMode::Interface {
                        subinterface: Some(_),
                        ..
                    }
                ) {
                    commands.extend(["shutdown", "no shutdown"]);
                }
                if switch {
                    commands.extend(switching::INTERFACE.iter().copied());
                    commands.extend([
                        "switchport mode access",
                        "switchport mode trunk",
                        "switchport access vlan <id>",
                        "no switchport access vlan",
                        "switchport trunk allowed vlan <list>",
                        "switchport trunk allowed vlan add <list>",
                        "switchport trunk allowed vlan remove <list>",
                        "switchport trunk native vlan <id>",
                    ]);
                } else {
                    commands.extend(["ip address <address> <mask>", "no ip address"]);
                    if matches!(
                        mode,
                        IosMode::Interface {
                            subinterface: Some(_),
                            ..
                        }
                    ) {
                        commands.push("encapsulation dot1q <id>");
                    }
                }
                commands.extend([
                    "speed 10",
                    "speed 100",
                    "speed 1000",
                    "speed 10000",
                    "speed 25000",
                    "speed auto",
                ]);
            }
            if matches!(mode, IosMode::PortChannel(_)) {
                commands.extend([
                    "port-channel min-links <count>",
                    "no port-channel min-links",
                ]);
            }
        }
        IosMode::ReloadConfirm => {}
    }
    commands
}

// Resolve literal keywords by unique prefix, keeping user values case-sensitive.
fn resolve<'a>(input: &str, grammar: &'a [&str]) -> Result<(&'a str, Vec<String>), String> {
    let words: Vec<_> = input.split_whitespace().collect();
    // Resolve each keyword before deciding whether a command is complete. For
    // example, `sh i` is ambiguous between `show ip` and `show interfaces`.
    let mut active = grammar.to_vec();
    for (i, word) in words.iter().enumerate() {
        let lower = word.to_ascii_lowercase();
        let mut keywords: Vec<_> = active
            .iter()
            .filter_map(|pattern| {
                let token = pattern.split_whitespace().nth(i)?;
                (!token.starts_with('<') && token.starts_with(&lower)).then_some(token)
            })
            .collect();
        keywords.sort_unstable();
        keywords.dedup();
        if keywords.contains(&lower.as_str()) {
            keywords.retain(|k| *k == lower);
        }
        if keywords.len() > 1 {
            return Err(format!("% Ambiguous command: {input}"));
        }
        active.retain(|pattern| {
            let tokens: Vec<_> = pattern.split_whitespace().collect();
            match tokens.get(i) {
                Some(token) if keywords.len() == 1 => *token == keywords[0],
                Some(token) => token.starts_with('<') || token.starts_with(&lower),
                None => tokens.last().is_some_and(|t| t.ends_with("...>")),
            }
        });
    }
    let mut matches = Vec::new();
    let mut incomplete = false;
    for pattern in &active {
        let tokens: Vec<_> = pattern.split_whitespace().collect();
        let mut args = Vec::new();
        let mut valid = true;
        let mut score = 0;
        let mut consumed = 0;
        for (i, token) in tokens.iter().enumerate() {
            let Some(word) = words.get(i) else {
                incomplete = valid || incomplete;
                valid = false;
                break;
            };
            if token.starts_with('<') {
                if token.ends_with("...>") {
                    args.push(words[i..].join(" "));
                    consumed = words.len();
                    break;
                }
                args.push((*word).to_owned());
            } else if token.starts_with(&word.to_ascii_lowercase()) {
                score += 1;
            } else {
                valid = false;
                break;
            }
            consumed = i + 1;
        }
        if valid && consumed == words.len() {
            matches.push((*pattern, args, score));
        }
    }
    let best = matches.iter().map(|v| v.2).max().unwrap_or(0);
    matches.retain(|v| v.2 == best);
    match matches.len() {
        1 => {
            let (pattern, args, _) = matches.remove(0);
            Ok((pattern, args))
        }
        0 if incomplete => Err("% Incomplete command.".into()),
        0 => Err(
            "% Invalid or unsupported command in this mode. Type ? for supported commands.".into(),
        ),
        _ => Err(format!("% Ambiguous command: {input}")),
    }
}

impl NetworkSim {
    pub fn console_management_ip(&self, device: DeviceId) -> Option<Ipv4Addr> {
        self.switch_management(device).map(|m| m.address)
    }

    pub fn console_hostname(&self, device: DeviceId) -> Option<&str> {
        let dev = self.device(device)?;
        match &dev.kind {
            DeviceKind::Server(server) => Some(&server.hostname),
            DeviceKind::Switch(_) | DeviceKind::Router(_) => Some(
                self.ios_configs
                    .get(&device)
                    .and_then(|config| config.hostname.as_deref())
                    .unwrap_or(if matches!(dev.kind, DeviceKind::Switch(_)) {
                        "Switch"
                    } else {
                        "Router"
                    }),
            ),
            _ => None,
        }
    }

    pub fn terminal_prompt(&self, device: DeviceId) -> String {
        if let Some(target) = self.ssh_sessions.get(&device) {
            return format!("ssh:{} {}", target, self.local_terminal_prompt(*target));
        }
        self.local_terminal_prompt(device)
    }

    fn local_terminal_prompt(&self, device: DeviceId) -> String {
        let Some(dev) = self.device(device) else {
            return "?>".into();
        };
        if let DeviceKind::Server(server) = &dev.kind {
            return self.server_os(device).map_or_else(
                || format!("root@{}:~#", server.hostname),
                |os| os.prompt(&server.hostname),
            );
        }
        if !matches!(dev.kind, DeviceKind::Switch(_) | DeviceKind::Router(_)) {
            return "no-console".into();
        }
        let name = self
            .console_hostname(device)
            .expect("console device has hostname");
        let suffix = match self.console_modes.get(&device).unwrap_or(&IosMode::User) {
            IosMode::User => ">",
            IosMode::Privileged => "#",
            IosMode::Global => "(config)#",
            IosMode::Vlan(_) => "(config-vlan)#",
            IosMode::PortChannel(_) => "(config-if)#",
            IosMode::Interface {
                ports,
                subinterface,
            } => {
                if subinterface.is_some() {
                    "(config-subif)#"
                } else if ports.len() > 1 {
                    "(config-if-range)#"
                } else {
                    "(config-if)#"
                }
            }
            IosMode::ReloadConfirm => " [confirm]",
        };
        format!("{name}{suffix}")
    }

    /// Execute one console line atomically. Invalid commands cannot partially configure a range.
    pub fn execute_console(&mut self, device: DeviceId, input: &str) -> TerminalOutput {
        let input = input.trim();
        if input == "exit" && self.ssh_sessions.remove(&device).is_some() {
            return reply(true, "SSH connection closed");
        }
        if let Some(target) = self.ssh_sessions.get(&device).copied() {
            if !self.device(target).is_some_and(|target| target.powered) {
                self.ssh_sessions.remove(&device);
                return reply(false, "SSH connection lost: target is offline");
            }
            return self.execute_console(target, input);
        }
        if let Some(address) = input
            .strip_prefix("ssh ")
            .filter(|address| !address.chars().any(char::is_whitespace))
        {
            let address = address.strip_prefix("root@").unwrap_or(address);
            let Ok(address) = address.parse::<Ipv4Addr>() else {
                return reply(false, "usage: ssh <management-ip>");
            };
            let source = self.device(device).and_then(|dev| match &dev.kind {
                DeviceKind::Server(server) => server
                    .ports
                    .iter()
                    .copied()
                    .find(|id| self.port(*id).is_some_and(|p| p.name == "mgmt0")),
                _ => None,
            });
            let Some(source) = source else {
                return reply(false, "SSH requires a server management interface");
            };
            let target = self.devices().find_map(|dev| match &dev.kind {
                DeviceKind::Server(server) if dev.id != device => server.ports.iter().find(|id| self.port(**id).is_some_and(|p| p.name == "mgmt0" && matches!(&p.config, PortConfig::Server(config) if config.ipv4.as_ref().is_some_and(|ip| ip.address == address)))).map(|_| dev.id),
                DeviceKind::Switch(_) if self.switch_management(dev.id).is_some_and(|config| config.address == address) => Some(dev.id),
                DeviceKind::Router(router) if router.interfaces.iter().any(|interface| interface.address == Some(address)) => Some(dev.id),
                _ => None,
            });
            let Some(target) = target else {
                return reply(false, "management IP not found");
            };
            let reachable = self.ping(source, address).reachable;
            if !reachable {
                return reply(false, "management IP is unreachable");
            }
            if matches!(
                self.device(target).map(|dev| &dev.kind),
                Some(DeviceKind::Server(_))
            ) && !self
                .guest_mut(target)
                .services
                .get("ssh")
                .is_some_and(|service| service.active)
            {
                return reply(
                    false,
                    "ssh: connect to host: Connection refused (ssh.service is stopped)",
                );
            }
            self.ssh_sessions.insert(device, target);
            return reply(
                true,
                format!(
                    "Connected to {} via SSH",
                    self.device(target).map_or("?", |dev| dev.name.as_str())
                ),
            );
        }
        let Some(dev) = self.device(device) else {
            return reply(false, "% Device not found.");
        };
        if !matches!(
            dev.kind,
            DeviceKind::Server(_) | DeviceKind::Switch(_) | DeviceKind::Router(_)
        ) {
            return reply(false, "% This device has no console.");
        }
        if !dev.powered {
            return reply(
                false,
                "% Device is powered off. Power it on in the inspector.",
            );
        }
        if matches!(dev.kind, DeviceKind::Server(_)) {
            return LinuxShell::execute(self, device, input);
        }
        let mut candidate = self.clone();
        let mut mode = candidate.console_modes.remove(&device).unwrap_or_default();
        match candidate.ios_command(device, &mut mode, input.trim()) {
            Ok(lines) => {
                candidate.console_modes.insert(device, mode);
                *self = candidate;
                TerminalOutput {
                    lines,
                    success: true,
                }
            }
            Err(error) => reply(false, error),
        }
    }

    pub fn console_help(&self, device: DeviceId, prefix: &str) -> Vec<String> {
        let device = self.ssh_sessions.get(&device).copied().unwrap_or(device);
        if self
            .device(device)
            .is_some_and(|dev| matches!(dev.kind, DeviceKind::Server(_)))
        {
            return CommandRegistry::standard()
                .names()
                .into_iter()
                .filter(|command| command.starts_with(prefix))
                .map(str::to_owned)
                .collect();
        }
        if !self
            .device(device)
            .is_some_and(|d| matches!(d.kind, DeviceKind::Switch(_) | DeviceKind::Router(_)))
        {
            return Vec::new();
        }
        let switch = self
            .device(device)
            .is_some_and(|d| matches!(d.kind, DeviceKind::Switch(_)));
        let mode = self.console_modes.get(&device).unwrap_or(&IosMode::User);
        let words: Vec<_> = prefix.split_whitespace().collect();
        grammar(mode, switch)
            .into_iter()
            .filter(|pattern| {
                let tokens: Vec<_> = pattern.split_whitespace().collect();
                words.iter().enumerate().all(|(i, word)| {
                    tokens.get(i).is_some_and(|token| {
                        token.starts_with('<') || token.starts_with(&word.to_ascii_lowercase())
                    })
                })
            })
            .map(str::to_owned)
            .collect()
    }

    fn ios_command(
        &mut self,
        device: DeviceId,
        mode: &mut IosMode,
        input: &str,
    ) -> Result<Vec<String>, String> {
        let switch = matches!(self.devices[&device].kind, DeviceKind::Switch(_));
        if *mode == IosMode::ReloadConfirm {
            if input.is_empty() || input.eq_ignore_ascii_case("yes") {
                let mut saved = self
                    .startup_configs
                    .get(&device)
                    .cloned()
                    .ok_or("% No startup configuration. Save with write memory first.")?;
                saved.normalize_interfaces();
                self.devices.get_mut(&device).unwrap().kind = saved.kind;
                for port in saved.ports {
                    self.ports.insert(port.id, port);
                }
                if let Some(network) = saved.network {
                    self.restore_device_network(device, network);
                }
                self.ios_configs.insert(device, saved.metadata);
                self.runtime
                    .device_started
                    .insert(device, self.simulation_time_ms());
                self.runtime.snmp.remove(&device);
                for port in self.devices[&device].ports() {
                    self.runtime.qos.remove(port);
                    self.runtime.qos_buckets.remove(port);
                }
                self.topology_revision += 1;
                self.routing_revision += 1;
                *mode = IosMode::User;
                return Ok(vec!["Device reloaded from startup-config.".into()]);
            }
            *mode = IosMode::Privileged;
            return Ok(vec!["Reload cancelled.".into()]);
        }
        if input.is_empty() || input == "!" {
            return Ok(vec![]);
        }
        let commands = grammar(mode, switch);
        if input.ends_with('?') || input.eq_ignore_ascii_case("help") {
            let prefix = input.trim_end_matches('?').trim();
            let words: Vec<_> = if prefix == "help" {
                vec![]
            } else {
                prefix.split_whitespace().collect()
            };
            return Ok(commands
                .into_iter()
                .filter(|pattern| {
                    let tokens: Vec<_> = pattern.split_whitespace().collect();
                    words.iter().enumerate().all(|(i, w)| {
                        tokens.get(i).is_some_and(|t| {
                            t.starts_with('<') || t.starts_with(&w.to_ascii_lowercase())
                        })
                    })
                })
                .map(str::to_owned)
                .collect());
        }
        let (command, args) = resolve(input, &commands)?;
        if command.starts_with("show ") {
            self.prepare_runtime();
            return self.ios_show(device, command, &args);
        }
        match command {
            "ping <address>" | "traceroute <address>" => {
                let address = args[0]
                    .parse::<Ipv4Addr>()
                    .map_err(|_| "% Invalid IPv4 address.")?;
                let result = self.ping_router_mut(device, address);
                if !result.reachable {
                    return Err(format!("% Destination unreachable: {:?}", result.failure));
                }
                let mut lines = vec![];
                if command.starts_with("traceroute") {
                    lines.extend(result.hops.iter().enumerate().map(|(i, hop)| {
                        format!(
                            "{}  {}  {}",
                            i + 1,
                            self.devices[&hop.device].name,
                            hop.note
                        )
                    }));
                }
                lines.push(format!(
                    "Reply from {address}: simulated reachability successful"
                ));
                return Ok(lines);
            }
            "enable" => *mode = IosMode::Privileged,
            "disable" => *mode = IosMode::User,
            "configure terminal" => *mode = IosMode::Global,
            "end" => *mode = IosMode::Privileged,
            "exit" => {
                *mode = match mode {
                    IosMode::Interface { .. } | IosMode::Vlan(_) | IosMode::PortChannel(_) => {
                        IosMode::Global
                    }
                    IosMode::Global => IosMode::Privileged,
                    _ => IosMode::User,
                }
            }
            "do <command...>" => {
                let mut exec = IosMode::Privileged;
                let exec_grammar = grammar(&exec, switch);
                let (exec_command, _) = resolve(&args[0], &exec_grammar)?;
                if !exec_command.starts_with("show ")
                    && !matches!(
                        exec_command,
                        "ping <address>"
                            | "traceroute <address>"
                            | "write memory"
                            | "copy running-config startup-config"
                    )
                {
                    return Err("% This command cannot run through do.".into());
                }
                return self.ios_command(device, &mut exec, &args[0]);
            }
            "ip route <network> <mask> <next-hop>"
            | "ip route <network> <mask> <interface> <next-hop>"
            | "no ip route <network> <mask> <next-hop>"
            | "no ip route <network> <mask> <interface> <next-hop>" => {
                self.ios_static_route(device, &args, command.starts_with("no "))?;
            }
            "hostname <name>" => {
                let name = &args[0];
                if name.len() > 63
                    || !name.starts_with(|c: char| c.is_ascii_alphabetic())
                    || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                {
                    return Err("% Invalid hostname (1–63 letters, digits or hyphens, starting with a letter).".into());
                }
                self.ios_configs.entry(device).or_default().hostname = Some(name.clone());
            }
            "no hostname" => self.ios_configs.entry(device).or_default().hostname = None,
            "management ip <address>" => {
                let address = args[0]
                    .parse::<Ipv4Addr>()
                    .map_err(|_| "% Invalid management IPv4 address.")?;
                if address.is_unspecified() || address.is_multicast() || address.is_broadcast() {
                    return Err("% Invalid management IPv4 address.".into());
                }
                let mut management = self.switch_management(device).unwrap_or(SwitchManagement {
                    switch: device,
                    address,
                    prefix: 24,
                    vlan: VlanId(1),
                    gateway: None,
                });
                management.address = address;
                self.ios_execute(Command::Provider(ProviderCommand::SetSwitchManagement(
                    management,
                )))?;
                self.ios_configs.entry(device).or_default().management_ip = Some(address);
            }
            "no management ip" => {
                self.clear_switch_management(device);
                self.ios_configs.entry(device).or_default().management_ip = None;
                self.routing_revision += 1;
            }
            "write memory" | "copy running-config startup-config" => {
                self.startup_configs.insert(
                    device,
                    IosStartupConfig {
                        kind: self.devices[&device].kind.clone(),
                        ports: self.devices[&device]
                            .ports()
                            .iter()
                            .map(|p| self.ports[p].clone())
                            .collect(),
                        metadata: self.ios_configs.get(&device).cloned().unwrap_or_default(),
                        network: Some(self.device_network_config(device)),
                    },
                );
                return Ok(vec!["Building configuration...".into(), "[OK]".into()]);
            }
            "reload" => {
                if !self.startup_configs.contains_key(&device) {
                    return Err("% No startup configuration. Use write memory first.".into());
                }
                *mode = IosMode::ReloadConfirm;
                return Ok(vec!["Reload discards unsaved running configuration. Press Enter to confirm; type cancel to keep it.".into()]);
            }
            "vlan <id>" => {
                let id = vlan_id(&args[0])?;
                if let DeviceKind::Switch(sw) = &self.devices[&device].kind
                    && !sw.vlans.iter().any(|v| v.id == id)
                {
                    self.ios_execute(Command::CreateVlan {
                        switch: device,
                        vlan: Vlan {
                            id,
                            name: format!("VLAN{:04}", id.0),
                        },
                    })?;
                }
                *mode = IosMode::Vlan(id);
            }
            "no vlan <id>" => {
                let id = vlan_id(&args[0])?;
                if id == VlanId(1) {
                    return Err("% Default VLAN 1 cannot be removed.".into());
                }
                if let DeviceKind::Switch(sw) = &mut self.devices.get_mut(&device).unwrap().kind {
                    sw.vlans.retain(|v| v.id != id);
                }
                self.topology_revision += 1;
            }
            "name <name>" => {
                let IosMode::Vlan(id) = mode else {
                    unreachable!()
                };
                self.ios_execute(Command::CreateVlan {
                    switch: device,
                    vlan: Vlan {
                        id: *id,
                        name: args[0].clone(),
                    },
                })?;
            }
            "interface <interface...>" | "interface range <range...>" => {
                let range = command.contains("range");
                if switch && let Some(group) = switching::channel_number(&args[0]) {
                    if range || self.channel_ports(device, group).is_empty() {
                        return Err(
                            "% Create the channel-group on physical interfaces first.".into()
                        );
                    }
                    *mode = IosMode::PortChannel(group);
                    return Ok(vec![]);
                }
                let (ports, subinterface) = self.ios_interfaces(device, &args[0], range)?;
                *mode = IosMode::Interface {
                    ports,
                    subinterface,
                };
            }
            _ => {
                if switching::GLOBAL.contains(&command) {
                    self.ios_switch_global(device, command, &args)?;
                    return Ok(vec![]);
                }
                if let IosMode::PortChannel(group) = mode {
                    self.ios_channel_command(device, *group, command, &args)?;
                    return Ok(vec![]);
                }
                let IosMode::Interface {
                    ports,
                    subinterface,
                } = mode
                else {
                    return Err("% Unsupported command.".into());
                };
                for port in ports.clone() {
                    self.ios_interface_command(device, port, *subinterface, command, &args)?;
                }
            }
        }
        Ok(vec![])
    }

    fn ios_execute(&mut self, command: Command) -> Result<(), String> {
        self.execute(command)
            .map(|_| ())
            .map_err(|e| format!("% {e}"))
    }

    pub fn ios_interface_name(&self, device: DeviceId, port: PortId) -> String {
        let dev = &self.devices[&device];
        let index = dev.ports().iter().position(|p| *p == port).unwrap_or(0);
        match dev.kind {
            DeviceKind::Switch(ref switch) => format!(
                "{}1/0/{}",
                if self.ports[&port].max_speed == LinkSpeed::Gbps10 {
                    "TenGigabitEthernet"
                } else {
                    "GigabitEthernet"
                },
                if switch.model == SwitchModel::Catalyst24T4X
                    && index >= switch.model.spec().copper_ports
                {
                    index + 1 - switch.model.spec().copper_ports
                } else {
                    index + 1
                }
            ),
            DeviceKind::Router(_) if index < 2 => format!("GigabitEthernet0/0/{index}"),
            DeviceKind::Router(_) => format!("GigabitEthernet0/1/{}", index - 2),
            _ => self.ports[&port].name.clone(),
        }
    }

    fn ios_find_interface(
        &self,
        device: DeviceId,
        value: &str,
    ) -> Result<(PortId, Option<u16>), String> {
        let value = value.replace(' ', "").to_ascii_lowercase();
        let (base, sub) = match value.split_once('.') {
            Some((base, sub)) => (
                base,
                Some(
                    sub.parse::<u16>()
                        .ok()
                        .filter(|n| *n > 0)
                        .ok_or("% Invalid subinterface number.")?,
                ),
            ),
            None => (value.as_str(), None),
        };
        for port in self.devices[&device].ports() {
            let full = self.ios_interface_name(device, *port).to_ascii_lowercase();
            let (suffix, aliases): (&str, &[&str]) =
                if let Some(suffix) = full.strip_prefix("tengigabitethernet") {
                    (suffix, &["te", "ten"])
                } else {
                    (
                        full.strip_prefix("gigabitethernet").unwrap_or(&full),
                        &["gi", "gig", "g"],
                    )
                };
            let matched = base == self.ports[port].name.to_ascii_lowercase()
                || base == full
                || aliases
                    .iter()
                    .any(|prefix| base.strip_prefix(prefix) == Some(suffix));
            if matched {
                if sub.is_some() && !matches!(self.devices[&device].kind, DeviceKind::Router(_)) {
                    return Err("% Subinterfaces are supported on routers only.".into());
                }
                return Ok((*port, sub));
            }
        }
        Err(format!(
            "% Unknown interface: {value}. Use show ip interface brief."
        ))
    }

    fn ios_interfaces(
        &self,
        device: DeviceId,
        value: &str,
        range: bool,
    ) -> Result<(Vec<PortId>, Option<u16>), String> {
        if !range {
            let (p, s) = self.ios_find_interface(device, value)?;
            return Ok((vec![p], s));
        }
        let compact = value.replace(' ', "");
        let (start, end) = compact
            .split_once('-')
            .ok_or("% Use interface range Gi1/0/1 - 4.")?;
        let (first, sub) = self.ios_find_interface(device, start)?;
        if sub.is_some() {
            return Err("% Subinterface ranges are unsupported.".into());
        }
        let end_name = if end.contains('/') {
            end.to_owned()
        } else {
            format!(
                "{}/{}",
                start
                    .rsplit_once('/')
                    .ok_or("% Use a GigabitEthernet interface range.")?
                    .0,
                end
            )
        };
        let (last, sub) = self.ios_find_interface(device, &end_name)?;
        if sub.is_some() {
            return Err("% Subinterface ranges are unsupported.".into());
        }
        let all = self.devices[&device].ports();
        let a = all.iter().position(|p| *p == first).unwrap();
        let b = all.iter().position(|p| *p == last).unwrap();
        if a > b {
            return Err("% Invalid descending interface range.".into());
        }
        Ok((all[a..=b].to_vec(), None))
    }

    fn ios_interface_command(
        &mut self,
        device: DeviceId,
        port: PortId,
        sub: Option<u16>,
        command: &str,
        args: &[String],
    ) -> Result<(), String> {
        if switching::INTERFACE.contains(&command) {
            return self.ios_switch_interface(device, port, command, args);
        }
        let name = format!(
            "{}{}",
            self.ios_interface_name(device, port),
            sub.map(|s| format!(".{s}")).unwrap_or_default()
        );
        match command {
            "description <text...>" => {
                self.ios_configs
                    .entry(device)
                    .or_default()
                    .descriptions
                    .insert(name, args[0].clone());
                return Ok(());
            }
            "no description" => {
                self.ios_configs
                    .entry(device)
                    .or_default()
                    .descriptions
                    .remove(&name);
                return Ok(());
            }
            "shutdown" | "no shutdown" => {
                self.ports.get_mut(&port).unwrap().enabled = command == "no shutdown";
                self.topology_revision += 1;
                return Ok(());
            }
            "speed 10" | "speed 100" | "speed 1000" | "speed 10000" | "speed 25000"
            | "speed auto" => {
                let speed = match command {
                    "speed 10" => LinkSpeed::Mbps10,
                    "speed 100" => LinkSpeed::Mbps100,
                    "speed 1000" => LinkSpeed::Gbps1,
                    "speed 10000" => LinkSpeed::Gbps10,
                    "speed 25000" => LinkSpeed::Gbps25,
                    "speed auto" => self.port(port).unwrap().max_speed,
                    _ => unreachable!(),
                };
                self.ios_execute(Command::SetPortSpeed { port, speed })?;
                return Ok(());
            }
            _ => {}
        }
        match self.ports[&port].config.clone() {
            PortConfig::Switch(config) => {
                let mode = match command {
                    "switchport mode access" => match config.mode {
                        mode @ SwitchPortMode::Access { .. } => mode,
                        _ => SwitchPortMode::Access { vlan: None },
                    },
                    "no switchport access vlan" => SwitchPortMode::Access { vlan: None },
                    "switchport access vlan <id>" => {
                        let vlan = vlan_id(&args[0])?;
                        if let DeviceKind::Switch(sw) = &self.devices[&device].kind
                            && !sw.vlans.iter().any(|v| v.id == vlan)
                        {
                            self.ios_execute(Command::CreateVlan {
                                switch: device,
                                vlan: Vlan {
                                    id: vlan,
                                    name: format!("VLAN{:04}", vlan.0),
                                },
                            })?;
                        }
                        SwitchPortMode::Access { vlan: Some(vlan) }
                    }
                    "switchport mode trunk" => match config.mode {
                        mode @ SwitchPortMode::Trunk { .. } => mode,
                        _ => SwitchPortMode::Trunk {
                            native_vlan: Some(VlanId(1)),
                            allowed: (1..4095).map(VlanId).collect(),
                        },
                    },
                    _ => {
                        let SwitchPortMode::Trunk {
                            mut native_vlan,
                            mut allowed,
                        } = config.mode
                        else {
                            return Err("% Configure switchport mode trunk first.".into());
                        };
                        match command {
                            "switchport trunk native vlan <id>" => {
                                native_vlan = Some(vlan_id(&args[0])?)
                            }
                            "switchport trunk allowed vlan <list>" => {
                                allowed = vlan_list(&args[0])?
                            }
                            "switchport trunk allowed vlan add <list>" => {
                                allowed.extend(vlan_list(&args[0])?);
                                allowed.sort_by_key(|v| v.0);
                                allowed.dedup();
                            }
                            "switchport trunk allowed vlan remove <list>" => {
                                let remove = vlan_list(&args[0])?;
                                allowed.retain(|v| !remove.contains(v));
                            }
                            _ => return Err("% Unsupported switchport command.".into()),
                        }
                        SwitchPortMode::Trunk {
                            native_vlan,
                            allowed,
                        }
                    }
                };
                self.ios_execute(Command::SetSwitchPortMode { port, mode })
            }
            PortConfig::Router(config) => {
                let old = config
                    .interfaces
                    .iter()
                    .find(|i| i.name == name || (sub.is_none() && !i.name.contains('.')))
                    .cloned();
                let mut interface = old.clone().unwrap_or(RouterInterface {
                    name: name.clone(),
                    port,
                    vlan: if sub.is_none() { Some(VlanId(1)) } else { None },
                    address: None,
                    prefix: 24,
                    dhcp: false,
                    internet_connected: false,
                });
                interface.name = name.clone();
                match command {
                    "encapsulation dot1q <id>" => interface.vlan = Some(vlan_id(&args[0])?),
                    "ip address <address> <mask>" => {
                        if sub.is_some() && interface.vlan.is_none() {
                            return Err(
                                "% Configure encapsulation dot1q before assigning an address."
                                    .into(),
                            );
                        }
                        interface.address = Some(
                            args[0]
                                .parse::<Ipv4Addr>()
                                .map_err(|_| "% Invalid IPv4 address.")?,
                        );
                        interface.prefix = mask_prefix(&args[1])?;
                    }
                    "no ip address" => interface.address = None,
                    _ => return Err("% Unsupported routed-interface command.".into()),
                }
                // Reject duplicate encapsulation rather than overwriting another subinterface.
                if config.interfaces.iter().any(|i| {
                    i.vlan == interface.vlan && old.as_ref().is_none_or(|o| i.name != o.name)
                }) {
                    return Err(
                        "% This VLAN is already configured on this physical interface.".into(),
                    );
                }
                if let Some(old) = old {
                    if let PortConfig::Router(c) = &mut self.ports.get_mut(&port).unwrap().config {
                        c.interfaces.retain(|i| i.name != old.name);
                    }
                    if let DeviceKind::Router(r) = &mut self.devices.get_mut(&device).unwrap().kind
                    {
                        r.interfaces
                            .retain(|i| !(i.port == port && i.name == old.name));
                    }
                }
                self.ios_execute(Command::ConfigureRouterInterface {
                    port,
                    name,
                    vlan: interface.vlan,
                    address: interface.address,
                    prefix: interface.prefix,
                    internet_connected: interface.internet_connected,
                })
            }
            _ => Err("% Not a network device interface.".into()),
        }
    }

    fn ios_show(
        &self,
        device: DeviceId,
        command: &str,
        args: &[String],
    ) -> Result<Vec<String>, String> {
        if switching::SHOW.contains(&command) {
            return self.ios_switch_show(device, command, args);
        }
        let dev = &self.devices[&device];
        if command == "show interfaces <interface>"
            && let Some(interface) = args.first()
            && let Some(group) = switching::channel_number(interface)
        {
            let members = self.channel_ports(device, group);
            if members.is_empty() {
                return Err("% Port-channel does not exist.".into());
            }
            let capacity = self.channel_capacity_mbps(device, group);
            return Ok(vec![
                format!(
                    "Port-channel{group} is {}, line protocol is {}",
                    if capacity > 0 { "up" } else { "down" },
                    if capacity > 0 { "up" } else { "down" }
                ),
                format!("BW {capacity} Mbps; each flow uses one physical member"),
                format!(
                    "Members: {}",
                    members
                        .iter()
                        .map(|p| self.ios_interface_name(device, *p))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ]);
        }
        match command {
            "show inventory"
            | "show interfaces transceiver"
            | "show interfaces <interface> transceiver" => {
                let ports = if let Some(interface) = args.first() {
                    let (port, subinterface) = self.ios_find_interface(device, interface)?;
                    if subinterface.is_some() {
                        return Err("% Transceivers belong to physical interfaces.".into());
                    }
                    vec![port]
                } else {
                    dev.ports()
                        .iter()
                        .copied()
                        .filter(|port| self.cage_profile(*port).is_some())
                        .collect()
                };
                let mut lines = Vec::new();
                if command == "show inventory"
                    && let DeviceKind::Switch(switch) = &dev.kind
                {
                    lines.push(format!(
                        "NAME: Chassis, PID: {}, 24 RJ45 / 4 uplinks, non-PoE, fanless",
                        switch.model.spec().name
                    ));
                }
                lines.extend(
                    ports
                        .into_iter()
                        .flat_map(|port| self.transceiver_report(port)),
                );
                Ok(lines)
            }
            "show version" => {
                let mut lines = vec![
                    "IOS-style network simulator (not Cisco IOS firmware)".into(),
                    format!("Hardware: {}", dev.name),
                    "Supported commands: ?   Configuration guide: docs/IOS_GUIDE.md".into(),
                ];
                if let DeviceKind::Switch(switch) = &dev.kind {
                    let spec = switch.model.spec();
                    lines.push(format!(
                        "ARM v7 {} MHz; {} MB DRAM; {} MB flash",
                        spec.cpu_mhz, spec.dram_mb, spec.flash_mb
                    ));
                    lines.push(format!(
                        "Switching capacity: {} Mbps; forwarding rating: {} kpps",
                        spec.switching_mbps, spec.forwarding_kpps
                    ));
                    lines.push(format!("Configured idle/full-traffic power: {:.2}/{:.2} W; simulation rounds up and adds transceiver draw",f64::from(spec.power.idle_mw)/1000.0,f64::from(spec.power.peak_mw)/1000.0));
                }
                Ok(lines)
            }
            "show running-config" => Ok(self.ios_running_config(device)),
            "show startup-config" => {
                let saved = self
                    .startup_configs
                    .get(&device)
                    .ok_or("% Startup configuration is not present.")?;
                let mut copy = self.clone();
                copy.devices.get_mut(&device).unwrap().kind = saved.kind.clone();
                for p in &saved.ports {
                    copy.ports.insert(p.id, p.clone());
                }
                if let Some(network) = &saved.network {
                    copy.restore_device_network(device, network.clone());
                }
                copy.ios_configs.insert(device, saved.metadata.clone());
                Ok(copy.ios_running_config(device))
            }
            "show vlan brief" => {
                let DeviceKind::Switch(sw) = &dev.kind else {
                    return Err("% VLAN database is available on switches only.".into());
                };
                let mut lines =
                    vec!["VLAN  Name                             Status   Ports".into()];
                let mut vlans = sw.vlans.clone();
                vlans.sort_by_key(|v| v.id.0);
                for vlan in vlans {
                    let ports = sw.ports.iter().filter(|p| matches!(&self.ports[p].config, PortConfig::Switch(c) if c.mode == SwitchPortMode::Access { vlan: Some(vlan.id) })).map(|p| self.ios_interface_name(device, *p)).collect::<Vec<_>>().join(", ");
                    lines.push(format!(
                        "{:<5} {:<32} active   {ports}",
                        vlan.id.0, vlan.name
                    ));
                }
                Ok(lines)
            }
            "show arp" | "show ip arp" => {
                let mut entries: Vec<_> = self
                    .runtime
                    .arp
                    .iter()
                    .filter(|((port, _, _), _)| dev.ports().contains(port))
                    .collect();
                entries.sort_by_key(|((port, address, vlan), _)| (*port, *vlan, *address));
                let mut lines = vec![
                    "Address          Hardware address    Interface                 VLAN".into(),
                ];
                lines.extend(entries.into_iter().map(|((port, address, vlan), mac)| {
                    format!(
                        "{address:<16} {mac}   {:<25} {}",
                        self.ios_interface_name(device, *port),
                        vlan.0
                    )
                }));
                Ok(lines)
            }
            "show ip route" => {
                let DeviceKind::Router(router) = &dev.kind else {
                    return Err("% Layer 3 routing is not implemented on this switch.".into());
                };
                let mut lines = vec![
                    "Codes: C - connected, S - static; configured routes on this device".into(),
                ];
                for iface in &router.interfaces {
                    if !self.ports[&iface.port].enabled {
                        continue;
                    }
                    if let Some(ip) = iface.address {
                        let network =
                            Ipv4Addr::from(u32::from(ip) & u32::from(prefix_mask(iface.prefix)));
                        lines.push(format!(
                            "C {network}/{} directly connected, {}",
                            iface.prefix, iface.name
                        ));
                    }
                    if iface.internet_connected {
                        lines.push(format!(
                            "Legacy WAN designation (does not install a route): {}",
                            self.ios_interface_name(device, iface.port)
                        ));
                    }
                }
                for route in &router.routes {
                    lines.push(format!(
                        "S {}/{} via {}, {}",
                        route.network,
                        route.prefix,
                        route
                            .via
                            .map_or_else(|| "on-link".into(), |ip| ip.to_string()),
                        self.ios_interface_name(device, route.egress)
                    ));
                }
                for route in &router.domain_routes {
                    lines.push(format!(
                        "S {} [{}] via {}, {}, VLAN {}, domain {}{}",
                        route.prefix,
                        route.preference,
                        route
                            .next_hop
                            .map_or_else(|| "on-link".into(), |ip| ip.to_string()),
                        self.ios_interface_name(device, route.port),
                        route.vlan.0,
                        route.domain.0,
                        if route.track_neighbor {
                            ", tracked"
                        } else {
                            ""
                        }
                    ));
                }
                Ok(lines)
            }
            "show cdp neighbors" => {
                let mut lines = vec![
                    "Device ID                  Local Interface          Remote Interface".into(),
                ];
                for port in dev.ports() {
                    let Some(other) = self.physical_path(*port).into_iter().find(|id| {
                        *id != *port
                            && self.port(*id).is_some_and(|p| {
                                !matches!(
                                    p.config,
                                    PortConfig::PatchPanel | PortConfig::CableManager
                                )
                            })
                    }) else {
                        continue;
                    };
                    let remote = &self.ports[&other];
                    let Some(owner) = self.devices.get(&remote.device).filter(|owner| {
                        matches!(owner.kind, DeviceKind::Switch(_) | DeviceKind::Router(_))
                    }) else {
                        continue;
                    };
                    let name = self
                        .ios_configs
                        .get(&remote.device)
                        .and_then(|c| c.hostname.as_deref())
                        .unwrap_or(&owner.name);
                    lines.push(format!(
                        "{name:<26} {}  {}",
                        self.ios_interface_name(device, *port),
                        self.ios_interface_name(remote.device, other)
                    ));
                }
                Ok(lines)
            }
            _ => {
                let ports = if args.is_empty() {
                    dev.ports().to_vec()
                } else {
                    let (port, sub) = self.ios_find_interface(device, &args[0])?;
                    if sub.is_some() {
                        return Err("% Use show ip interface brief for subinterfaces.".into());
                    }
                    vec![port]
                };
                let mut lines = vec!["Interface                      IP-Address       Status                   Speed / Configuration".into()];
                for id in ports {
                    let port = &self.ports[&id];
                    let up = self.port_link_up(id);
                    let status = if !port.enabled {
                        "administratively down"
                    } else if up {
                        "up"
                    } else {
                        "down"
                    };
                    let speed = self
                        .port_link_speed(id)
                        .unwrap_or(port.advertised_speed)
                        .mbps();
                    let name = self.ios_interface_name(device, id);
                    match &port.config {
                        PortConfig::Switch(c) => {
                            if command == "show interfaces trunk"
                                && !matches!(c.mode, SwitchPortMode::Trunk { .. })
                            {
                                continue;
                            }
                            lines.push(format!(
                                "{name:<30} unassigned       {status:<24} {speed:>4}Mbps {}",
                                switch_mode_text(&c.mode),
                            ));
                        }
                        PortConfig::Router(c) => {
                            if command == "show interfaces trunk" || command.ends_with("switchport")
                            {
                                return Err("% This is a routed interface in the simulator.".into());
                            }
                            if c.interfaces.is_empty() {
                                lines.push(format!(
                                    "{name:<30} unassigned       {status:<24} {speed:>4}Mbps"
                                ));
                            }
                            for iface in &c.interfaces {
                                lines.push(format!(
                                    "{:<30} {:<16} {status:<24} {speed:>4}Mbps {}",
                                    if iface.name.contains('.') {
                                        iface.name.clone()
                                    } else {
                                        name.clone()
                                    },
                                    iface
                                        .address
                                        .map(|a| a.to_string())
                                        .unwrap_or("unassigned".into()),
                                    if up { "up" } else { "down" }
                                ));
                            }
                        }
                        _ => {}
                    }
                    if command == "show interfaces <interface>"
                        && let Some(description) = self
                            .ios_configs
                            .get(&device)
                            .and_then(|c| c.descriptions.get(&name))
                    {
                        lines.push(format!("  Description: {description}"));
                    }
                }
                Ok(lines)
            }
        }
    }

    fn ios_running_config(&self, device: DeviceId) -> Vec<String> {
        let dev = &self.devices[&device];
        let meta = self.ios_configs.get(&device).cloned().unwrap_or_default();
        let mut lines = vec![
            "! Simulator running configuration".into(),
            format!(
                "hostname {}",
                meta.hostname.unwrap_or(
                    if matches!(dev.kind, DeviceKind::Switch(_)) {
                        "Switch"
                    } else {
                        "Router"
                    }
                    .into()
                )
            ),
        ];
        if let Some(address) = self.console_management_ip(device) {
            lines.push(format!("management ip {address}"));
        }
        lines.extend(self.ios_switch_config(device));
        if let DeviceKind::Switch(sw) = &dev.kind {
            let mut vlans = sw.vlans.clone();
            vlans.sort_by_key(|v| v.id.0);
            for v in vlans {
                lines.extend([
                    format!("vlan {}", v.id.0),
                    format!(" name {}", v.name),
                    " exit".into(),
                ]);
            }
        }
        for id in dev.ports() {
            let port = &self.ports[id];
            let name = self.ios_interface_name(device, *id);
            lines.push(format!("interface {name}"));
            if let Some(description) = meta.descriptions.get(&name) {
                lines.push(format!(" description {description}"));
            }
            lines.push(
                if port.enabled {
                    " no shutdown"
                } else {
                    " shutdown"
                }
                .into(),
            );
            lines.push(format!(" speed {}", port.advertised_speed.mbps()));
            match &port.config {
                PortConfig::Switch(c) => match &c.mode {
                    SwitchPortMode::Access { vlan } => lines.extend([
                        " switchport mode access".into(),
                        vlan.map(|v| format!(" switchport access vlan {}", v.0))
                            .unwrap_or_else(|| " no switchport access vlan".into()),
                    ]),
                    SwitchPortMode::Trunk {
                        native_vlan,
                        allowed,
                    } => {
                        lines.push(" switchport mode trunk".into());
                        if let Some(vlan) = native_vlan {
                            lines.push(format!(" switchport trunk native vlan {}", vlan.0));
                        }
                        lines.push(format!(
                            " switchport trunk allowed vlan {}",
                            format_vlan_list(allowed)
                        ));
                    }
                },
                PortConfig::Router(c) => {
                    for iface in c.interfaces.iter().filter(|i| !i.name.contains('.')) {
                        if let Some(ip) = iface.address {
                            lines.push(format!(" ip address {ip} {}", prefix_mask(iface.prefix)));
                        }
                        if iface.internet_connected {
                            lines.push(
                                " ! Legacy WAN designation; configure explicit transit routes"
                                    .into(),
                            );
                        }
                    }
                }
                _ => {}
            }
            lines.extend(self.ios_switch_port_config(device, *id));
            lines.push(" exit".into());
            if let PortConfig::Router(c) = &port.config {
                for iface in c.interfaces.iter().filter(|i| i.name.contains('.')) {
                    lines.push(format!("interface {}", iface.name));
                    if let Some(description) = meta.descriptions.get(&iface.name) {
                        lines.push(format!(" description {description}"));
                    }
                    if let Some(vlan) = iface.vlan {
                        lines.push(format!(" encapsulation dot1q {}", vlan.0));
                    }
                    if let Some(ip) = iface.address {
                        lines.push(format!(" ip address {ip} {}", prefix_mask(iface.prefix)));
                    }
                    lines.push(" exit".into());
                }
            }
        }
        lines.extend(self.ios_route_config(device));
        lines.extend(self.ios_channel_config(device));
        lines.push("end".into());
        lines
    }
}

fn vlan_id(value: &str) -> Result<VlanId, String> {
    value
        .parse::<u16>()
        .ok()
        .filter(|v| (1..4095).contains(v))
        .map(VlanId)
        .ok_or("% VLAN must be between 1 and 4094.".into())
}
fn vlan_list(value: &str) -> Result<Vec<VlanId>, String> {
    if value.eq_ignore_ascii_case("all") {
        return Ok((1..4095).map(VlanId).collect());
    }
    if value.eq_ignore_ascii_case("none") {
        return Ok(vec![]);
    }
    let mut vlans = vec![];
    for part in value.split(',') {
        if let Some((a, b)) = part.split_once('-') {
            let (a, b) = (vlan_id(a)?.0, vlan_id(b)?.0);
            if a > b {
                return Err("% Invalid descending VLAN range.".into());
            }
            vlans.extend((a..=b).map(VlanId));
        } else {
            vlans.push(vlan_id(part)?);
        }
    }
    vlans.sort_by_key(|v| v.0);
    vlans.dedup();
    Ok(vlans)
}
fn format_vlan_list(vlans: &[VlanId]) -> String {
    if vlans.is_empty() {
        return "none".into();
    }
    let mut values: Vec<_> = vlans.iter().map(|v| v.0).collect();
    values.sort();
    values.dedup();
    let mut groups = vec![];
    let mut start = values[0];
    let mut end = start;
    for value in values.into_iter().skip(1) {
        if value == end + 1 {
            end = value;
        } else {
            groups.push(if start == end {
                start.to_string()
            } else {
                format!("{start}-{end}")
            });
            start = value;
            end = value;
        }
    }
    groups.push(if start == end {
        start.to_string()
    } else {
        format!("{start}-{end}")
    });
    groups.join(",")
}
fn switch_mode_text(mode: &SwitchPortMode) -> String {
    match mode {
        SwitchPortMode::Access { vlan } => vlan
            .map(|v| format!("access VLAN {}", v.0))
            .unwrap_or_else(|| "access VLAN 1 (untagged)".into()),
        SwitchPortMode::Trunk {
            native_vlan,
            allowed,
        } => format!(
            "trunk native {} allowed {}",
            native_vlan.map(|v| v.0).unwrap_or(1),
            format_vlan_list(allowed)
        ),
    }
}
fn mask_prefix(value: &str) -> Result<u8, String> {
    let mask = u32::from(
        value
            .parse::<Ipv4Addr>()
            .map_err(|_| "% Invalid subnet mask.")?,
    );
    let prefix = mask.leading_ones() as u8;
    if u32::from(prefix_mask(prefix)) != mask {
        return Err("% Subnet mask must be contiguous.".into());
    }
    Ok(prefix)
}
fn prefix_mask(prefix: u8) -> Ipv4Addr {
    Ipv4Addr::from(if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    })
}
fn reply(success: bool, text: impl Into<String>) -> TerminalOutput {
    TerminalOutput {
        lines: vec![text.into()],
        success,
    }
}
