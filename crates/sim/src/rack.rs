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
            width_cm: 1200,
            depth_cm: 800,
            rack_positions: HashMap::new(),
            cable_anchors: Vec::new(),
        }
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
