use super::{GuestFilesystem, LinuxShell, network::LinuxNetwork};
use crate::*;

pub(super) struct LinuxServices;

impl LinuxServices {
    pub fn execute(
        sim: &mut NetworkSim,
        device: DeviceId,
        command: &str,
        args: &[String],
    ) -> Result<Vec<String>, String> {
        match command {
            "ifup" | "ifdown" => {
                let name = args.first().ok_or("usage: ifup|ifdown IFACE|-a")?;
                if command == "ifdown" {
                    if name == "-a" {
                        let names: Vec<_> = sim
                            .device(device)
                            .unwrap()
                            .ports()
                            .iter()
                            .filter_map(|id| sim.port(*id).map(|port| port.name.clone()))
                            .collect();
                        for name in names {
                            Self::down(sim, device, &name)?;
                        }
                    } else {
                        Self::down(sim, device, name)?;
                    }
                } else {
                    Self::configure_network(
                        sim,
                        device,
                        if name == "-a" { None } else { Some(name) },
                    )?;
                }
                Ok(Vec::new())
            }
            "reboot" => {
                let mut candidate = sim.clone();
                let ids = candidate.device(device).unwrap().ports().to_vec();
                for id in ids {
                    if let PortConfig::Server(config) =
                        &mut candidate.ports.get_mut(&id).unwrap().config
                    {
                        config.ipv4 = None;
                        config.additional_ipv4.clear();
                    }
                }
                candidate.guest_mut(device).routes.clear();
                candidate.guest_mut(device).loopback_up = true;
                for service in candidate.guest_mut(device).services.values_mut() {
                    service.active = service.enabled;
                    service.log.push("Guest rebooted".into());
                }
                if candidate
                    .guest_mut(device)
                    .services
                    .get("networking")
                    .is_some_and(|service| service.enabled)
                {
                    Self::configure_network(&mut candidate, device, None)?;
                }
                candidate.topology_revision += 1;
                *sim = candidate;
                Ok(vec![
                    "Guest rebooted; persistent network configuration applied".into(),
                ])
            }
            "shutdown" | "poweroff" => {
                sim.execute(Command::SetPower {
                    device,
                    powered: false,
                })
                .map_err(|error| error.to_string())?;
                Ok(vec!["System halted".into()])
            }
            "journalctl" => {
                let name = args
                    .windows(2)
                    .find(|pair| pair[0] == "-u")
                    .map(|pair| Self::unit(&pair[1]));
                let services = &sim.guest_mut(device).services;
                if let Some(name) = name {
                    return Ok(services.get(name).ok_or("No such unit")?.log.clone());
                }
                Ok(services
                    .iter()
                    .flat_map(|(name, service)| {
                        service
                            .log
                            .iter()
                            .map(move |line| format!("{name}.service: {line}"))
                    })
                    .collect())
            }
            "service" => {
                if args.len() != 2 {
                    return Err("usage: service NAME ACTION".into());
                }
                Self::systemctl(sim, device, &[args[1].clone(), args[0].clone()])
            }
            _ => Self::systemctl(sim, device, args),
        }
    }

    fn unit(name: &str) -> &str {
        let name = name.strip_suffix(".service").unwrap_or(name);
        if name == "sshd" { "ssh" } else { name }
    }

