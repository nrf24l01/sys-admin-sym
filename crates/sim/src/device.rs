use crate::{DeviceId, PortId, RackPlacement, Route, RouterInterface, SourceId, Vlan};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub id: DeviceId,
    pub name: String,
    pub powered: bool,
    pub rack: Option<RackPlacement>,
    pub kind: DeviceKind,
}

impl Device {
    pub fn ports(&self) -> &[PortId] {
        match &self.kind {
            DeviceKind::Server(v) => &v.ports,
            DeviceKind::Switch(v) => &v.ports,
            DeviceKind::Router(v) => &v.ports,
            DeviceKind::PatchPanel(v) => &v.ports,
            DeviceKind::CableManager(v) => &v.ports,
            DeviceKind::Ups(v) => &v.ports,
            DeviceKind::Pdu(v) => &v.ports,
        }
    }

    pub fn template(&self) -> DeviceTemplate {
        match self.kind {
            DeviceKind::Server(_) => DeviceTemplate::Server,
            DeviceKind::Switch(_) => DeviceTemplate::Switch,
            DeviceKind::Router(_) => DeviceTemplate::Router,
            DeviceKind::PatchPanel(_) => DeviceTemplate::PatchPanel,
            DeviceKind::CableManager(_) => DeviceTemplate::CableManager,
            DeviceKind::Ups(_) => DeviceTemplate::Ups,
            DeviceKind::Pdu(_) => DeviceTemplate::Pdu,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceKind {
    Server(Server),
    Switch(Switch),
    Router(Router),
    PatchPanel(PatchPanel),
    CableManager(CableManager),
    Ups(Ups),
    Pdu(Pdu),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Ups {
    pub ports: Vec<PortId>,
    #[serde(default)]
    pub source: Option<SourceId>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Pdu {
    pub ports: Vec<PortId>,
    #[serde(default)]
    pub source: Option<SourceId>,
}

pub const RACK_FACE_WIDTH_CM: f32 = 48.26;
pub const RACK_FACE_HEIGHT_CM: f32 = 4.445;
pub const RACK_GAP_CM: f32 = 0.5;

impl DeviceKind {
    /// Socket centers on the equipment face, shared by cable cuts and rendering.
    pub fn port_position_normalized(&self, index: usize) -> (f32, f32) {
        match self {
            Self::Switch(_) if index < 24 => (
                0.342 + (index / 12) as f32 * 0.263 + ((index % 12) / 2) as f32 * 0.0336,
                0.37 + (index % 2) as f32 * 0.29,
            ),
            Self::Switch(_) => (0.833 + (index - 24) as f32 * 0.040, 0.69),
            Self::Server(_) => (0.155 + index as f32 * 0.053, 0.47),
            Self::Router(_) if index < 2 => (0.432 + index as f32 * 0.065, 0.63),
            Self::Router(_) => {
                let lan = index - 2;
                (
                    0.201 + (lan % 4) as f32 * 0.0465,
                    0.32 + (lan / 4) as f32 * 0.32,
                )
            }
            Self::PatchPanel(_) => {
                let slot = (index / 2).min(23);
                (
                    (0.16 + (slot % 12) as f32 * 0.062).min(0.90),
                    0.30 + (slot / 12) as f32 * 0.38,
                )
            }
            Self::CableManager(_) => (0.5, 0.5),
            Self::Ups(_) | Self::Pdu(_) => (0.5, 0.5),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    pub hostname: String,
    pub ports: Vec<PortId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Switch {
    pub ports: Vec<PortId>,
    pub vlans: Vec<Vlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Router {
    pub ports: Vec<PortId>,
    pub interfaces: Vec<RouterInterface>,
    pub routes: Vec<Route>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchPanel {
    pub ports: Vec<PortId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CableManager {
    pub ports: Vec<PortId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceTemplate {
    Server,
    Switch,
    Router,
    PatchPanel,
    CableManager,
    Ups,
    Pdu,
}

impl DeviceTemplate {
    pub fn price(self) -> i64 {
        match self {
            Self::Server => 1_000,
            Self::Switch | Self::Router => 500,
            Self::PatchPanel => 150,
            Self::CableManager => 75,
            Self::Ups => 1_200,
            Self::Pdu => 250,
        }
    }

    pub fn rack_units(self) -> u8 {
        if matches!(self, Self::Ups) { 2 } else { 1 }
    }
}
