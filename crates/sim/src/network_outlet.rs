use crate::{CableRoutePoint, DeviceId, PortId, RackId, RackSide, RoomPosition};
use serde::{Deserialize, Serialize};

/// A permanently installed RJ45 socket owned by the data center room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkOutletKind {
    Uplink { position: RoomPosition },
    Lan { rack: RackId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkOutlet {
    pub port: PortId,
    pub kind: NetworkOutletKind,
}

impl NetworkOutlet {
    /// Reserved owner ID for permanent infrastructure ports.
    pub const OWNER: DeviceId = DeviceId(0);

    pub fn name(&self) -> String {
        match self.kind {
            NetworkOutletKind::Uplink { .. } => "Global uplink".into(),
            NetworkOutletKind::Lan { rack } => format!("Rack {} LAN", rack.0),
        }
    }

    /// Cable projection point used by the rack renderer. A room uplink is
    /// outside every rack, so the visible rack segment ends at its rail.
    pub fn cable_route_point(&self, rack_units: u8) -> CableRoutePoint {
        match self.kind {
            NetworkOutletKind::Uplink { .. } => CableRoutePoint {
                rack: RackId(0),
                unit: 0,
                side: RackSide::Rear,
                offset_cm: 0,
            },
            NetworkOutletKind::Lan { rack } => CableRoutePoint {
                rack,
                unit: rack_units,
                side: RackSide::Rear,
                offset_cm: 24,
            },
        }
    }
}
