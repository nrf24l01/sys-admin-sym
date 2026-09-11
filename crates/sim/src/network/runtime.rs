//! Runtime state for the synchronous Ethernet/IP simulator.
//!
//! Configuration lives in `NetworkSim`; this module contains only learned and
//! transient state.  Keeping the two separate makes loading a saved world
//! deterministic: the first packet simply learns the same state again.

use crate::{DeviceId, PortId, VlanId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct MacAddress(pub [u8; 6]);

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

impl MacAddress {
    /// Stable locally administered address derived from the port id.
    pub fn for_port(port: PortId) -> Self {
        let n = port.0;
        Self([0x02, 0x53, 0x49, (n >> 16) as u8, (n >> 8) as u8, n as u8])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PortTelemetry {
    pub tx_frames: u64,
    pub rx_frames: u64,
    pub last_tx_ms: Option<u64>,
    pub last_rx_ms: Option<u64>,
    pub link_up: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EthernetFrame {
    pub source: MacAddress,
    pub destination: MacAddress,
    /// `None` is an untagged access frame; `Some` is an 802.1Q tag on a trunk.
    pub vlan: Option<VlanId>,
    pub payload: EthernetPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EthernetPayload {
    Arp(ArpPacket),
    Ipv4 {
        packet: Ipv4Packet,
        icmp: IcmpMessage,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArpPacket {
    Request {
        sender_ip: Ipv4Addr,
        sender_mac: MacAddress,
        target_ip: Ipv4Addr,
        target_mac: Option<MacAddress>,
    },
    Reply {
        sender_ip: Ipv4Addr,
        sender_mac: MacAddress,
        target_ip: Ipv4Addr,
        target_mac: MacAddress,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv4Packet {
    pub source: Ipv4Addr,
    pub destination: Ipv4Addr,
    pub ttl: u8,
    pub protocol: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IcmpMessage {
    EchoRequest { identifier: u16, sequence: u16 },
    EchoReply { identifier: u16, sequence: u16 },
    TimeExceeded,
    DestinationUnreachable,
}

impl PortTelemetry {
    pub fn activity(self, now_ms: u64, window_ms: u64) -> bool {
        self.last_tx_ms
            .into_iter()
            .chain(self.last_rx_ms)
            .any(|at| now_ms.saturating_sub(at) <= window_ms)
    }
}

#[derive(Debug, Clone, Default)]
pub struct NetworkRuntime {
    pub(crate) now_ms: u64,
    pub(crate) telemetry: HashMap<PortId, PortTelemetry>,
    pub(crate) arp: HashMap<(PortId, Ipv4Addr, VlanId), MacAddress>,
    pub(crate) mac_learning: HashMap<(DeviceId, VlanId, MacAddress), PortId>,
    pub(crate) mac_learning_revision: u64,
}

impl NetworkRuntime {
    /// Drop learned and transient state when a saved/replaced topology is loaded.
    pub(crate) fn reset(&mut self) {
        self.now_ms = 0;
        self.telemetry.clear();
        self.arp.clear();
        self.mac_learning.clear();
        self.mac_learning_revision = 0;
    }

    /// Synchronize learned state with the current topology before a lookup.
    pub(crate) fn prepare(&mut self, topology_revision: u64) {
        if self.mac_learning_revision != topology_revision {
            self.arp.clear();
            self.mac_learning.clear();
            self.mac_learning_revision = topology_revision;
        }
    }

    pub fn simulation_time_ms(&self) -> u64 {
        self.now_ms
    }
    pub fn advance_time(&mut self, ms: u64) {
        self.now_ms = self.now_ms.saturating_add(ms);
    }

    pub fn port_telemetry(&self, port: PortId) -> PortTelemetry {
        self.telemetry.get(&port).copied().unwrap_or_default()
    }

    pub fn activity(&self, port: PortId, window_ms: u64) -> bool {
        self.port_telemetry(port).activity(self.now_ms, window_ms)
    }

    pub(crate) fn tx(&mut self, port: PortId) {
        let t = self.telemetry.entry(port).or_default();
        t.tx_frames += 1;
        t.last_tx_ms = Some(self.now_ms);
    }

    /// Account for one synchronous wire crossing. Callers pass the physical
    /// ingress and egress ports, so a frame can never appear as activity on a
    /// disconnected endpoint by accident.
    pub(crate) fn send_frame(&mut self, egress: PortId, ingress: PortId) {
        self.tx(egress);
        self.rx(ingress);
    }

    pub(crate) fn rx(&mut self, port: PortId) {
        let t = self.telemetry.entry(port).or_default();
        t.rx_frames += 1;
        t.last_rx_ms = Some(self.now_ms);
    }

    pub(crate) fn learn_arp(&mut self, port: PortId, ip: Ipv4Addr, vlan: VlanId, mac: MacAddress) {
        self.arp.insert((port, ip, vlan), mac);
    }

    pub(crate) fn arp_entries(
        &self,
        port: PortId,
    ) -> impl Iterator<Item = (Ipv4Addr, MacAddress)> + '_ {
        self.arp
            .iter()
            .filter_map(move |((p, ip, _), mac)| (*p == port).then_some((*ip, *mac)))
    }

    pub(crate) fn learn_mac(
        &mut self,
        device: DeviceId,
        vlan: VlanId,
        mac: MacAddress,
        port: PortId,
    ) {
        self.mac_learning.insert((device, vlan, mac), port);
    }

    pub(crate) fn learned_mac(
        &self,
        device: DeviceId,
        vlan: VlanId,
        mac: MacAddress,
    ) -> Option<PortId> {
        self.mac_learning.get(&(device, vlan, mac)).copied()
    }
}
