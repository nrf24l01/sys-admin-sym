use super::Ipv4Prefix;
use crate::{DeviceId, PortId, VlanId};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RoutingDomain(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkRole {
    Public,
    Private,
    Management,
    Storage,
    OutOfBand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceBinding {
    pub port: PortId,
    pub vlan: VlanId,
    pub domain: RoutingDomain,
    pub role: NetworkRole,
}

/// A provider-owned prefix; ownership does not install a forwarding route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressPool {
    pub prefix: Ipv4Prefix,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefixAuthorization {
    pub prefix: Ipv4Prefix,
    pub max_length: u8,
    pub origin_asn: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpstreamRoute {
    pub prefix: Ipv4Prefix,
    pub next_hop: Ipv4Addr,
}

/// External upstream endpoint. It neither allocates addresses nor configures the edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitCircuit {
    pub port: PortId,
    pub name: String,
    pub address: Ipv4Addr,
    pub prefix: u8,
    pub asn: u32,
    pub capacity_mbps: u32,
    pub enabled: bool,
    /// Explicit routes supplied under a static routing contract.
    pub routes: Vec<UpstreamRoute>,
    /// Upstream-originated routes available to an established BGP session.
    pub offered_routes: Vec<Ipv4Prefix>,
    /// Routes accepted from this provider (a simplified ROA/filter contract).
    pub authorizations: Vec<PrefixAuthorization>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BgpSession {
    pub port: PortId,
    pub vlan: VlanId,
    pub circuit: PortId,
    pub local_asn: u32,
    pub peer_asn: u32,
    pub enabled: bool,
    pub import_prefixes: Vec<Ipv4Prefix>,
    pub export_prefixes: Vec<Ipv4Prefix>,
    pub max_prefixes: usize,
    pub preference: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BgpState {
    Disabled,
    LinkDown,
    PeerMismatch,
    PrefixLimit,
    Established,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyAction {
    Permit,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacketRule {
    pub source: Ipv4Prefix,
    pub destination: Ipv4Prefix,
    pub protocol: Option<u8>,
    pub action: PolicyAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacketPolicy {
    pub port: PortId,
    pub vlan: VlanId,
    /// Empty disables source validation. Prefixes are assigned source ranges.
    pub allowed_sources: Vec<Ipv4Prefix>,
    pub rules: Vec<PacketRule>,
    pub default_action: PolicyAction,
}
impl PacketPolicy {
    pub fn permits(&self, packet: crate::Ipv4Packet, validate_source: bool) -> bool {
        if validate_source
            && !self.allowed_sources.is_empty()
            && !self
                .allowed_sources
                .iter()
                .any(|p| p.contains(packet.source))
        {
            return false;
        }
        self.rules
            .iter()
            .find(|rule| {
                rule.source.contains(packet.source)
                    && rule.destination.contains(packet.destination)
                    && rule
                        .protocol
                        .is_none_or(|protocol| protocol == packet.protocol)
            })
            .map_or(self.default_action, |rule| rule.action)
            == PolicyAction::Permit
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyAttachment {
    pub ingress: bool,
    pub policy: PacketPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwitchManagement {
    pub switch: DeviceId,
    pub address: Ipv4Addr,
    pub prefix: u8,
    pub vlan: VlanId,
    pub gateway: Option<Ipv4Addr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DhcpPool {
    pub server: PortId,
    pub vlan: VlanId,
    pub prefix: Ipv4Prefix,
    pub first: Ipv4Addr,
    pub last: Ipv4Addr,
    pub gateway: Option<Ipv4Addr>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DhcpLease {
    pub client: PortId,
    pub server: PortId,
    pub vlan: VlanId,
    pub address: Ipv4Addr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainRoute {
    pub router: DeviceId,
    pub domain: RoutingDomain,
    pub prefix: Ipv4Prefix,
    pub port: PortId,
    pub vlan: VlanId,
    pub next_hop: Option<Ipv4Addr>,
    pub preference: u32,
    pub track_neighbor: bool,
}

/// Route attributes are separate from legacy saved Route records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutePreference {
    pub router: DeviceId,
    pub prefix: Ipv4Prefix,
    pub egress: PortId,
    pub via: Option<Ipv4Addr>,
    pub preference: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderCommand {
    RouteOwnedRange {
        prefix: Ipv4Prefix,
        uplink: Option<PortId>,
    },
    AllocateAddress {
        port: PortId,
        prefix: Ipv4Prefix,
        gateway: Option<Ipv4Addr>,
        vlan: Option<VlanId>,
    },
    SetExternalHost(super::ExternalHost),
    RemoveExternalHost(Ipv4Addr),
    SetDomainRoute(DomainRoute),
    ReplaceDomainRoute {
        previous: DomainRoute,
        route: DomainRoute,
    },
    RemoveDomainRoute(DomainRoute),
    SetDhcp(DhcpPool),
    RemoveDhcp {
        server: PortId,
        vlan: VlanId,
    },
    SetSwitchManagement(SwitchManagement),
    SetTransit(TransitCircuit),
    RemoveTransit(PortId),
    AddPool(AddressPool),
    RemovePool(Ipv4Prefix),
    BindInterface(InterfaceBinding),
    RemoveBinding {
        port: PortId,
        vlan: VlanId,
    },
    SetBgp(BgpSession),
    RemoveBgp {
        port: PortId,
        vlan: VlanId,
    },
    SetPolicy(PolicyAttachment),
    RemovePolicy {
        port: PortId,
        vlan: VlanId,
        ingress: bool,
    },
    SetRoutePreference(RoutePreference),
    SetSpanningTree {
        switch: DeviceId,
        enabled: bool,
    },
}
