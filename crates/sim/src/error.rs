use crate::{DeviceId, PortConnector, PortId, RackId, VlanId};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SimError {
    #[error("device {0} does not exist")]
    DeviceNotFound(DeviceId),
    #[error("port {0} does not exist")]
    PortNotFound(PortId),
    #[error("rack {0} does not exist")]
    RackNotFound(RackId),
    #[error("port {0} is already connected")]
    PortAlreadyConnected(PortId),
    #[error("cannot connect two server ports in this MVP")]
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
