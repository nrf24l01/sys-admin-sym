use crate::{DeviceId, PortId, RackPlacement, Route, RouterInterface, SourceId, Vlan};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerInletConnector {
    IecC14,
    CiscoFourPin,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PowerInlet {
    pub connector: PowerInletConnector,
    pub side: crate::RackSide,
    pub position: (f32, f32),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub id: DeviceId,
    pub name: String,
    pub powered: bool,
    pub rack: Option<RackPlacement>,
    pub kind: DeviceKind,
}

impl Device {
    pub fn rack_units(&self) -> u8 {
        if let DeviceKind::Server(server) = &self.kind {
            server.model().rack_units
        } else if let Some(model) = self.kind.network_model() {
            model.rack_units
        } else {
            self.template().rack_units()
        }
    }
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
            DeviceKind::Router(ref r) => r.model().category.template(),
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
    pub fn network_model(&self) -> Option<&'static crate::NetworkDeviceModel> {
        match self {
            Self::Switch(s) => Some(s.model.spec()),
            Self::Router(r) => Some(r.model()),
            _ => None,
        }
    }
    pub fn power_inlet_at(&self, index: usize) -> Option<PowerInlet> {
        if let Some(model) = self.network_model() {
            let p = model.power_inlets.get(index)?;
            Some(PowerInlet {
                connector: p.connector,
                side: p.side,
                position: p.socket.center(),
            })
        } else {
            self.power_inlet()
        }
    }
    /// Authoritative physical power inlet metadata shared by simulation and UI.
    pub fn power_inlet(&self) -> Option<PowerInlet> {
        match self {
            Self::Router(r) => r.model().power_inlet(),
            Self::Server(server) => Some(PowerInlet {
                connector: PowerInletConnector::IecC14,
                side: crate::RackSide::Rear,
                position: server
                    .model()
                    .power_inlet_position(0)
                    .unwrap_or((0.945, 0.52)),
            }),
            Self::Switch(s) => s.model.spec().power_inlet(),
            Self::Ups(_) => Some(PowerInlet {
                connector: PowerInletConnector::IecC14,
                side: crate::RackSide::Rear,
                position: (0.920, 0.48),
            }),
            Self::Pdu(_) => Some(PowerInlet {
                connector: PowerInletConnector::IecC14,
                side: crate::RackSide::Rear,
                position: (0.887, 0.46),
            }),
            _ => None,
        }
    }
    /// Socket centers on the equipment face, shared by cable cuts and rendering.
    pub fn port_position_normalized(&self, index: usize) -> (f32, f32) {
        match self {
            Self::Switch(s) => s
                .model
                .spec()
                .ports
                .get(index)
                .map_or((0.5, 0.5), |p| p.socket.center()),
            Self::Server(server) => {
                if let Some(hardware) = &server.hardware
                    && let Some(port) = server.ports.get(index)
                {
                    for (slot_index, ports) in hardware.card_ports.iter().enumerate() {
                        if let Some(card_index) = ports.iter().position(|id| id == port)
                            && let Some(slot) = server.model().chassis.pcie_slots.get(slot_index)
                        {
                            return slot.port_position(card_index, ports.len());
                        }
                    }
                }
                server.model().port_position(index).unwrap_or((0.5, 0.5))
            }
            Self::Router(r) => r
                .model()
                .ports
                .get(index)
                .map_or((0.5, 0.5), |p| p.socket.center()),
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
    #[serde(default)]
    pub model: crate::ServerModelId,
    pub hostname: String,
    pub ports: Vec<PortId>,
    #[serde(default)]
    pub hardware: Option<crate::ServerHardware>,
}

impl Server {
    pub fn model(&self) -> &'static crate::ServerModel {
        crate::server_catalog()
            .model(self.model.as_str())
            .expect("validated server model identity")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Switch {
    pub ports: Vec<PortId>,
    pub vlans: Vec<Vlan>,
    #[serde(default)]
    pub model: crate::SwitchModel,
    #[serde(default)]
    pub services: crate::SwitchServices,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Router {
    #[serde(default)]
    pub model: crate::NetworkModelId,
    #[serde(default)]
    pub services: crate::SwitchServices,
    pub ports: Vec<PortId>,
    pub interfaces: Vec<RouterInterface>,
    pub routes: Vec<Route>,
    #[serde(default)]
    pub domain_routes: Vec<crate::DomainRoute>,
    #[serde(default = "default_router_vlans")]
    pub vlans: Vec<Vlan>,
    #[serde(default)]
    pub svi_ports: Vec<PortId>,
    #[serde(default = "routing_enabled_default")]
    pub routing_enabled: bool,
}

impl Router {
    pub fn model(&self) -> &'static crate::NetworkDeviceModel {
        self.model.spec()
    }
}

pub(crate) fn default_router_vlans() -> Vec<Vlan> {
    vec![Vlan {
        id: crate::VlanId(1),
        name: "Default".into(),
    }]
}
fn routing_enabled_default() -> bool {
    true
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
            Self::Server => crate::server_catalog().default_model().price,
            Self::Switch => crate::SwitchModel::default().spec().price,
            Self::Router => crate::NetworkModelId::default().spec().price,
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
