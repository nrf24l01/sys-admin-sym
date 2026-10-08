//! Estimated device demand, separate from PSU/output capacity ratings.
use super::profiles::{router_power_profile, server_power_profile};
use crate::*;
use serde::{Deserialize, Serialize};

/// Sustained simulated demand. Values are per mille, not host CPU usage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceWorkload {
    pub cpu: u16,
    pub memory: u16,
    pub storage: u16,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerComponentKind {
    Board,
    Cpu,
    Memory,
    Storage,
    Network,
    Optics,
    Fans,
    Conversion,
}
impl PowerComponentKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::Board => "board",
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Storage => "storage",
            Self::Network => "network",
            Self::Optics => "optics",
            Self::Fans => "fans",
            Self::Conversion => "conversion",
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct PowerComponent {
    pub kind: PowerComponentKind,
    pub milliwatts: u32,
    pub peak_milliwatts: u32,
}
#[derive(Debug, Clone, Default)]
pub struct DeviceConsumption {
    pub components: Vec<PowerComponent>,
    pub utilization: DeviceWorkload,
    pub network_utilization: u16,
    pub demand_mw: u32,
    pub peak_mw: u32,
    pub current: ElectricalLoad,
}
impl DeviceConsumption {
    pub(super) fn component(&mut self, kind: PowerComponentKind, current: u32, peak: u32) {
        self.components.push(PowerComponent {
            kind,
            milliwatts: current,
            peak_milliwatts: peak,
        });
        self.demand_mw = self.demand_mw.saturating_add(current);
        self.peak_mw = self.peak_mw.saturating_add(peak);
    }
}

impl NetworkSim {
    pub(crate) fn device_power_factor(&self, device: DeviceId) -> u16 {
        match self.device(device).map(|d| &d.kind) {
            Some(DeviceKind::Server(_)) => server_power_profile().power_factor_percent,
            Some(DeviceKind::Switch(switch)) => switch.model.spec().power.power_factor_percent,
            Some(DeviceKind::Router(_)) => router_power_profile().load.power_factor_percent,
            _ => 100,
        }
    }

    pub fn device_workload(&self, device: DeviceId) -> DeviceWorkload {
        self.device_workloads
            .get(&device)
            .copied()
            .unwrap_or_default()
    }
    pub(crate) fn set_device_workload(
        &mut self,
        device: DeviceId,
        workload: DeviceWorkload,
    ) -> Result<(), SimError> {
        let dev = self
            .device(device)
            .ok_or(SimError::DeviceNotFound(device))?;
        if !matches!(dev.kind, DeviceKind::Server(_) | DeviceKind::Router(_)) {
            return Err(SimError::Power(
                "Workloads require a server or router.".into(),
            ));
        }
        if [workload.cpu, workload.memory, workload.storage]
            .iter()
            .any(|value| *value > 1000)
            || (matches!(dev.kind, DeviceKind::Router(_))
                && (workload.memory != 0 || workload.storage != 0))
        {
            return Err(SimError::Power(
                "Utilization must be 0–1000; routers support CPU demand only.".into(),
            ));
        }
        if workload == DeviceWorkload::default() {
            self.device_workloads.remove(&device);
        } else {
            self.device_workloads.insert(device, workload);
        }
        Ok(())
    }

    /// Actual connected link speed determines network idle overhead. Utilization
    /// uses real recent byte counts and each installed port's rated capacity.
    pub(crate) fn network_power_utilization(&self, ports: &[PortId]) -> (u16, u16) {
        let now = self.simulation_time_ms();
        let mut capacity = 0u64;
        let mut links = 0u64;
        let mut traffic = 0u64;
        for id in ports {
            if let Some(port) = self.port(*id) {
                let rated = u64::from(port.max_speed.mbps()) * 1_000_000;
                capacity = capacity.saturating_add(rated);
                if let Some(speed) = self.physical_link_speed(*id) {
                    links += u64::from(speed.mbps()) * 1_000_000;
                    let bits = self
                        .runtime
                        .power_activity
                        .network
                        .get(id)
                        .map_or(0, |w| w.total(now))
                        .saturating_mul(8);
                    // Full duplex: TX + RX may use twice the physical rate.
                    traffic += (bits / 2).min(u64::from(speed.mbps()) * 1_000_000);
                }
            }
        }
        let ratio = |value: u64| (value.saturating_mul(1000) / capacity.max(1)).min(1000) as u16;
        (ratio(links), ratio(traffic))
    }

