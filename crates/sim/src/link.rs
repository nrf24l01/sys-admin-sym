use crate::{CableColor, LinkId, PortId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub id: LinkId,
    pub a: PortId,
    pub b: PortId,
    pub enabled: bool,
    #[serde(default = "legacy_cable_length")]
    pub length_cm: u32,
    #[serde(default = "legacy_auto_length")]
    pub auto_length: bool,
    #[serde(default)]
    pub color: CableColor,
}

fn legacy_auto_length() -> bool {
    true
}

fn legacy_cable_length() -> u32 {
    100
}

impl Link {
    pub fn other(&self, port: PortId) -> Option<PortId> {
        if self.a == port {
            Some(self.b)
        } else if self.b == port {
            Some(self.a)
        } else {
            None
        }
    }
}
