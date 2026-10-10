use crate::{CableColor, CableRoutePoint, DeviceId, LinkId, LinkSpeed, LocalizedText, PortId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TransceiverId(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CableAssemblyId(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CageKind {
    Sfp,
    SfpPlus,
    Sfp28,
}
impl CageKind {
    pub fn accepts(self, module: Self) -> bool {
        self == module
            || matches!(
                (self, module),
                (Self::SfpPlus | Self::Sfp28, Self::Sfp) | (Self::Sfp28, Self::SfpPlus)
            )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Fec {
    #[default]
    None,
    BaseR,
    Rs,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthernetMode {
    pub speed: LinkSpeed,
    #[serde(default = "one_lane")]
    pub lanes: u8,
    #[serde(default)]
    pub fec: Fec,
}
fn one_lane() -> u8 {
    1
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CageProfile {
    pub kind: CageKind,
    pub modes: Vec<EthernetMode>,
    pub max_power_mw: u32,
}
impl CageProfile {
    pub fn sfp() -> Self {
        Self::for_speed(LinkSpeed::Gbps1)
    }
    pub fn for_speed(speed: LinkSpeed) -> Self {
        Self {
            kind: match speed {
                LinkSpeed::Gbps25 => CageKind::Sfp28,
                LinkSpeed::Gbps10 => CageKind::SfpPlus,
                _ => CageKind::Sfp,
            },
            modes: [LinkSpeed::Gbps1, speed]
                .into_iter()
                .enumerate()
                .filter(|(index, rate)| *index == 0 || *rate != LinkSpeed::Gbps1)
                .map(|(_, rate)| rate)
                .map(|speed| EthernetMode {
                    speed,
                    lanes: 1,
                    fec: Fec::None,
                })
                .collect(),
            max_power_mw: 2000,
        }
    }
    pub fn supports(&self, module: &TransceiverModel) -> bool {
        self.kind.accepts(module.cage)
            && module.power.peak_mw <= self.max_power_mw
            && module.modes.iter().any(|mode| self.modes.contains(mode))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FiberClass {
    Om3,
    Om4,
    Os2,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FiberReach {
    pub fiber: FiberClass,
    pub max_length_cm: u32,
    pub attenuation_mdb_per_km: u32,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModuleMedium {
    Copper,
    Optical {
        strands: u8,
        tx_nm: u16,
        rx_nm: u16,
        tx_mdbm: i32,
        sensitivity_mdbm: i32,
        overload_mdbm: i32,
        reaches: Vec<FiberReach>,
    },
    DirectAttach,
}
#[derive(Debug, Clone, Deserialize)]
pub struct TransceiverModel {
    pub id: String,
    pub display_name: LocalizedText,
    pub desc: LocalizedText,
    pub cage: CageKind,
    pub modes: Vec<EthernetMode>,
    pub power: crate::PowerProfile,
    pub dom: bool,
    pub price: i64,
    #[serde(flatten)]
    pub medium: ModuleMedium,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssemblyMedium {
    Fiber {
        fiber: FiberClass,
        strands: u8,
        connector_loss_mdb: u32,
    },
    Dac {
        transceiver: String,
    },
    Aoc {
        transceiver: String,
    },
}
#[derive(Debug, Clone, Deserialize)]
pub struct CableAssemblyModel {
    pub id: String,
    pub display_name: LocalizedText,
    pub desc: LocalizedText,
    pub length_cm: u32,
    pub price: i64,
    pub color: CableColor,
    #[serde(flatten)]
    pub medium: AssemblyMedium,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransceiverInstance {
    pub id: TransceiverId,
    pub model_id: String,
    pub port: Option<PortId>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CableAssemblyInstance {
    pub id: CableAssemblyId,
    pub model_id: String,
    pub link: Option<LinkId>,
    #[serde(default = "crossed")]
    pub crossed: bool,
}
fn crossed() -> bool {
    true
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OpticsState {
    pub transceivers: BTreeMap<TransceiverId, TransceiverInstance>,
    pub assemblies: BTreeMap<CableAssemblyId, CableAssemblyInstance>,
    pub cages: BTreeMap<PortId, CageProfile>,
    pub device_models: BTreeMap<DeviceId, String>,
    pub(crate) next_transceiver: u64,
    pub(crate) next_assembly: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpticsCommand {
    BuyTransceiver {
        model: String,
    },
    InstallTransceiver {
        port: PortId,
        module: TransceiverId,
    },
    RemoveTransceiver {
        port: PortId,
    },
    BuyAssembly {
        model: String,
    },
    ConnectAssembly {
        assembly: CableAssemblyId,
        a: PortId,
        b: PortId,
        route: Vec<CableRoutePoint>,
    },
    FlipPolarity {
        assembly: CableAssemblyId,
    },
    BuyHardware {
        model: String,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum OpticsError {
    #[error("install an optical transceiver before connecting a fiber cable")]
    MissingTransceiver,
    #[error("unknown optical hardware model")]
    UnknownModel,
    #[error("item is not available in inventory")]
    NotOwned,
    #[error("this port has no pluggable cage")]
    NotCage,
    #[error("this cage is occupied")]
    CageOccupied,
    #[error("module is incompatible with the host cage or power budget")]
    IncompatibleHost,
    #[error("cable connector does not fit this endpoint")]
    ConnectorMismatch,
    #[error("disconnect the attached cable assembly first")]
    AttachedCable,
    #[error("only fiber cable polarity can be changed")]
    NotFiber,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkFault {
    NoCable,
    EmptyCage,
    Disabled,
    Unpowered,
    NotInstalled,
    UnsupportedModule,
    ConnectorMismatch,
    ModeMismatch,
    FiberMismatch,
    WavelengthMismatch,
    PolarityMismatch,
    CableTooShort,
    TooLong,
    LowLight,
    ReceiverOverload,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpticalReading {
    pub port: PortId,
    pub tx_mdbm: i32,
    pub rx_mdbm: i32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkStatus {
    pub speed: Option<LinkSpeed>,
    pub fault: Option<LinkFault>,
    pub path: Vec<PortId>,
    pub optical: Vec<OpticalReading>,
}
impl LinkStatus {
    pub(crate) fn down(fault: LinkFault, path: Vec<PortId>) -> Self {
        Self {
            speed: None,
            fault: Some(fault),
            path,
            optical: Vec::new(),
        }
    }
}
