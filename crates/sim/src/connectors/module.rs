use super::*;
use crate::{
    CageKind, CageProfile, DeviceKind, PciCard, PortConnector, ServerPartKind, TransceiverModel,
};

/// Parent interface for connectors that receive or insert a pluggable module.
/// Host acceptance is directional; rate, lane, FEC and power checks stay in optics.
pub trait ModuleConnector: Connector {
    fn cage_kind(&self) -> CageKind;

    fn module_definition(&self) -> &'static ModuleConnectorDefinition {
        self.definition()
            .module
            .as_ref()
            .expect("validated module connector")
    }

    fn accepts_module(&self, module: &dyn ModuleConnector) -> bool {
        self.can_mate(module)
    }
}

impl ConnectorType {
    pub fn module_kind(self) -> Option<CageKind> {
        match self {
            Self::Sfp => Some(CageKind::Sfp),
            Self::SfpPlus => Some(CageKind::SfpPlus),
            Self::Sfp28 => Some(CageKind::Sfp28),
            Self::QsfpPlus => Some(CageKind::QsfpPlus),
            Self::Qsfp28 => Some(CageKind::Qsfp28),
            _ => None,
        }
    }
}

impl Connector for CageKind {
    fn connector_type(&self) -> ConnectorType {
        match self {
            Self::Sfp => ConnectorType::Sfp,
            Self::SfpPlus => ConnectorType::SfpPlus,
            Self::Sfp28 => ConnectorType::Sfp28,
            Self::QsfpPlus => ConnectorType::QsfpPlus,
            Self::Qsfp28 => ConnectorType::Qsfp28,
        }
    }
}
impl ModuleConnector for CageKind {
    fn cage_kind(&self) -> CageKind {
        *self
    }
}
impl Connector for CageProfile {
    fn connector_type(&self) -> ConnectorType {
        self.kind.connector_type()
    }
}
impl ModuleConnector for CageProfile {
    fn cage_kind(&self) -> CageKind {
        self.kind
    }
}
impl Connector for TransceiverModel {
    fn connector_type(&self) -> ConnectorType {
        self.cage.connector_type()
    }
}
impl ModuleConnector for TransceiverModel {
    fn cage_kind(&self) -> CageKind {
        self.cage
    }
}

/// A physical module socket combines host capabilities with the common geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleSocket {
    pub socket: ConnectorSocket,
    pub profile: CageProfile,
}
impl Connector for ModuleSocket {
    fn connector_type(&self) -> ConnectorType {
        self.profile.connector_type()
    }
}
impl ModuleConnector for ModuleSocket {
    fn cage_kind(&self) -> CageKind {
        self.profile.kind
    }
}

impl DeviceKind {
    /// Capabilities belong to the hardware model, independently of installed media.
    pub fn module_connector_profile(&self, index: usize) -> Option<CageProfile> {
        if let Some(model) = self.network_model() {
            return model.ports.get(index)?.cage.clone();
        }
        let Self::Server(server) = self else {
            return None;
        };
        if index < server.model().onboard_ports.len() {
            return None;
        }
        let hardware = server.hardware.as_ref()?;
        let port = server.ports.get(index)?;
        let slot = hardware
            .card_ports
            .iter()
            .position(|ports| ports.contains(port))?;
        let part_id = hardware.pcie.get(slot)?.as_deref()?;
        let part = crate::server_catalog()
            .parts
            .iter()
            .find(|part| part.id == part_id)?;
        let ServerPartKind::PciCard {
            card:
                PciCard::Ethernet {
                    connector: PortConnector::Sfp,
                    cage,
                    speed_mbps,
                    ..
                },
        } = &part.kind
        else {
            return None;
        };
        Some(cage.clone().unwrap_or_else(|| {
            CageProfile::for_speed(crate::LinkSpeed::from_mbps(*speed_mbps).unwrap_or_default())
        }))
    }

    pub fn module_connector_socket(&self, side: RackSide, index: usize) -> Option<ModuleSocket> {
        let profile = self.module_connector_profile(index)?;
        let socket = self.connector_socket(&profile, side, index)?;
        Some(ModuleSocket { socket, profile })
    }
}
