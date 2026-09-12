//! IOS-style console for the simulator. No Cisco firmware is executed.
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IosDeviceConfig {
    pub hostname: Option<String>,
    pub descriptions: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IosStartupConfig {
    kind: DeviceKind,
    ports: Vec<Port>,
    metadata: IosDeviceConfig,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum IosMode {
    #[default]
    User,
    Privileged,
    Global,
    Vlan(VlanId),
    Interface {
        ports: Vec<PortId>,
        subinterface: Option<u16>,
    },
    ReloadConfirm,
}

const SHOW: &[&str] = &[
    "show version",
    "show interfaces",
    "show interfaces status",
    "show interfaces <interface>",
    "show interfaces <interface> switchport",
    "show ip interface brief",
    "show ip route",
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
        IosMode::Global | IosMode::Vlan(_) | IosMode::Interface { .. } => {
            commands.extend(["end", "do <command...>"]);
            if matches!(mode, IosMode::Global) {
                commands.extend([
                    "hostname <name>",
                    "no hostname",
                    "interface <interface...>",
                    "interface range <range...>",
                ]);
                if switch {
                    commands.extend(["vlan <id>", "no vlan <id>"]);
                }
            }
            if matches!(mode, IosMode::Vlan(_)) {
                commands.push("name <name>");
            }
            if let IosMode::Interface { subinterface, .. } = mode {
                commands.extend(["description <text...>", "no description"]);
                if subinterface.is_none() {
                    commands.extend(["shutdown", "no shutdown"]);
                }
                if switch {
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
                    if subinterface.is_some() {
                        commands.push("encapsulation dot1q <id>");
                    }
                }
                commands.extend(["speed 10", "speed 100", "speed 1000", "speed auto"]);
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
    pub fn terminal_prompt(&self, device: DeviceId) -> String {
        let Some(dev) = self.device(device) else {
            return "?>".into();
        };
        if let DeviceKind::Server(server) = &dev.kind {
            return format!("{}$", server.hostname);
        }
        if !matches!(dev.kind, DeviceKind::Switch(_) | DeviceKind::Router(_)) {
            return "no-console".into();
        }
        let name = self
            .ios_configs
            .get(&device)
            .and_then(|c| c.hostname.as_deref())
            .unwrap_or(if matches!(dev.kind, DeviceKind::Switch(_)) {
                "Switch"
            } else {
                "Router"
            });
        let suffix = match self.console_modes.get(&device).unwrap_or(&IosMode::User) {
            IosMode::User => ">",
            IosMode::Privileged => "#",
            IosMode::Global => "(config)#",
            IosMode::Vlan(_) => "(config-vlan)#",
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
            return match parse_terminal_command(input) {
                Ok(command) => self.execute_terminal(device, command),
                Err(error) => reply(false, error),
            };
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
                let saved = self
                    .startup_configs
                    .get(&device)
                    .cloned()
                    .ok_or("% No startup configuration. Save with write memory first.")?;
                self.devices.get_mut(&device).unwrap().kind = saved.kind;
                for port in saved.ports {
                    self.ports.insert(port.id, port);
                }
                self.ios_configs.insert(device, saved.metadata);
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
                    IosMode::Interface { .. } | IosMode::Vlan(_) => IosMode::Global,
                    IosMode::Global => IosMode::Privileged,
                    _ => IosMode::User,
                }
            }
            "do <command...>" => {
                let mut exec = IosMode::Privileged;
                if !args[0]
                    .split_whitespace()
                    .next()
                    .is_some_and(|w| "show".starts_with(&w.to_ascii_lowercase()))
                {
                    return Err("% Only do show is supported in configuration mode.".into());
                }
                return self.ios_command(device, &mut exec, &args[0]);
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
                let (ports, subinterface) = self.ios_interfaces(device, &args[0], range)?;
                *mode = IosMode::Interface {
                    ports,
                    subinterface,
                };
            }
            _ => {
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
            DeviceKind::Switch(_) => format!("GigabitEthernet1/0/{}", index + 1),
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
            let suffix = full.strip_prefix("gigabitethernet").unwrap_or(&full);
            let matched = base == self.ports[port].name.to_ascii_lowercase()
                || base == full
                || ["gi", "gig", "g"]
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
            "speed 10" | "speed 100" | "speed 1000" | "speed auto" => {
                let speed = match command {
                    "speed 10" => LinkSpeed::Mbps10,
                    "speed 100" => LinkSpeed::Mbps100,
                    "speed 1000" | "speed auto" => LinkSpeed::Gbps1,
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
                    .find(|i| {
                        i.name == name
                            || (sub.is_none() && !i.name.contains('.') && i.vlan == Some(VlanId(1)))
                    })
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
        let dev = &self.devices[&device];
        match command {
            "show version" => Ok(vec![
                "IOS-style network simulator (not Cisco IOS firmware)".into(),
                format!("Hardware: {}", dev.name),
                "Supported commands: ?   Configuration guide: docs/IOS_GUIDE.md".into(),
            ]),
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
            "show ip route" => {
                let DeviceKind::Router(router) = &dev.kind else {
                    return Err("% Layer 3 routing is not implemented on this switch.".into());
                };
                let mut lines =
                    vec!["Codes: C - connected; simulated internet uplink shown separately".into()];
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
                            "Simulated internet uplink: {}",
                            self.ios_interface_name(device, iface.port)
                        ));
                    }
                }
                Ok(lines)
            }
            "show cdp neighbors" => {
                let mut lines = vec![
                    "Device ID                  Local Interface          Remote Interface".into(),
                ];
                for port in dev.ports() {
                    if let Some(other) = self
                        .link_for_port(*port)
                        .filter(|l| l.enabled)
                        .and_then(|l| l.other(*port))
                    {
                        let remote = &self.ports[&other];
                        let owner = &self.devices[&remote.device];
                        if !remote.enabled
                            || !self.ports[port].enabled
                            || !owner.powered
                            || owner.rack.is_none()
                            || dev.rack.is_none()
                            || matches!(owner.kind, DeviceKind::Server(_))
                        {
                            continue;
                        }
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
                            lines.push(" ! Simulated internet uplink enabled in Inspector".into());
                        }
                    }
                }
                _ => {}
            }
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
