use crate::{DeviceId, Ipv4InterfaceConfig, SwitchPortMode};
use crate::{PortId, RouterInterface};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Port {
    pub id: PortId,
    pub device: DeviceId,
    pub name: String,
    pub enabled: bool,
    pub config: PortConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortConfig {
    Server(ServerPortConfig),
    Switch(SwitchPortConfig),
    Router(RouterPortConfig),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ServerPortConfig {
    pub ipv4: Option<Ipv4InterfaceConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwitchPortConfig {
    pub mode: SwitchPortMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RouterPortConfig {
    /// Subinterfaces are mirrored here so reachability can remain port-centric.
    pub interfaces: Vec<RouterInterface>,
}
