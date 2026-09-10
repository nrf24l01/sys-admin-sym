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
