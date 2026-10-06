use serde::{Deserialize, Serialize};
use std::{fmt, net::Ipv4Addr, str::FromStr};

/// Canonical IPv4 network. Construction and deserialization enforce its invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Ipv4Prefix {
    network: Ipv4Addr,
    length: u8,
}

impl Ipv4Prefix {
    pub fn new(address: Ipv4Addr, length: u8) -> Result<Self, String> {
        if length > 32 {
            return Err("IPv4 prefix length must be 0..32".into());
        }
        Ok(Self {
            network: Ipv4Addr::from(
                u32::from(address) & u32::MAX.checked_shl(u32::from(32 - length)).unwrap_or(0),
            ),
            length,
        })
    }
    pub fn network(self) -> Ipv4Addr {
        self.network
    }
    pub fn length(self) -> u8 {
        self.length
    }
    pub fn contains(self, address: Ipv4Addr) -> bool {
        crate::same_subnet(self.network, address, self.length)
    }
    pub fn contains_prefix(self, other: Self) -> bool {
        self.length <= other.length && self.contains(other.network)
    }
    pub fn usable(self, address: Ipv4Addr) -> bool {
        self.contains(address)
            && (self.length >= 31 || (address != self.network && u32::from(address) != self.last()))
    }
    pub fn last(self) -> u32 {
        u32::from(self.network) | (u32::MAX.checked_shr(u32::from(self.length)).unwrap_or(0))
    }
}
impl fmt::Display for Ipv4Prefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.network, self.length)
    }
}
impl FromStr for Ipv4Prefix {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (address, length) = value.split_once('/').ok_or("expected IPv4/prefix")?;
        Self::new(
            address.parse().map_err(|_| "invalid IPv4 address")?,
            length.parse().map_err(|_| "invalid prefix length")?,
        )
    }
}
impl TryFrom<String> for Ipv4Prefix {
    type Error = String;
    fn try_from(value: String) -> Result<Self, String> {
        value.parse()
    }
}
impl From<Ipv4Prefix> for String {
    fn from(value: Ipv4Prefix) -> Self {
        value.to_string()
    }
}
