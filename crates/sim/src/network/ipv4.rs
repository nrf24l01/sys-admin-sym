use crate::VlanId;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ipv4InterfaceConfig {
    pub address: Ipv4Addr,
    pub prefix: u8,
    pub gateway: Option<Ipv4Addr>,
    #[serde(default, deserialize_with = "deserialize_vlan_option")]
    pub vlan: Option<VlanId>,
}

impl Ipv4InterfaceConfig {
    pub fn new(address: Ipv4Addr, prefix: u8, gateway: Option<Ipv4Addr>, vlan: VlanId) -> Self {
        Self {
            address,
            prefix,
            gateway,
            vlan: Some(vlan),
        }
    }

    pub fn contains(&self, address: Ipv4Addr) -> bool {
        same_subnet(self.address, address, self.prefix)
    }

    pub fn validate(&self) -> bool {
        self.prefix <= 32
            && !self.address.is_unspecified()
            && self.vlan.is_none_or(|v| v.0 > 0 && v.0 < 4095)
    }
}

fn deserialize_vlan_option<'de, D>(deserializer: D) -> Result<Option<VlanId>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct VlanVisitor;
    impl<'de> serde::de::Visitor<'de> for VlanVisitor {
        type Value = Option<VlanId>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a VLAN number or null")
        }
        fn visit_none<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_unit<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
            u16::try_from(v)
                .map(|v| Some(VlanId(v)))
                .map_err(|_| E::custom("VLAN out of range"))
        }
        fn visit_u32<E: serde::de::Error>(self, v: u32) -> Result<Self::Value, E> {
            self.visit_u64(v as u64)
        }
        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
            if v >= 0 {
                self.visit_u64(v as u64)
            } else {
                Err(E::custom("negative VLAN"))
            }
        }
    }
    deserializer.deserialize_any(VlanVisitor)
}

pub fn same_subnet(a: Ipv4Addr, b: Ipv4Addr, prefix: u8) -> bool {
    if prefix > 32 {
        return false;
    }
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    (u32::from(a) & mask) == (u32::from(b) & mask)
}
