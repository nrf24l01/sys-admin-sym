use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelMode {
    On,
    Active,
    Passive,
    Desirable,
    Auto,
}

impl ChannelMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Active => "active",
            Self::Passive => "passive",
            Self::Desirable => "desirable",
            Self::Auto => "auto",
        }
    }
    fn protocol(self) -> u8 {
        match self {
            Self::On => 0,
            Self::Active | Self::Passive => 1,
            Self::Desirable | Self::Auto => 2,
        }
    }
    fn compatible(self, other: Self) -> bool {
        self.protocol() == other.protocol()
            && !matches!(
                (self, other),
                (Self::Passive, Self::Passive) | (Self::Auto, Self::Auto)
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelMember {
    pub group: u8,
    pub mode: ChannelMode,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelHash {
    #[default]
    SourceMac,
    DestinationMac,
    SourceDestinationMac,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EtherChannelConfig {
    pub members: BTreeMap<PortId, ChannelMember>,
    pub min_links: BTreeMap<u8, u8>,
    pub disabled: BTreeSet<u8>,
    pub system_priority: u16,
    pub port_priority: BTreeMap<PortId, u16>,
    pub hash: ChannelHash,
}
impl Default for EtherChannelConfig {
    fn default() -> Self {
        Self {
            members: BTreeMap::new(),
            min_links: BTreeMap::new(),
            disabled: BTreeSet::new(),
            system_priority: 32768,
            port_priority: BTreeMap::new(),
            hash: ChannelHash::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelState {
    Down,
    Suspended,
    Standby,
    Bundled,
}

fn channel_vlan_matches(a: &PortConfig, b: &PortConfig) -> bool {
    match (a, b) {
        (PortConfig::Switch(a), PortConfig::Switch(b)) => match (&a.mode, &b.mode) {
            (SwitchPortMode::Access { vlan: a }, SwitchPortMode::Access { vlan: b }) => {
                a.unwrap_or(VlanId(1)) == b.unwrap_or(VlanId(1))
            }
            _ => a == b,
        },
        _ => false,
    }
}

impl NetworkSim {
    pub fn channel_member(&self, port: PortId) -> Option<ChannelMember> {
        self.switch_services(self.port(port)?.device)?
            .etherchannel
            .members
            .get(&port)
            .copied()
    }

    pub fn channel_ports(&self, device: DeviceId, group: u8) -> Vec<PortId> {
        self.switch_services(device)
            .into_iter()
            .flat_map(|services| &services.etherchannel.members)
            .filter_map(|(port, member)| (member.group == group).then_some(*port))
            .collect()
    }

    pub fn set_channel_group(
        &mut self,
        port: PortId,
        group: Option<(u8, ChannelMode)>,
    ) -> Result<(), String> {
        let device = self.port(port).ok_or("% Interface not found.")?.device;
        if let Some((number, mode)) = group {
            if !(1..=6).contains(&number) {
                return Err("% Channel group must be between 1 and 6.".into());
            }
            let members = self.channel_ports(device, number);
            let limit = if mode.protocol() == 1 { 16 } else { 8 };
            if !members.contains(&port) && members.len() >= limit {
                return Err(format!("% This protocol allows at most {limit} members."));
            }
            if let Some(existing) = members.iter().find(|id| **id != port) {
                let member = self.channel_member(*existing).unwrap();
                if member.mode.protocol() != mode.protocol()
                    || self.ports[existing].max_speed != self.ports[&port].max_speed
                    || self.ports[existing].advertised_speed != self.ports[&port].advertised_speed
                    || self.ports[existing].connector != self.ports[&port].connector
                    || !channel_vlan_matches(
                        &self.ports[existing].config,
                        &self.ports[&port].config,
                    )
                {
                    return Err("% EtherChannel members must share protocol, port type, speed and VLAN configuration.".into());
                }
            }
            self.switch_services_mut(device)?
                .etherchannel
                .members
                .insert(
                    port,
                    ChannelMember {
                        group: number,
                        mode,
                    },
                );
        } else {
            self.switch_services_mut(device)?
                .etherchannel
                .members
                .remove(&port);
        }
        self.topology_revision += 1;
        Ok(())
    }

    /// Candidate pairs use the same physical carrier evaluator as normal links.
    fn channel_candidates(&self, port: PortId) -> Vec<(PortId, PortId)> {
        let Some(member) = self.channel_member(port) else {
            return vec![];
        };
        let device = self.ports[&port].device;
        let services = &self.switch_services(device).unwrap().etherchannel;
        if services.disabled.contains(&member.group) {
            return vec![];
        }
        let members = self.channel_ports(device, member.group);
        let Some(reference) = members.first().and_then(|id| self.port(*id)) else {
            return vec![];
        };
        let mut candidates = Vec::new();
        for local in members {
            let p = &self.ports[&local];
            if !channel_vlan_matches(&p.config, &reference.config)
                || p.advertised_speed != reference.advertised_speed
            {
                continue;
            }
            let path = self.physical_path(local);
            let Some(peer) = path.iter().copied().find(|id| {
                *id != local
                    && self
                        .port(*id)
                        .is_some_and(|p| matches!(p.config, PortConfig::Switch(_)))
            }) else {
                continue;
            };
            let Some(remote) = self.channel_member(peer) else {
                continue;
            };
            let peer_device = self.ports[&peer].device;
            let remote_config = &self.switch_services(peer_device).unwrap().etherchannel;
            if remote_config.disabled.contains(&remote.group)
                || !self
                    .channel_member(local)
                    .unwrap()
                    .mode
                    .compatible(remote.mode)
                || !channel_vlan_matches(&self.ports[&peer].config, &reference.config)
            {
                continue;
            }
            let remote_members = self.channel_ports(peer_device, remote.group);
            if remote_members.first().is_none_or(|id| {
                !channel_vlan_matches(&self.ports[id].config, &reference.config)
                    || self.ports[id].advertised_speed != p.advertised_speed
            }) {
                continue;
            }
            if let Some(speed) = self.physical_link_speed(local) {
                candidates.push((peer_device, remote.group, speed.mbps(), local, peer));
            }
        }
        candidates.sort();
        let Some((peer, remote_group, speed, _, _)) = candidates.first().copied() else {
            return vec![];
        };
        let remote_services = &self.switch_services(peer).unwrap().etherchannel;
        let local_controls =
            (services.system_priority, device) <= (remote_services.system_priority, peer);
        let mut pairs: Vec<_> = candidates
            .into_iter()
            .filter(|c| (c.0, c.1, c.2) == (peer, remote_group, speed))
            .map(|c| (c.3, c.4))
            .collect();
        pairs.sort_by_key(|(a, b)| {
            if local_controls {
                (*services.port_priority.get(a).unwrap_or(&32768), *a, *b)
            } else {
                (
                    *remote_services.port_priority.get(b).unwrap_or(&32768),
                    *b,
                    *a,
                )
            }
        });
        pairs
    }

    pub fn channel_active_pairs(&self, port: PortId) -> Vec<(PortId, PortId)> {
        let pairs = self.channel_candidates(port);
        let Some((local, peer)) = pairs.first().copied() else {
            return vec![];
        };
        let local_member = self.channel_member(local).unwrap();
        let remote_member = self.channel_member(peer).unwrap();
        let required = self
            .switch_services(self.ports[&local].device)
            .unwrap()
            .etherchannel
            .min_links
            .get(&local_member.group)
            .copied()
            .unwrap_or(1)
            .max(
                self.switch_services(self.ports[&peer].device)
                    .unwrap()
                    .etherchannel
                    .min_links
                    .get(&remote_member.group)
                    .copied()
                    .unwrap_or(1),
            );
        if pairs.len().min(8) < usize::from(required) {
            return vec![];
        }
        pairs.into_iter().take(8).collect()
    }

    pub fn channel_state(&self, port: PortId) -> ChannelState {
        if !self.physical_link_up(port) {
            return ChannelState::Down;
        }
        if self
            .channel_active_pairs(port)
            .iter()
            .any(|(local, _)| *local == port)
        {
            return ChannelState::Bundled;
        }
        if self
            .channel_candidates(port)
            .iter()
            .skip(8)
            .any(|(local, _)| *local == port)
        {
            ChannelState::Standby
        } else {
            ChannelState::Suspended
        }
    }

    pub fn channel_capacity_mbps(&self, device: DeviceId, group: u8) -> u32 {
        self.channel_ports(device, group)
            .first()
            .into_iter()
            .flat_map(|port| self.channel_active_pairs(*port))
            .filter_map(|(port, _)| self.physical_link_speed(port))
            .map(|speed| speed.mbps())
            .sum()
    }

    pub(crate) fn channel_forwarding(&self, port: PortId) -> bool {
        self.channel_member(port).is_none() || self.channel_state(port) == ChannelState::Bundled
    }

    pub(crate) fn same_channel(&self, a: PortId, b: PortId) -> bool {
        self.port(a).zip(self.port(b)).is_some_and(|(pa, pb)| {
            pa.device == pb.device
                && self
                    .channel_member(a)
                    .zip(self.channel_member(b))
                    .is_some_and(|(ma, mb)| ma.group == mb.group)
        })
    }

    pub(crate) fn channel_targets(
        &self,
        ingress: PortId,
        targets: Vec<PortId>,
        frame: &EthernetFrame,
    ) -> Vec<PortId> {
        let mut result = BTreeSet::new();
        let mut handled = BTreeSet::new();
        for target in targets {
            if self.same_channel(ingress, target) {
                continue;
            }
            if let Some(member) = self.channel_member(target) {
                if !handled.insert(member.group) {
                    continue;
                }
                let pairs = self.channel_active_pairs(target);
                if pairs.is_empty() {
                    continue;
                }
                let hash = self
                    .switch_services(self.ports[&target].device)
                    .unwrap()
                    .etherchannel
                    .hash;
                let bytes: Vec<_> = match hash {
                    ChannelHash::SourceMac => frame.source.0.to_vec(),
                    ChannelHash::DestinationMac => frame.destination.0.to_vec(),
                    ChannelHash::SourceDestinationMac => frame
                        .source
                        .0
                        .into_iter()
                        .chain(frame.destination.0)
                        .collect(),
                };
                let value = bytes.iter().fold(0usize, |value, byte| {
                    value.wrapping_mul(31).wrapping_add(usize::from(*byte))
                });
                result.insert(pairs[value % pairs.len()].0);
            } else {
                result.insert(target);
            }
        }
        result.into_iter().collect()
    }
}
