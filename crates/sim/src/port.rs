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
    Gbps10,
    Gbps25,
}

impl LinkSpeed {
    pub fn from_mbps(mbps: u32) -> Option<Self> {
        match mbps {
            10 => Some(Self::Mbps10),
            100 => Some(Self::Mbps100),
            1000 => Some(Self::Gbps1),
            10000 => Some(Self::Gbps10),
            25000 => Some(Self::Gbps25),
            _ => None,
        }
    }
    pub const fn mbps(self) -> u32 {
        match self {
            Self::Mbps10 => 10,
            Self::Mbps100 => 100,
            Self::Gbps1 => 1_000,
            Self::Gbps10 => 10_000,
            Self::Gbps25 => 25_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PortConnector {
    #[default]
    Rj45,
    Sfp,
    Lc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RackSide {
    #[default]
    Rear,
    Front,
}

impl PortConnector {
    pub fn supports_cabling(self) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Port {
    pub id: PortId,
    pub device: DeviceId,
    pub name: String,
    #[serde(default)]
    pub side: RackSide,
    #[serde(default)]
    pub paired_port: Option<PortId>,
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
    PatchPanel,
    CableManager,
    Infrastructure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ServerPortConfig {
    pub ipv4: Option<Ipv4InterfaceConfig>,
    #[serde(default)]
    pub additional_ipv4: Vec<Ipv4InterfaceConfig>,
}

impl ServerPortConfig {
    pub fn addresses(&self) -> impl Iterator<Item = &Ipv4InterfaceConfig> {
        self.ipv4.iter().chain(self.additional_ipv4.iter())
    }
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
