use crate::VlanId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vlan {
    pub id: VlanId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwitchPortMode {
    Access {
        vlan: Option<VlanId>,
    },
    Trunk {
        native_vlan: Option<VlanId>,
        allowed: Vec<VlanId>,
    },
}

impl SwitchPortMode {
    pub fn carries(&self, vlan: VlanId) -> bool {
        match self {
            Self::Access { vlan: access } => access.unwrap_or(VlanId(1)) == vlan,
            Self::Trunk { allowed, .. } => allowed.contains(&vlan),
        }
    }
}
