use crate::*;

/// Capacity is measured separately from compute inventory and reachability.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NetworkCapacity {
    pub server_ports_mbps: u64,
    pub active_transit_mbps: u64,
    pub largest_transit_failure_mbps: u64,
}
impl NetworkSim {
    pub fn network_capacity(&self) -> NetworkCapacity {
        let server_ports_mbps = self
            .devices()
            .map(|d| self.server_resources(d.id).network_mbps)
            .sum();
        let rates: Vec<_> = self
            .provider()
            .circuits()
            .iter()
            .filter(|c| c.enabled)
            .filter_map(|c| {
                self.port_link_speed(c.port)
                    .map(|s| u64::from(c.capacity_mbps.min(s.mbps())))
            })
            .collect();
        let active_transit_mbps: u64 = rates.iter().sum();
        NetworkCapacity {
            server_ports_mbps,
            active_transit_mbps,
            largest_transit_failure_mbps: active_transit_mbps
                .saturating_sub(rates.into_iter().max().unwrap_or(0)),
        }
    }
    /// Bottleneck on a successfully traced route, not the sum of interface rates.
    pub fn path_capacity_mbps(&self, result: &ReachabilityResult) -> u32 {
        if !result.reachable {
            return 0;
        }
        result
            .hops
            .iter()
            .flat_map(|h| h.ingress.into_iter().chain(h.egress))
            .filter_map(|p| {
                self.port_link_speed(p).map(|s| {
                    self.provider()
                        .circuit(p)
                        .map_or(s.mbps(), |c| s.mbps().min(c.capacity_mbps))
                })
            })
            .min()
            .unwrap_or(0)
    }
}
