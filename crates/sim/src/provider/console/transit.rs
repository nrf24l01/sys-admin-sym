use super::*;

impl ProviderConsole {
    pub(super) fn parse_transit(
        sim: &NetworkSim,
        words: &[&str],
    ) -> Result<ProviderCommand, String> {
        Ok(match words {
            ["transit", "add", id, address, asn, mbps, name] => {
                let (address, prefix) = cidr(address)?;
                let id = port(id)?;
                if sim.provider().circuit(id).is_some() {
                    return Err(
                        "circuit already exists; delete it before replacing its handoff".into(),
                    );
                }
                ProviderCommand::SetTransit(TransitCircuit {
                    port: id,
                    name: (*name).into(),
                    address,
                    prefix,
                    asn: parse(asn)?,
                    capacity_mbps: parse(mbps)?,
                    enabled: true,
                    routes: vec![],
                    offered_routes: vec![],
                    authorizations: vec![],
                })
            }
            ["transit", "state", id, enabled] => {
                let mut circuit = Self::circuit(sim, id)?;
                circuit.enabled = state(enabled)?;
                ProviderCommand::SetTransit(circuit)
            }
            ["transit", "delete", id] => ProviderCommand::RemoveTransit(port(id)?),
            ["upstream-route", op, id, prefix, next] => {
                let mut circuit = Self::circuit(sim, id)?;
                let route = UpstreamRoute {
                    prefix: parse(prefix)?,
                    next_hop: parse(next)?,
                };
                circuit.routes.retain(|r| *r != route);
                if operation(op)? {
                    circuit.routes.push(route);
                }
                ProviderCommand::SetTransit(circuit)
            }
            ["offer", op, id, prefix] => {
                let mut circuit = Self::circuit(sim, id)?;
                let prefix = parse(prefix)?;
                circuit.offered_routes.retain(|p| *p != prefix);
                if operation(op)? {
                    circuit.offered_routes.push(prefix);
                }
                ProviderCommand::SetTransit(circuit)
            }
            ["authorize", op, id, prefix, max, asn] => {
                let mut circuit = Self::circuit(sim, id)?;
                let authorization = PrefixAuthorization {
                    prefix: parse(prefix)?,
                    max_length: parse(max)?,
                    origin_asn: parse(asn)?,
                };
                circuit.authorizations.retain(|a| *a != authorization);
                if operation(op)? {
                    circuit.authorizations.push(authorization);
                }
                ProviderCommand::SetTransit(circuit)
            }
            _ => return Err("invalid transit command; use netctl help".into()),
        })
    }
}
