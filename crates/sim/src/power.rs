//! Deterministic rack power model.
#![allow(clippy::possible_missing_else, clippy::collapsible_if)]
mod activity;
mod components;
mod consumption;
mod profiles;
mod redundancy;
pub(crate) use activity::PowerActivity;
pub use consumption::*;
pub use profiles::*;
pub use redundancy::*;

use crate::{CableRoutePoint, DeviceId, RackId};
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

/// The physical cord used between an AC outlet and a powered device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PowerCordKind {
    #[default]
    IecC13C14,
    Cisco66WAdapter,
}

impl PowerCordKind {
    pub fn allows_load(self, load: ElectricalLoad) -> bool {
        match self {
            Self::IecC13C14 => true,
            Self::Cisco66WAdapter => {
                let adapter = &router_power_profile().adapter;
                load.watts <= adapter.output_watts
                    && u64::from(load.watts) * 1000
                        <= u64::from(adapter.output_volts) * u64::from(adapter.output_current_ma)
            }
        }
    }
    /// AC load presented to the upstream outlet for a DC device load.
    pub fn input_load(self, load: ElectricalLoad) -> ElectricalLoad {
        if !self.allows_load(load) {
            return ElectricalLoad::default();
        }
        match self {
            Self::IecC13C14 => load,
            Self::Cisco66WAdapter => {
                let adapter = &router_power_profile().adapter;
                let watts = ceil(
                    u64::from(load.watts) * 100,
                    u64::from(adapter.efficiency_percent),
                );
                ElectricalLoad::from_watts_pf(s32(watts), adapter.power_factor_percent)
            }
        }
    }
}

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
    /// The first inlet, retained for save compatibility.
    Device(DeviceId),
    DevicePsu {
        device: DeviceId,
        inlet: u8,
    },
    Source(SourceId),
}
impl PowerEndpoint {
    pub fn device_id(self) -> Option<DeviceId> {
        match self {
            Self::Device(id) | Self::DevicePsu { device: id, .. } => Some(id),
            Self::Source(_) => None,
        }
    }
    pub fn inlet(self) -> u8 {
        match self {
            Self::DevicePsu { inlet, .. } => inlet,
            _ => 0,
        }
    }
    pub fn device_inlet(device: DeviceId, inlet: u8) -> Self {
        if inlet == 0 {
            Self::Device(device)
        } else {
            Self::DevicePsu { device, inlet }
        }
    }
    pub fn same_inlet(self, other: Self) -> bool {
        self == other
            || (self.device_id().is_some()
                && self.device_id() == other.device_id()
                && self.inlet() == other.inlet())
    }
    pub fn canonical(self) -> Self {
        self.device_id()
            .map_or(self, |id| Self::device_inlet(id, self.inlet()))
    }
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
        let power = ups_power_profile();
        Self {
            watts: power.capacity_watts,
            va: power.capacity_va,
            battery_wh: power.battery_wh,
            efficiency_percent: power.efficiency_percent,
            charge_watts: power.charge_watts,
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
        let power = pdu_power_profile();
        Self {
            watts: power.capacity_watts,
            va: power.capacity_va,
            current_ma: power.current_ma,
            outlets: power.outlets,
            overhead_watts: power.self_watts,
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
    /// Server loads are DC demand; PSU losses are computed separately per live feed.
    #[serde(default)]
    pub psus: Option<DevicePsus>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PowerTelemetry {
    pub output: ElectricalLoad,
    pub input: ElectricalLoad,
    pub available: bool,
    pub input_available: bool,
    pub battery_mwh: u64,
    pub runtime_seconds: Option<u64>,
    #[serde(default)]
    pub self_consumption_watts: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerSystem {
    pub racks: HashMap<RackId, RackPower>,
    pub ups: HashMap<u64, UpsState>,
    pub pdus: HashMap<u64, PduState>,
    pub devices: HashMap<DeviceId, DevicePower>,
    pub connections: HashMap<OutletId, PowerEndpoint>,
    /// Cord type parallel to `connections`; absent legacy entries are inferred by NetworkSim.
    #[serde(default)]
    pub cord_kinds: HashMap<OutletId, PowerCordKind>,
    #[serde(default)]
    pub cord_routes: HashMap<OutletId, Vec<CableRoutePoint>>,
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
    #[error("invalid device power inlet")]
    InvalidInlet,
    #[error("Cisco 66 W adapter is overloaded")]
    AdapterOverload,
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
            cord_kinds: HashMap::new(),
            cord_routes: HashMap::new(),
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
                psus: None,
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
            SourceId::Ups(i) => self
                .ups
                .get(&i)
                .map_or(0, |_| usize::from(ups_power_profile().outlets)),
            SourceId::Pdu(i) => self
                .pdus
                .get(&i)
                .map_or(0, |p| usize::from(p.outlets.min(8))),
        }
    }
    pub fn connect(&mut self, o: OutletId, e: PowerEndpoint) -> Result<(), PowerError> {
        self.connect_with_kind(o, e, PowerCordKind::IecC13C14)
    }
    pub fn connect_with_kind(
        &mut self,
        o: OutletId,
        e: PowerEndpoint,
        kind: PowerCordKind,
    ) -> Result<(), PowerError> {
        let e = e.canonical();
        if !self.source_exists(o.source) {
            return Err(PowerError::MissingSource);
        }
        if usize::from(o.index) >= self.outlets(o.source) {
            return Err(PowerError::InvalidOutlet);
        }
        if self.connections.contains_key(&o) {
            return Err(PowerError::OutletOccupied(o));
        }
        if self.outlet_for_endpoint(e).is_some() {
            return Err(PowerError::EndpointOccupied);
        }
        if let Some(id) = e.device_id() {
            let device = self.devices.get(&id).ok_or(PowerError::MissingDevice)?;
            if e.inlet() >= device.psus.as_ref().map_or(1, |p| p.count) {
                return Err(PowerError::InvalidInlet);
            }
        }
        match e {
            PowerEndpoint::Device(d) | PowerEndpoint::DevicePsu { device: d, .. }
                if !self.devices.contains_key(&d) =>
            {
                return Err(PowerError::MissingDevice);
            }
            PowerEndpoint::Device(d) | PowerEndpoint::DevicePsu { device: d, .. }
                if matches!(kind, PowerCordKind::Cisco66WAdapter)
                    && self
                        .devices
                        .get(&d)
                        .is_some_and(|x| !kind.allows_load(x.load)) =>
            {
                return Err(PowerError::AdapterOverload);
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
        self.cord_kinds.insert(o, kind);
        self.recompute();
        Ok(())
    }
    pub fn disconnect(&mut self, o: OutletId) -> bool {
        let x = self.connections.remove(&o).is_some();
        self.cord_kinds.remove(&o);
        self.cord_routes.remove(&o);
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
    pub fn device_status(&self, id: DeviceId) -> Option<&DevicePower> {
        self.devices.get(&id)
    }
    pub fn cord_kind(&self, outlet: OutletId) -> PowerCordKind {
        self.cord_kinds.get(&outlet).copied().unwrap_or_default()
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
        let self_consumption_watts = match s {
            SourceId::Rack(_) => 0,
            SourceId::Pdu(_) => input.watts.saturating_sub(output.watts),
            SourceId::Ups(_) if input_available => input.watts.saturating_sub(output.watts),
            SourceId::Ups(id) if available => {
                let watts = u64::from(output.watts) + u64::from(ups_power_profile().self_watts);
                s32(ceil(
                    watts * 100,
                    u64::from(self.ups[&id].spec.efficiency_percent.clamp(1, 100)),
                ))
                .saturating_sub(output.watts)
            }
            SourceId::Ups(_) => 0,
        };
        Some(PowerTelemetry {
            self_consumption_watts,
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
                u.online = u.enabled;
                let n = u128::from(u.spec.charge_watts) * u128::from(ms) * 1000
                    + u128::from(u.charge_remainder);
                let add = n / 3_600_000;
                u.charge_remainder = (n % 3_600_000) as u64;
                u.battery_mwh = u.battery_mwh.saturating_add(s64(add)).min(cap)
            } else if !u.enabled {
                u.online = false
            } else if u.battery_mwh > 0 {
                u.online = true;
                let eff = u64::from(u.spec.efficiency_percent.clamp(1, 100));
                let n = u128::from(out.watts.saturating_add(ups_power_profile().self_watts))
                    * u128::from(ms)
                    * 100
                    * 1000
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
                PowerEndpoint::Device(_) | PowerEndpoint::DevicePsu { .. } => {
                    self.cord_load(*outlet)
                }
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
                PowerEndpoint::Device(_) | PowerEndpoint::DevicePsu { .. } => {
                    self.cord_load(*outlet)
                }
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
                if !u.enabled || u.tripped {
                    o = ElectricalLoad::default();
                }
                if u.enabled && !u.tripped {
                    o = plus(
                        o,
                        ElectricalLoad::from_watts_pf(ups_power_profile().self_watts, 100),
                    );
                }
                let eff = u64::from(u.spec.efficiency_percent.clamp(1, 100));
                let watts = s32(ceil(u64::from(o.watts) * 100, eff));
                let va = s32(ceil(u64::from(o.va) * 100, eff));
                let mut converted = ElectricalLoad::from_watts_pf(watts, 100);
                converted.va = va;
                converted.current_ma = s32(ceil(u64::from(va) * 1000, u64::from(MAINS_VOLTAGE)));
                let mains = self.parent_available(s, &mut HashSet::new());
                return if !mains || u.tripped {
                    ElectricalLoad::default()
                } else if u.battery_mwh < u64::from(u.spec.battery_wh) * 1000 {
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
    fn runtime(&self, id: u64, output: ElectricalLoad) -> Option<u64> {
        self.ups
            .get(&id)
            .filter(|u| u.enabled && !u.tripped)
            .and_then(|u| {
                let watts = u64::from(output.watts) + u64::from(ups_power_profile().self_watts);
                if watts == 0 {
                    return None;
                }
                Some(
                    (u128::from(u.battery_mwh)
                        * 3600
                        * u128::from(u.spec.efficiency_percent.clamp(1, 100))
                        / (u128::from(watts) * 1000 * 100)) as u64,
                )
            })
    }
    fn tripped_count(&self) -> usize {
        self.racks.values().filter(|r| !r.breaker_on).count()
            + self.ups.values().filter(|u| u.tripped).count()
            + self.pdus.values().filter(|p| p.tripped).count()
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
        // A protection trip can transfer load to another feed upstream of a
        // source already visited. Iterate until no additional source trips.
        for _ in 0..=ss.len() {
            let before = self.tripped_count();
            for s in ss.iter().copied() {
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
                                Some(
                                    PowerEndpoint::Device(_) | PowerEndpoint::DevicePsu { .. },
                                ) => {
                                    let x = self.cord_load(o);
                                    x.watts > RACK_OUTLET_WATTS || x.current_ma > RACK_OUTLET_MA
                                }
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
                                    Some(
                                        PowerEndpoint::Device(_) | PowerEndpoint::DevicePsu { .. },
                                    ) => {
                                        let q = self.cord_load(OutletId {
                                            source: s,
                                            index: n as u8,
                                        });
                                        q.watts > max_w || q.va > max_va || q.current_ma > max_ma
                                    }
                                    Some(PowerEndpoint::Source(c)) => {
                                        let q = self.input_load(*c, &mut HashSet::new());
                                        q.watts > max_w || q.va > max_va || q.current_ma > max_ma
                                    }
                                    None => false,
                                }
                            });
                            if outlet_over
                                || l.watts > spec.watts
                                || l.va > spec.va
                                || l.current_ma > spec.current_ma
                            {
                                if let Some(x) = self.pdus.get_mut(&i) {
                                    x.tripped = true;
                                }
                            }
                        }
                    }
                }
            }
            if self.tripped_count() == before {
                break;
            }
        }
        let ids: Vec<_> = self.devices.keys().copied().collect();
        for d in ids {
            let on = self.device_can_run(d);
            self.devices.get_mut(&d).unwrap().effective = on;
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
    fn idle_ups_runtime_accounts_for_controller_power() {
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
        assert_eq!(t.input.watts, 9);
        assert_eq!(t.runtime_seconds, Some(364_500));
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
        assert_eq!(ut.input.watts, 126);
        assert_eq!(rt.output.watts, 126);
        p.ups.get_mut(&u).unwrap().battery_mwh = 899_000;
        let ut = p.source_telemetry(SourceId::Ups(u)).unwrap();
        let rt = p.source_telemetry(SourceId::Rack(r)).unwrap();
        assert_eq!(ut.input.watts, 246);
        assert_eq!(rt.output.watts, 246);
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
        p.ups.get_mut(&u).unwrap().battery_mwh = 899_000;
        p.set_source_enabled(SourceId::Ups(u), false);
        assert!(!p.device_status(d).unwrap().effective);
        assert_eq!(
            p.source_telemetry(SourceId::Ups(u)).unwrap().input.watts,
            120
        );
        p.ups.get_mut(&u).unwrap().battery_mwh = u64::from(p.ups[&u].spec.battery_wh) * 1000;
        p.recompute();
        assert_eq!(p.source_telemetry(SourceId::Ups(u)).unwrap().input.watts, 0);
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
