use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Deserialize)]
pub struct CpuMemoryLimits {
    pub memory_types: Vec<String>,
    pub channels: usize,
    pub dimms_per_channel: usize,
    pub max_dimms: usize,
    pub max_capacity_gb: u32,
    pub max_speed_mt_s: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChassisMemoryLimits {
    pub max_capacity_gb: u32,
    pub channels_per_cpu: usize,
    pub dimms_per_channel: usize,
    pub identical_dimms: bool,
    pub supported_dimm_counts: Vec<usize>,
    pub population_order: Vec<usize>,
    pub slots: Vec<DimmSlot>,
    pub supported_modules: Vec<SupportedMemoryModule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DimmSlot {
    pub name: String,
    pub cpu_socket: usize,
    pub channel: usize,
    /// Zero-based order within a channel; primary socket must be populated first.
    pub position: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SupportedMemoryModule {
    pub capacity_gb: u16,
    pub ranks: u8,
    pub voltage_mv: u16,
    pub rated_speeds_mt_s: Vec<u32>,
    /// Index zero is 1 DIMM per channel, index one is 2 DPC, etc.
    pub operating_speeds_mt_s: Vec<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerMemoryStatus {
    pub capacity_gb: u32,
    pub max_capacity_gb: u32,
    pub populated_channels: usize,
    pub available_channels: usize,
    pub speed_mt_s: u32,
    pub balanced: bool,
}

type ChannelModules<'a> =
    BTreeMap<(usize, usize), Vec<(usize, u32, &'a SupportedMemoryModule, u32)>>;

impl ServerCatalog {
    pub fn validate(&self) -> Result<(), String> {
        let c = &self.chassis;
        let m = &c.memory;
        if c.cpu_sockets == 0
            || !(1..=2).contains(&c.psu_bays)
            || c.max_cpu_tdp_w == 0
            || c.supported_cpu_ids.is_empty()
            || c.dimm_slots != m.slots.len()
            || m.max_capacity_gb == 0
            || m.channels_per_cpu == 0
            || m.dimms_per_channel == 0
        {
            return Err(
                "chassis CPU and memory limits must be nonzero and match DIMM slots".into(),
            );
        }
        let mut positions = BTreeSet::new();
        if c.pcie_slots.iter().any(|s| {
            s.cpu_socket >= c.cpu_sockets || s.lanes == 0 || s.width < s.lanes || s.generation == 0
        }) {
            return Err("PCIe slot CPU, width, lanes or generation are invalid".into());
        }
        let mut names = BTreeSet::new();
        for slot in &m.slots {
            if slot.name.is_empty()
                || !names.insert(&slot.name)
                || slot.cpu_socket >= c.cpu_sockets
                || slot.channel >= m.channels_per_cpu
                || slot.position >= m.dimms_per_channel
                || !positions.insert((slot.cpu_socket, slot.channel, slot.position))
            {
                return Err("DIMM slot names/topology are invalid or duplicated".into());
            }
        }
        if m.population_order.iter().copied().collect::<BTreeSet<_>>()
            != (0..c.dimm_slots).collect()
            || m.population_order.len() != c.dimm_slots
            || m.supported_dimm_counts.is_empty()
            || m.supported_dimm_counts
                .iter()
                .any(|n| *n == 0 || *n > c.dimm_slots)
            || m.supported_modules.is_empty()
            || m.supported_modules.iter().any(|s| {
                s.capacity_gb == 0
                    || s.ranks == 0
                    || s.voltage_mv == 0
                    || s.rated_speeds_mt_s.is_empty()
                    || s.rated_speeds_mt_s.contains(&0)
                    || s.operating_speeds_mt_s.len() != m.dimms_per_channel
                    || s.operating_speeds_mt_s.contains(&0)
            })
        {
            return Err("memory population order, counts or speed matrix are invalid".into());
        }
        let mut ids = BTreeSet::new();
        for part in &self.parts {
            if !ids.insert(&part.id) {
                return Err(format!("duplicate part ID {}", part.id));
            }
            match &part.kind {
                ServerPartKind::Cpu {
                    cores,
                    threads,
                    frequency_mhz,
                    max_sockets,
                    pcie_lanes,
                    pcie_generation,
                    tdp_w,
                    memory,
                    ..
                } if *cores == 0
                    || threads < cores
                    || threads % cores != 0
                    || *tdp_w == 0
                    || *frequency_mhz == 0
                    || *max_sockets == 0
                    || *pcie_lanes == 0
                    || *pcie_generation == 0
                    || memory.channels == 0
                    || memory.dimms_per_channel == 0
                    || memory.max_dimms == 0
                    || memory.max_dimms > memory.channels * memory.dimms_per_channel
                    || memory.max_capacity_gb == 0
                    || memory.max_speed_mt_s == 0
                    || memory.memory_types.is_empty() =>
                {
                    return Err(format!("invalid CPU limits: {}", part.id));
                }
                ServerPartKind::Ram {
                    capacity_gb,
                    speed_mt_s,
                    ranks,
                    voltage_mv,
                    ..
                } if *capacity_gb == 0 || *speed_mt_s == 0 || *ranks == 0 || *voltage_mv == 0 => {
                    return Err(format!("invalid DIMM specifications: {}", part.id));
                }
                _ => {}
            }
        }
        for id in &c.supported_cpu_ids {
            if !self
                .parts
                .iter()
                .any(|p| p.id == *id && matches!(p.kind, ServerPartKind::Cpu { .. }))
            {
                return Err(format!("supported CPU {id} is missing from catalog"));
            }
        }
        Ok(())
    }
}

impl ServerHardware {
    pub fn dimm_slot_indices(&self, catalog: &ServerCatalog) -> Vec<usize> {
        if self.ram_slot_indices.is_empty() && !self.ram.is_empty() {
            catalog
                .chassis
                .memory
                .population_order
                .iter()
                .copied()
                .take(self.ram.len())
                .collect()
        } else {
            self.ram_slot_indices.clone()
        }
    }

    pub(crate) fn normalize_dimm_slots(&mut self) {
        self.ram_slot_indices = self.dimm_slot_indices(server_catalog());
    }

    /// Validate against an explicit catalog so configuration overrides use the same rules.
    pub fn validate_limits(&self, catalog: &ServerCatalog) -> Result<(), String> {
        let c = &catalog.chassis;
        if self.cpus.len() > c.cpu_sockets {
            return Err(format!("CPU sockets full (maximum {})", c.cpu_sockets));
        }
        for id in &self.cpus {
            let Some(ServerPart {
                kind:
                    ServerPartKind::Cpu {
                        socket,
                        tdp_w,
                        max_sockets,
                        ..
                    },
                ..
            }) = catalog.parts.iter().find(|p| p.id == *id)
            else {
                return Err(format!("unknown installed CPU {id}"));
            };
            if socket != &c.cpu_socket
                || !c.supported_cpu_ids.contains(id)
                || *tdp_w > c.max_cpu_tdp_w
                || self.cpus.len() > *max_sockets
                || (c.identical_cpus && self.cpus.iter().any(|other| other != id))
            {
                return Err(format!(
                    "CPU {id} is incompatible with this chassis or socket count"
                ));
            }
        }
        self.memory_status_with_catalog(catalog)?;
        let available: Vec<u16> = self
            .cpus
            .iter()
            .map(|id| {
                catalog
                    .parts
                    .iter()
                    .find(|p| p.id == *id)
                    .and_then(|p| match p.kind {
                        ServerPartKind::Cpu { pcie_lanes, .. } => Some(u16::from(pcie_lanes)),
                        _ => None,
                    })
                    .unwrap_or(0)
            })
            .collect();
        let mut used = vec![0u16; c.cpu_sockets];
        for (index, id) in self
            .pcie
            .iter()
            .enumerate()
            .filter_map(|(i, id)| id.as_ref().map(|id| (i, id)))
        {
            let Some(slot) = c.pcie_slots.get(index) else {
                return Err("PCIe slot does not exist".into());
            };
            let Some(ServerPart {
                kind:
                    ServerPartKind::PciCard {
                        card: PciCard::Ethernet { lanes, width, .. },
                    },
                ..
            }) = catalog.parts.iter().find(|p| p.id == *id)
            else {
                return Err(format!("unknown PCIe card {id}"));
            };
            if *width > slot.width || *lanes > slot.lanes {
                return Err(format!("PCIe card {id} does not fit {}", slot.name));
            }
            used[slot.cpu_socket] += u16::from(*lanes);
        }
        for (socket, used) in used.into_iter().enumerate() {
            let available = available.get(socket).copied().unwrap_or(0);
            if used > available {
                return Err(format!(
                    "CPU {} PCIe lane budget exceeded: need {used}, available {available}",
                    socket + 1
                ));
            }
        }
        if self.power_supplies.len() > c.psu_bays {
            return Err("power supply bays full".into());
        }
        for (index, id) in self
            .drives
            .iter()
            .enumerate()
            .filter_map(|(i, id)| id.as_ref().map(|id| (i, id)))
        {
            let Some(bay) = c.drive_bays.get(index) else {
                return Err("drive bay does not exist".into());
            };
            let Some(drive) = crate::drive_catalog().drives.iter().find(|d| d.id == *id) else {
                return Err(format!("unknown drive {id}"));
            };
            if drive.interface != bay.interface {
                return Err(format!("drive {id} is incompatible with {}", bay.name));
            }
        }
        Ok(())
    }

    pub fn memory_status(&self) -> Result<ServerMemoryStatus, String> {
        self.memory_status_with_catalog(server_catalog())
    }

    pub fn memory_status_with_catalog(
        &self,
        catalog: &ServerCatalog,
    ) -> Result<ServerMemoryStatus, String> {
        let c = &catalog.chassis;
        let m = &c.memory;
        let indices = self.dimm_slot_indices(catalog);
        if indices.len() != self.ram.len()
            || indices.iter().copied().collect::<BTreeSet<_>>().len() != indices.len()
        {
            return Err("DIMM slot assignments are missing or duplicated".into());
        }
        let mut status = ServerMemoryStatus {
            max_capacity_gb: m.max_capacity_gb,
            ..Default::default()
        };
        let mut cpu_limits = Vec::new();
        for id in &self.cpus {
            let Some(ServerPart {
                kind: ServerPartKind::Cpu { memory, .. },
                ..
            }) = catalog.parts.iter().find(|p| p.id == *id)
            else {
                return Err(format!("unknown CPU {id}"));
            };
            status.available_channels += memory.channels.min(m.channels_per_cpu);
            cpu_limits.push(memory);
        }
        if !cpu_limits.is_empty() {
            status.max_capacity_gb = status
                .max_capacity_gb
                .min(cpu_limits.iter().map(|cpu| cpu.max_capacity_gb).sum());
        }
        if self.ram.is_empty() {
            return Ok(status);
        }
        if !m.supported_dimm_counts.contains(&self.ram.len()) {
            return Err("unsupported DIMM population count".into());
        }
        let mut channels = ChannelModules::new();
        let mut capacities = vec![0u32; self.cpus.len()];
        let mut dimm_counts = vec![0usize; self.cpus.len()];
        let mut first_spec = None;
        for (id, index) in self.ram.iter().zip(indices) {
            let Some(slot) = m.slots.get(index) else {
                return Err(format!("DIMM slot {index} does not exist"));
            };
            let Some(cpu) = cpu_limits.get(slot.cpu_socket) else {
                return Err(format!(
                    "install CPU {} before populating {}",
                    slot.cpu_socket + 1,
                    slot.name
                ));
            };
            let Some(ServerPart {
                kind:
                    ServerPartKind::Ram {
                        memory_type,
                        capacity_gb,
                        speed_mt_s,
                        ranks,
                        voltage_mv,
                    },
                ..
            }) = catalog.parts.iter().find(|p| p.id == *id)
            else {
                return Err(format!("unknown DIMM {id}"));
            };
            if memory_type != &c.memory_type || !cpu.memory_types.contains(memory_type) {
                return Err(format!(
                    "DIMM type {memory_type} is incompatible with chassis/CPU"
                ));
            }
            if slot.channel >= cpu.channels || slot.position >= cpu.dimms_per_channel {
                return Err(format!(
                    "{} exceeds CPU memory channel/DIMM limits",
                    slot.name
                ));
            }
            let spec = (memory_type, capacity_gb, speed_mt_s, ranks, voltage_mv);
            if m.identical_dimms && first_spec.is_some_and(|first| first != spec) {
                return Err(
                    "this chassis requires DIMMs with identical capacity, rank, speed and voltage"
                        .into(),
                );
            }
            first_spec = Some(spec);
            let Some(module) = m.supported_modules.iter().find(|s| {
                s.capacity_gb == *capacity_gb
                    && s.ranks == *ranks
                    && s.voltage_mv == *voltage_mv
                    && s.rated_speeds_mt_s.contains(speed_mt_s)
            }) else {
                return Err(format!(
                    "unsupported DIMM capacity/rank/speed/voltage: {id}"
                ));
            };
            let capacity = u32::from(*capacity_gb);
            status.capacity_gb += capacity;
            capacities[slot.cpu_socket] += capacity;
            dimm_counts[slot.cpu_socket] += 1;
            if dimm_counts[slot.cpu_socket] > cpu.max_dimms {
                return Err(format!(
                    "CPU {} supports at most {} DIMMs",
                    slot.cpu_socket + 1,
                    cpu.max_dimms
                ));
            }
            channels
                .entry((slot.cpu_socket, slot.channel))
                .or_default()
                .push((
                    slot.position,
                    capacity,
                    module,
                    (*speed_mt_s).min(cpu.max_speed_mt_s),
                ));
        }
        if status.capacity_gb > m.max_capacity_gb
            || capacities
                .iter()
                .zip(&cpu_limits)
                .any(|(gb, cpu)| *gb > cpu.max_capacity_gb)
        {
            return Err(format!(
                "memory capacity {} GB exceeds chassis/CPU maximum {} GB",
                status.capacity_gb, status.max_capacity_gb
            ));
        }
        let mut speed = u32::MAX;
        for entries in channels.values_mut() {
            entries.sort_by_key(|entry| entry.0);
            if entries
                .iter()
                .enumerate()
                .any(|(position, entry)| position != entry.0)
            {
                return Err(
                    "populate primary DIMM socket before secondary socket in each channel".into(),
                );
            }
            for entry in entries.iter() {
                speed = speed
                    .min(entry.3)
                    .min(entry.2.operating_speeds_mt_s[entries.len() - 1]);
            }
        }
        status.speed_mt_s = speed;
        status.populated_channels = channels.len();
        let channel_capacities: BTreeSet<u32> = channels
            .values()
            .map(|entries| entries.iter().map(|entry| entry.1).sum())
            .collect();
        status.balanced =
            channels.len() == status.available_channels && channel_capacities.len() == 1;
        Ok(status)
    }
}
