use super::*;
use crate::*;

fn invalid(message: impl Into<String>) -> SimError {
    SimError::Provider(message.into())
}
fn unicast(ip: std::net::Ipv4Addr) -> bool {
    !ip.is_unspecified() && !ip.is_multicast() && !ip.is_broadcast() && !ip.is_loopback()
}

impl NetworkSim {
    pub fn provider(&self) -> &ProviderNetwork {
        &self.provider
    }

    fn validate_provider_interface(&self, port: PortId, vlan: VlanId) -> Result<(), SimError> {
        if !(1..4095).contains(&vlan.0) {
            return Err(SimError::InvalidIpv4Vlan);
        }
        let port = self.port(port).ok_or(SimError::PortNotFound(port))?;
        if !matches!(
            port.config,
            PortConfig::Server(_) | PortConfig::Router(_) | PortConfig::Switch(_)
        ) {
            return Err(SimError::WrongPortType);
        }
        Ok(())
    }

    fn validate_domain_route(&self, route: &DomainRoute) -> Result<(), SimError> {
        self.validate_provider_interface(route.port, route.vlan)?;
        if self.provider.domain(route.port, route.vlan) != route.domain {
            return Err(invalid(
                "route egress does not belong to its routing domain",
            ));
        }
        let port = self.port(route.port).expect("validated port");
        let PortConfig::Router(config) = &port.config else {
            return Err(SimError::WrongPortType);
        };
        let valid = port.device == route.router
            && config.interfaces.iter().any(|interface| {
                interface.vlan.unwrap_or(VlanId(1)) == route.vlan
                    && interface.address.is_some_and(|address| {
                        route.next_hop.is_none_or(|next| {
                            unicast(next)
                                && next != address
                                && Ipv4Prefix::new(address, interface.prefix)
                                    .is_ok_and(|prefix| prefix.usable(next))
                        })
                    })
            });
        if !valid {
            return Err(invalid(
                "route requires a router interface and a usable on-link next hop",
            ));
        }
        Ok(())
    }