    fn systemctl(
        sim: &mut NetworkSim,
        device: DeviceId,
        args: &[String],
    ) -> Result<Vec<String>, String> {
        let now = args.iter().any(|arg| arg == "--now");
        let quiet = args.iter().any(|arg| arg == "--quiet" || arg == "-q");
        let args: Vec<_> = args
            .iter()
            .filter(|arg| !arg.starts_with('-'))
            .map(String::as_str)
            .collect();
        let action = args.first().copied().unwrap_or("status");
        if matches!(action, "list-units" | "list-unit-files" | "status") && args.len() == 1 {
            return Ok(sim
                .guest_mut(device)
                .services
                .iter()
                .map(|(name, service)| {
                    format!(
                        "{name}.service loaded {} {}",
                        if service.active { "active" } else { "inactive" },
                        if service.enabled {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    )
                })
                .collect());
        }
        let name = Self::unit(args.get(1).ok_or("systemctl: missing unit")?);
        if !sim.guest_mut(device).services.contains_key(name) {
            return Err(format!("Unit {name}.service could not be found"));
        }
        if (matches!(action, "start" | "restart" | "reload") || (action == "enable" && now))
            && name == "networking"
        {
            Self::configure_network(sim, device, None)?;
        }
        let service = sim.guest_mut(device).services.get_mut(name).unwrap();
        match action {
            "status" => Ok(vec![
                format!("● {name}.service"),
                format!(
                    "   Loaded: loaded ({})",
                    if service.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ),
                format!(
                    "   Active: {}",
                    if service.active {
                        "active (running)"
                    } else {
                        "inactive (dead)"
                    }
                ),
            ]),
            "is-active" => {
                if service.active {
                    Ok(if quiet {
                        Vec::new()
                    } else {
                        vec!["active".into()]
                    })
                } else {
                    Err(if quiet {
                        String::new()
                    } else {
                        "inactive".into()
                    })
                }
            }
            "is-enabled" => {
                if service.enabled {
                    Ok(if quiet {
                        Vec::new()
                    } else {
                        vec!["enabled".into()]
                    })
                } else {
                    Err(if quiet {
                        String::new()
                    } else {
                        "disabled".into()
                    })
                }
            }
            "start" | "restart" | "reload" => {
                service.active = true;
                service.log.push(format!("{action} {name}.service"));
                Ok(Vec::new())
            }
            "stop" => {
                service.active = false;
                service.log.push(format!("Stopped {name}.service"));
                Ok(Vec::new())
            }
            "enable" => {
                service.enabled = true;
                if now {
                    service.active = true;
                }
                Ok(vec![format!("Enabled {name}.service")])
            }
            "disable" => {
                service.enabled = false;
                if now {
                    service.active = false;
                }
                Ok(vec![format!("Disabled {name}.service")])
            }
            _ => Err(format!("systemctl: unknown operation {action}")),
        }
    }

    fn down(sim: &mut NetworkSim, device: DeviceId, name: &str) -> Result<(), String> {
        let port = LinuxNetwork::interface(sim, device, name)?;
        if let PortConfig::Server(config) = &mut sim.ports.get_mut(&port).unwrap().config {
            config.ipv4 = None;
            config.additional_ipv4.clear();
        }
        sim.guest_mut(device)
            .routes
            .retain(|route| route.port != port);
        sim.execute(Command::SetPortEnabled {
            port,
            enabled: false,
        })
        .map_err(|e| e.to_string())?;
        sim.topology_revision += 1;
        Ok(())
    }

    pub fn configure_network(
        sim: &mut NetworkSim,
        device: DeviceId,
        selected: Option<&str>,
    ) -> Result<(), String> {
        let contents = sim
            .guest_mut(device)
            .filesystem
            .read("/etc/network/interfaces")?;
        let stanzas = InterfaceFile::parse(&contents)?;
        if selected.is_some_and(|name| !stanzas.iter().any(|stanza| stanza.name == name)) {
            return Err("ifup: interface is not configured in /etc/network/interfaces".into());
        }
        let mut candidate = sim.clone();
        for stanza in stanzas
            .iter()
            .filter(|stanza| selected.map_or(stanza.auto, |name| stanza.name == name))
        {
            if stanza.name == "lo" {
                candidate.guest_mut(device).loopback_up = true;
                continue;
            }
            let port = LinuxNetwork::interface(&candidate, device, &stanza.name)?;
            candidate
                .execute(Command::SetPortEnabled {
                    port,
                    enabled: true,
                })
                .map_err(|e| e.to_string())?;
            match stanza.mode.as_str() {
                "static" => {
                    let address = stanza
                        .address
                        .as_deref()
                        .ok_or("static interface needs an address")?;
                    let cidr = if address.contains('/') {
                        address.into()
                    } else {
                        format!(
                            "{address}/{}",
                            stanza
                                .prefix
                                .ok_or("static address needs a prefix or netmask")?
                        )
                    };
                    LinuxNetwork::cidr(&cidr)?;
                    Self::down_addresses(&mut candidate, port);
                    candidate
                        .guest_mut(device)
                        .routes
                        .retain(|route| route.port != port);
                    let output = LinuxShell::execute(
                        &mut candidate,
                        device,
                        &format!("ip addr add {cidr} dev {}", stanza.name),
                    );
                    if !output.success {
                        return Err(output.lines.join("\n"));
                    }
                    if let Some(gateway) = &stanza.gateway {
                        let output = LinuxShell::execute(
                            &mut candidate,
                            device,
                            &format!("ip route replace default via {gateway} dev {}", stanza.name),
                        );
                        if !output.success {
                            return Err(output.lines.join("\n"));
                        }
                    }
                }
                "dhcp" => {
                    Self::down_addresses(&mut candidate, port);
                    if !candidate.network_reaches(port, false) {
                        return Err(format!(
                            "{}: no DHCP lease available on the room LAN",
                            stanza.name
                        ));
                    }
                    candidate.assign_lan_ipv4(port).map_err(|e| e.to_string())?;
                }
                "manual" => {}
                _ => return Err(format!("unsupported interface mode {}", stanza.mode)),
            }
            for command in &stanza.post_up {
                let output = LinuxShell::execute(&mut candidate, device, command);
                if !output.success {
                    return Err(output.lines.join("\n"));
                }
            }
            if let Some(resolvers) = &stanza.resolvers {
                let contents = resolvers
                    .split_whitespace()
                    .map(|resolver| format!("nameserver {resolver}\n"))
                    .collect::<String>();
                candidate.guest_mut(device).filesystem.write(
                    "/etc/resolv.conf",
                    &contents,
                    false,
                )?;
            }
        }
        candidate.topology_revision += 1;
        *sim = candidate;
        Ok(())
    }

    fn down_addresses(sim: &mut NetworkSim, port: PortId) {
        if let PortConfig::Server(config) = &mut sim.ports.get_mut(&port).unwrap().config {
            config.ipv4 = None;
            config.additional_ipv4.clear();
        }
    }

    pub fn script(
        sim: &mut NetworkSim,
        device: DeviceId,
        path: &str,
    ) -> Result<Vec<String>, String> {
        let cwd = sim.guest_mut(device).cwd.clone();
        let contents = sim
            .guest_mut(device)
            .filesystem
            .read(&GuestFilesystem::normalize(&cwd, path))?;
        let mut lines = Vec::new();
        for command in contents
            .lines()
            .filter(|line| !line.trim().starts_with('#'))
        {
            let output = LinuxShell::execute(sim, device, command);
            lines.extend(output.lines);
            if !output.success {
                return Err(lines.join("\n"));
            }
        }
        Ok(lines)
    }
}

struct InterfaceFile {
    name: String,
    mode: String,
    auto: bool,
    address: Option<String>,
    prefix: Option<u8>,
    gateway: Option<String>,
    resolvers: Option<String>,
    post_up: Vec<String>,
}

impl InterfaceFile {
    fn parse(contents: &str) -> Result<Vec<Self>, String> {
        let mut automatic = Vec::new();
        let mut stanzas: Vec<Self> = Vec::new();
        for (number, line) in contents.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            let words: Vec<_> = line.split_whitespace().collect();
            match words.as_slice() {
                [] => {}
                ["auto" | "allow-hotplug", names @ ..] => {
                    automatic.extend(names.iter().map(|name| name.to_string()))
                }
                ["iface", name, "inet", mode] => stanzas.push(Self {
                    name: name.to_string(),
                    mode: mode.to_string(),
                    auto: false,
                    address: None,
                    prefix: None,
                    gateway: None,
                    resolvers: None,
                    post_up: Vec::new(),
                }),
                [key, values @ ..] => {
                    let stanza = stanzas.last_mut().ok_or_else(|| {
                        format!(
                            "interfaces:{}: directive outside an iface stanza",
                            number + 1
                        )
                    })?;
                    match (*key, values) {
                        ("address", [value]) => stanza.address = Some(value.to_string()),
                        ("gateway", [value]) => {
                            value
                                .parse::<std::net::Ipv4Addr>()
                                .map_err(|_| "invalid gateway")?;
                            stanza.gateway = Some(value.to_string());
                        }
                        ("netmask", [value]) => {
                            let mask: std::net::Ipv4Addr =
                                value.parse().map_err(|_| "invalid netmask")?;
                            let bits = u32::from(mask);
                            let prefix = bits.count_ones() as u8;
                            if LinuxNetwork::network(std::net::Ipv4Addr::BROADCAST, prefix) != mask
                            {
                                return Err("non-contiguous netmask".into());
                            }
                            stanza.prefix = Some(prefix);
                        }
                        ("post-up" | "up", command) if !command.is_empty() => {
                            stanza.post_up.push(command.join(" "))
                        }
                        ("dns-nameservers", resolvers) => {
                            for resolver in resolvers {
                                resolver
                                    .parse::<std::net::Ipv4Addr>()
                                    .map_err(|_| "invalid DNS resolver")?;
                            }
                            stanza.resolvers = Some(resolvers.join(" "));
                        }
                        _ => {
                            return Err(format!(
                                "interfaces:{}: unsupported directive {key}",
                                number + 1
                            ));
                        }
                    }
                }
            }
        }
        for stanza in &mut stanzas {
            stanza.auto = automatic.contains(&stanza.name);
        }
        Ok(stanzas)
    }
}
