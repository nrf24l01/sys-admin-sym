mod allocation;
mod capacity;
mod config;
pub mod console;
mod dhcp;
mod external;
mod forwarding;
mod lifecycle;
mod management;
mod prefix;
mod ranges;
mod spanning_tree;
mod startup;
mod transit;
mod validation;

use crate::{DeviceId, PortId, VlanId};
pub use capacity::*;
pub use config::*;
pub use external::ExternalHost;
pub use prefix::*;
pub use ranges::RoutedRangeHandoff;
use serde::{Deserialize, Serialize};
pub(crate) use startup::DeviceNetworkConfig;

/// Authoritative provider configuration, composed of independently owned concerns.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderNetwork {
    legacy_inventory_imported: bool,
    external: external::ExternalNetwork,
    circuits: Vec<TransitCircuit>,
    // Read old saves; active routes now belong to each Router.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    routes: Vec<DomainRoute>,
    dhcp: Vec<DhcpPool>,
    leases: Vec<DhcpLease>,
    management: Vec<SwitchManagement>,
    pools: Vec<AddressPool>,
    bindings: Vec<InterfaceBinding>,
    sessions: Vec<BgpSession>,
    policies: Vec<PolicyAttachment>,
    preferences: Vec<RoutePreference>,
    spanning_tree_disabled: Vec<DeviceId>,
}
impl ProviderNetwork {
    pub fn external_hosts(&self) -> &[ExternalHost] {
        &self.external.hosts
    }
    pub fn dhcp_pools(&self) -> &[DhcpPool] {
        &self.dhcp
    }
    pub fn leases(&self) -> &[DhcpLease] {
        &self.leases
    }
    pub fn circuits(&self) -> &[TransitCircuit] {
        &self.circuits
    }
    pub fn pools(&self) -> &[AddressPool] {
        &self.pools
    }
    pub fn bindings(&self) -> &[InterfaceBinding] {
        &self.bindings
    }
    pub fn sessions(&self) -> &[BgpSession] {
        &self.sessions
    }
    pub fn policies(&self) -> &[PolicyAttachment] {
        &self.policies
    }
    pub fn circuit(&self, port: PortId) -> Option<&TransitCircuit> {
        self.circuits.iter().find(|c| c.port == port)
    }
    pub fn domain(&self, port: PortId, vlan: VlanId) -> RoutingDomain {
        self.bindings
            .iter()
            .find(|b| b.port == port && b.vlan == vlan)
            .map_or(RoutingDomain(0), |b| b.domain)
    }
    pub fn spanning_tree_enabled(&self, switch: DeviceId) -> bool {
        !self.spanning_tree_disabled.contains(&switch)
    }
    pub fn owns(&self, prefix: Ipv4Prefix) -> bool {
        self.pools.iter().any(|p| p.prefix.contains_prefix(prefix))
    }
}
