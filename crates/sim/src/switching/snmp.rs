//! Semantic SNMPv2c management. No host sockets or Cisco firmware are used.
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnmpAccess {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SnmpConfig {
    pub communities: BTreeMap<String, SnmpAccess>,
    pub contact: String,
    pub location: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnmpValue {
    Text(String),
    Integer(u64),
    TimeTicks(u64),
    Counter32(u32),
    Counter64(u64),
    Gauge32(u32),
}
impl std::fmt::Display for SnmpValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(v) => write!(f, "STRING: {v}"),
            Self::Integer(v) => write!(f, "INTEGER: {v}"),
            Self::TimeTicks(v) => write!(f, "Timeticks: ({v})"),
            Self::Counter32(v) => write!(f, "Counter32: {v}"),
            Self::Counter64(v) => write!(f, "Counter64: {v}"),
            Self::Gauge32(v) => write!(f, "Gauge32: {v}"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SnmpCounters {
    pub requests: u64,
    pub authentication_failures: u64,
}

pub fn snmp_oid(value: &str) -> Result<String, String> {
    let aliases = [
        ("sysDescr.0", 1),
        ("sysUpTime.0", 3),
        ("sysContact.0", 4),
        ("sysName.0", 5),
        ("sysLocation.0", 6),
    ];
    let value = value
        .rsplit("::")
        .next()
        .unwrap_or(value)
        .trim_start_matches('.');
    if let Some((_, column)) = aliases.iter().find(|(name, _)| *name == value) {
        return Ok(format!("1.3.6.1.2.1.1.{column}.0"));
    }
    if value == "ifNumber.0" {
        return Ok("1.3.6.1.2.1.2.1.0".into());
    }
    if let Some((name, index)) = value.rsplit_once('.')
        && let Some(column) = match name {
            "ifName" => Some(1),
            "ifHCInOctets" => Some(6),
            "ifHCOutOctets" => Some(10),
            "ifHighSpeed" => Some(15),
            _ => None,
        }
    {
        index
            .parse::<u32>()
            .map_err(|_| "Invalid interface index.")?;
        return Ok(format!("1.3.6.1.2.1.31.1.1.1.{column}.{index}"));
    }
    let columns = [
        ("ifIndex", 1),
        ("ifDescr", 2),
        ("ifType", 3),
        ("ifMtu", 4),
        ("ifSpeed", 5),
        ("ifAdminStatus", 7),
        ("ifOperStatus", 8),
        ("ifInOctets", 10),
        ("ifOutOctets", 16),
    ];
    if let Some((name, index)) = value.rsplit_once('.')
        && let Some((_, column)) = columns.iter().find(|(name2, _)| *name2 == name)
    {
        index
            .parse::<u32>()
            .map_err(|_| "Invalid interface index.")?;
        return Ok(format!("1.3.6.1.2.1.2.2.1.{column}.{index}"));
    }
    if value.split('.').all(|part| part.parse::<u32>().is_ok()) && value.contains('.') {
        Ok(value.into())
    } else {
        Err("Invalid or unsupported OID name; use a numeric OID.".into())
    }
}

impl NetworkSim {
    pub fn snmp_counters(&self, device: DeviceId) -> SnmpCounters {
        self.runtime.snmp.get(&device).copied().unwrap_or_default()
    }

    fn snmp_authorize(
        &mut self,
        device: DeviceId,
        community: &str,
        write: bool,
    ) -> Result<(), String> {
        if !self.device(device).is_some_and(|d| d.powered) {
            return Err("SNMP target is offline.".into());
        }
        let access = self
            .switch_services(device)
            .and_then(|s| s.snmp.communities.get(community))
            .copied();
        let counters = self.runtime.snmp.entry(device).or_default();
        counters.requests += 1;
        if access.is_none() || (write && access != Some(SnmpAccess::ReadWrite)) {
            counters.authentication_failures += 1;
            return Err("SNMP authorization failed.".into());
        }
        Ok(())
    }

    pub fn snmp_get(
        &mut self,
        device: DeviceId,
        community: &str,
        oid: &str,
    ) -> Result<SnmpValue, String> {
        let oid = snmp_oid(oid)?;
        self.snmp_authorize(device, community, false)?;
        self.snmp_value(device, &oid)
    }

    fn snmp_value(&self, device: DeviceId, oid: &str) -> Result<SnmpValue, String> {
        let dev = self.device(device).ok_or("SNMP device not found.")?;
        let services = self
            .switch_services(device)
            .ok_or("SNMP target is not a switch.")?;
        match oid {
            "1.3.6.1.2.1.1.1.0" => {
                return Ok(SnmpValue::Text(match &dev.kind {
                    DeviceKind::Switch(s) => s.model.spec().name.clone(),
                    _ => dev.name.clone(),
                }));
            }
            "1.3.6.1.2.1.1.3.0" => {
                return Ok(SnmpValue::TimeTicks(
                    self.simulation_time_ms().saturating_sub(
                        self.runtime
                            .device_started
                            .get(&device)
                            .copied()
                            .unwrap_or(0),
                    ) / 10,
                ));
            }
            "1.3.6.1.2.1.1.4.0" => return Ok(SnmpValue::Text(services.snmp.contact.clone())),
            "1.3.6.1.2.1.1.5.0" => {
                return Ok(SnmpValue::Text(
                    self.console_hostname(device).unwrap_or("Switch").into(),
                ));
            }
            "1.3.6.1.2.1.1.6.0" => return Ok(SnmpValue::Text(services.snmp.location.clone())),
            "1.3.6.1.2.1.2.1.0" => return Ok(SnmpValue::Integer(dev.ports().len() as u64)),
            _ => {}
        }
        if let Some((column, index)) = oid
            .strip_prefix("1.3.6.1.2.1.31.1.1.1.")
            .and_then(|suffix| suffix.split_once('.'))
        {
            let index = index.parse::<usize>().map_err(|_| "noSuchInstance")?;
            let id = *index
                .checked_sub(1)
                .and_then(|i| dev.ports().get(i))
                .ok_or("noSuchInstance")?;
            let counters = self.port_telemetry(id);
            return match column {
                "1" => Ok(SnmpValue::Text(self.ios_interface_name(device, id))),
                "6" => Ok(SnmpValue::Counter64(counters.rx_bytes)),
                "10" => Ok(SnmpValue::Counter64(counters.tx_bytes)),
                "15" => {
                    Ok(SnmpValue::Gauge32(self.physical_link_speed(id).map_or(
                        self.ports[&id].advertised_speed.mbps(),
                        |speed| speed.mbps(),
                    )))
                }
                _ => Err("noSuchObject".into()),
            };
        }
        let Some((column, index)) = oid
            .strip_prefix("1.3.6.1.2.1.2.2.1.")
            .and_then(|suffix| suffix.split_once('.'))
        else {
            return Err("noSuchObject".into());
        };
        let index = index.parse::<usize>().map_err(|_| "noSuchInstance")?;
        let id = *index
            .checked_sub(1)
            .and_then(|index| dev.ports().get(index))
            .ok_or("noSuchInstance")?;
        let port = self.port(id).ok_or("noSuchInstance")?;
        let counters = self.port_telemetry(id);
        match column {
            "1" => Ok(SnmpValue::Integer(index as u64)),
            "2" => Ok(SnmpValue::Text(self.ios_interface_name(device, id))),
            "3" => Ok(SnmpValue::Integer(6)),
            "4" => Ok(SnmpValue::Integer(1500)),
            "5" => Ok(SnmpValue::Gauge32(
                (u64::from(
                    self.physical_link_speed(id)
                        .map_or(port.advertised_speed.mbps(), |s| s.mbps()),
                ) * 1_000_000)
                    .min(u64::from(u32::MAX)) as u32,
            )),
            "7" => Ok(SnmpValue::Integer(if port.enabled { 1 } else { 2 })),
            "8" => Ok(SnmpValue::Integer(
                if self.physical_link_up(id) && self.channel_forwarding(id) {
                    1
                } else {
                    2
                },
            )),
            "10" => Ok(SnmpValue::Counter32(counters.rx_bytes as u32)),
            "16" => Ok(SnmpValue::Counter32(counters.tx_bytes as u32)),
            _ => Err("noSuchObject".into()),
        }
    }

    pub fn snmp_set(
        &mut self,
        device: DeviceId,
        community: &str,
        oid: &str,
        value: &str,
    ) -> Result<(), String> {
        let oid = snmp_oid(oid)?;
        self.snmp_authorize(device, community, true)?;
        match oid.as_str() {
            "1.3.6.1.2.1.1.4.0" => self.switch_services_mut(device)?.snmp.contact = value.into(),
            "1.3.6.1.2.1.1.6.0" => self.switch_services_mut(device)?.snmp.location = value.into(),
            "1.3.6.1.2.1.1.5.0" => {
                if value.is_empty() || value.chars().any(char::is_whitespace) {
                    return Err("Invalid hostname.".into());
                }
                self.ios_configs.entry(device).or_default().hostname = Some(value.into());
            }
            _ => {
                let Some(index) = oid
                    .strip_prefix("1.3.6.1.2.1.2.2.1.7.")
                    .and_then(|n| n.parse::<usize>().ok())
                    .and_then(|n| n.checked_sub(1))
                else {
                    return Err("notWritable".into());
                };
                let port = *self.devices[&device]
                    .ports()
                    .get(index)
                    .ok_or("noSuchInstance")?;
                let enabled = match value {
                    "1" => true,
                    "2" => false,
                    _ => return Err("wrongValue: ifAdminStatus must be 1 or 2".into()),
                };
                self.ports.get_mut(&port).unwrap().enabled = enabled;
                self.topology_revision += 1;
            }
        }
        Ok(())
    }

    pub fn snmp_walk(
        &mut self,
        device: DeviceId,
        community: &str,
        root: &str,
    ) -> Result<Vec<(String, SnmpValue)>, String> {
        let root = snmp_oid(root)?;
        self.snmp_authorize(device, community, false)?;
        let mut oids: Vec<String> = [1, 3, 4, 5, 6]
            .into_iter()
            .map(|column| format!("1.3.6.1.2.1.1.{column}.0"))
            .collect();
        oids.push("1.3.6.1.2.1.2.1.0".into());
        for column in [1, 2, 3, 4, 5, 7, 8, 10, 16] {
            for index in 1..=self.devices[&device].ports().len() {
                oids.push(format!("1.3.6.1.2.1.2.2.1.{column}.{index}"));
            }
        }
        for column in [1, 6, 10, 15] {
            for index in 1..=self.devices[&device].ports().len() {
                oids.push(format!("1.3.6.1.2.1.31.1.1.1.{column}.{index}"));
            }
        }
        oids.sort_by_key(|oid| {
            oid.split('.')
                .map(|p| p.parse::<u32>().unwrap())
                .collect::<Vec<_>>()
        });
        oids.into_iter()
            .filter(|oid| {
                oid == &root
                    || oid
                        .strip_prefix(&root)
                        .is_some_and(|suffix| suffix.starts_with('.'))
            })
            .map(|oid| self.snmp_value(device, &oid).map(|value| (oid, value)))
            .collect()
    }

    pub fn snmp_target(&mut self, source: PortId, address: Ipv4Addr) -> Result<DeviceId, String> {
        let target = self
            .devices()
            .find(|device| {
                self.switch_management(device.id)
                    .is_some_and(|m| m.address == address)
            })
            .map(|device| device.id)
            .ok_or("SNMP management IP not found.")?;
        if !self.probe_ipv4_protocol(source, address, 64, 17).reachable {
            return Err("SNMP management IP is unreachable.".into());
        }
        Ok(target)
    }
}
