use bevy::prelude::*;
use cloud_provider_sim::{DeviceId, LinkId, NetworkSim, PortId};
use std::collections::HashMap;

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
    pub cable_length_cm: Option<u32>,
    pub notice: Option<(String, bool)>,
    pub terminals: HashMap<DeviceId, ConsoleState>,
    pub new_vlan_id: String,
    pub new_vlan_name: String,
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
