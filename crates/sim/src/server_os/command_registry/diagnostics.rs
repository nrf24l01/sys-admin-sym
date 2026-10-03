use super::super::network::LinuxNetwork;
use crate::*;
use std::net::Ipv4Addr;

pub(super) struct LinuxDiagnostics;

impl LinuxDiagnostics {
    pub(super) fn hostname(
        sim: &mut NetworkSim,
        device: DeviceId,
        command: &str,
        args: &[String],
    ) -> Result<Vec<String>, String> {
        let name = if command == "hostnamectl" {
            if args.first().is_some_and(|arg| arg == "set-hostname") {
                args.get(1)
            } else {
                None
            }
        } else {
            args.first()
        };
        if let Some(name) = name {
            sim.execute(Command::SetHostname {
                device,
                hostname: name.clone(),
            })
            .map_err(|e| e.to_string())?;
            sim.guest_mut(device)
                .filesystem
                .write("/etc/hostname", &format!("{name}\n"), false)?;
            sim.guest_mut(device)
                .environment
                .insert("HOSTNAME".into(), name.clone());
            return Ok(Vec::new());
        }
        let Some(DeviceKind::Server(server)) = sim.device(device).map(|device| &device.kind) else {
            return Err("server not found".into());
        };
        if command == "hostnamectl" {
            Ok(vec![
                format!("Static hostname: {}", server.hostname),
                "Operating System: Debian GNU/Linux 12 (simulated)".into(),
                "Kernel: Linux 6.6.0-sim".into(),
                "Architecture: x86-64".into(),
            ])
        } else {
            Ok(vec![server.hostname.clone()])
        }
    }

    pub(super) fn resolve_host(
        sim: &mut NetworkSim,
        device: DeviceId,
        name: &str,
    ) -> Result<Ipv4Addr, String> {
        if let Ok(address) = name.parse() {
            return Ok(address);
        }
        let hosts = sim.guest_mut(device).filesystem.read("/etc/hosts")?;
        for line in hosts.lines() {
            let words: Vec<_> = line
                .split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .collect();
            if words.iter().skip(1).any(|alias| *alias == name) {
                return words[0].parse().map_err(|_| "invalid hosts entry".into());
            }
        }
        for device in sim.devices() {
            if let DeviceKind::Server(server) = &device.kind
                && server.hostname == name
                && let Some(address) =
                    server
                        .ports
                        .iter()
                        .find_map(|id| match &sim.port(*id)?.config {
                            PortConfig::Server(config) => config.ipv4.as_ref().map(|ip| ip.address),
                            _ => None,
                        })
            {
                return Ok(address);
            }
        }
        Err(format!(
            "{name}: Name or service not known; configure /etc/hosts"
        ))
    }

    pub(super) fn ping(
        sim: &mut NetworkSim,
        device: DeviceId,
        command: &str,
        args: &[String],
    ) -> Result<Vec<String>, String> {
        let mut destination = None;
        let mut interface = None;
        let mut count = 1usize;
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "-c" => {
                    index += 1;
                    count = args
                        .get(index)
                        .ok_or("missing ping count")?
                        .parse()
                        .map_err(|_| "invalid ping count")?;
                    if !(1..=64).contains(&count) {
                        return Err("ping count must be between 1 and 64".into());
                    }
                }
                "-I" => {
                    index += 1;
                    interface = Some(args.get(index).ok_or("missing ping interface")?.as_str());
                }
                "-W" | "-w" => {
                    index += 1;
                    args.get(index)
                        .ok_or("missing timeout")?
                        .parse::<u32>()
                        .map_err(|_| "invalid timeout")?;
                }
                flag if flag.starts_with('-') => {
                    return Err(format!("ping: unsupported option {flag}"));
                }
                name => {
                    if destination.replace(name).is_some() {
                        return Err("ping: too many destinations".into());
                    }
                }
            }
            index += 1;
        }
        let address = Self::resolve_host(
            sim,
            device,
            destination.ok_or("usage: ping [-c COUNT] [-I IFACE] ADDRESS")?,
        )?;
        if address.is_loopback() {
            if sim.guest_mut(device).loopback_up {
                return Ok(vec![format!(
                    "64 bytes from {address}: icmp_seq=1 ttl=64 time=0 ms"
                )]);
            }
            return Err("ping: Network is unreachable (lo is down)".into());
        }
        let local = sim
            .device(device)
            .unwrap()
            .ports()
            .iter()
            .any(|port| sim.address_config(*port, address).is_some());
        if local {
            return Ok(vec![format!(
                "64 bytes from {address}: icmp_seq=1 ttl=64 time=0 ms"
            )]);
        }
        let selected = interface
            .map(|name| LinuxNetwork::interface(sim, device, name))
            .transpose()?;
        let route = sim
            .server_route_selection(device, address, selected)
            .ok_or("ping: Network is unreachable")?;
        let mut lines = vec![format!(
            "PING {address} ({address}) from {}",
            route.source.address
        )];
        for sequence in 1..=count {
            let result = sim.transmit_icmp(route.port, address);
            if !result.reachable {
                return Err(format!(
                    "From {}: {:?}",
                    route.source.address,
                    result.failure.unwrap_or(ReachabilityFailure::NoRoute)
                ));
            }
            if command != "ping" {
                lines.extend(
                    result
                        .hops
                        .iter()
                        .enumerate()
                        .map(|(index, hop)| format!("{}  {}  {}", index + 1, hop.device, hop.note)),
                );
            }
            lines.push(format!(
                "64 bytes from {address}: icmp_seq={sequence} ttl=64 time=1 ms"
            ));
        }
        lines.push(format!(
            "{count} packets transmitted, {count} received, 0% packet loss"
        ));
        Ok(lines)
    }
}
