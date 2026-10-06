use crate::*;
use std::net::Ipv4Addr;

/// Instructions for a routed address allocation. The carrier routes the public
/// prefix across a separate handoff subnet; the player configures both router NICs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutedRangeHandoff {
    pub range: Ipv4Prefix,
    pub uplink: PortId,
    pub upstream_subnet: Ipv4Prefix,
    pub upstream_gateway: Ipv4Addr,
    pub router_address: Ipv4Addr,
    pub local_gateway: Ipv4Addr,
}

impl NetworkSim {
    pub fn range_uplink(&self, range: Ipv4Prefix) -> Option<PortId> {
        self.provider
            .circuits
            .iter()
            .find(|c| c.routes.iter().any(|r| r.prefix == range))
            .map(|c| c.port)
    }

    /// Preview uses the same contract that assignment will install. It does not
    /// configure the player router or create a physical connection.
    pub fn range_handoff(
        &self,
        range: Ipv4Prefix,
        uplink: PortId,
    ) -> Result<RoutedRangeHandoff, SimError> {
        if !self.provider.owns(range) {
            return Err(SimError::PublicIpv4BlockNotOwned);
        }
        let circuit = self.range_circuit(uplink)?;
        let subnet =
            Ipv4Prefix::new(circuit.address, circuit.prefix).map_err(SimError::Provider)?;
        let first = u32::from(subnet.network()) + u32::from(subnet.length() < 31);
        let last = subnet.last() - u32::from(subnet.length() < 31);
        let next = if first == u32::from(circuit.address) {
            first.checked_add(1)
        } else {
            Some(first)
        };
        let router_address = circuit
            .routes
            .iter()
            .find(|r| r.prefix == range)
            .or_else(|| circuit.routes.first())
            .map(|r| r.next_hop)
            .or_else(|| next.filter(|n| *n <= last).map(Ipv4Addr::from))
            .ok_or_else(|| {
                SimError::Provider("The handoff subnet needs an address for your router".into())
            })?;
        if !subnet.usable(router_address)
            || router_address == circuit.address
            || range.contains(circuit.address)
            || range.contains(router_address)
        {
            return Err(SimError::Provider(
                "The upstream handoff must use a separate subnet with a usable router address"
                    .into(),
            ));
        }
        Ok(RoutedRangeHandoff {
            range,
            uplink,
            upstream_subnet: subnet,
            upstream_gateway: circuit.address,
            router_address,
            local_gateway: Ipv4Addr::from(
                u32::from(range.network()) + u32::from(range.length() < 31),
            ),
        })
    }

    fn range_circuit(&self, uplink: PortId) -> Result<TransitCircuit, SimError> {
        let mut uplinks: Vec<_> = self
            .network_outlets()
            .filter(|o| matches!(o.kind, NetworkOutletKind::Uplink { .. }))
            .map(|o| o.port)
            .collect();
        uplinks.sort();
        let index = uplinks
            .iter()
            .position(|port| *port == uplink)
            .ok_or(SimError::InvalidPublicUplink)?;
        if let Some(circuit) = self.provider.circuit(uplink) {
            return Ok(circuit.clone());
        }
        let address = u32::from(Ipv4Addr::new(192, 0, 2, 1))
            + u32::try_from(index).map_err(|_| SimError::InvalidPublicUplink)? * 4;
        Ok(TransitCircuit {
            port: uplink,
            name: "Upstream carrier".into(),
            address: address.into(),
            prefix: 30,
            asn: 64501,
            capacity_mbps: 1000,
            enabled: true,
            routes: Vec::new(),
            offered_routes: Vec::new(),
            authorizations: Vec::new(),
        })
    }

    pub(crate) fn route_owned_range(
        &mut self,
        range: Ipv4Prefix,
        uplink: Option<PortId>,
    ) -> Result<(), SimError> {
        if !self.provider.owns(range) {
            return Err(SimError::PublicIpv4BlockNotOwned);
        }
        let mut circuits = self.provider.circuits.clone();
        for circuit in &mut circuits {
            circuit.routes.retain(|r| r.prefix != range);
        }
        if let Some(uplink) = uplink {
            let handoff = self.range_handoff(range, uplink)?;
            let index = if let Some(index) = circuits.iter().position(|c| c.port == uplink) {
                index
            } else {
                circuits.push(self.range_circuit(uplink)?);
                circuits.len() - 1
            };
            circuits[index].routes.push(UpstreamRoute {
                prefix: range,
                next_hop: handoff.router_address,
            });
        }
        // Validate the entire replacement before changing any carrier table.
        for circuit in &circuits {
            self.validate_provider_command(&ProviderCommand::SetTransit(circuit.clone()))?;
        }
        circuits.sort_by_key(|c| c.port);
        self.provider.circuits = circuits;
        self.routing_revision += 1;
        self.topology_revision += 1;
        Ok(())
    }
}
