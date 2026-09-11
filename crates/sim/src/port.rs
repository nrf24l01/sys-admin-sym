use crate::{DeviceId, Ipv4InterfaceConfig, SwitchPortMode};
use crate::{PortId, RouterInterface};
use serde::{Deserialize, Serialize};

/// Ethernet link rates supported by the simulated physical ports.
///
/// The ordering is intentional: it allows negotiation to select the slower
/// of the two endpoint advertisements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum LinkSpeed {
    Mbps10,
    Mbps100,
    #[default]
    Gbps1,
}

impl LinkSpeed {
    pub const fn mbps(self) -> u32 {
        match self {
            Self::Mbps10 => 10,
            Self::Mbps100 => 100,
            Self::Gbps1 => 1_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PortConnector {
    #[default]
    Rj45,
    Sfp,
}

impl PortConnector {
    pub fn supports_cabling(self) -> bool {
        matches!(self, Self::Rj45)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Port {
    pub id: PortId,
    pub device: DeviceId,
    pub name: String,
    pub enabled: bool,
    #[serde(default)]
    pub connector: PortConnector,
    /// Highest rate this port advertises during auto-negotiation.
    #[serde(default)]
    pub advertised_speed: LinkSpeed,
    /// Hardware ceiling for this port.
    #[serde(default)]
    pub max_speed: LinkSpeed,
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
