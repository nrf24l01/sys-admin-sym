use super::*;

impl ProviderConsole {
    pub(super) fn parse_policy(
        sim: &NetworkSim,
        words: &[&str],
    ) -> Result<ProviderCommand, String> {
        Ok(match words {
            ["policy", direction, id, tag, default]
                if *direction == "in" || *direction == "out" =>
            {
                let mut attachment = Self::policy(sim, direction, id, tag)?;
                attachment.policy.default_action = action(default)?;
                ProviderCommand::SetPolicy(attachment)
            }
            ["policy", "delete", direction, id, tag] => ProviderCommand::RemovePolicy {
                port: port(id)?,
                vlan: vlan(tag)?,
                ingress: ingress(direction)?,
            },
            [
                "rule",
                direction,
                id,
                tag,
                source,
                destination,
                protocol,
                verdict,
            ] => {
                let mut attachment = Self::policy(sim, direction, id, tag)?;
                attachment.policy.rules.push(PacketRule {
                    source: parse(source)?,
                    destination: parse(destination)?,
                    protocol: if *protocol == "any" {
                        None
                    } else {
                        Some(parse(protocol)?)
                    },
                    action: action(verdict)?,
                });
                ProviderCommand::SetPolicy(attachment)
            }
            ["sources", id, tag, prefixes] => {
                let mut attachment = Self::policy(sim, "in", id, tag)?;
                attachment.policy.allowed_sources = if *prefixes == "none" {
                    vec![]
                } else {
                    prefixes.split(',').map(parse).collect::<Result<_, _>>()?
                };
                ProviderCommand::SetPolicy(attachment)
            }
            _ => return Err("invalid policy command; use netctl help".into()),
        })
    }
}
