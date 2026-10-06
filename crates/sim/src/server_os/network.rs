use crate::*;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinuxRoute {
    pub network: Ipv4Addr,
    pub prefix: u8,
    pub via: Option<Ipv4Addr>,
    pub port: PortId,
    pub metric: u32,
}

#[derive(Debug, Clone)]
pub struct RouteSelection {
    pub port: PortId,
    pub source: Ipv4InterfaceConfig,
    pub next_hop: Ipv4Addr,
    pub prefix: u8,
    pub metric: u32,
}

impl NetworkSim {
    pub fn server_os(&self, device: DeviceId) -> Option<&ServerOs> {
        self.server_operating_systems.get(&device)
    }

    pub(crate) fn guest_mut(&mut self, device: DeviceId) -> &mut ServerOs {
        self.server_operating_systems.entry(device).or_default()
    }

    pub fn server_route_selection(
        &self,
        device: DeviceId,
        destination: Ipv4Addr,
        selected: Option<PortId>,
    ) -> Option<RouteSelection> {
        self.server_route_in_domain(device, destination, selected, None)
    }

    pub(crate) fn server_route_in_domain(
        &self,
        device: DeviceId,
        destination: Ipv4Addr,
        selected: Option<PortId>,
        domain: Option<RoutingDomain>,
    ) -> Option<RouteSelection> {
        let ports = self.device(device)?.ports();
        let mut candidates = Vec::new();
        for id in ports {
            let port = self.port(*id)?;
            if !port.enabled || selected.is_some_and(|selected| selected != *id) {
                continue;
            }
            let PortConfig::Server(config) = &port.config else {
                continue;
            };
            for ip in config.addresses() {
                if domain
                    .is_some_and(|d| self.provider.domain(*id, ip.vlan.unwrap_or(VlanId(1))) != d)
                {
                    continue;
                }
                if ip.contains(destination) {
                    candidates.push(RouteSelection {
                        port: *id,
                        source: ip.clone(),
                        next_hop: destination,
                        prefix: ip.prefix,
                        metric: 0,
                    });
                }
                if let Some(gateway) = ip.gateway
                    && ip.contains(gateway)
                {
                    candidates.push(RouteSelection {
                        port: *id,
                        source: ip.clone(),
                        next_hop: gateway,
                        prefix: 0,
                        metric: 0,
                    });
                }
            }
        }
        if let Some(os) = self.server_os(device) {
            // Explicit defaults supersede legacy per-interface gateway projections.
            if os
                .routes
                .iter()
                .any(|route| route.prefix == 0 && selected.is_none_or(|id| id == route.port))
            {
                candidates.retain(|route| route.prefix != 0);
            }
            for route in &os.routes {
                if !same_subnet(route.network, destination, route.prefix)
                    || selected.is_some_and(|id| id != route.port)
                {
                    continue;
                }
                let Some(port) = self.port(route.port).filter(|port| port.enabled) else {
                    continue;
                };
                let PortConfig::Server(config) = &port.config else {
                    continue;
                };
                let source = config
                    .addresses()
                    .find(|ip| ip.contains(route.via.unwrap_or(destination)))
                    .or(config.ipv4.as_ref());
                if let Some(source) = source {
                    if domain.is_some_and(|d| {
                        self.provider
                            .domain(route.port, source.vlan.unwrap_or(VlanId(1)))
                            != d
                    }) {
                        continue;
                    }
                    candidates.push(RouteSelection {
                        port: route.port,
                        source: source.clone(),
                        next_hop: route.via.unwrap_or(destination),
                        prefix: route.prefix,
                        metric: route.metric,
                    });
                }
            }
        }
        candidates
            .into_iter()
            .min_by_key(|route| (std::cmp::Reverse(route.prefix), route.metric, route.port))
    }

    pub(crate) fn address_config(
        &self,
        port: PortId,
        address: Ipv4Addr,
    ) -> Option<Ipv4InterfaceConfig> {
        let PortConfig::Server(config) = &self.port(port)?.config else {
            return None;
        };
        config.addresses().find(|ip| ip.address == address).cloned()
    }
}

