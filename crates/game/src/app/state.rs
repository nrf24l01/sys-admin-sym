use bevy::prelude::*;
use cloud_provider_sim::{
    CableColor, CableRoutePoint, DeviceId, LinkId, NetworkSim, PortId, RackId, RackSide,
};
use std::collections::{HashMap, HashSet};

#[derive(Resource, Clone)]
pub struct SimSnapshot(pub NetworkSim);

impl Default for SimSnapshot {
    fn default() -> Self {
        Self(NetworkSim::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Workspace {
    Rack,
    #[default]
    Room,
    Topology,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CableVisibility {
    #[default]
    All,
    Selected,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Selection {
    #[default]
    None,
    Device(DeviceId),
    Port(PortId),
    Link(LinkId),
    PowerCable(cloud_provider_sim::OutletId),
}

#[derive(Default)]
pub struct NetworkSummaryCache {
    pub revision: Option<(u64, u64)>,
    pub resources: cloud_provider_sim::DataCenterResources,
}

#[derive(Resource, Default)]
pub struct UiState {
    pub network_summary: NetworkSummaryCache,
    pub routing_device: Option<DeviceId>,
    pub ranges_open: bool,
    pub selected_range: Option<cloud_provider_sim::Ipv4Prefix>,
    pub range_uplink: Option<PortId>,
    pub range_loaded_for: Option<cloud_provider_sim::Ipv4Prefix>,
    pub shop: super::ShopState,
    pub settings: super::SettingsWindowState,
    pub workspace: Workspace,
    pub selected: Selection,
    pub pending_cable: Option<PortId>,
    pub pending_assembly: Option<cloud_provider_sim::CableAssemblyId>,
    /// Route points collected between choosing the source and destination plug.
    pub pending_cable_route: Vec<CableRoutePoint>,
    pub cable_length_cm: Option<u32>,
    pub cable_color: CableColor,
    pub terminal_windows: HashSet<DeviceId>,
    pub terminal_window_focus: HashSet<DeviceId>,
    pub rack_side: RackSide,
    pub active_rack: Option<RackId>,
    pub cable_visibility: CableVisibility,
    pub notice: Option<(crate::localization::UiMessage, bool)>,
    pub error_dialog: Option<crate::localization::UiMessage>,
    pub terminals: HashMap<DeviceId, ConsoleState>,
    pub new_vlan_id: String,
    pub new_vlan_name: String,
    pub pending_power_outlet: Option<cloud_provider_sim::OutletId>,
    pub pending_power_inlet: Option<cloud_provider_sim::PowerEndpoint>,
    /// Shared rail anchors collected while a power cord is being connected.
    pub pending_power_route: Vec<CableRoutePoint>,
}

#[derive(Default)]
pub struct ConsoleState {
    pub script_mode: bool,
    pub input: String,
    pub lines: Vec<String>,
    pub history: Vec<String>,
    pub history_position: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct ServerDraft {
    pub synced_ipv4: Option<cloud_provider_sim::Ipv4InterfaceConfig>,
    pub address: String,
    pub prefix: String,
    pub gateway: String,
    pub vlan: String,
    pub hostname: String,
}

#[derive(Debug, Clone, Default)]
pub struct SwitchPortDraft {
    pub vlan: String,
    pub allowed: String,
    pub trunk: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RouterDraft {
    pub name: String,
    pub address: String,
    pub prefix: String,
    pub vlan: String,
    pub internet: bool,
}

#[derive(Resource, Default)]
pub struct EditorDrafts {
    pub servers: HashMap<PortId, ServerDraft>,
    pub switches: HashMap<PortId, SwitchPortDraft>,
    pub routers: HashMap<PortId, RouterDraft>,
    pub routes: HashMap<DeviceId, super::RouteDraft>,
}
