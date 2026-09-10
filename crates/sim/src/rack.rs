use crate::{DeviceId, RackId};
use serde::{Deserialize, Serialize};

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
