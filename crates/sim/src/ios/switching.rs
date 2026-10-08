use crate::*;
use std::collections::BTreeSet;

pub(super) const GLOBAL: &[&str] = &[
    "mls qos",
    "no mls qos",
    "mls qos srr-queue output dscp-map queue <queue> <values...>",
    "no mls qos srr-queue output dscp-map",
    "snmp-server community <community> ro",
    "snmp-server community <community> rw",
    "no snmp-server community <community>",
    "snmp-server contact <text...>",
    "snmp-server location <text...>",
    "no snmp-server contact",
    "no snmp-server location",
    "port-channel load-balance <method>",
    "lacp system-priority <priority>",
];
pub(super) const INTERFACE: &[&str] = &[
    "channel-group <group> mode <mode>",
    "no channel-group",
    "lacp port-priority <priority>",
    "mls qos trust dscp",
    "mls qos trust cos",
    "no mls qos trust",
    "mls qos cos <cos>",
    "srr-queue bandwidth share <one> <two> <three> <four>",
    "no srr-queue bandwidth share",
    "srr-queue bandwidth limit <percent>",
    "no srr-queue bandwidth limit",
    "priority-queue out",
    "no priority-queue out",
];
pub(super) const SHOW: &[&str] = &[
    "show etherchannel summary",
    "show etherchannel <group> detail",
    "show mls qos",
    "show mls qos interface <interface>",
    "show mls qos interface <interface> statistics",
    "show snmp",
    "show snmp community",
];

fn number(value: &str, minimum: u16, maximum: u16) -> Result<u16, String> {
    value
        .parse::<u16>()
        .ok()
        .filter(|n| (minimum..=maximum).contains(n))
        .ok_or_else(|| format!("% Value must be between {minimum} and {maximum}."))
}

pub(super) fn channel_number(value: &str) -> Option<u8> {
    let value = value.replace(' ', "").to_ascii_lowercase();
    value
        .strip_prefix("port-channel")
        .or_else(|| value.strip_prefix("po"))?
        .parse::<u8>()
        .ok()
        .filter(|n| (1..=6).contains(n))
}

impl NetworkSim {
    pub(super) fn ios_switch_global(
        &mut self,
        device: DeviceId,
        command: &str,
        args: &[String],
    ) -> Result<(), String> {
        match command {
            "mls qos" | "no mls qos" => {
                self.switch_services_mut(device)?.qos.enabled = command == "mls qos"
            }
            "mls qos srr-queue output dscp-map queue <queue> <values...>" => {
                let queue = number(&args[0], 1, 4)? as u8;
                let values = args[1]
                    .split_whitespace()
                    .map(|value| number(value, 0, 63).map(|v| v as u8))
                    .collect::<Result<Vec<_>, _>>()?;
                if values.is_empty() || values.len() > 8 {
                    return Err("% Specify one to eight DSCP values.".into());
                }
                for value in values {
                    self.switch_services_mut(device)?
                        .qos
                        .dscp_queue
                        .insert(value, queue);
                }
            }
            "no mls qos srr-queue output dscp-map" => {
                self.switch_services_mut(device)?.qos.dscp_queue.clear()
            }
            "snmp-server community <community> ro" | "snmp-server community <community> rw" => {
                if args[0].is_empty() || args[0].contains('@') {
                    return Err("% Invalid community string.".into());
                }
                self.switch_services_mut(device)?.snmp.communities.insert(
                    args[0].clone(),
                    if command.ends_with(" rw") {
                        SnmpAccess::ReadWrite
                    } else {
                        SnmpAccess::ReadOnly
                    },
                );
            }
            "no snmp-server community <community>" => {
                self.switch_services_mut(device)?
                    .snmp
                    .communities
                    .remove(&args[0]);
            }
            "snmp-server contact <text...>" => {
                self.switch_services_mut(device)?.snmp.contact = args[0].clone()
            }
            "snmp-server location <text...>" => {
                self.switch_services_mut(device)?.snmp.location = args[0].clone()
            }
            "no snmp-server contact" => self.switch_services_mut(device)?.snmp.contact.clear(),
            "no snmp-server location" => self.switch_services_mut(device)?.snmp.location.clear(),
            "lacp system-priority <priority>" => {
                self.switch_services_mut(device)?
                    .etherchannel
                    .system_priority = number(&args[0], 1, 65535)?
            }
            "port-channel load-balance <method>" => {
                self.switch_services_mut(device)?.etherchannel.hash = match args[0].as_str() {
                    "src-mac" => ChannelHash::SourceMac,
                    "dst-mac" => ChannelHash::DestinationMac,
                    "src-dst-mac" => ChannelHash::SourceDestinationMac,
                    _ => return Err("% Supported methods: src-mac, dst-mac, src-dst-mac.".into()),
                }
            }
            _ => return Err("% Unsupported switch configuration command.".into()),
        }
        self.topology_revision += 1;
        Ok(())
    }

