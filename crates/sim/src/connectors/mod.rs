//! Common physical connector interface, independent of transport and presentation.
mod catalog;
mod equipment;
pub use catalog::*;
pub use equipment::*;

use crate::{Port, PortConnector, PowerInlet, PowerInletConnector, RackSide};
use serde::{Deserialize, Serialize};

/// All physical connector families share this identity and the `Connector` interface.
/// Existing network/power enums are adapters retained for saved-game compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectorType {
    #[serde(rename = "rj-45")]
    Rj45,
    #[serde(rename = "sfp")]
    Sfp,
    #[serde(rename = "lc")]
    Lc,
    #[serde(rename = "c14")]
    IecC14,
    #[serde(rename = "c13")]
    IecC13,
    #[serde(rename = "cisco-four-pin")]
    CiscoFourPin,
    #[serde(rename = "anchor")]
    Anchor,
}

impl ConnectorType {
    pub fn key(self) -> &'static str {
        match self {
            Self::Rj45 => "rj-45",
            Self::Sfp => "sfp",
            Self::Lc => "lc",
            Self::IecC14 => "c14",
            Self::IecC13 => "c13",
            Self::CiscoFourPin => "cisco-four-pin",
            Self::Anchor => "anchor",
        }
    }
}

pub trait Connector {
    fn connector_type(&self) -> ConnectorType;

    fn definition(&self) -> &'static ConnectorDefinition {
        connector_catalog().definition(self.connector_type())
    }

    /// Physical mating only. Optical modules, power ratings and link capabilities
    /// are validated by their own simulation subsystems.
    fn can_mate(&self, other: &dyn Connector) -> bool {
        self.definition()
            .mates_with
            .contains(&other.connector_type())
    }
}

impl Connector for ConnectorType {
    fn connector_type(&self) -> ConnectorType {
        *self
    }
}
impl Connector for PortConnector {
    fn connector_type(&self) -> ConnectorType {
        match self {
            Self::Rj45 => ConnectorType::Rj45,
            Self::Sfp => ConnectorType::Sfp,
            Self::Lc => ConnectorType::Lc,
        }
    }
}
impl Connector for PowerInletConnector {
    fn connector_type(&self) -> ConnectorType {
        match self {
            Self::IecC14 => ConnectorType::IecC14,
            Self::CiscoFourPin => ConnectorType::CiscoFourPin,
        }
    }
}
impl Connector for Port {
    fn connector_type(&self) -> ConnectorType {
        self.connector.connector_type()
    }
}
impl Connector for PowerInlet {
    fn connector_type(&self) -> ConnectorType {
        self.connector.connector_type()
    }
}

/// A resolved physical socket. The normalized face rectangle is authoritative
/// for rendering, clicking, and physical cable endpoints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConnectorSocket {
    pub connector: ConnectorType,
    pub index: usize,
    pub side: RackSide,
    pub rect: [f32; 4],
}
impl Connector for ConnectorSocket {
    fn connector_type(&self) -> ConnectorType {
        self.connector
    }
}
impl ConnectorSocket {
    pub fn center(&self) -> (f32, f32) {
        let [l, t, r, b] = self.rect;
        ((l + r) * 0.5, (t + b) * 0.5)
    }
}
