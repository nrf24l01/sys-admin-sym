//! Deterministic rack power model.
#![allow(clippy::possible_missing_else, clippy::collapsible_if)]
use crate::{DeviceId, RackId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;
fn default_true() -> bool {
    true
}

pub const MAINS_VOLTAGE: u32 = 230;
pub const RACK_C13_OUTLETS: usize = 4;
pub const RACK_OUTLET_WATTS: u32 = 2_300;
pub const RACK_OUTLET_MA: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceId {
    Rack(RackId),
    Ups(u64),
    Pdu(u64),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OutletId {
    pub source: SourceId,
    pub index: u8,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerEndpoint {
    Device(DeviceId),
    Source(SourceId),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ElectricalLoad {
    pub watts: u32,
    pub va: u32,
    pub current_ma: u32,
}
impl ElectricalLoad {
    pub fn from_watts_pf(watts: u32, pf: u16) -> Self {
        let pf = u32::from(pf.clamp(1, 100));
        let va = ceil(u64::from(watts) * 100, u64::from(pf));
        Self {
            watts,
            va: s32(va),
            current_ma: s32(ceil(va * 1000, u64::from(MAINS_VOLTAGE))),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RackPower {
    pub mains_on: bool,
    pub breaker_on: bool,
    pub outlets: [bool; RACK_C13_OUTLETS],
}
impl Default for RackPower {
    fn default() -> Self {
        Self {
            mains_on: true,
            breaker_on: true,
            outlets: [true; 4],
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpsSpec {
    pub watts: u32,
    pub va: u32,
    pub battery_wh: u32,
    pub efficiency_percent: u16,
    pub charge_watts: u32,
}
impl Default for UpsSpec {
    fn default() -> Self {
        Self {
            watts: 1000,
            va: 1500,
            battery_wh: 900,
            efficiency_percent: 90,
            charge_watts: 120,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpsState {
    pub spec: UpsSpec,
    pub battery_wh: u32,
    #[serde(default)]
    pub battery_mwh: u64,
    pub online: bool,
    pub tripped: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub elapsed_seconds: u64,
    #[serde(default)]
    elapsed_milliseconds: u64,
    #[serde(default)]
    charge_remainder: u64,
    discharge_remainder: u64,
}
impl UpsState {
    pub fn new(spec: UpsSpec) -> Self {
        Self {
            battery_wh: spec.battery_wh,
            battery_mwh: u64::from(spec.battery_wh) * 1000,
            spec,
            online: true,
            tripped: false,
            enabled: true,
            elapsed_seconds: 0,
            elapsed_milliseconds: 0,
            charge_remainder: 0,
            discharge_remainder: 0,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PduState {
    pub watts: u32,
    pub va: u32,
    pub current_ma: u32,
    pub outlets: u8,
    pub overhead_watts: u32,
    pub tripped: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}
impl Default for PduState {
    fn default() -> Self {
        Self {
            watts: 3680,
            va: 3680,
            current_ma: 16000,
            outlets: 8,
            overhead_watts: 5,
            tripped: false,
            enabled: true,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevicePower {
    pub requested: bool,
    pub load: ElectricalLoad,
    pub effective: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PowerTelemetry {
    pub output: ElectricalLoad,
    pub input: ElectricalLoad,
    pub available: bool,
    pub input_available: bool,
    pub battery_mwh: u64,
    pub runtime_seconds: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerSystem {
    pub racks: HashMap<RackId, RackPower>,
    pub ups: HashMap<u64, UpsState>,
    pub pdus: HashMap<u64, PduState>,
    pub devices: HashMap<DeviceId, DevicePower>,
    pub connections: HashMap<OutletId, PowerEndpoint>,
    #[serde(default)]
    pub next_source_id: u64,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PowerError {
    #[error("source does not exist")]
    MissingSource,
    #[error("outlet {0:?} is occupied")]
    OutletOccupied(OutletId),
    #[error("invalid outlet")]
    InvalidOutlet,
    #[error("power connection would create a cycle")]
    Cycle,
    #[error("endpoint already has a power connection")]
    EndpointOccupied,
    #[error("power source is tripped")]
    Tripped,
    #[error("device does not exist")]
    MissingDevice,
}
impl Default for PowerSystem {
    fn default() -> Self {
        Self::new()
    }
}
impl PowerSystem {
    pub fn source_exists_public(&self, s: SourceId) -> bool {
        self.source_exists(s)
    }
    pub fn recompute_now(&mut self) {
        self.recompute();
    }
    pub fn new() -> Self {
        Self {
            racks: HashMap::new(),
            ups: HashMap::new(),
            pdus: HashMap::new(),
            devices: HashMap::new(),
            connections: HashMap::new(),
            next_source_id: 1,
        }
    }
    pub fn add_rack(&mut self, id: RackId) {
        self.racks.entry(id).or_default();
    }
    pub fn add_device(&mut self, id: DeviceId, watts: u32, pf: u16) {
        self.devices.insert(
            id,
            DevicePower {
                requested: true,
                load: ElectricalLoad::from_watts_pf(watts, pf),
                effective: false,
            },
        );
        self.recompute();
    }
    pub fn add_ups(&mut self, spec: UpsSpec) -> u64 {
        let id = self.next_source_id.max(1);
        self.next_source_id = id.saturating_add(1);
        self.ups.insert(id, UpsState::new(spec));
        id
    }
    pub fn add_pdu(&mut self, state: PduState) -> u64 {
        let id = self.next_source_id.max(1);
        self.next_source_id = id.saturating_add(1);
        self.pdus.insert(id, state);
        id
    }
    pub fn outlets(&self, s: SourceId) -> usize {
        match s {
            SourceId::Rack(i) => self.racks.get(&i).map_or(0, |_| 4),
            SourceId::Ups(i) => self.ups.get(&i).map_or(0, |_| 4),
            SourceId::Pdu(i) => self
                .pdus
                .get(&i)
                .map_or(0, |p| usize::from(p.outlets.min(8))),
        }
    }
    pub fn connect(&mut self, o: OutletId, e: PowerEndpoint) -> Result<(), PowerError> {
        if !self.source_exists(o.source) {
            return Err(PowerError::MissingSource);
        }
        if usize::from(o.index) >= self.outlets(o.source) {
            return Err(PowerError::InvalidOutlet);
        }
        if self.connections.contains_key(&o) {
            return Err(PowerError::OutletOccupied(o));
        }
        if self.connections.values().any(|x| *x == e) {
            return Err(PowerError::EndpointOccupied);
        }
        match e {
            PowerEndpoint::Device(d) if !self.devices.contains_key(&d) => {
                return Err(PowerError::MissingDevice);
            }
            PowerEndpoint::Source(s) => {
                if !self.source_exists(s) {
                    return Err(PowerError::MissingSource);
                }
                if matches!(s, SourceId::Rack(_))
                    || s == o.source
                    || self.reaches(s, o.source, &mut HashSet::new())
                    || matches!((o.source, s), (SourceId::Ups(_), SourceId::Ups(_)))
                    || (self.ancestor_has_ups(o.source, &mut HashSet::new())
                        && self.tree_has_ups(s, &mut HashSet::new()))
                {
                    return Err(PowerError::Cycle);
                }
            }
            _ => {}
        }
        self.connections.insert(o, e);
        self.recompute();
        Ok(())
    }
    pub fn disconnect(&mut self, o: OutletId) -> bool {
        let x = self.connections.remove(&o).is_some();
        if x {
            self.recompute()
        }
        x
    }
    pub fn set_requested(&mut self, d: DeviceId, r: bool) -> Result<(), PowerError> {
        self.devices
            .get_mut(&d)
            .ok_or(PowerError::MissingDevice)?
            .requested = r;
        self.recompute();
        Ok(())
    }
    pub fn reset_breaker(&mut self, s: SourceId) {
        match s {
            SourceId::Rack(i) => {
                if let Some(x) = self.racks.get_mut(&i) {
                    x.breaker_on = true
                }
            }
            SourceId::Ups(i) => {
                if let Some(x) = self.ups.get_mut(&i) {
                    x.tripped = false
                }
            }
            SourceId::Pdu(i) => {
                if let Some(x) = self.pdus.get_mut(&i) {
                    x.tripped = false
                }
            }
        }
        self.recompute()
    }
    pub fn set_rack_mains(&mut self, id: RackId, on: bool) {
        if let Some(x) = self.racks.get_mut(&id) {
            x.mains_on = on
        }
        self.recompute()
    }
    pub fn set_source_enabled(&mut self, s: SourceId, enabled: bool) {
        match s {
            SourceId::Rack(i) => {
                if let Some(r) = self.racks.get_mut(&i) {
                    r.mains_on = enabled;
                }
            }
            SourceId::Ups(i) => {
                if let Some(u) = self.ups.get_mut(&i) {
                    u.enabled = enabled;
                }
            }
            SourceId::Pdu(i) => {
                if let Some(p) = self.pdus.get_mut(&i) {
                    p.enabled = enabled;
                }
            }
        }
        self.recompute();
    }
    pub fn source_enabled(&self, s: SourceId) -> bool {
        match s {
            SourceId::Rack(i) => self.racks.get(&i).is_some_and(|r| r.mains_on),
            SourceId::Ups(i) => self.ups.get(&i).is_some_and(|u| u.enabled),
            SourceId::Pdu(i) => self.pdus.get(&i).is_some_and(|p| p.enabled),
        }
    }
    pub fn device_status(&self, id: DeviceId) -> Option<DevicePower> {
        self.devices.get(&id).cloned()
    }
    pub fn source_telemetry(&self, s: SourceId) -> Option<PowerTelemetry> {
        if !self.source_exists(s) {
            return None;
        }
        let output = self.source_load(s, &mut HashSet::new());
        let input = self.input_load(s, &mut HashSet::new());
        let available = self.available(s, &mut HashSet::new());
        let input_available = match s {
            SourceId::Rack(_) => available,
            SourceId::Ups(_) | SourceId::Pdu(_) => self.parent_available(s, &mut HashSet::new()),
        };
        let (b, r) = match s {
            SourceId::Ups(i) => self
                .ups
                .get(&i)
                .map_or((0, None), |u| (u.battery_mwh, self.runtime(i, output))),
            _ => (0, None),
        };
        Some(PowerTelemetry {
            output,
            input,
            available,
            input_available,
            battery_mwh: b,
            runtime_seconds: r,
        })
    }
    pub fn tick(&mut self, seconds: u64) {
        self.tick_ms(seconds.saturating_mul(1000))
    }
    pub fn tick_ms(&mut self, ms: u64) {
        if ms == 0 {
            return;
        }
        self.recompute();
        let ids: Vec<_> = self.ups.keys().copied().collect();
        for id in ids {
            let s = SourceId::Ups(id);
            let out = self.source_load(s, &mut HashSet::new());
            let mains = self.parent_available(s, &mut HashSet::new());
            let u = self.ups.get_mut(&id).unwrap();
            let cap = u64::from(u.spec.battery_wh) * 1000;
            if u.tripped {
                u.online = false
            } else if mains {
                u.online = true;
                let n = u128::from(u.spec.charge_watts) * u128::from(ms) * 1000
                    + u128::from(u.charge_remainder);
                let add = n / 3_600_000;
                u.charge_remainder = (n % 3_600_000) as u64;
                u.battery_mwh = u.battery_mwh.saturating_add(s64(add)).min(cap)
            } else if out.watts > 0 && u.battery_mwh > 0 {
                u.online = true;
                let eff = u64::from(u.spec.efficiency_percent.clamp(1, 100));
                let n = u128::from(out.watts) * u128::from(ms) * 100 * 1000
                    + u128::from(u.discharge_remainder);
                let used = n / (u128::from(eff) * 3_600_000);
                u.discharge_remainder = (n % (u128::from(eff) * 3_600_000)) as u64;
                u.battery_mwh = u.battery_mwh.saturating_sub(s64(used));
                if u.battery_mwh == 0 {
                    u.online = false
                }
            } else {
                u.online = false
            }
            u.battery_wh = s32(u.battery_mwh / 1000);
            u.elapsed_milliseconds = u.elapsed_milliseconds.saturating_add(ms);
            u.elapsed_seconds = u.elapsed_milliseconds / 1000
        }
        self.recompute()
    }
    fn source_exists(&self, s: SourceId) -> bool {
        match s {
            SourceId::Rack(i) => self.racks.contains_key(&i),
            SourceId::Ups(i) => self.ups.contains_key(&i),
            SourceId::Pdu(i) => self.pdus.contains_key(&i),
        }
    }
    fn reaches(&self, f: SourceId, t: SourceId, seen: &mut HashSet<SourceId>) -> bool {
        if !seen.insert(f) {
            return false;
        }
        self.connections.iter().any(|(o, e)| {
            o.source == f && matches!(e,PowerEndpoint::Source(s)if *s==t||self.reaches(*s,t,seen))
        })
    }
    fn ancestor_has_ups(&self, s: SourceId, seen: &mut HashSet<SourceId>) -> bool {
        if !seen.insert(s) {
            return false;
        }
        matches!(s, SourceId::Ups(_))
            || self.connections.iter().any(|(o, e)| {
                matches!(e, PowerEndpoint::Source(x) if *x == s)
                    && self.ancestor_has_ups(o.source, seen)
            })
    }
    fn tree_has_ups(&self, s: SourceId, seen: &mut HashSet<SourceId>) -> bool {
        if !seen.insert(s) {
            return false;
        }
        matches!(s, SourceId::Ups(_))
            || self.connections.iter().any(|(o, e)| {
                o.source == s
                    && matches!(e, PowerEndpoint::Source(x) if self.tree_has_ups(*x, seen))
            })
    }
    fn parent_available(&self, t: SourceId, seen: &mut HashSet<SourceId>) -> bool {
        let Some((parent, index)) = self
            .connections
            .iter()
            .filter_map(|(o, e)| {
                matches!(e, PowerEndpoint::Source(s) if *s == t).then_some((o.source, o.index))
            })
            .min_by_key(|(s, i)| (format!("{s:?}"), *i))
        else {
            return false;
        };
        if let SourceId::Rack(id) = parent {
            if !self
                .racks
                .get(&id)
                .is_some_and(|r| r.outlets.get(usize::from(index)).copied().unwrap_or(false))
            {
                return false;
            }
        }
        self.available(parent, seen)
    }
    fn available(&self, s: SourceId, seen: &mut HashSet<SourceId>) -> bool {
        if !seen.insert(s) {
            return false;
        }
        match s {
            SourceId::Rack(i) => self
                .racks
                .get(&i)
                .is_some_and(|r| r.mains_on && r.breaker_on),
            SourceId::Ups(i) => self.ups.get(&i).is_some_and(|u| {
                u.enabled && !u.tripped && (self.parent_available(s, seen) || u.battery_mwh > 0)
            }),
            SourceId::Pdu(i) => {
                self.pdus.get(&i).is_some_and(|p| p.enabled && !p.tripped)
                    && self.parent_available(s, seen)
            }
        }
    }
    fn source_load(&self, s: SourceId, seen: &mut HashSet<SourceId>) -> ElectricalLoad {
        if !seen.insert(s) || !self.available(s, &mut HashSet::new()) {
            return ElectricalLoad::default();
        }
        let mut out = ElectricalLoad::default();
        let mut xs: Vec<_> = self
            .connections
            .iter()
            .filter(|(o, _)| o.source == s)
            .collect();
        xs.sort_by_key(|(o, _)| o.index);
        for (outlet, e) in xs {
            if let SourceId::Rack(id) = s {
                if !self.racks.get(&id).is_some_and(|r| {
                    r.outlets
                        .get(usize::from(outlet.index))
                        .copied()
                        .unwrap_or(false)
                }) {
                    continue;
                }
            }
            let l = match e {
                PowerEndpoint::Device(d) => self
                    .devices
                    .get(d)
                    .filter(|x| x.requested)
                    .map_or_default(|x| x.load),
                PowerEndpoint::Source(c) => self.input_load(*c, seen),
            };
            out = plus(out, l)
        }
        out
    }
    fn input_load(&self, s: SourceId, seen: &mut HashSet<SourceId>) -> ElectricalLoad {
        if !seen.insert(s)
            || (!self.available(s, &mut HashSet::new()) && !matches!(s, SourceId::Ups(_)))
        {
            return ElectricalLoad::default();
        }
        let mut o = ElectricalLoad::default();
        let mut xs: Vec<_> = self
            .connections
            .iter()
            .filter(|(x, _)| x.source == s)
            .collect();
        xs.sort_by_key(|(x, _)| x.index);
        for (outlet, e) in xs {
            if let SourceId::Rack(id) = s
                && !self.racks.get(&id).is_some_and(|r| {
                    r.outlets
                        .get(usize::from(outlet.index))
                        .copied()
                        .unwrap_or(false)
                })
            {
                continue;
            }
            let l = match e {
                PowerEndpoint::Device(d) => self
                    .devices
                    .get(d)
                    .filter(|x| x.requested)
                    .map_or_default(|x| x.load),
                PowerEndpoint::Source(c) => self.input_load(*c, seen),
            };
            o = plus(o, l);
        }
        if let SourceId::Pdu(i) = s {
            if let Some(p) = self.pdus.get(&i) {
                o = plus(o, ElectricalLoad::from_watts_pf(p.overhead_watts, 100));
            }
        }
        if let SourceId::Ups(i) = s {
            if let Some(u) = self.ups.get(&i) {
                if !u.enabled {
                    o = ElectricalLoad::default();
                }
                let eff = u64::from(u.spec.efficiency_percent.clamp(1, 100));
                let watts = s32(ceil(u64::from(o.watts) * 100, eff));
                let va = s32(ceil(u64::from(o.va) * 100, eff));
                let mut converted = ElectricalLoad::from_watts_pf(watts, 100);
                converted.va = va;
                converted.current_ma = s32(ceil(u64::from(va) * 1000, u64::from(MAINS_VOLTAGE)));
                let mains = self.parent_available(s, &mut HashSet::new());
                return if !mains {
                    ElectricalLoad::default()
                } else if !u.enabled || u.battery_mwh < u64::from(u.spec.battery_wh) * 1000 {
                    plus(
                        converted,
                        ElectricalLoad::from_watts_pf(u.spec.charge_watts, 100),
                    )
                } else {
                    converted
                };
            }
        }
        o
    }
    fn runtime(&self, id: u64, o: ElectricalLoad) -> Option<u64> {
        self.ups.get(&id).and_then(|u| {
            (o.watts > 0).then(|| {
                u.battery_mwh * 3600
                    / (u64::from(o.watts) * 1000 * 100
                        / u64::from(u.spec.efficiency_percent.max(1)))
            })
        })
    }
    fn recompute(&mut self) {
        let mut ss: Vec<_> = self
            .racks
            .keys()
            .copied()
            .map(SourceId::Rack)
            .chain(self.ups.keys().copied().map(SourceId::Ups))
            .chain(self.pdus.keys().copied().map(SourceId::Pdu))
            .collect();
        ss.sort_by_key(|s| format!("{s:?}"));
        for s in ss {
            let l = self.input_load(s, &mut HashSet::new());
            match s {
                SourceId::Rack(i) => {
                    if !self.available(s, &mut HashSet::new()) {
                        continue;
                    }
                    let outlet_over = (0..4).any(|n| {
                        let o = OutletId {
                            source: s,
                            index: n,
                        };
                        if !self.racks.get(&i).is_some_and(|r| r.outlets[n as usize]) {
                            return false;
                        }
                        match self.connections.get(&o) {
                            Some(PowerEndpoint::Device(d)) => self
                                .devices
                                .get(d)
                                .filter(|x| x.requested)
                                .is_some_and(|x| {
                                    x.load.watts > RACK_OUTLET_WATTS
                                        || x.load.current_ma > RACK_OUTLET_MA
                                }),
                            Some(PowerEndpoint::Source(c)) => {
                                let x = self.input_load(*c, &mut HashSet::new());
                                x.watts > RACK_OUTLET_WATTS || x.current_ma > RACK_OUTLET_MA
                            }
                            None => false,
                        }
                    });
                    if outlet_over || l.current_ma > 16_000 {
                        if let Some(x) = self.racks.get_mut(&i) {
                            x.breaker_on = false
                        }
                    }
                }
                SourceId::Ups(i) => {
                    let out = self.source_load(s, &mut HashSet::new());
                    if let Some(x) = self.ups.get_mut(&i) {
                        if out.watts > x.spec.watts || out.va > x.spec.va {
                            x.tripped = true;
                            x.online = false
                        }
                    }
                }
                SourceId::Pdu(i) => {
                    if !self.available(s, &mut HashSet::new()) {
                        continue;
                    }
                    if let Some(spec) = self.pdus.get(&i) {
                        let (max_w, max_va, max_ma, count) = (
                            RACK_OUTLET_WATTS,
                            RACK_OUTLET_WATTS,
                            RACK_OUTLET_MA,
                            usize::from(spec.outlets.min(8)),
                        );
                        let outlet_over = (0..count).any(|n| {
                            match self.connections.get(&OutletId {
                                source: s,
                                index: n as u8,
                            }) {
                                Some(PowerEndpoint::Device(d)) => self
                                    .devices
                                    .get(d)
                                    .filter(|d| d.requested)
                                    .is_some_and(|d| {
                                        d.load.watts > max_w
                                            || d.load.va > max_va
                                            || d.load.current_ma > max_ma
                                    }),
                                Some(PowerEndpoint::Source(c)) => {
                                    let q = self.source_load(*c, &mut HashSet::new());
                                    q.watts > max_w || q.va > max_va || q.current_ma > max_ma
                                }
                                None => false,
                            }
                        });
                        if outlet_over || l.watts > max_w || l.va > max_va || l.current_ma > max_ma
                        {
                            if let Some(x) = self.pdus.get_mut(&i) {
                                x.tripped = true;
                            }
                        }
                    }
                }
            }
        }
        let ids: Vec<_> = self.devices.keys().copied().collect();
        for d in ids {
            let p = self
                .connections
                .iter()
                .filter_map(|(o, e)| {
                    matches!(e,PowerEndpoint::Device(x)if *x==d).then_some((o.source, o.index))
                })
                .min_by_key(|s| format!("{s:?}"));
            let on = p.is_some_and(|(s, index)| {
                (if let SourceId::Rack(id) = s {
                    self.racks.get(&id).is_some_and(|r| {
                        r.outlets.get(usize::from(index)).copied().unwrap_or(false)
                    })
                } else {
                    true
                }) && self.available(s, &mut HashSet::new())
            });
            if let Some(x) = self.devices.get_mut(&d) {
                x.effective = x.requested && on
            }
        }
    }
}
#[allow(clippy::manual_checked_ops)]
fn ceil(a: u64, b: u64) -> u64 {
    if b == 0 {
        u64::MAX
    } else {
        a.saturating_add(b - 1) / b
    }
}
fn s32(x: u64) -> u32 {
    x.min(u64::from(u32::MAX)) as u32
}
fn s64(x: u128) -> u64 {
    x.min(u128::from(u64::MAX)) as u64
}
fn plus(a: ElectricalLoad, b: ElectricalLoad) -> ElectricalLoad {
    ElectricalLoad {
        watts: s32(u64::from(a.watts) + u64::from(b.watts)),
        va: s32(u64::from(a.va) + u64::from(b.va)),
        current_ma: s32(u64::from(a.current_ma) + u64::from(b.current_ma)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chain_outage() {
        let r = RackId(1);
        let d = DeviceId(1);
        let mut p = PowerSystem::new();
        p.add_rack(r);
        p.add_device(d, 100, 80);
        let u = p.add_ups(UpsSpec::default());
        let q = p.add_pdu(PduState::default());
        p.connect(
            OutletId {
                source: SourceId::Rack(r),
                index: 0,
            },
            PowerEndpoint::Source(SourceId::Ups(u)),
        )
        .unwrap();
        p.connect(
            OutletId {
                source: SourceId::Ups(u),
                index: 0,
            },
            PowerEndpoint::Source(SourceId::Pdu(q)),
        )
        .unwrap();
        p.connect(
            OutletId {
                source: SourceId::Pdu(q),
                index: 0,
            },
            PowerEndpoint::Device(d),
        )
        .unwrap();
        assert!(p.device_status(d).unwrap().effective);
        p.set_rack_mains(r, false);
        assert!(p.device_status(d).unwrap().effective);
        p.tick_ms(3_600_000);
        assert!(p.ups[&u].battery_mwh < 900_000)
    }
    #[test]
    fn tick_invariant() {
        let r = RackId(1);
        let d = DeviceId(1);
        let mut a = PowerSystem::new();
        a.add_rack(r);
        a.add_device(d, 101, 100);
        let u = a.add_ups(UpsSpec::default());
        a.connect(
            OutletId {
                source: SourceId::Rack(r),
                index: 0,
            },
            PowerEndpoint::Source(SourceId::Ups(u)),
        )
        .unwrap();
        a.connect(
            OutletId {
                source: SourceId::Ups(u),
                index: 0,
            },
            PowerEndpoint::Device(d),
        )
        .unwrap();
        a.set_rack_mains(r, false);
        let mut b = a.clone();
        a.tick_ms(10_000);
        for _ in 0..10 {
            b.tick_ms(1_000)
        }
        assert_eq!(a.ups[&u].battery_mwh, b.ups[&u].battery_mwh)
    }
    #[test]
    fn trip_reset() {
        let r = RackId(1);
        let d = DeviceId(1);
        let mut p = PowerSystem::new();
        p.add_rack(r);
        p.add_device(d, 20_000, 100);
        p.connect(
            OutletId {
                source: SourceId::Rack(r),
                index: 0,
            },
            PowerEndpoint::Device(d),
        )
        .unwrap();
        assert!(!p.racks[&r].breaker_on);
        p.reset_breaker(SourceId::Rack(r));
        assert!(!p.racks[&r].breaker_on)
    }

    #[test]
    fn idle_ups_runtime_has_no_division_or_fake_runtime() {
        let r = RackId(9);
        let mut p = PowerSystem::new();
        p.add_rack(r);
        let u = p.add_ups(UpsSpec::default());
        p.connect(
            OutletId {
                source: SourceId::Rack(r),
                index: 0,
            },
            PowerEndpoint::Source(SourceId::Ups(u)),
        )
        .unwrap();
        let t = p.source_telemetry(SourceId::Ups(u)).unwrap();
        assert_eq!(t.output.watts, 0);
        assert_eq!(t.runtime_seconds, None);
        p.tick_ms(u64::MAX);
        assert_eq!(p.ups[&u].battery_mwh, 900_000);
    }

    #[test]
    fn exact_chain_watts_va_input_and_battery_modes() {
        let r = RackId(10);
        let d = DeviceId(10);
        let mut p = PowerSystem::new();
        p.add_rack(r);
        p.add_device(d, 100, 100);
        let u = p.add_ups(UpsSpec::default());
        let q = p.add_pdu(PduState::default());
        p.connect(
            OutletId {
                source: SourceId::Rack(r),
                index: 0,
            },
            PowerEndpoint::Source(SourceId::Ups(u)),
        )
        .unwrap();
        p.connect(
            OutletId {
                source: SourceId::Ups(u),
                index: 0,
            },
            PowerEndpoint::Source(SourceId::Pdu(q)),
        )
        .unwrap();
        p.connect(
            OutletId {
                source: SourceId::Pdu(q),
                index: 0,
            },
            PowerEndpoint::Device(d),
        )
        .unwrap();
        let ut = p.source_telemetry(SourceId::Ups(u)).unwrap();
        let rt = p.source_telemetry(SourceId::Rack(r)).unwrap();
        assert_eq!(
            p.source_telemetry(SourceId::Pdu(q)).unwrap().output.watts,
            100
        );
        assert_eq!(ut.output.watts, 105);
        assert_eq!(ut.input.watts, 117);
        assert_eq!(rt.output.watts, 117);
        p.ups.get_mut(&u).unwrap().battery_mwh = 899_000;
        let ut = p.source_telemetry(SourceId::Ups(u)).unwrap();
        let rt = p.source_telemetry(SourceId::Rack(r)).unwrap();
        assert_eq!(ut.input.watts, 237);
        assert_eq!(rt.output.watts, 237);
        p.set_rack_mains(r, false);
        let ut = p.source_telemetry(SourceId::Ups(u)).unwrap();
        let rt = p.source_telemetry(SourceId::Rack(r)).unwrap();
        assert_eq!(ut.output.watts, 105);
        assert_eq!(ut.input.watts, 0);
        assert_eq!(rt.output.watts, 0);
    }

    #[test]
    fn disabled_source_has_no_output_or_drain_and_recharges_on_mains() {
        let r = RackId(11);
        let d = DeviceId(11);
        let mut p = PowerSystem::new();
        p.add_rack(r);
        p.add_device(d, 100, 100);
        let u = p.add_ups(UpsSpec::default());
        p.connect(
            OutletId {
                source: SourceId::Rack(r),
                index: 0,
            },
            PowerEndpoint::Source(SourceId::Ups(u)),
        )
        .unwrap();
        p.connect(
            OutletId {
                source: SourceId::Ups(u),
                index: 0,
            },
            PowerEndpoint::Device(d),
        )
        .unwrap();
        p.set_source_enabled(SourceId::Ups(u), false);
        assert!(!p.device_status(d).unwrap().effective);
        assert_eq!(
            p.source_telemetry(SourceId::Ups(u)).unwrap().input.watts,
            120
        );
        let before = p.ups[&u].battery_mwh;
        p.set_rack_mains(r, false);
        p.tick_ms(3_600_000);
        assert_eq!(p.ups[&u].battery_mwh, before);
        p.set_source_enabled(SourceId::Ups(u), true);
        p.set_rack_mains(r, true);
        p.tick_ms(1_000);
        assert!(p.ups[&u].battery_mwh >= before);
    }
}
