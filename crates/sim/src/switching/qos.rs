use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameQos {
    pub dscp: u8,
    pub cos: u8,
    pub length_bytes: u16,
}
impl Default for FrameQos {
    fn default() -> Self {
        Self {
            dscp: 0,
            cos: 0,
            length_bytes: 64,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum QosTrust {
    #[default]
    Untrusted,
    Dscp,
    Cos,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct QosPort {
    pub trust: QosTrust,
    pub default_cos: u8,
    pub bandwidth_percent: u8,
    pub weights: [u8; 4],
    pub priority_queue: bool,
}
impl Default for QosPort {
    fn default() -> Self {
        Self {
            trust: QosTrust::Untrusted,
            default_cos: 0,
            bandwidth_percent: 100,
            weights: [25; 4],
            priority_queue: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct QosConfig {
    pub enabled: bool,
    pub ports: BTreeMap<PortId, QosPort>,
    pub dscp_queue: BTreeMap<u8, u8>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QosCounters {
    pub transmitted: [u64; 4],
    pub dropped: [u64; 4],
    pub bytes: [u64; 4],
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct QosBucket {
    pub at_ms: u64,
    pub bits: u64,
    pub rate: u64,
}

impl NetworkSim {
    pub fn qos_counters(&self, port: PortId) -> QosCounters {
        self.runtime.qos.get(&port).copied().unwrap_or_default()
    }

    pub(crate) fn classify_switch_frame(
        &self,
        ingress: PortId,
        mut frame: EthernetFrame,
    ) -> EthernetFrame {
        if let EthernetPayload::Ipv4 { packet, .. } = frame.payload {
            frame.qos.dscp = packet.dscp;
        }
        let Some(config) = self
            .port(ingress)
            .and_then(|port| self.switch_services(port.device))
            .map(|services| &services.qos)
            .filter(|qos| qos.enabled)
        else {
            return frame;
        };
        let port = config.ports.get(&ingress).cloned().unwrap_or_default();
        let ipv4 = matches!(frame.payload, EthernetPayload::Ipv4 { .. });
        match port.trust {
            QosTrust::Dscp if ipv4 => {
                frame.qos.dscp = frame.qos.dscp.min(63);
                frame.qos.cos = frame.qos.dscp / 8;
            }
            QosTrust::Cos => {
                frame.qos.cos = if frame.vlan.is_some() {
                    frame.qos.cos.min(7)
                } else {
                    port.default_cos
                };
                frame.qos.dscp = frame.qos.cos * 8;
            }
            _ => {
                frame.qos.cos = port.default_cos;
                frame.qos.dscp = port.default_cos * 8;
            }
        }
        if let EthernetPayload::Ipv4 { packet, .. } = &mut frame.payload {
            packet.dscp = frame.qos.dscp;
        }
        frame
    }

    fn qos_queue(&self, port: PortId, frame: &EthernetFrame) -> usize {
        self.port(port)
            .and_then(|p| self.switch_services(p.device))
            .map_or(1, |services| {
                let dscp = frame.qos.dscp.min(63);
                let queue = services
                    .qos
                    .dscp_queue
                    .get(&dscp)
                    .copied()
                    .unwrap_or(match dscp {
                        40..=47 => 1,
                        16..=31 => 3,
                        32..=39 | 48..=63 => 4,
                        _ => 2,
                    });
                usize::from(queue.clamp(1, 4) - 1)
            })
    }

    /// Weighted queue service within a synchronous packet batch; queue 1 can
    /// receive strict priority. Time-dependent rate budgets use simulation time.
    pub(crate) fn qos_order(&self, port: PortId, frame: &EthernetFrame) -> u64 {
        let Some(qos) = self
            .port(port)
            .and_then(|p| self.switch_services(p.device))
            .map(|s| &s.qos)
            .filter(|q| q.enabled)
        else {
            return 0;
        };
        let settings = qos.ports.get(&port).cloned().unwrap_or_default();
        let queue = self.qos_queue(port, frame);
        if queue == 0 && settings.priority_queue {
            return 0;
        }
        1 + self.qos_counters(port).bytes[queue].saturating_mul(100)
            / u64::from(settings.weights[queue].max(1))
    }

    pub(crate) fn qos_transmit(&mut self, port: PortId, frame: &EthernetFrame) -> bool {
        let Some(qos) = self
            .port(port)
            .and_then(|p| self.switch_services(p.device))
            .map(|s| &s.qos)
            .filter(|q| q.enabled)
        else {
            return true;
        };
        let settings = qos.ports.get(&port).cloned().unwrap_or_default();
        let queue = self.qos_queue(port, frame);
        let bits = u64::from(frame.qos.length_bytes.max(64)) * 8;
        let rate = u64::from(
            self.physical_link_speed(port)
                .map_or(0, |speed| speed.mbps()),
        ) * 1_000_000
            * u64::from(settings.bandwidth_percent)
            / 100;
        let mut allowed = true;
        if settings.bandwidth_percent < 100 {
            let capacity = (rate / 100).max(12_000); // A 10 ms burst, at least one MTU.
            let now = self.runtime.simulation_time_ms();
            let budget = self.runtime.qos_buckets.entry(port).or_insert(QosBucket {
                at_ms: now,
                bits: capacity,
                rate,
            });
            if budget.rate != rate {
                budget.bits = capacity;
                budget.rate = rate;
            }
            let elapsed = now.saturating_sub(budget.at_ms);
            budget.bits = budget
                .bits
                .saturating_add(rate.saturating_mul(elapsed) / 1000)
                .min(capacity);
            budget.at_ms = now;
            allowed = budget.bits >= bits;
            if allowed {
                budget.bits -= bits;
            }
        }
        let counters = self.runtime.qos.entry(port).or_default();
        if allowed {
            counters.transmitted[queue] += 1;
            counters.bytes[queue] += bits / 8;
        } else {
            counters.dropped[queue] += 1;
        }
        allowed
    }
}
