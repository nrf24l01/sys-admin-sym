mod bgp;
mod policy;
mod status;
mod transit;

// Infrastructure administration; registered once in the simulated Linux shell.
use crate::*;
use std::{net::Ipv4Addr, str::FromStr};

pub struct ProviderConsole;
const HELP: &str = "netctl show [ports|transit|pools|bgp|bindings|policies|routes|capacity|external]\n\
netctl external add ADDRESS echo|silent up|down | netctl external delete ADDRESS\n\
netctl transit add PORT ADDRESS/PREFIX ASN MBPS NAME\n\
netctl transit state PORT up|down\n\
netctl transit delete PORT\n\
netctl upstream-route add|delete CIRCUIT PREFIX NEXT-HOP\n\
netctl offer add|delete CIRCUIT PREFIX\n\
netctl authorize add|delete CIRCUIT PREFIX MAX-LENGTH ORIGIN-ASN\n\
netctl pool add PREFIX DESCRIPTION | netctl pool delete PREFIX\n\
netctl allocate PORT PREFIX GATEWAY|none VLAN|untagged\n\
netctl bind PORT VLAN DOMAIN public|private|management|storage|oob\n\
netctl unbind PORT VLAN\n\
netctl bgp add PORT VLAN CIRCUIT LOCAL-AS PEER-AS MAX-PREFIXES PREFERENCE\n\
netctl bgp delete PORT VLAN\n\
netctl bgp state PORT VLAN up|down\n\
netctl bgp import|export PORT VLAN PREFIX add|delete\n\
netctl policy in|out PORT VLAN permit|deny\n\
netctl policy delete in|out PORT VLAN\n\
netctl rule in|out PORT VLAN SOURCE-PREFIX DEST-PREFIX PROTOCOL|any permit|deny\n\
netctl sources PORT VLAN PREFIX[,PREFIX...]|none\n\
netctl route add|delete ROUTER DOMAIN PREFIX PORT VLAN NEXT-HOP|direct PREFERENCE track|keep\n\
netctl preference ROUTER PREFIX EGRESS NEXT-HOP|direct PREFERENCE\n\
netctl stp SWITCH on|off\n\
netctl management SWITCH ADDRESS/PREFIX VLAN GATEWAY|none\n\
netctl dhcp add SERVER VLAN PREFIX FIRST LAST GATEWAY|none\n\
netctl dhcp delete SERVER VLAN\n\
netctl ping-in ADDRESS\n\
All identifiers are simulation IDs. Circuit routing and server IP configuration are separate.\n";

fn parse<T: FromStr>(value: &str) -> Result<T, String> {
    value.parse().map_err(|_| format!("invalid value: {value}"))
}
fn port(value: &str) -> Result<PortId, String> {
    Ok(PortId(parse(value)?))
}
fn vlan(value: &str) -> Result<VlanId, String> {
    Ok(VlanId(parse(value)?))
}
fn cidr(value: &str) -> Result<(Ipv4Addr, u8), String> {
    let (address, length) = value.split_once('/').ok_or("expected address/prefix")?;
    Ok((parse(address)?, parse(length)?))
}
fn state(value: &str) -> Result<bool, String> {
    match value {
        "up" | "on" => Ok(true),
        "down" | "off" => Ok(false),
        _ => Err("expected up/down or on/off".into()),
    }
}
fn action(value: &str) -> Result<PolicyAction, String> {
    match value {
        "permit" => Ok(PolicyAction::Permit),
        "deny" => Ok(PolicyAction::Deny),
        _ => Err("expected permit or deny".into()),
    }
}
fn ingress(value: &str) -> Result<bool, String> {
    match value {
        "in" => Ok(true),
        "out" => Ok(false),
        _ => Err("expected in or out".into()),
    }
}
fn operation(value: &str) -> Result<bool, String> {
    match value {
        "add" => Ok(true),
        "delete" => Ok(false),
        _ => Err("expected add or delete".into()),
    }
}

impl LinuxCommand for ProviderConsole {
    fn name(&self) -> &'static str {
        "netctl"
    }
    fn execute(
        &self,
        sim: &mut NetworkSim,
        device: DeviceId,
        args: &[String],
        _stdin: &str,
    ) -> Result<String, String> {
        Self::run_scoped(sim, args, Some(device))
    }
    fn suggest(&self, context: &CompletionContext<'_>) -> Vec<String> {
        if context.args.is_empty() {
            CompletionContext::choices(&[
                "show",
                "transit",
                "external",
                "upstream-route",
                "offer",
                "authorize",
                "pool",
                "allocate",
                "bind",
                "unbind",
                "bgp",
                "policy",
                "rule",
                "sources",
                "route",
                "preference",
                "stp",
                "management",
                "dhcp",
                "ping-in",
                "help",
            ])
        } else if context.args[0] == "show" {
            CompletionContext::choices(&[
                "ports", "transit", "pools", "bgp", "bindings", "policies", "routes", "capacity",
                "external",
            ])
        } else {
            Vec::new()
        }
    }
}

