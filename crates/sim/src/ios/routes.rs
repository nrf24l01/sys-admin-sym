use super::{mask_prefix, prefix_mask};
use crate::*;
use std::net::Ipv4Addr;

impl NetworkSim {
    pub(super) fn ios_static_route(
        &mut self,
        device: DeviceId,
        args: &[String],
        remove: bool,
    ) -> Result<(), String> {
        let network: Ipv4Addr = args[0]
            .parse()
            .map_err(|_| "% Invalid destination IPv4 address.")?;
        let prefix =
            Ipv4Prefix::new(network, mask_prefix(&args[1])?).map_err(|e| format!("% {e}"))?;
        if network != prefix.network() {
            return Err("% Inconsistent destination address and mask.".into());
        }
        let next: Ipv4Addr = args
            .last()
            .expect("route grammar")
            .parse()
            .map_err(|_| "% Invalid next-hop IPv4 address.")?;
        let explicit = if args.len() == 4 {
            Some(self.ios_find_interface(device, &args[2])?)
        } else {
            None
        };
        let DeviceKind::Router(router) = &self.devices[&device].kind else {
            return Err("% Static routing requires a router.".into());
        };
        let interface_matches = |interface: &RouterInterface| {
            explicit.is_none_or(|(port, sub)| {
                interface.port == port
                    && match sub {
                        Some(sub) => {
                            interface.name
                                == format!("{}.{}", self.ios_interface_name(device, port), sub)
                        }
                        None => !interface.name.contains('.'),
                    }
            })
        };
        if remove {
            let routes: Vec<_> = router
                .domain_routes
                .iter()
                .filter(|r| {
                    r.domain == RoutingDomain(0)
                        && r.prefix == prefix
                        && r.next_hop == Some(next)
                        && router.interfaces.iter().any(|i| {
                            i.port == r.port
                                && i.vlan.unwrap_or(VlanId(1)) == r.vlan
                                && interface_matches(i)
                        })
                })
                .copied()
                .collect();
            let legacy: Vec<_> = router
                .routes
                .iter()
                .filter(|r| {
                    r.network == prefix.network()
                        && r.prefix == prefix.length()
                        && r.via == Some(next)
                        && explicit.is_none_or(|(port, _)| r.egress == port)
                })
                .cloned()
                .collect();
            for route in routes {
                self.ios_execute(Command::Provider(ProviderCommand::RemoveDomainRoute(route)))?;
            }
            for route in legacy {
                self.ios_execute(Command::RemoveStaticRoute {
                    router: device,
                    route,
                })?;
            }
            return Ok(());
        }
        let mut interfaces: Vec<_> = router
            .interfaces
            .iter()
            .filter(|i| {
                interface_matches(i)
                    && self.provider().domain(i.port, i.vlan.unwrap_or(VlanId(1)))
                        == RoutingDomain(0)
                    && i.address.is_some_and(|address| {
                        address != next
                            && Ipv4Prefix::new(address, i.prefix)
                                .is_ok_and(|subnet| subnet.usable(next))
                    })
            })
            .collect();
        interfaces.sort_by_key(|i| std::cmp::Reverse(i.prefix));
        let Some(interface) = interfaces.first() else {
            return Err(
                "% Next hop must be a usable address on a configured router interface's subnet."
                    .into(),
            );
        };
        if interfaces
            .get(1)
            .is_some_and(|other| other.prefix == interface.prefix)
        {
            return Err("% Next hop matches multiple interfaces; use ip route NETWORK MASK INTERFACE NEXT-HOP.".into());
        }
        self.ios_execute(Command::Provider(ProviderCommand::SetDomainRoute(
            DomainRoute {
                router: device,
                domain: RoutingDomain(0),
                prefix,
                port: interface.port,
                vlan: interface.vlan.unwrap_or(VlanId(1)),
                next_hop: Some(next),
                preference: 1,
                track_neighbor: false,
            },
        )))
    }

    pub(super) fn ios_route_config(&self, device: DeviceId) -> Vec<String> {
        let DeviceKind::Router(router) = &self.devices[&device].kind else {
            return Vec::new();
        };
        let mut lines = Vec::new();
        for route in &router.routes {
            if let Some(next) = route.via {
                lines.push(format!(
                    "ip route {} {} {} {next}",
                    route.network,
                    prefix_mask(route.prefix),
                    self.ios_interface_name(device, route.egress)
                ));
            }
        }
        for route in &router.domain_routes {
            if route.domain == RoutingDomain(0)
                && route.preference == 1
                && !route.track_neighbor
                && let Some(next) = route.next_hop
            {
                let interface = router
                    .interfaces
                    .iter()
                    .find(|i| i.port == route.port && i.vlan.unwrap_or(VlanId(1)) == route.vlan);
                let name = interface.filter(|i| i.name.contains('.')).map_or_else(
                    || self.ios_interface_name(device, route.port),
                    |i| i.name.clone(),
                );
                lines.push(format!(
                    "ip route {} {} {name} {next}",
                    route.prefix.network(),
                    prefix_mask(route.prefix.length())
                ));
            } else {
                lines.push(format!(
                    "! GUI route: {} domain {} port {} VLAN {} preference {} tracked {}",
                    route.prefix,
                    route.domain.0,
                    route.port,
                    route.vlan.0,
                    route.preference,
                    route.track_neighbor
                ));
            }
        }
        lines
    }
}
