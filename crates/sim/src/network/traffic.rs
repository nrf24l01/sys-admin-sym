//! Aggregate traffic uses routes traced by the packet engine, negotiated physical
//! link rates and transit contracts. Bounded queues account for bytes, delay and
//! dropped packets without emitting one object per customer packet.
use crate::*;
use std::collections::{BTreeMap, BTreeSet};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Copy)]
pub struct TrafficFlow {
    pub id: u64,
    pub source: PortId,
    pub source_ipv4: Option<Ipv4Addr>,
    pub destination: Ipv4Addr,
    pub demand_mbps: f64,
    pub limit_mbps: f64,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct TrafficResult {
    pub reachable: bool,
    pub delivered_mbps: f64,
    pub latency_ms: f64,
    pub loss: f64,
    pub bytes: u64,
}
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TrafficQueue {
    pub megabits: f64,
    pub loss: f64,
    pub drop_credit: f64,
}

impl NetworkSim {
    pub fn simulate_traffic(
        &mut self,
        flows: &[TrafficFlow],
        seconds: f64,
    ) -> BTreeMap<u64, TrafficResult> {
        let mut paths = BTreeMap::new();
        let mut loads: BTreeMap<PortId, f64> = BTreeMap::new();
        self.prepare_runtime();
        self.runtime.traffic_probe = true;
        for flow in flows {
            let key = (flow.source, flow.destination, flow.source_ipv4);
            let (reachable, ports) = if let Some(path) = self.runtime.traffic_paths.get(&key) {
                path.clone()
            } else {
                let trace = if let Some(address) = flow.source_ipv4 {
                    self.probe_ipv4_address(flow.source, address, flow.destination)
                } else {
                    self.transmit_icmp(flow.source, flow.destination)
                };
                let ports: BTreeSet<_> = trace
                    .hops
                    .iter()
                    .flat_map(|hop| hop.ingress.into_iter().chain(hop.egress))
                    .filter(|p| self.virtual_uplink(*p).is_none())
                    .collect();
                let result = (trace.reachable, ports);
                self.runtime.traffic_paths.insert(key, result.clone());
                result
            };
            if reachable {
                for port in &ports {
                    *loads.entry(*port).or_default() +=
                        flow.demand_mbps.min(flow.limit_mbps).max(0.0);
                }
            }
            paths.insert(flow.id, (reachable, ports));
        }
        self.runtime.traffic_probe = false;
        let mut constraints = BTreeMap::new();
        for (port, load) in &loads {
            let capacity = self
                .port_link_speed(*port)
                .map_or(0.0, |s| f64::from(s.mbps()))
                .min(
                    self.provider
                        .circuit(*port)
                        .map_or(f64::INFINITY, |c| f64::from(c.capacity_mbps)),
                );
            let queue = self.runtime.traffic_queues.entry(*port).or_default();
            let offered = load * seconds;
            let remaining = (queue.megabits + offered - capacity * seconds).max(0.0);
            let buffer = capacity * 0.25;
            let dropped = (remaining - buffer).max(0.0);
            queue.megabits = remaining.min(buffer);
            queue.loss = if offered > 0.0 {
                (dropped / offered).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let delay = if capacity > 0.0 {
                queue.megabits / capacity * 1000.0
            } else {
                0.0
            };
            constraints.insert(
                *port,
                ((capacity / load.max(0.001)).min(1.0), delay, queue.loss),
            );
        }
        self.runtime
            .traffic_queues
            .retain(|port, _| loads.contains_key(port));
        let mut results = BTreeMap::new();
        for flow in flows {
            let (reachable, path) = &paths[&flow.id];
            let mut fraction: f64 = if *reachable { 1.0 } else { 0.0 };
            let mut latency = 0.0;
            let mut loss: f64 = 0.0;
            for port in path {
                if let Some((share, delay, dropped)) = constraints.get(port) {
                    fraction = fraction.min(*share);
                    latency += delay;
                    loss = loss.max(*dropped);
                }
            }
            let rate = flow.demand_mbps.min(flow.limit_mbps).max(0.0) * fraction;
            let bytes = (rate * 125000.0 * seconds) as u64;
            for port in path {
                let t = self.runtime.telemetry.entry(*port).or_default();
                t.tx_bytes = t.tx_bytes.saturating_add(bytes);
                t.rx_bytes = t.rx_bytes.saturating_add(bytes);
                t.tx_frames = t.tx_frames.saturating_add(bytes.div_ceil(1500));
                t.rx_frames = t.rx_frames.saturating_add(bytes.div_ceil(1500));
                if bytes > 0 {
                    t.last_tx_ms = Some(self.runtime.now_ms);
                    t.last_rx_ms = Some(self.runtime.now_ms);
                }
            }
            if bytes > 0 && !path.contains(&flow.source) {
                let t = self.runtime.telemetry.entry(flow.source).or_default();
                t.tx_bytes = t.tx_bytes.saturating_add(bytes);
                t.tx_frames = t.tx_frames.saturating_add(bytes.div_ceil(1500));
                t.last_tx_ms = Some(self.runtime.now_ms);
            }
            results.insert(
                flow.id,
                TrafficResult {
                    reachable: *reachable,
                    delivered_mbps: rate,
                    latency_ms: latency,
                    loss,
                    bytes,
                },
            );
        }
        results
    }
}