    pub(super) fn ios_switch_interface(
        &mut self,
        device: DeviceId,
        port: PortId,
        command: &str,
        args: &[String],
    ) -> Result<(), String> {
        match command {
            "channel-group <group> mode <mode>" => {
                let group = number(&args[0], 1, 6)? as u8;
                let mode = match args[1].as_str() {
                    "on" => ChannelMode::On,
                    "active" => ChannelMode::Active,
                    "passive" => ChannelMode::Passive,
                    "auto" => ChannelMode::Auto,
                    "desirable" => ChannelMode::Desirable,
                    _ => {
                        return Err("% Mode must be on, active, passive, auto or desirable.".into());
                    }
                };
                return self.set_channel_group(port, Some((group, mode)));
            }
            "no channel-group" => return self.set_channel_group(port, None),
            "lacp port-priority <priority>" => {
                self.switch_services_mut(device)?
                    .etherchannel
                    .port_priority
                    .insert(port, number(&args[0], 1, 65535)?);
            }
            _ => {
                let mut settings = self
                    .switch_services(device)
                    .ok_or("% Switch not found.")?
                    .qos
                    .ports
                    .get(&port)
                    .cloned()
                    .unwrap_or_default();
                match command {
                    "mls qos trust dscp" => settings.trust = QosTrust::Dscp,
                    "mls qos trust cos" => settings.trust = QosTrust::Cos,
                    "no mls qos trust" => settings.trust = QosTrust::Untrusted,
                    "mls qos cos <cos>" => settings.default_cos = number(&args[0], 0, 7)? as u8,
                    "srr-queue bandwidth share <one> <two> <three> <four>" => {
                        for (index, value) in args.iter().enumerate() {
                            settings.weights[index] = number(value, 1, 255)? as u8;
                        }
                    }
                    "no srr-queue bandwidth share" => settings.weights = [25; 4],
                    "srr-queue bandwidth limit <percent>" => {
                        settings.bandwidth_percent = number(&args[0], 10, 90)? as u8
                    }
                    "no srr-queue bandwidth limit" => settings.bandwidth_percent = 100,
                    "priority-queue out" => settings.priority_queue = true,
                    "no priority-queue out" => settings.priority_queue = false,
                    _ => return Err("% Unsupported switch interface command.".into()),
                }
                self.switch_services_mut(device)?
                    .qos
                    .ports
                    .insert(port, settings);
            }
        }
        self.topology_revision += 1;
        Ok(())
    }

