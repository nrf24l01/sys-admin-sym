use super::{CompletionContext, LinuxCommand};
use crate::*;
use std::net::Ipv4Addr;

pub(super) struct SnmpCommand(pub &'static str);

impl LinuxCommand for SnmpCommand {
    fn name(&self) -> &'static str {
        self.0
    }
    fn execute(
        &self,
        sim: &mut NetworkSim,
        device: DeviceId,
        args: &[String],
        _stdin: &str,
    ) -> Result<String, String> {
        let mut index = 0;
        let mut version = None;
        let mut community = None;
        while let Some(option) = args.get(index).filter(|arg| arg.starts_with('-')) {
            let value = args.get(index + 1).ok_or("SNMP option requires a value.")?;
            match option.as_str() {
                "-v" => version = Some(value.as_str()),
                "-c" => community = Some(value.as_str()),
                _ => return Err("Supported options: -v 2c -c COMMUNITY".into()),
            }
            index += 2;
        }
        if version != Some("2c") {
            return Err("Specify -v 2c; the simulator implements semantic SNMPv2c.".into());
        }
        let community = community.ok_or("Specify -c COMMUNITY")?;
        let address: Ipv4Addr = args
            .get(index)
            .ok_or("Missing management address.")?
            .parse()
            .map_err(|_| "Invalid management IPv4 address.")?;
        let oid = args
            .get(index + 1)
            .map(String::as_str)
            .unwrap_or("1.3.6.1.2.1");
        let source = sim
            .server_route_selection(device, address, None)
            .map(|route| route.port)
            .ok_or(
                "SNMP management IP is unreachable; configure and connect a server interface.",
            )?;
        let target = sim.snmp_target(source, address)?;
        let output = match self.0 {
            "snmpget" => {
                if args.len() != index + 2 {
                    return Err("usage: snmpget -v 2c -c COMMUNITY ADDRESS OID".into());
                }
                format!(
                    "{} = {}",
                    snmp_oid(oid)?,
                    sim.snmp_get(target, community, oid)?
                )
            }
            "snmpwalk" => {
                if args.len() > index + 2 {
                    return Err("usage: snmpwalk -v 2c -c COMMUNITY ADDRESS [OID]".into());
                }
                sim.snmp_walk(target, community, oid)?
                    .into_iter()
                    .map(|(oid, value)| format!("{oid} = {value}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            "snmpset" => {
                if args.len() != index + 4 {
                    return Err("usage: snmpset -v 2c -c COMMUNITY ADDRESS OID s|i VALUE".into());
                }
                let oid = snmp_oid(oid)?;
                let expected = if oid.starts_with("1.3.6.1.2.1.2.2.1.7.") {
                    "i"
                } else {
                    "s"
                };
                if args[index + 2] != expected {
                    return Err(format!("wrongType: expected {expected}"));
                }
                sim.snmp_set(target, community, &oid, &args[index + 3])?;
                format!("{oid} updated")
            }
            _ => unreachable!(),
        };
        Ok(format!("{output}\n"))
    }
    fn suggest(&self, context: &CompletionContext<'_>) -> Vec<String> {
        match context.last() {
            Some("-v") => vec!["2c".into()],
            Some("-c") => vec![],
            _ => CompletionContext::choices(&[
                "-v",
                "-c",
                "sysDescr.0",
                "sysName.0",
                "sysUpTime.0",
                "ifNumber.0",
            ]),
        }
    }
}