pub(super) struct LinuxNetwork;

impl LinuxNetwork {
    pub fn execute(
        sim: &mut NetworkSim,
        device: DeviceId,
        words: &[String],
    ) -> Result<Vec<String>, String> {
        let mut args: Vec<&str> = words.iter().skip(1).map(String::as_str).collect();
        let mut brief = false;
        let mut oneline = false;
        let mut stats = false;
        while args.first().is_some_and(|word| word.starts_with('-')) {
            match args.remove(0) {
                "-4" => {}
                "-br" | "-brief" => brief = true,
                "-o" | "-oneline" => oneline = true,
                "-s" | "-statistics" => stats = true,
                flag => return Err(format!("ip: unsupported option {flag}")),
            }
        }
        if args.is_empty() {
            return Ok(vec![
                "Usage: ip [ -4 | -br | -o | -s ] { address | link | route | neighbor } COMMAND"
                    .into(),
            ]);
        }
        match args.remove(0) {
            "a" | "addr" | "address" => Self::address(sim, device, &args, brief, oneline),
            "l" | "link" => Self::link(sim, device, &args, brief, stats),
            "r" | "route" => Self::route(sim, device, &args),
            "n" | "neigh" | "neighbor" => Self::neighbor(sim, device, &args),
            object => Err(format!("Object \"{object}\" is unknown")),
        }
    }

    pub fn interface(sim: &NetworkSim, device: DeviceId, name: &str) -> Result<PortId, String> {
        sim.device(device)
            .into_iter()
            .flat_map(|device| device.ports())
            .copied()
            .find(|id| sim.port(*id).is_some_and(|port| port.name == name))
            .ok_or_else(|| format!("Cannot find device \"{name}\""))
    }

