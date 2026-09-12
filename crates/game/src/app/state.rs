use bevy::prelude::*;
use cloud_provider_sim::{
    CableColor, CableRoutePoint, DeviceId, LinkId, NetworkSim, PortId, RackSide,
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
    #[default]
    Rack,
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
}

#[derive(Resource, Default)]
pub struct UiState {
    pub workspace: Workspace,
    pub selected: Selection,
    pub pending_cable: Option<PortId>,
    /// Route points collected between choosing the source and destination plug.
    pub pending_cable_route: Vec<CableRoutePoint>,
    pub cable_length_cm: Option<u32>,
    pub cable_color: CableColor,
    pub terminal_windows: HashSet<DeviceId>,
    pub terminal_window_focus: HashSet<DeviceId>,
    pub rack_side: RackSide,
    pub cable_visibility: CableVisibility,
    pub notice: Option<(String, bool)>,
    pub error_dialog: Option<String>,
    pub terminals: HashMap<DeviceId, ConsoleState>,
    pub new_vlan_id: String,
    pub new_vlan_name: String,
    pub power_source: Option<cloud_provider_sim::SourceId>,
    pub power_outlet: u8,
    pub pending_power_outlet: Option<cloud_provider_sim::OutletId>,
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
}
