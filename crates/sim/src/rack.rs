use crate::{DeviceId, RackId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomPosition {
    pub x_cm: u16,
    pub y_cm: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomCableAnchor {
    pub id: u8,
    pub position: RoomPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataCenterRoom {
    pub name: String,
    pub width_cm: u16,
    pub depth_cm: u16,
    #[serde(default)]
    pub rack_positions: HashMap<RackId, RoomPosition>,
    #[serde(default)]
    pub cable_anchors: Vec<RoomCableAnchor>,
}

impl Default for DataCenterRoom {
    fn default() -> Self {
        Self {
            name: "Room 01".into(),
            width_cm: 1600,
            depth_cm: 2800,
            rack_positions: HashMap::new(),
            cable_anchors: Vec::new(),
        }
    }
}

pub const DATACENTER_RACK_ROWS: u64 = 10;
pub const DATACENTER_RACKS_PER_ROW: u64 = 5;
pub const DATACENTER_RACK_COUNT: u64 = DATACENTER_RACK_ROWS * DATACENTER_RACKS_PER_ROW;
/// Column order keeps IDs 1–10 at the original B/C tray for saved routes.
pub const DATACENTER_CABLE_COLUMNS_CM: [u16; 4] = [630, 330, 930, 1230];

pub struct RoomCableLayout;

impl RoomCableLayout {
    pub const ANCHOR_COUNT: usize = 220;
    pub const RACK_ACCESS_OFFSET_CM: u16 = 100;

    pub fn rack_access_positions(rack: RackId) -> [RoomPosition; 2] {
        let center = predefined_rack_position(rack);
        [
            RoomPosition {
                y_cm: center.y_cm - Self::RACK_ACCESS_OFFSET_CM,
                ..center
            },
            RoomPosition {
                y_cm: center.y_cm + Self::RACK_ACCESS_OFFSET_CM,
                ..center
            },
        ]
    }

    pub fn horizontal_rows_cm() -> impl Iterator<Item = u16> {
        (0..DATACENTER_RACK_ROWS).flat_map(|row| {
            let center = 180 + row as u16 * 270;
            [
                center - Self::RACK_ACCESS_OFFSET_CM,
                center + Self::RACK_ACCESS_OFFSET_CM,
            ]
        })
    }

    pub fn anchors() -> Vec<RoomCableAnchor> {
        // Preserve the original vertical-tray IDs so saved routes keep their positions.
        let mut anchors: Vec<_> = (1..=40)
            .map(|id| RoomCableAnchor {
                id,
                position: predefined_cable_anchor_position(id),
            })
            .collect();
        let mut append = |position| {
            let id = (anchors.len() + 1) as u8;
            anchors.push(RoomCableAnchor { id, position });
        };
        for rack in 1..=DATACENTER_RACK_COUNT {
            for position in Self::rack_access_positions(RackId(rack)) {
                append(position);
            }
        }
        for y_cm in Self::horizontal_rows_cm() {
            for x_cm in DATACENTER_CABLE_COLUMNS_CM {
                append(RoomPosition { x_cm, y_cm });
            }
        }
        anchors
    }
}

pub fn predefined_cable_anchor_position(id: u8) -> RoomPosition {
    let index = usize::from(id.saturating_sub(1));
    RoomPosition {
        x_cm: DATACENTER_CABLE_COLUMNS_CM[index / 10],
        y_cm: 180 + (index % 10) as u16 * 270,
    }
}

pub fn predefined_rack_position(id: RackId) -> RoomPosition {
    let index = id.0.saturating_sub(1);
    RoomPosition {
        x_cm: 180 + (index % DATACENTER_RACKS_PER_ROW) as u16 * 300,
        y_cm: 180 + (index / DATACENTER_RACKS_PER_ROW) as u16 * 270,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RackPlacement {
    pub rack: RackId,
    pub unit: u8,
    pub height: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rack {
    pub id: RackId,
    pub name: String,
    pub units: u8,
    pub placements: Vec<(DeviceId, RackPlacement)>,
}

impl Rack {
    pub fn occupies(&self, unit: u8) -> Option<DeviceId> {
        self.placements.iter().find_map(|(id, p)| {
            (unit >= p.unit && unit < p.unit.saturating_add(p.height)).then_some(*id)
        })
    }
}
