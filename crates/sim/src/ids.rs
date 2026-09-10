use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! id_type {
    ($name:ident, $inner:ty) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        pub struct $name(pub $inner);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

id_type!(DeviceId, u64);
id_type!(PortId, u64);
id_type!(LinkId, u64);
id_type!(RackId, u64);
id_type!(VlanId, u16);

impl Default for VlanId {
    fn default() -> Self {
        Self(1)
    }
}
