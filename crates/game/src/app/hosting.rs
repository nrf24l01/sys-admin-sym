use cloud_provider_sim::{
    DeviceId, HostingMarketConfig, HostingNodeNetwork, HostingProduct, HypervisorPolicy,
    Ipv4Prefix, PortId,
};
use std::collections::BTreeSet;

#[derive(Default)]
pub struct HostingUiState {
    pub open: bool,
    pub provider_public_ipv4: String,
    pub provider_loaded: bool,
    pub tab: usize,
    pub selected_service: Option<u64>,
    pub ip_node_filter: Option<DeviceId>,
    pub product: HostingProduct,
    pub selected_node: Option<DeviceId>,
    pub policy: HypervisorPolicy,
    pub policy_loaded_for: Option<DeviceId>,
    pub node_ipv4_ranges: Option<BTreeSet<Ipv4Prefix>>,
    pub node_range_input: String,
    pub node_networks: Option<Vec<HostingNodeNetwork>>,
    pub node_network_port: Option<PortId>,
    pub node_network_subnet: String,
    pub node_network_tagged: bool,
    pub node_network_vlan: u16,
    pub market: HostingMarketConfig,
    pub market_loaded: bool,
}