    fn dev_filter<'a>(args: &'a [&str]) -> Result<Option<&'a str>, String> {
        match args {
            [] => Ok(None),
            ["dev", name] | [name] => Ok(Some(name)),
            _ => Err("expected [dev IFACE]".into()),
        }
    }

    fn addresses(sim: &NetworkSim, port: PortId) -> Vec<Ipv4InterfaceConfig> {
        match sim.port(port).map(|port| &port.config) {
            Some(PortConfig::Server(config)) => config.addresses().cloned().collect(),
            _ => Vec::new(),
        }
    }

    fn address(
        sim: &mut NetworkSim,
        device: DeviceId,
        args: &[&str],
        brief: bool,
        oneline: bool,
    ) -> Result<Vec<String>, String> {
        let action = args.first().copied().unwrap_or("show");
        if matches!(action, "add" | "replace" | "del" | "delete" | "flush") {
            let (address, name) = if action == "flush" {
                (
                    None,
                    Self::dev_filter(&args[1..])?.ok_or("usage: ip addr flush dev IFACE")?,
                )
            } else {
                if args.len() != 4 || args[2] != "dev" {
                    return Err("usage: ip addr add|del|replace ADDRESS/PREFIX dev IFACE".into());
                }
                (Some(Self::cidr(args[1])?), args[3])
            };
            if name == "lo" {
                return Err("the guest loopback address is fixed at 127.0.0.1/8".into());
            }
            let id = Self::interface(sim, device, name)?;
            let PortConfig::Server(mut config) = sim.port(id).unwrap().config.clone() else {
                return Err("not a server interface".into());
            };
            let existing = address.and_then(|(ip, prefix)| {
                config
                    .addresses()
                    .find(|old| old.address == ip && old.prefix == prefix)
                    .cloned()
            });
            match action {
                "flush" => {
                    config.ipv4 = None;
                    config.additional_ipv4.clear();
                }
                "del" | "delete" => {
                    let old =
                        existing.ok_or("RTNETLINK answers: Cannot assign requested address")?;
                    if config.ipv4.as_ref() == Some(&old) {
                        config.ipv4 = if config.additional_ipv4.is_empty() {
                            None
                        } else {
                            Some(config.additional_ipv4.remove(0))
                        };
                    } else {
                        config.additional_ipv4.retain(|ip| ip != &old);
                    }
                }
                _ => {
                    if existing.is_some() {
                        return if action == "replace" {
                            Ok(Vec::new())
                        } else {
                            Err("RTNETLINK answers: File exists".into())
                        };
                    }
                    let (ip, prefix) = address.unwrap();
                    let vlan = config.ipv4.as_ref().and_then(|ip| ip.vlan);
                    let ip = Ipv4InterfaceConfig {
                        address: ip,
                        prefix,
                        gateway: None,
                        vlan,
                    };
                    if config.ipv4.is_none() {
                        config.ipv4 = Some(ip);
                    } else {
                        config.additional_ipv4.push(ip);
                    }
                }
            }
            sim.ports.get_mut(&id).unwrap().config = PortConfig::Server(config);
            sim.topology_revision += 1;
            sim.routing_revision += 1;
            return Ok(Vec::new());
        }
        let filter = Self::dev_filter(if matches!(action, "show" | "list" | "lst") {
            &args[1.min(args.len())..]
        } else {
            args
        })?;
        let mut lines = Vec::new();
        if filter.is_none_or(|name| name == "lo") {
            let up = sim.server_os(device).is_none_or(|os| os.loopback_up);
            lines.push(if brief {
                format!(
                    "lo               {} 127.0.0.1/8",
                    if up { "UNKNOWN" } else { "DOWN" }
                )
            } else {
                Self::loopback(sim, device)
            });
            if !brief {
                lines.push("    inet 127.0.0.1/8 scope host lo".into());
            }
        }
        if filter == Some("lo") {
            return Ok(lines);
        }
        let ids = Self::selected_ports(sim, device, filter)?;
        for (index, id) in ids.iter().enumerate() {
            let port = sim.port(*id).unwrap();
            let (flags, state) = Self::link_state(sim, port);
            let addresses = Self::addresses(sim, *id);
            if brief {
                lines.push(format!(
                    "{:<16} {:<10} {}",
                    port.name,
                    state,
                    addresses
                        .iter()
                        .map(|ip| format!("{}/{}", ip.address, ip.prefix))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            } else {
                let header = format!(
                    "{}: {}: <{flags}> mtu 1500 state {state}",
                    index + 2,
                    port.name
                );
                if oneline {
                    for ip in addresses {
                        lines.push(format!(
                            "{header} inet {}/{} scope global {}",
                            ip.address, ip.prefix, port.name
                        ));
                    }
                } else {
                    lines.push(header);
                    lines.push(format!(
                        "    link/ether {} brd ff:ff:ff:ff:ff:ff",
                        MacAddress::for_port(*id)
                    ));
                    for ip in addresses {
                        lines.push(format!(
                            "    inet {}/{} scope global {}",
                            ip.address, ip.prefix, port.name
                        ));
                    }
                }
            }
        }
        Ok(lines)
    }

    fn selected_ports(
        sim: &NetworkSim,
        device: DeviceId,
        filter: Option<&str>,
    ) -> Result<Vec<PortId>, String> {
        if let Some(name) = filter {
            Ok(vec![Self::interface(sim, device, name)?])
        } else {
            Ok(sim
                .device(device)
                .ok_or("server not found")?
                .ports()
                .to_vec())
        }
    }

    fn loopback(sim: &NetworkSim, device: DeviceId) -> String {
        if sim.server_os(device).is_none_or(|os| os.loopback_up) {
            "1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536 state UNKNOWN".into()
        } else {
            "1: lo: <LOOPBACK> mtu 65536 state DOWN".into()
        }
    }

    fn link_state(sim: &NetworkSim, port: &Port) -> (&'static str, &'static str) {
        if !port.enabled {
            ("DOWN", "DOWN")
        } else if sim.port_link_up(port.id) {
            ("BROADCAST,MULTICAST,UP,LOWER_UP", "UP")
        } else {
            ("BROADCAST,MULTICAST,UP,NO-CARRIER", "DOWN")
        }
    }

    fn link(
        sim: &mut NetworkSim,
        device: DeviceId,
        args: &[&str],
        brief: bool,
        stats: bool,
    ) -> Result<Vec<String>, String> {
        if args.first() == Some(&"set") {
            let rest = &args[1..];
            let rest = rest.strip_prefix(&["dev"]).unwrap_or(rest);
            if let [name, state] = rest
                && matches!(*state, "up" | "down")
            {
                if *name == "lo" {
                    sim.guest_mut(device).loopback_up = *state == "up";
                } else {
                    let port = Self::interface(sim, device, name)?;
                    sim.execute(Command::SetPortEnabled {
                        port,
                        enabled: *state == "up",
                    })
                    .map_err(|e| e.to_string())?;
                }
                return Ok(Vec::new());
            }
            return Err("usage: ip link set [dev] IFACE up|down".into());
        }
        let rest = args.strip_prefix(&["show"]).unwrap_or(args);
        let filter = Self::dev_filter(rest)?;
        if filter == Some("lo") {
            return Ok(vec![Self::loopback(sim, device)]);
        }
        let mut lines = Vec::new();
        if filter.is_none() {
            lines.push(Self::loopback(sim, device));
        }
        for (index, id) in Self::selected_ports(sim, device, filter)?
            .iter()
            .enumerate()
        {
            let port = sim.port(*id).unwrap();
            let (flags, state) = Self::link_state(sim, port);
            lines.push(if brief {
                format!(
                    "{:<16} {state:<10} {}",
                    port.name,
                    MacAddress::for_port(*id)
                )
            } else {
                format!(
                    "{}: {}: <{flags}> mtu 1500 state {state}",
                    index + 2,
                    port.name
                )
            });
            if stats {
                let stats = sim.port_telemetry(*id);
                lines.extend([
                    format!("    RX: packets {}", stats.rx_frames),
                    format!("    TX: packets {}", stats.tx_frames),
                ]);
            }
        }
        Ok(lines)
    }

    pub fn cidr(value: &str) -> Result<(Ipv4Addr, u8), String> {
        let (address, prefix) = value
            .split_once('/')
            .ok_or("address must use CIDR notation")?;
        let address: Ipv4Addr = address.parse().map_err(|_| "invalid IPv4 address")?;
        let prefix: u8 = prefix.parse().map_err(|_| "invalid prefix")?;
        if prefix > 32
            || address.is_unspecified()
            || address.is_multicast()
            || address == Ipv4Addr::BROADCAST
        {
            return Err("invalid IPv4 interface address".into());
        }
        Ok((address, prefix))
    }

    pub fn network(address: Ipv4Addr, prefix: u8) -> Ipv4Addr {
        Ipv4Addr::from(
            u32::from(address)
                & if prefix == 0 {
                    0
                } else {
                    u32::MAX << (32 - prefix)
                },
        )
    }

    fn route(sim: &mut NetworkSim, device: DeviceId, args: &[&str]) -> Result<Vec<String>, String> {
        let action = args.first().copied().unwrap_or("show");
        if matches!(action, "show" | "list") {
            let mut lines = Vec::new();
            let os_routes = sim
                .server_os(device)
                .map(|os| os.routes.clone())
                .unwrap_or_default();
            let filter = Self::dev_filter(&args[1.min(args.len())..])?;
            if let Some(name) = filter {
                Self::interface(sim, device, name)?;
            }
            for id in sim.device(device).unwrap().ports() {
                let port = sim.port(*id).unwrap();
                if filter.is_some_and(|name| name != port.name) {
                    continue;
                }
                for ip in Self::addresses(sim, *id) {
                    if let Some(gateway) = ip.gateway
                        && !os_routes
                            .iter()
                            .any(|route| route.prefix == 0 && route.port == *id)
                    {
                        lines.push(format!("default via {gateway} dev {}", port.name));
                    }
                    lines.push(format!(
                        "{}/{} dev {} proto kernel scope link src {}",
                        Self::network(ip.address, ip.prefix),
                        ip.prefix,
                        port.name,
                        ip.address
                    ));
                }
            }
            for route in os_routes {
                let Some(port) = sim.port(route.port) else {
                    continue;
                };
                if filter.is_some_and(|name| name != port.name) {
                    continue;
                }
                let destination = if route.prefix == 0 {
                    "default".into()
                } else {
                    format!("{}/{}", route.network, route.prefix)
                };
                lines.push(format!(
                    "{destination}{} dev {} metric {}",
                    route
                        .via
                        .map_or_else(String::new, |via| format!(" via {via}")),
                    port.name,
                    route.metric
                ));
            }
            lines.sort();
            lines.dedup();
            return Ok(lines);
        }
        if action == "get" {
            let address: Ipv4Addr = args
                .get(1)
                .ok_or("usage: ip route get ADDRESS")?
                .parse()
                .map_err(|_| "invalid destination")?;
            let route = sim
                .server_route_selection(device, address, None)
                .ok_or("RTNETLINK answers: Network is unreachable")?;
            let name = &sim.port(route.port).unwrap().name;
            return Ok(vec![format!(
                "{address}{} dev {name} src {} metric {}",
                if route.next_hop == address {
                    String::new()
                } else {
                    format!(" via {}", route.next_hop)
                },
                route.source.address,
                route.metric
            )]);
        }
        if !matches!(action, "add" | "replace" | "del" | "delete" | "flush") {
            return Err("usage: ip route show|add|replace|del|get".into());
        }
        if action == "flush" {
            let filter = Self::dev_filter(&args[1..])?;
            let ports = Self::selected_ports(sim, device, filter)?;
            sim.guest_mut(device)
                .routes
                .retain(|route| !ports.contains(&route.port));
            for port in ports {
                Self::clear_gateway(sim, port, None);
            }
            sim.routing_revision += 1;
            return Ok(Vec::new());
        }
        let destination = args.get(1).ok_or("missing route destination")?;
        let (network, prefix) = if *destination == "default" {
            (Ipv4Addr::UNSPECIFIED, 0)
        } else {
            let (address, prefix) = if destination.contains('/') {
                let (address, prefix) = destination.split_once('/').unwrap();
                let address: Ipv4Addr = address.parse().map_err(|_| "invalid route destination")?;
                let prefix: u8 = prefix.parse().map_err(|_| "invalid prefix")?;
                if prefix > 32 || Self::network(address, prefix) != address {
                    return Err("Error: Invalid prefix for given prefix length".into());
                }
                (address, prefix)
            } else {
                (
                    destination
                        .parse()
                        .map_err(|_| "invalid route destination")?,
                    32,
                )
            };
            (Self::network(address, prefix), prefix)
        };
        let mut name = None;
        let mut via = None;
        let mut metric = 0;
        let mut index = 2;
        while index < args.len() {
            let value = args.get(index + 1).ok_or("route option needs a value")?;
            match args[index] {
                "dev" => name = Some(*value),
                "via" => {
                    let gateway: Ipv4Addr = value.parse().map_err(|_| "invalid gateway")?;
                    if gateway.is_unspecified() || gateway.is_multicast() {
                        return Err("invalid gateway".into());
                    }
                    via = Some(gateway);
                }
                "metric" => metric = value.parse().map_err(|_| "invalid route metric")?,
                _ => return Err(format!("unknown route option {}", args[index])),
            }
            index += 2;
        }
        let delete = matches!(action, "del" | "delete");
        let port = if let Some(name) = name {
            Some(Self::interface(sim, device, name)?)
        } else if delete {
            None
        } else {
            via.and_then(|gateway| sim.server_route_selection(device, gateway, None))
                .filter(|route| route.next_hop == via.unwrap())
                .map(|route| route.port)
        };
        if delete {
            let routes = &mut sim.guest_mut(device).routes;
            let index = routes.iter().position(|route| {
                route.network == network
                    && route.prefix == prefix
                    && port.is_none_or(|id| id == route.port)
                    && via.is_none_or(|gateway| Some(gateway) == route.via)
            });
            if let Some(index) = index {
                let removed = routes.remove(index);
                if removed.prefix == 0 {
                    Self::clear_gateway(sim, removed.port, removed.via);
                }
            } else if prefix == 0 {
                let legacy = sim
                    .device(device)
                    .unwrap()
                    .ports()
                    .iter()
                    .copied()
                    .find(|id| {
                        port.is_none_or(|port| port == *id)
                            && Self::addresses(sim, *id).iter().any(|ip| {
                                ip.gateway.is_some()
                                    && via.is_none_or(|gateway| ip.gateway == Some(gateway))
                            })
                    });
                Self::clear_gateway(
                    sim,
                    legacy.ok_or("RTNETLINK answers: No such process")?,
                    via,
                );
            } else {
                return Err("RTNETLINK answers: No such process".into());
            }
        } else {
            let port = port.ok_or("cannot determine egress interface; specify dev IFACE")?;
            let addresses = Self::addresses(sim, port);
            if addresses.is_empty() {
                return Err("interface has no IPv4 address".into());
            }
            if via.is_some_and(|gateway| !addresses.iter().any(|ip| ip.contains(gateway))) {
                return Err("Error: Nexthop has invalid gateway".into());
            }
            let existing = sim
                .server_os(device)
                .into_iter()
                .flat_map(|os| &os.routes)
                .any(|route| {
                    route.network == network && route.prefix == prefix && route.metric == metric
                });
            let explicit_defaults = sim
                .server_os(device)
                .is_some_and(|os| os.routes.iter().any(|route| route.prefix == 0));
            let legacy = prefix == 0
                && metric == 0
                && !explicit_defaults
                && addresses.iter().any(|ip| ip.gateway.is_some());
            if action == "add" && (existing || legacy) {
                return Err("RTNETLINK answers: File exists".into());
            }
            if prefix == 0 && action == "replace" && metric == 0 {
                let ports = sim.device(device).unwrap().ports().to_vec();
                for port in ports {
                    Self::clear_gateway(sim, port, None);
                }
            }
            if action == "replace" && prefix == 0 {
                let previous: Vec<_> = sim
                    .guest_mut(device)
                    .routes
                    .iter()
                    .filter(|route| route.prefix == 0 && route.metric == metric)
                    .cloned()
                    .collect();
                for route in previous {
                    Self::clear_gateway(sim, route.port, route.via);
                }
            }
            let routes = &mut sim.guest_mut(device).routes;
            if action == "replace" {
                routes.retain(|route| {
                    !(route.network == network && route.prefix == prefix && route.metric == metric)
                });
            }
            routes.push(LinuxRoute {
                network,
                prefix,
                via,
                port,
                metric,
            });
            if prefix == 0
                && let PortConfig::Server(config) = &mut sim.ports.get_mut(&port).unwrap().config
                && let Some(ip) = &mut config.ipv4
            {
                ip.gateway = via;
            }
        }
        sim.routing_revision += 1;
        Ok(Vec::new())
    }

    fn clear_gateway(sim: &mut NetworkSim, port: PortId, via: Option<Ipv4Addr>) {
        if let PortConfig::Server(config) = &mut sim.ports.get_mut(&port).unwrap().config {
            for ip in config.ipv4.iter_mut().chain(&mut config.additional_ipv4) {
                if via.is_none_or(|via| ip.gateway == Some(via)) {
                    ip.gateway = None;
                }
            }
        }
    }

    fn neighbor(
        sim: &mut NetworkSim,
        device: DeviceId,
        args: &[&str],
    ) -> Result<Vec<String>, String> {
        let rest = args.strip_prefix(&["show"]).unwrap_or(args);
        if rest.first() == Some(&"flush") {
            let filter = Self::dev_filter(&rest[1..])?;
            let ports = Self::selected_ports(sim, device, filter)?;
            sim.runtime
                .arp
                .retain(|(port, _, _), _| !ports.contains(port));
            return Ok(Vec::new());
        }
        let filter = Self::dev_filter(rest)?;
        let mut lines = Vec::new();
        for id in Self::selected_ports(sim, device, filter)? {
            for (address, mac) in sim.arp_entries(id) {
                lines.push(format!(
                    "{address} dev {} lladdr {mac} REACHABLE",
                    sim.port(id).unwrap().name
                ));
            }
        }
        Ok(lines)
    }
}
