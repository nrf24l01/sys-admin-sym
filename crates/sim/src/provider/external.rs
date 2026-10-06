use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

/// A known endpoint beyond the provider's transit boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalHost {
    pub address: Ipv4Addr,
    pub available: bool,
    pub echo_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ExternalNetwork {
    pub hosts: Vec<ExternalHost>,
}
impl Default for ExternalNetwork {
    fn default() -> Self {
        // Deterministic lab endpoints. They never cause a host Internet request.
        Self {
            hosts: [
                Ipv4Addr::new(198, 51, 100, 1),
                Ipv4Addr::new(8, 8, 8, 8),
                Ipv4Addr::new(1, 1, 1, 1),
            ]
            .into_iter()
            .map(|address| ExternalHost {
                address,
                available: true,
                echo_enabled: true,
            })
            .collect(),
        }
    }
}
impl ExternalNetwork {
    pub fn available(&self, address: Ipv4Addr) -> bool {
        self.hosts
            .iter()
            .any(|h| h.address == address && h.available)
    }
    pub fn responds(&self, address: Ipv4Addr) -> bool {
        self.hosts
            .iter()
            .any(|h| h.address == address && h.available && h.echo_enabled)
    }
    pub fn set_host(&mut self, host: ExternalHost) {
        self.hosts.retain(|h| h.address != host.address);
        self.hosts.push(host);
        self.hosts.sort_by_key(|h| h.address);
    }
}