    pub(super) fn validate_provider_command(
        &self,
        command: &ProviderCommand,
    ) -> Result<(), SimError> {
        match command {
            ProviderCommand::SetDomainRoute(route) => self.validate_domain_route(route)?,
            ProviderCommand::ReplaceDomainRoute { previous, route } => {
                self.validate_domain_route(route)?;
                if previous.router != route.router || !self.device(previous.router).is_some_and(|d| {
                    matches!(&d.kind, DeviceKind::Router(r) if r.domain_routes.contains(previous))
                }) {
                    return Err(invalid("the original route no longer exists on this router"));
                }
            }
            ProviderCommand::RemoveDomainRoute(route) => {
                if !self
                    .device(route.router)
                    .is_some_and(|d| matches!(&d.kind, DeviceKind::Router(_)))
                {
                    return Err(SimError::WrongPortType);
                }
            }
            ProviderCommand::SetExternalHost(host)
                if !Self::simulated_public_address(host.address) =>
            {
                return Err(invalid(
                    "external host must have a simulated public unicast address",
                ));
            }
            ProviderCommand::SetDhcp(pool) => {
                self.validate_provider_interface(pool.server, pool.vlan)?;
                let address = self
                    .interface_ipv4(pool.server, pool.vlan)
                    .ok_or_else(|| invalid("DHCP server needs an address on its interface"))?;
                if !pool.prefix.usable(address)
                    || !pool.prefix.usable(pool.first)
                    || !pool.prefix.usable(pool.last)
                    || pool.first > pool.last
                    || pool
                        .gateway
                        .is_some_and(|gateway| !pool.prefix.usable(gateway))
                {
                    return Err(invalid(
                        "DHCP server, range and gateway must belong to the pool subnet",
                    ));
                }
            }
            ProviderCommand::SetSwitchManagement(management) => {
                if !self
                    .device(management.switch)
                    .is_some_and(|d| matches!(d.kind, DeviceKind::Switch(_)))
                {
                    return Err(SimError::WrongPortType);
                }
                let subnet =
                    Ipv4Prefix::new(management.address, management.prefix).map_err(invalid)?;
                if !(1..4095).contains(&management.vlan.0)
                    || !unicast(management.address)
                    || !subnet.usable(management.address)
                    || management.gateway.is_some_and(|gateway| {
                        !unicast(gateway)
                            || !subnet.usable(gateway)
                            || gateway == management.address
                    })
                {
                    return Err(invalid("invalid switch management interface"));
                }
            }
            ProviderCommand::SetTransit(circuit) => {
                if !self
                    .network_outlet(circuit.port)
                    .is_some_and(|o| matches!(o.kind, NetworkOutletKind::Uplink { .. }))
                {
                    return Err(SimError::InvalidPublicUplink);
                }
                let subnet = Ipv4Prefix::new(circuit.address, circuit.prefix).map_err(invalid)?;
                if circuit.prefix == 0
                    || !subnet.usable(circuit.address)
                    || !unicast(circuit.address)
                    || circuit.asn == 0
                    || circuit.capacity_mbps == 0
                    || circuit.name.trim().is_empty()
                {
                    return Err(invalid(
                        "transit requires a unicast handoff address, ASN, name and positive capacity",
                    ));
                }
                for route in &circuit.routes {
                    if !subnet.usable(route.next_hop)
                        || !unicast(route.next_hop)
                        || route.next_hop == circuit.address
                    {
                        return Err(invalid(
                            "upstream route next hop must be a different usable address on the handoff subnet",
                        ));
                    }
                }
                for authorization in &circuit.authorizations {
                    if authorization.origin_asn == 0
                        || authorization.max_length < authorization.prefix.length()
                        || authorization.max_length > 32
                    {
                        return Err(invalid("invalid prefix authorization"));
                    }
                }
            }
            ProviderCommand::AddPool(pool) => {
                if pool.prefix.length() == 0
                    || !unicast(pool.prefix.network())
                    || pool.description.trim().is_empty()
                {
                    return Err(invalid(
                        "address pool requires a unicast prefix and description",
                    ));
                }
                if self.provider.pools.iter().any(|existing| {
                    existing.prefix != pool.prefix
                        && (existing.prefix.contains_prefix(pool.prefix)
                            || pool.prefix.contains_prefix(existing.prefix))
                }) {
                    return Err(invalid("address pools must not overlap"));
                }
            }
            ProviderCommand::RemovePool(prefix) => {
                if self.provider.sessions.iter().any(|session| {
                    session
                        .export_prefixes
                        .iter()
                        .any(|export| prefix.contains_prefix(*export))
                }) {
                    return Err(invalid(
                        "remove BGP exports before deleting their address pool",
                    ));
                }
            }
            ProviderCommand::BindInterface(binding) => {
                self.validate_provider_interface(binding.port, binding.vlan)?
            }
            ProviderCommand::SetBgp(session) => {
                self.validate_provider_interface(session.port, session.vlan)?;
                if !matches!(
                    self.port(session.port).map(|p| &p.config),
                    Some(PortConfig::Router(_))
                ) {
                    return Err(SimError::WrongPortType);
                }
                if session.local_asn == 0
                    || session.peer_asn == 0
                    || session.local_asn == session.peer_asn
                    || session.max_prefixes == 0
                    || self.provider.circuit(session.circuit).is_none()
                {
                    return Err(invalid(
                        "eBGP requires distinct nonzero ASNs, a prefix limit and a configured circuit",
                    ));
                }
                for prefix in &session.export_prefixes {
                    if !self.provider.owns(*prefix) {
                        return Err(invalid(format!(
                            "export {prefix} is outside owned address pools"
                        )));
                    }
                }
            }
            ProviderCommand::SetPolicy(attachment) => {
                self.validate_provider_interface(attachment.policy.port, attachment.policy.vlan)?
            }
            ProviderCommand::SetRoutePreference(preference) => {
                let valid = self.device(preference.router).is_some_and(|device| {
                    let DeviceKind::Router(router) = &device.kind else {
                        return false;
                    };
                    router.routes.iter().any(|route| {
                        route.egress == preference.egress
                            && route.via == preference.via
                            && route.network == preference.prefix.network()
                            && route.prefix == preference.prefix.length()
                    })
                });
                if !valid {
                    return Err(invalid(
                        "configure the matching static route before its preference",
                    ));
                }
            }
            ProviderCommand::SetSpanningTree { switch, .. }
                if !self
                    .device(*switch)
                    .is_some_and(|device| matches!(device.kind, DeviceKind::Switch(_))) =>
            {
                return Err(SimError::WrongPortType);
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn configure_provider(&mut self, command: ProviderCommand) -> Result<(), SimError> {
        if let ProviderCommand::RouteOwnedRange { prefix, uplink } = command {
            return self.route_owned_range(prefix, uplink);
        }
        if let ProviderCommand::AllocateAddress {
            port,
            prefix,
            gateway,
            vlan,
        } = command
        {
            self.allocate_ipv4(port, prefix, gateway, vlan)?;
            return Ok(());
        }
        self.validate_provider_command(&command)?;
        let provider = &mut self.provider;
        match command {
            ProviderCommand::AllocateAddress { .. } | ProviderCommand::RouteOwnedRange { .. } => {
                unreachable!("handled above")
            }
            ProviderCommand::SetExternalHost(host) => provider.external.set_host(host),
            ProviderCommand::RemoveExternalHost(address) => {
                provider.external.hosts.retain(|h| h.address != address)
            }

            ProviderCommand::SetDomainRoute(route)
            | ProviderCommand::ReplaceDomainRoute { route, .. }
            | ProviderCommand::RemoveDomainRoute(route) => {
                let device = self
                    .devices
                    .get_mut(&route.router)
                    .expect("validated router");
                let DeviceKind::Router(router) = &mut device.kind else {
                    unreachable!()
                };
                if let ProviderCommand::ReplaceDomainRoute { previous, .. } = command {
                    router.domain_routes.retain(|old| *old != previous);
                }
                router.domain_routes.retain(|old| {
                    old.domain != route.domain
                        || old.prefix != route.prefix
                        || old.port != route.port
                        || old.vlan != route.vlan
                        || old.next_hop != route.next_hop
                });
                if !matches!(command, ProviderCommand::RemoveDomainRoute(_)) {
                    router.domain_routes.push(route);
                }
            }

            ProviderCommand::SetDhcp(pool) => {
                provider
                    .dhcp
                    .retain(|old| old.server != pool.server || old.vlan != pool.vlan);
                provider.leases.retain(|lease| {
                    lease.server != pool.server
                        || lease.vlan != pool.vlan
                        || (lease.address >= pool.first && lease.address <= pool.last)
                });
                provider.dhcp.push(pool);
            }
            ProviderCommand::RemoveDhcp { server, vlan } => {
                provider
                    .dhcp
                    .retain(|p| p.server != server || p.vlan != vlan);
                provider
                    .leases
                    .retain(|p| p.server != server || p.vlan != vlan);
            }

            ProviderCommand::SetSwitchManagement(m) => {
                provider.management.retain(|old| old.switch != m.switch);
                provider.management.push(m);
            }
            ProviderCommand::SetTransit(c) => {
                provider.circuits.retain(|old| old.port != c.port);
                provider.circuits.push(c);
                provider.circuits.sort_by_key(|c| c.port);
            }
            ProviderCommand::RemoveTransit(port) => {
                provider.circuits.retain(|c| c.port != port);
                provider.sessions.retain(|s| s.circuit != port);
            }
            ProviderCommand::AddPool(p) => {
                provider.pools.retain(|old| old.prefix != p.prefix);
                provider.pools.push(p);
                provider.pools.sort_by_key(|p| p.prefix);
            }
            ProviderCommand::RemovePool(prefix) => provider.pools.retain(|p| p.prefix != prefix),
            ProviderCommand::BindInterface(b) => {
                provider
                    .bindings
                    .retain(|old| old.port != b.port || old.vlan != b.vlan);
                provider.bindings.push(b);
            }
            ProviderCommand::RemoveBinding { port, vlan } => provider
                .bindings
                .retain(|b| b.port != port || b.vlan != vlan),
            ProviderCommand::SetBgp(s) => {
                provider
                    .sessions
                    .retain(|old| old.port != s.port || old.vlan != s.vlan);
                provider.sessions.push(s);
                provider.sessions.sort_by_key(|s| (s.port, s.vlan));
            }
            ProviderCommand::RemoveBgp { port, vlan } => provider
                .sessions
                .retain(|s| s.port != port || s.vlan != vlan),
            ProviderCommand::SetPolicy(a) => {
                provider.policies.retain(|old| {
                    old.ingress != a.ingress
                        || old.policy.port != a.policy.port
                        || old.policy.vlan != a.policy.vlan
                });
                provider.policies.push(a);
            }
            ProviderCommand::RemovePolicy {
                port,
                vlan,
                ingress,
            } => provider
                .policies
                .retain(|a| a.ingress != ingress || a.policy.port != port || a.policy.vlan != vlan),
            ProviderCommand::SetRoutePreference(p) => {
                provider.preferences.retain(|old| {
                    old.router != p.router
                        || old.prefix != p.prefix
                        || old.egress != p.egress
                        || old.via != p.via
                });
                provider.preferences.push(p);
            }
            ProviderCommand::SetSpanningTree { switch, enabled } => {
                provider.spanning_tree_disabled.retain(|id| *id != switch);
                if !enabled {
                    provider.spanning_tree_disabled.push(switch);
                }
            }
        }
        self.routing_revision += 1;
        self.topology_revision += 1;
        Ok(())
    }
}
