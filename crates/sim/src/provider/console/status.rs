use super::*;

impl ProviderConsole {
    pub(super) fn show(sim: &NetworkSim, topic: &str) -> Result<String, String> {
        Ok(match topic {
            "external" => sim
                .provider()
                .external_hosts()
                .iter()
                .map(|host| {
                    format!(
                        "{} {} ICMP {}\n",
                        host.address,
                        if host.available { "up" } else { "down" },
                        if host.echo_enabled {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    )
                })
                .collect(),
            "ports" => {
                let mut ports: Vec<_> = sim.ports().collect();
                ports.sort_by_key(|p| p.id);
                ports
                    .iter()
                    .map(|p| {
                        format!(
                            "{} device={} {} {}\n",
                            p.id,
                            p.device,
                            p.name,
                            if sim.port_link_up(p.id) { "up" } else { "down" }
                        )
                    })
                    .collect()
            }
            "transit" => sim
                .provider()
                .circuits()
                .iter()
                .map(|c| {
                    format!(
                        "{} {} {}/{} AS{} {} Mb/s {} routes={:?}\n",
                        c.port,
                        c.name,
                        c.address,
                        c.prefix,
                        c.asn,
                        c.capacity_mbps,
                        if c.enabled && sim.port_link_up(c.port) {
                            "up"
                        } else {
                            "down"
                        },
                        c.routes
                    )
                })
                .collect(),
            "pools" => sim
                .provider()
                .pools()
                .iter()
                .map(|p| format!("{} {}\n", p.prefix, p.description))
                .collect(),
            "bindings" => sim
                .provider()
                .bindings()
                .iter()
                .map(|b| {
                    format!(
                        "port={} vlan={} domain={} {:?}\n",
                        b.port, b.vlan, b.domain.0, b.role
                    )
                })
                .collect(),
            "routes" => sim
                .devices()
                .filter_map(|d| match &d.kind {
                    DeviceKind::Router(router) => Some(&router.domain_routes),
                    _ => None,
                })
                .flatten()
                .map(|route| {
                    format!(
                        "router={} domain={} {} via {} port={} vlan={} preference={} {}\n",
                        route.router,
                        route.domain.0,
                        route.prefix,
                        route
                            .next_hop
                            .map_or_else(|| "direct".into(), |ip| ip.to_string()),
                        route.port,
                        route.vlan,
                        route.preference,
                        if route.track_neighbor {
                            "tracked"
                        } else {
                            "untracked"
                        }
                    )
                })
                .collect(),
            "policies" => sim
                .provider()
                .policies()
                .iter()
                .map(|attachment| {
                    let policy = &attachment.policy;
                    let verdict = |action| {
                        if action == PolicyAction::Permit {
                            "permit"
                        } else {
                            "deny"
                        }
                    };
                    let mut output = format!(
                        "{} port={} vlan={} default={} sources={}\n",
                        if attachment.ingress { "in" } else { "out" },
                        policy.port,
                        policy.vlan,
                        verdict(policy.default_action),
                        if policy.allowed_sources.is_empty() {
                            "unrestricted".into()
                        } else {
                            policy
                                .allowed_sources
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(",")
                        }
                    );
                    for (index, rule) in policy.rules.iter().enumerate() {
                        output.push_str(&format!(
                            "  {} {} -> {} protocol={} {}\n",
                            index + 1,
                            rule.source,
                            rule.destination,
                            rule.protocol
                                .map_or_else(|| "any".into(), |p| p.to_string()),
                            verdict(rule.action)
                        ));
                    }
                    output
                })
                .collect(),
            "capacity" => {
                let c = sim.network_capacity();
                format!(
                    "Server ports: {} Mb/s\nActive transit: {} Mb/s\nAfter largest circuit failure: {} Mb/s\n",
                    c.server_ports_mbps, c.active_transit_mbps, c.largest_transit_failure_mbps
                )
            }
            "bgp" => sim
                .provider()
                .sessions()
                .iter()
                .map(|s| {
                    format!(
                        "port={} vlan={} circuit={} AS{} -> AS{} {:?}; exports: {}\n",
                        s.port,
                        s.vlan,
                        s.circuit,
                        s.local_asn,
                        s.peer_asn,
                        sim.bgp_state(s),
                        s.export_prefixes
                            .iter()
                            .map(|p| format!(
                                "{} {}",
                                p,
                                if sim.bgp_export_accepted(s, *p) {
                                    "accepted"
                                } else {
                                    "rejected"
                                }
                            ))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
                .collect(),
            _ => return Err("unknown topic; use netctl help".into()),
        })
    }
}