    fn calculate_device_consumption(&self, device: DeviceId) -> Option<DeviceConsumption> {
        let dev = self.device(device)?;
        let mut result = DeviceConsumption::default();
        let now = self.simulation_time_ms();
        let mut utilization = self.device_workload(device);
        let cpu = self
            .runtime
            .power_activity
            .cpu
            .get(&device)
            .map_or(0, |w| w.total(now));
        utilization.cpu = utilization
            .cpu
            .saturating_add((cpu / 1000).min(1000) as u16)
            .min(1000);
        let (linked, network) = self.network_power_utilization(dev.ports());
        result.network_utilization = network;
        utilization.cpu = utilization.cpu.saturating_add(network / 2).min(1000);
        let modules = self.module_power_milliwatts(device);
        match &dev.kind {
            DeviceKind::Server(server) => {
                let services = self
                    .server_operating_systems
                    .get(&device)
                    .map_or(3, |os| os.services.values().filter(|s| s.active).count());
                utilization.cpu = utilization
                    .cpu
                    .saturating_add((services.min(10) * 2) as u16)
                    .min(1000);
                if let Some(hardware) = &server.hardware {
                    if !hardware.ready() {
                        return Some(result);
                    }
                    let throughput: u64 = hardware
                        .drives
                        .iter()
                        .flatten()
                        .filter_map(|id| drive_catalog().drives.iter().find(|d| &d.id == id))
                        .map(|d| u64::from(d.read_mb_s.max(d.write_mb_s)) * 1_000_000)
                        .sum();
                    let bytes = self
                        .runtime
                        .power_activity
                        .storage
                        .get(&device)
                        .map_or(0, |w| w.total(now));
                    utilization.storage = utilization
                        .storage
                        .saturating_add(
                            (bytes.saturating_mul(1000) / throughput.max(1)).min(1000) as u16
                        )
                        .min(1000);
                    result = self.server_consumption(hardware, utilization, modules);
                } else {
                    let curve = &server_power_profile().legacy;
                    result.component(
                        PowerComponentKind::Board,
                        curve.draw_mw(utilization.cpu.max(utilization.storage)),
                        curve.peak_mw,
                    );
                    result.component(PowerComponentKind::Optics, modules.0, modules.1);
                }
            }
            DeviceKind::Switch(switch) => {
                let spec = switch.model.spec();
                let curve = spec.power;
                let activity = curve.network_activity(linked, network);
                result.component(PowerComponentKind::Board, curve.idle_mw, curve.idle_mw);
                result.component(
                    PowerComponentKind::Network,
                    curve.draw_mw(activity) - curve.idle_mw,
                    curve.peak_mw - curve.idle_mw,
                );
                result.component(PowerComponentKind::Optics, modules.0, modules.1);
            }
            DeviceKind::Router(_) => {
                let curve = &router_power_profile().load;
                let activity = utilization
                    .cpu
                    .max(network)
                    .saturating_add(
                        (u32::from(linked) * u32::from(curve.link_share_permille) / 1000) as u16,
                    )
                    .min(1000);
                result.component(PowerComponentKind::Board, curve.idle_mw, curve.idle_mw);
                result.component(
                    PowerComponentKind::Cpu,
                    curve.draw_mw(activity) - curve.idle_mw,
                    curve.peak_mw - curve.idle_mw,
                );
                result.component(PowerComponentKind::Optics, modules.0, modules.1);
            }
            _ => return Some(result),
        }
        result.utilization = utilization;
        result.network_utilization = network;
        if dev.powered {
            result.current = ElectricalLoad::from_watts_pf(
                result.demand_mw.div_ceil(1000),
                self.device_power_factor(device),
            );
        }
        Some(result)
    }

    pub fn device_consumption(&self, device: DeviceId) -> Option<DeviceConsumption> {
        let mut result = self
            .runtime
            .consumption
            .get(&device)
            .cloned()
            .or_else(|| self.calculate_device_consumption(device))?;
        result.current = if self.device(device)?.powered {
            ElectricalLoad::from_watts_pf(
                result.demand_mw.div_ceil(1000),
                self.device_power_factor(device),
            )
        } else {
            ElectricalLoad::default()
        };
        Some(result)
    }

    pub(crate) fn refresh_device_loads(&mut self) {
        let loads: Vec<_> = self
            .power
            .devices
            .keys()
            .filter_map(|id| self.calculate_device_consumption(*id).map(|c| (*id, c)))
            .collect();
        self.runtime
            .consumption
            .retain(|id, _| self.devices.contains_key(id));
        let mut changed = false;
        for (id, consumption) in loads {
            let watts = consumption.demand_mw.div_ceil(1000);
            self.runtime.consumption.insert(id, consumption);
            let load = ElectricalLoad::from_watts_pf(watts, self.device_power_factor(id));
            let power = self.power.devices.get_mut(&id).unwrap();
            if power.load != load {
                power.load = load;
                changed = true;
            }
        }
        if changed {
            self.power.recompute_now();
        }
    }

    pub(crate) fn record_guest_work(&mut self, device: DeviceId, cpu_us: u64, storage_bytes: u64) {
        let now = self.simulation_time_ms();
        if cpu_us > 0 {
            self.runtime
                .power_activity
                .cpu
                .entry(device)
                .or_default()
                .add(now, cpu_us);
        }
        if storage_bytes > 0 {
            self.runtime
                .power_activity
                .storage
                .entry(device)
                .or_default()
                .add(now, storage_bytes);
        }
        self.refresh_device_loads();
        self.sync_effective_power();
    }
}
