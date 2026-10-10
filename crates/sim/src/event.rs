use crate::{DeviceId, LinkId, PortId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimEvent {
    BankChanged,
    CableSuppliesPurchased(crate::CableSupply),
    DeviceAdded(DeviceId),
    DeviceMoved { device: DeviceId },
    DeviceRemoved(DeviceId),
    DevicePowerChanged { device: DeviceId, powered: bool },
    LinkCreated(LinkId),
    LinkRemoved(LinkId),
    PortConfigChanged(PortId),
    RouterRoutesChanged(DeviceId),
    TopologyChanged { revision: u64 },
    ConnectivityChanged,
    PowerChanged,
}
