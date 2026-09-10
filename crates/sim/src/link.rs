use crate::{LinkId, PortId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub id: LinkId,
    pub a: PortId,
    pub b: PortId,
    pub enabled: bool,
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
