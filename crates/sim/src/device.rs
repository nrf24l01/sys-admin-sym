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
        } else {
            self.template().rack_units()
        }
    }
    pub fn ports(&self) -> &[PortId] {
        self.kind.ports()
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
    pub fn ports(&self) -> &[PortId] {
        match self {
            DeviceKind::Server(v) => &v.ports,
            DeviceKind::Switch(v) => &v.ports,
            DeviceKind::Router(v) => &v.ports,
            DeviceKind::PatchPanel(v) => &v.ports,
            DeviceKind::CableManager(v) => &v.ports,
            DeviceKind::Ups(v) => &v.ports,
            DeviceKind::Pdu(v) => &v.ports,
        }
    }

    /// Legacy power inlet view backed by the common JSON connector resolver.
    pub fn power_inlet(&self) -> Option<PowerInlet> {
        let socket = self.power_connector_socket(0)?;
        let connector = match socket.connector {
            crate::ConnectorType::IecC14 => PowerInletConnector::IecC14,
            crate::ConnectorType::CiscoFourPin => PowerInletConnector::CiscoFourPin,
            _ => return None,
        };
        Some(PowerInlet {
            connector,
            side: socket.side,
            position: socket.center(),
        })
    }

    /// Socket centers shared by cable cuts and rendering. New callers with port
    /// metadata should use `connector_socket` directly.
    pub fn port_position_normalized(&self, index: usize) -> (f32, f32) {
        let (connector, side) = match self {
            Self::Server(server) => {
                let connector = server
                    .hardware
                    .as_ref()
                    .and_then(|hardware| {
                        let port = server.ports.get(index)?;
                        let slot = hardware
                            .card_ports
                            .iter()
                            .position(|ports| ports.contains(port))?;
                        let part_id = hardware.pcie.get(slot)?.as_deref()?;
                        let part = crate::server_catalog()
                            .parts
                            .iter()
                            .find(|p| p.id == part_id)?;
                        if let crate::ServerPartKind::PciCard {
                            card: crate::PciCard::Ethernet { connector, .. },
                        } = &part.kind
                        {
                            Some(*connector)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(crate::PortConnector::Rj45);
                (connector, crate::RackSide::Rear)
            }
            Self::Switch(sw) => (
                if index < sw.model.spec().copper_ports {
                    crate::PortConnector::Rj45
                } else {
                    crate::PortConnector::Sfp
                },
                crate::RackSide::Front,
            ),
            Self::Router(_) => (crate::PortConnector::Rj45, crate::RackSide::Front),
            Self::PatchPanel(_) => (
                crate::PortConnector::Rj45,
                if index.is_multiple_of(2) {
                    crate::RackSide::Rear
                } else {
                    crate::RackSide::Front
                },
            ),
            _ => return (0.5, 0.5),
        };
        self.connector_socket(&connector, side, index)
            .or_else(|| self.find_connector_socket(&connector, index))
            .map(|socket| socket.center())
            .unwrap_or((0.5, 0.5))
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
