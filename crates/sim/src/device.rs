use crate::{DeviceId, PortId, RackPlacement, Route, RouterInterface, Vlan};
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
        }
    }

    pub fn template(&self) -> DeviceTemplate {
        match self.kind {
            DeviceKind::Server(_) => DeviceTemplate::Server,
            DeviceKind::Switch(_) => DeviceTemplate::Switch,
            DeviceKind::Router(_) => DeviceTemplate::Router,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceKind {
    Server(Server),
    Switch(Switch),
    Router(Router),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceTemplate {
    Server,
    Switch,
    Router,
}

impl DeviceTemplate {
    pub fn price(self) -> i64 {
        match self {
            Self::Server => 1_000,
            Self::Switch | Self::Router => 500,
        }
    }

    pub fn rack_units(self) -> u8 {
        1
    }
}
