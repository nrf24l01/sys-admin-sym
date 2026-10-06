use super::*;

impl ProviderConsole {
    pub(super) fn parse_bgp(sim: &NetworkSim, words: &[&str]) -> Result<ProviderCommand, String> {
        Ok(match words {
            ["bgp", "add", id, tag, circuit, local, peer, max, preference] => {
                ProviderCommand::SetBgp(BgpSession {
                    port: port(id)?,
                    vlan: vlan(tag)?,
                    circuit: port(circuit)?,
                    local_asn: parse(local)?,
                    peer_asn: parse(peer)?,
                    max_prefixes: parse(max)?,
                    preference: parse(preference)?,
                    enabled: true,
                    import_prefixes: vec![],
                    export_prefixes: vec![],
                })
            }
            ["bgp", "delete", id, tag] => ProviderCommand::RemoveBgp {
                port: port(id)?,
                vlan: vlan(tag)?,
            },
            ["bgp", "state", id, tag, enabled] => {
                let mut session = Self::session(sim, id, tag)?;
                session.enabled = state(enabled)?;
                ProviderCommand::SetBgp(session)
            }
            ["bgp", direction, id, tag, prefix, op]
                if *direction == "import" || *direction == "export" =>
            {
                let mut session = Self::session(sim, id, tag)?;
                let prefix = parse(prefix)?;
                let list = if *direction == "import" {
                    &mut session.import_prefixes
                } else {
                    &mut session.export_prefixes
                };
                list.retain(|p| *p != prefix);
                if operation(op)? {
                    list.push(prefix);
                }
                ProviderCommand::SetBgp(session)
            }
            _ => return Err("invalid bgp command; use netctl help".into()),
        })
    }
}