    pub(super) fn ios_channel_command(
        &mut self,
        device: DeviceId,
        group: u8,
        command: &str,
        args: &[String],
    ) -> Result<(), String> {
        match command {
            "port-channel min-links <count>" => {
                self.switch_services_mut(device)?
                    .etherchannel
                    .min_links
                    .insert(group, number(&args[0], 2, 8)? as u8);
            }
            "no port-channel min-links" => {
                self.switch_services_mut(device)?
                    .etherchannel
                    .min_links
                    .remove(&group);
            }
            "shutdown" => {
                self.switch_services_mut(device)?
                    .etherchannel
                    .disabled
                    .insert(group);
            }
            "no shutdown" => {
                self.switch_services_mut(device)?
                    .etherchannel
                    .disabled
                    .remove(&group);
            }
            _ => {
                if command.starts_with("channel-group") || command == "no channel-group" {
                    return Err("% Assign channel-group on physical members.".into());
                }
                for port in self.channel_ports(device, group) {
                    self.ios_interface_command(device, port, None, command, args)?;
                }
            }
        }
        self.topology_revision += 1;
        Ok(())
    }

    pub(super) fn ios_switch_show(
        &self,
        device: DeviceId,
        command: &str,
        args: &[String],
    ) -> Result<Vec<String>, String> {
        let services = self
            .switch_services(device)
            .ok_or("% This command requires a switch.")?;
        if command.starts_with("show etherchannel") {
            let groups: BTreeSet<_> = services
                .etherchannel
                .members
                .values()
                .map(|member| member.group)
                .collect();
            let selected = args
                .first()
                .map(|number_| number(number_, 1, 6).map(|n| n as u8))
                .transpose()?;
            let mut lines = vec![
                "Group  Port-channel  Protocol  Ports (P=bundled H=standby s=suspended D=down)"
                    .into(),
            ];
            for group in groups
                .into_iter()
                .filter(|g| selected.is_none_or(|selected| *g == selected))
            {
                let ports = self.channel_ports(device, group);
                let first = ports[0];
                let protocol = match self.channel_member(first).unwrap().mode {
                    ChannelMode::On => "STATIC",
                    ChannelMode::Active | ChannelMode::Passive => "LACP",
                    _ => "PAgP",
                };
                let capacity = self.channel_capacity_mbps(device, group);
                let members = ports
                    .iter()
                    .map(|port| {
                        format!(
                            "{}({})",
                            self.ios_interface_name(device, *port),
                            match self.channel_state(*port) {
                                ChannelState::Bundled => "P",
                                ChannelState::Standby => "H",
                                ChannelState::Suspended => "s",
                                ChannelState::Down => "D",
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                lines.push(format!(
                    "{group:<6} Po{group}({}) {protocol:<9} {members}",
                    if capacity > 0 { "SU" } else { "SD" }
                ));
                if selected.is_some() {
                    lines.push(format!(
                        "Aggregate capacity: {capacity} Mbps; each flow uses one member"
                    ));
                }
            }
            return Ok(lines);
        }
        if command.starts_with("show mls qos") {
            let mut lines = vec![format!(
                "QoS is {}",
                if services.qos.enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            )];
            if let Some(interface) = args.first() {
                let (port, _) = self.ios_find_interface(device, interface)?;
                let settings = services.qos.ports.get(&port).cloned().unwrap_or_default();
                lines.push(format!(
                    "{} trust {:?}; default CoS {}; bandwidth {}%; weights {:?}; priority queue {}",
                    self.ios_interface_name(device, port),
                    settings.trust,
                    settings.default_cos,
                    settings.bandwidth_percent,
                    settings.weights,
                    settings.priority_queue
                ));
                let stats = self.qos_counters(port);
                for queue in 0..4 {
                    lines.push(format!(
                        "Queue {}: {} packets, {} bytes, {} drops",
                        queue + 1,
                        stats.transmitted[queue],
                        stats.bytes[queue],
                        stats.dropped[queue]
                    ));
                }
            }
            return Ok(lines);
        }
        let mut lines = vec![
            format!("SNMP communities: {}", services.snmp.communities.len()),
            format!("Contact: {}", services.snmp.contact),
            format!("Location: {}", services.snmp.location),
        ];
        let stats = self.snmp_counters(device);
        lines.push(format!(
            "Requests: {}; authorization failures: {}",
            stats.requests, stats.authentication_failures
        ));
        if command == "show snmp community" {
            lines.extend(
                services
                    .snmp
                    .communities
                    .iter()
                    .map(|(name, access)| format!("{name}: {access:?}")),
            );
        }
        Ok(lines)
    }

    pub(super) fn ios_switch_config(&self, device: DeviceId) -> Vec<String> {
        let Some(services) = self.switch_services(device) else {
            return vec![];
        };
        let mut lines = Vec::new();
        if services.qos.enabled {
            lines.push("mls qos".into());
        }
        for (dscp, queue) in &services.qos.dscp_queue {
            lines.push(format!(
                "mls qos srr-queue output dscp-map queue {queue} {dscp}"
            ));
        }
        for (community, access) in &services.snmp.communities {
            lines.push(format!(
                "snmp-server community {community} {}",
                if *access == SnmpAccess::ReadOnly {
                    "ro"
                } else {
                    "rw"
                }
            ));
        }
        if !services.snmp.contact.is_empty() {
            lines.push(format!("snmp-server contact {}", services.snmp.contact));
        }
        if !services.snmp.location.is_empty() {
            lines.push(format!("snmp-server location {}", services.snmp.location));
        }
        lines.push(format!(
            "port-channel load-balance {}",
            match services.etherchannel.hash {
                ChannelHash::SourceMac => "src-mac",
                ChannelHash::DestinationMac => "dst-mac",
                ChannelHash::SourceDestinationMac => "src-dst-mac",
            }
        ));
        lines.push(format!(
            "lacp system-priority {}",
            services.etherchannel.system_priority
        ));
        lines
    }

    pub(super) fn ios_switch_port_config(&self, device: DeviceId, port: PortId) -> Vec<String> {
        let Some(services) = self.switch_services(device) else {
            return vec![];
        };
        let mut lines = Vec::new();
        if let Some(member) = self.channel_member(port) {
            lines.push(format!(
                " channel-group {} mode {}",
                member.group,
                member.mode.name()
            ));
        }
        if let Some(priority) = services.etherchannel.port_priority.get(&port) {
            lines.push(format!(" lacp port-priority {priority}"));
        }
        if let Some(qos) = services.qos.ports.get(&port) {
            match qos.trust {
                QosTrust::Dscp => lines.push(" mls qos trust dscp".into()),
                QosTrust::Cos => lines.push(" mls qos trust cos".into()),
                QosTrust::Untrusted => {}
            }
            if qos.default_cos != 0 {
                lines.push(format!(" mls qos cos {}", qos.default_cos));
            }
            if qos.weights != [25; 4] {
                lines.push(format!(
                    " srr-queue bandwidth share {} {} {} {}",
                    qos.weights[0], qos.weights[1], qos.weights[2], qos.weights[3]
                ));
            }
            if qos.bandwidth_percent != 100 {
                lines.push(format!(
                    " srr-queue bandwidth limit {}",
                    qos.bandwidth_percent
                ));
            }
            if qos.priority_queue {
                lines.push(" priority-queue out".into());
            }
        }
        lines
    }

    pub(super) fn ios_channel_config(&self, device: DeviceId) -> Vec<String> {
        let Some(services) = self.switch_services(device) else {
            return vec![];
        };
        let mut lines = Vec::new();
        let groups: BTreeSet<_> = services
            .etherchannel
            .members
            .values()
            .map(|m| m.group)
            .collect();
        for group in groups {
            lines.push(format!("interface Port-channel{group}"));
            if let Some(min) = services.etherchannel.min_links.get(&group) {
                lines.push(format!(" port-channel min-links {min}"));
            }
            if services.etherchannel.disabled.contains(&group) {
                lines.push(" shutdown".into());
            }
            lines.push(" exit".into());
        }
        lines
    }
}
