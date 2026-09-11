use crate::{DeviceId, PortConnector, PortId, RackId, VlanId};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SimError {
    #[error("install both devices in a rack before connecting a cable")]
    CableDevicesNotInstalled,
    #[error("cable too short: this route needs at least {minimum_cm} cm")]
    CableTooShort { minimum_cm: u32 },
    #[error("Ethernet cable length cannot exceed 100 m")]
    CableTooLong,
    #[error("not enough cable: need {needed_cm} cm, have {available_cm} cm; buy a 305 m box")]
    InsufficientCable { needed_cm: u32, available_cm: u32 },
    #[error("need two RJ45 connectors, have {available}; buy a connector pack")]
    InsufficientConnectors { available: u32 },
    #[error("cable inventory capacity exceeded")]
    CableInventoryFull,
    #[error("device {0} does not exist")]
    DeviceNotFound(DeviceId),
    #[error("port {0} does not exist")]
    PortNotFound(PortId),
    #[error("rack {0} does not exist")]
    RackNotFound(RackId),
    #[error("port {0} is already connected")]
    PortAlreadyConnected(PortId),
    #[error("supported cables: server–switch, router–switch, switch–switch, and server–router")]
    UnsupportedConnection,
    #[error("port {port} uses unsupported {connector:?}; only RJ45 cabling is implemented")]
    UnsupportedConnector {
        port: PortId,
        connector: PortConnector,
    },
    #[error("cannot connect a port to itself")]
    SamePort,
    #[error("rack unit {unit} is occupied in rack {rack}")]
    RackUnitOccupied { rack: RackId, unit: u8 },
    #[error("rack placement is outside the rack")]
    RackPlacementOutOfBounds,
    #[error("VLAN {0} does not exist on the switch")]
    VlanNotFound(VlanId),
    #[error("invalid IPv4 interface configuration")]
    InvalidIpv4,
    #[error("port has the wrong device type for this command")]
    WrongPortType,
    #[error("not enough money: need ${needed}, have ${available}")]
    InsufficientFunds { needed: i64, available: i64 },
    #[error("link does not exist")]
    LinkNotFound,
    #[error("device must be removed from its rack first")]
    DeviceInstalled,
    #[error("invalid hostname")]
    InvalidHostname,
}