impl ProviderConsole {
    /// Scenario tooling entry point. Guest consoles use the device-scoped path.
    pub fn run(sim: &mut NetworkSim, args: &[String]) -> Result<String, String> {
        Self::run_scoped(sim, args, None)
    }

    fn run_scoped(
        sim: &mut NetworkSim,
        args: &[String],
        device: Option<DeviceId>,
    ) -> Result<String, String> {
        let words: Vec<_> = args.iter().map(String::as_str).collect();
        let command = match words.as_slice() {
            [] | ["help"] => return Ok(HELP.into()),
            ["show"] => return Self::show(sim, "transit"),
            ["show", topic] => return Self::show(sim, topic),
            ["external", "add", address, echo, enabled] => {
                ProviderCommand::SetExternalHost(ExternalHost {
                    address: parse(address)?,
                    available: state(enabled)?,
                    echo_enabled: match *echo {
                        "echo" => true,
                        "silent" => false,
                        _ => return Err("expected echo or silent".into()),
                    },
                })
            }
            ["external", "delete", address] => ProviderCommand::RemoveExternalHost(parse(address)?),
            ["ping-in", address] => {
                return Ok(format!("{:?}\n", sim.ping_from_internet(parse(address)?)));
            }
            ["dhcp", "add", server, tag, prefix, first, last, gateway] => {
                ProviderCommand::SetDhcp(DhcpPool {
                    server: port(server)?,
                    vlan: vlan(tag)?,
                    prefix: parse(prefix)?,
                    first: parse(first)?,
                    last: parse(last)?,
                    gateway: if *gateway == "none" {
                        None
                    } else {
                        Some(parse(gateway)?)
                    },
                })
            }
            ["dhcp", "delete", server, tag] => ProviderCommand::RemoveDhcp {
                server: port(server)?,
                vlan: vlan(tag)?,
            },
            [category, ..]
                if matches!(
                    *category,
                    "transit" | "upstream-route" | "offer" | "authorize"
                ) =>
            {
                Self::parse_transit(sim, words.as_slice())?
            }
            ["pool", "add", prefix, description] => ProviderCommand::AddPool(AddressPool {
                prefix: parse(prefix)?,
                description: (*description).into(),
            }),
            ["pool", "delete", prefix] => ProviderCommand::RemovePool(parse(prefix)?),
            ["allocate", id, prefix, gateway, tag] => ProviderCommand::AllocateAddress {
                port: port(id)?,
                prefix: parse(prefix)?,
                gateway: if *gateway == "none" {
                    None
                } else {
                    Some(parse(gateway)?)
                },
                vlan: if *tag == "untagged" {
                    None
                } else {
                    Some(vlan(tag)?)
                },
            },
            ["bind", id, tag, domain, role] => ProviderCommand::BindInterface(InterfaceBinding {
                port: port(id)?,
                vlan: vlan(tag)?,
                domain: RoutingDomain(parse(domain)?),
                role: match *role {
                    "public" => NetworkRole::Public,
                    "private" => NetworkRole::Private,
                    "management" => NetworkRole::Management,
                    "storage" => NetworkRole::Storage,
                    "oob" => NetworkRole::OutOfBand,
                    _ => return Err("unknown network role".into()),
                },
            }),
            ["unbind", id, tag] => ProviderCommand::RemoveBinding {
                port: port(id)?,
                vlan: vlan(tag)?,
            },
            [category, ..] if matches!(*category, "bgp") => Self::parse_bgp(sim, words.as_slice())?,
            [category, ..] if matches!(*category, "policy" | "rule" | "sources") => {
                Self::parse_policy(sim, words.as_slice())?
            }
            [
                "route",
                op,
                router,
                domain,
                prefix,
                id,
                tag,
                next,
                preference,
                tracking,
            ] => {
                let route = DomainRoute {
                    router: DeviceId(parse(router)?),
                    domain: RoutingDomain(parse(domain)?),
                    prefix: parse(prefix)?,
                    port: port(id)?,
                    vlan: vlan(tag)?,
                    next_hop: if *next == "direct" {
                        None
                    } else {
                        Some(parse(next)?)
                    },
                    preference: parse(preference)?,
                    track_neighbor: match *tracking {
                        "track" => true,
                        "keep" => false,
                        _ => return Err("expected track or keep".into()),
                    },
                };
                if operation(op)? {
                    ProviderCommand::SetDomainRoute(route)
                } else {
                    ProviderCommand::RemoveDomainRoute(route)
                }
            }
            ["preference", router, prefix, egress, next, preference] => {
                ProviderCommand::SetRoutePreference(RoutePreference {
                    router: DeviceId(parse(router)?),
                    prefix: parse(prefix)?,
                    egress: port(egress)?,
                    via: if *next == "direct" {
                        None
                    } else {
                        Some(parse(next)?)
                    },
                    preference: parse(preference)?,
                })
            }
            ["stp", switch, enabled] => ProviderCommand::SetSpanningTree {
                switch: DeviceId(parse(switch)?),
                enabled: state(enabled)?,
            },
            ["management", switch, address, tag, gateway] => {
                let (address, prefix) = cidr(address)?;
                ProviderCommand::SetSwitchManagement(SwitchManagement {
                    switch: DeviceId(parse(switch)?),
                    address,
                    prefix,
                    vlan: vlan(tag)?,
                    gateway: if *gateway == "none" {
                        None
                    } else {
                        Some(parse(gateway)?)
                    },
                })
            }
            _ => return Err(format!("invalid netctl command\n{HELP}")),
        };
        if let Some(device) = device {
            let owned_port = |port| sim.port(port).is_some_and(|p| p.device == device);
            let local = match &command {
                ProviderCommand::AllocateAddress { port, .. }
                | ProviderCommand::RemoveBinding { port, .. }
                | ProviderCommand::RemoveBgp { port, .. }
                | ProviderCommand::RemovePolicy { port, .. } => owned_port(*port),
                ProviderCommand::SetDomainRoute(r) | ProviderCommand::RemoveDomainRoute(r) => {
                    r.router == device && owned_port(r.port)
                }
                ProviderCommand::ReplaceDomainRoute { previous, route } => {
                    previous.router == device && route.router == device && owned_port(route.port)
                }
                ProviderCommand::SetDhcp(pool) => owned_port(pool.server),
                ProviderCommand::RemoveDhcp { server, .. } => owned_port(*server),
                ProviderCommand::SetSwitchManagement(m) => m.switch == device,
                ProviderCommand::BindInterface(b) => owned_port(b.port),
                ProviderCommand::SetBgp(b) => owned_port(b.port),
                ProviderCommand::SetPolicy(p) => owned_port(p.policy.port),
                ProviderCommand::SetRoutePreference(p) => {
                    p.router == device && owned_port(p.egress)
                }
                ProviderCommand::SetSpanningTree { switch, .. } => *switch == device,
                _ => false,
            };
            if !local {
                return Err("Configure this device's own interfaces only. Configure other devices in their inspectors and carrier service on the uplink socket.".into());
            }
        }
        sim.execute(Command::Provider(command))
            .map_err(|e| e.to_string())?;
        Ok("Configuration applied\n".into())
    }
    fn circuit(sim: &NetworkSim, id: &str) -> Result<TransitCircuit, String> {
        sim.provider()
            .circuit(port(id)?)
            .cloned()
            .ok_or("circuit not found".into())
    }
    fn session(sim: &NetworkSim, id: &str, tag: &str) -> Result<BgpSession, String> {
        let (id, tag) = (port(id)?, vlan(tag)?);
        sim.provider()
            .sessions()
            .iter()
            .find(|s| s.port == id && s.vlan == tag)
            .cloned()
            .ok_or("BGP session not found".into())
    }
    fn policy(
        sim: &NetworkSim,
        direction: &str,
        id: &str,
        tag: &str,
    ) -> Result<PolicyAttachment, String> {
        let (port, vlan, ingress) = (port(id)?, vlan(tag)?, ingress(direction)?);
        Ok(sim
            .provider()
            .policies()
            .iter()
            .find(|a| a.policy.port == port && a.policy.vlan == vlan && a.ingress == ingress)
            .cloned()
            .unwrap_or(PolicyAttachment {
                ingress,
                policy: PacketPolicy {
                    port,
                    vlan,
                    rules: vec![],
                    allowed_sources: vec![],
                    default_action: PolicyAction::Permit,
                },
            }))
    }
}
