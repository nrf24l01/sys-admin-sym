use cloud_provider_sim::*;

pub struct RouteDraft {
    pub destination: String,
    pub next_hop: String,
    pub interface: Option<(PortId, VlanId)>,
    pub preference: u32,
    pub direct: bool,
    pub track_neighbor: bool,
    pub editing: Option<DomainRoute>,
    pub error: Option<crate::localization::UiMessage>,
}

impl Default for RouteDraft {
    fn default() -> Self {
        Self {
            destination: "0.0.0.0/0".into(),
            next_hop: String::new(),
            interface: None,
            preference: 1,
            direct: false,
            track_neighbor: false,
            editing: None,
            error: None,
        }
    }
}

impl RouteDraft {
    pub fn edit(route: DomainRoute) -> Self {
        Self {
            destination: route.prefix.to_string(),
            next_hop: route.next_hop.map_or_else(String::new, |ip| ip.to_string()),
            interface: Some((route.port, route.vlan)),
            preference: route.preference,
            direct: route.next_hop.is_none(),
            track_neighbor: route.track_neighbor,
            editing: Some(route),
            error: None,
        }
    }

    pub fn command(
        &self,
        sim: &NetworkSim,
        router: DeviceId,
    ) -> Result<Command, crate::localization::UiMessage> {
        let (port, vlan) = self.interface.ok_or("ui.select-an-outgoing-interface")?;
        if !sim
            .port(port)
            .is_some_and(|p| p.device == router && matches!(p.config, PortConfig::Router(_)))
        {
            return Err("ui.select-an-interface-on-this-router".into());
        }
        let route = DomainRoute {
            router,
            port,
            vlan,
            domain: sim.provider().domain(port, vlan),
            prefix: self
                .destination
                .trim()
                .parse()
                .map_err(|_| "ui.enter-a-destination-such-as-10-20")?,
            next_hop: if self.direct {
                None
            } else {
                Some(
                    self.next_hop
                        .trim()
                        .parse()
                        .map_err(|_| "ui.enter-the-next-hop-ipv4-address")?,
                )
            },
            preference: self.preference,
            track_neighbor: self.track_neighbor && !self.direct,
        };
        Ok(Command::Provider(match self.editing {
            Some(previous) => ProviderCommand::ReplaceDomainRoute { previous, route },
            None => ProviderCommand::SetDomainRoute(route),
        }))
    }
}
