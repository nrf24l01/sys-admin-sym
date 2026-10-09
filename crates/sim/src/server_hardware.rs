use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

mod limits;
pub use limits::*;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerCatalog {
    pub chassis: ServerChassis,
    pub parts: Vec<ServerPart>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerChassis {
    pub model: String,
    pub cpu_socket: String,
    pub cpu_sockets: usize,
    pub dimm_slots: usize,
    pub memory_type: String,
    pub supported_cpu_ids: Vec<String>,
    pub max_cpu_tdp_w: u16,
    pub identical_cpus: bool,
    pub parts_hot_swappable: bool,
    pub memory: ChassisMemoryLimits,
    pub psu_bays: usize,
    pub drive_bays: Vec<DriveBay>,
    pub pcie_slots: Vec<PcieSlot>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DriveBay {
    pub name: String,
    pub interface: String,
    pub face_rect: [f32; 4],
}

#[derive(Debug, Clone, Deserialize)]
pub struct PcieSlot {
    pub name: String,
    pub cpu_socket: usize,
    pub lanes: u8,
    pub generation: u8,
    pub width: u8,
    /// Normalized rectangle on the rear face: left, top, right, bottom.
    pub face_rect: [f32; 4],
}

impl PcieSlot {
    pub fn port_position(&self, index: usize, count: usize) -> (f32, f32) {
        let [left, top, right, bottom] = self.face_rect;
        (
            left + (right - left) * (index + 1) as f32 / (count + 1) as f32,
            (top + bottom) * 0.5,
        )
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerPart {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub display_name: crate::LocalizedText,
    #[serde(default)]
    pub desc: crate::LocalizedText,
    pub price: i64,
    pub power: crate::PowerProfile,
    #[serde(flatten)]
    pub kind: ServerPartKind,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServerPartKind {
    Cpu {
        socket: String,
        #[serde(default)]
        cores: u16,
        #[serde(default)]
        frequency_mhz: u32,
        pcie_lanes: u8,
        tdp_w: u16,
        threads: u16,
        max_sockets: usize,
        pcie_generation: u8,
        memory: CpuMemoryLimits,
    },
    Ram {
        memory_type: String,
        capacity_gb: u16,
        speed_mt_s: u32,
        ranks: u8,
        voltage_mv: u16,
    },
    PowerSupply {
        capacity_w: u16,
    },
    PciCard {
        card: PciCard,
    },
}

impl ServerPartKind {
    pub fn memory_score_gb(&self) -> Option<u64> {
        let Self::Ram {
            memory_type,
            capacity_gb,
            ..
        } = self
        else {
            return None;
        };
        let generation = if memory_type.contains("DDR5") {
            5
        } else if memory_type.contains("DDR4") {
            4
        } else if memory_type.contains("DDR3") {
            3
        } else {
            return None;
        };
        let module = if memory_type.contains("RDIMM") {
            12
        } else if memory_type.contains("UDIMM") {
            10
        } else {
            9
        };
        Some(u64::from(*capacity_gb) * generation * module / 30)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PciCard {
    Ethernet {
        lanes: u8,
        generation: u8,
        width: u8,
        #[serde(alias = "rj45_ports")]
        ports: u8,
        #[serde(default)]
        connector: crate::PortConnector,
        #[serde(default)]
        cage: Option<crate::CageProfile>,
        speed_mbps: u32,
    },
}

pub fn server_catalog() -> &'static ServerCatalog {
    static CATALOG: OnceLock<ServerCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let catalog: ServerCatalog = crate::equipment_config::equipment_catalog(
            "server_parts.json",
            include_str!("../../../assets/equipment/server_parts.json"),
        );
        catalog.validate().unwrap_or_else(|error| {
            panic!("invalid equipment config assets/equipment/server_parts.json: {error}")
        });
        catalog
    })
}

/// The ready-to-use server offered by the equipment shop.
pub struct ServerFullPack;

impl ServerFullPack {
    pub const CPU: &'static str = "xeon_e_2434";
    pub const RAM: &'static str = "ddr5_ecc_16gb";
    pub const NIC: &'static str = "intel_i350_t4";
    pub const DRIVE: &'static str = "enterprise_ssd_960gb";

    pub fn price() -> i64 {
        let parts = server_catalog();
        let drive = crate::drive_catalog();
        crate::DeviceTemplate::Server.price()
            + [Self::CPU, Self::RAM, Self::NIC]
                .iter()
                .map(|id| {
                    parts
                        .parts
                        .iter()
                        .find(|part| part.id == *id)
                        .expect("full pack part exists")
                        .price
                })
                .sum::<i64>()
            + drive
                .drives
                .iter()
                .find(|model| model.id == Self::DRIVE)
                .expect("full pack drive exists")
                .price
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ServerHardware {
    pub cpus: Vec<String>,
    pub ram: Vec<String>,
    /// Physical DIMM indices parallel to `ram`; absent in legacy saves.
    #[serde(default)]
    pub ram_slot_indices: Vec<usize>,
    pub power_supplies: Vec<String>,
    /// Read old saves and refund previously purchased fans once on load.
    #[serde(default, rename = "cooling", skip_serializing)]
    pub legacy_fans: Vec<String>,
    /// One entry per physical chassis slot. `None` means the slot is empty.
    pub pcie: Vec<Option<String>>,
    #[serde(default)]
    pub drives: Vec<Option<String>>,
    #[serde(default)]
    pub card_ports: Vec<Vec<crate::PortId>>,
}

impl ServerHardware {
    pub fn pcie_link_generation(&self, index: usize) -> Option<u8> {
        let catalog = server_catalog();
        let slot = catalog.chassis.pcie_slots.get(index)?;
        let cpu = catalog
            .parts
            .iter()
            .find(|p| self.cpus.get(slot.cpu_socket) == Some(&p.id))?;
        let card = catalog
            .parts
            .iter()
            .find(|p| self.pcie.get(index).and_then(Option::as_ref) == Some(&p.id))?;
        let ServerPartKind::Cpu {
            pcie_generation, ..
        } = &cpu.kind
        else {
            return None;
        };
        let ServerPartKind::PciCard {
            card: PciCard::Ethernet { generation, .. },
        } = &card.kind
        else {
            return None;
        };
        Some(slot.generation.min(*pcie_generation).min(*generation))
    }

    pub fn compute_mhz(&self) -> u64 {
        let catalog = server_catalog();
        self.cpus
            .iter()
            .filter_map(|id| catalog.parts.iter().find(|part| &part.id == id))
            .filter_map(|part| match part.kind {
                ServerPartKind::Cpu {
                    cores,
                    frequency_mhz,
                    ..
                } => Some(u64::from(cores) * u64::from(frequency_mhz)),
                _ => None,
            })
            .sum()
    }

    /// Memory capacity weighted by generation and module class, in equivalent GB.
    pub fn memory_score_gb(&self) -> u64 {
        let catalog = server_catalog();
        self.ram
            .iter()
            .filter_map(|id| catalog.parts.iter().find(|part| &part.id == id))
            .filter_map(|part| part.kind.memory_score_gb())
            .sum()
    }

    /// Estimated maximum component load for sizing; current consumption is
    /// available through NetworkSim::device_consumption.
    pub fn load_watts(&self) -> u32 {
        self.peak_load_watts()
    }

    pub fn ready(&self) -> bool {
        if self.cpus.is_empty() || self.ram.is_empty() {
            return false;
        }
        self.validate_limits(server_catalog()).is_ok()
            && crate::server_power_profile().psu.capacity_watts >= self.load_watts()
    }
}

impl crate::NetworkSim {
    /// Replace the prototype NIC identity without recreating its saved interfaces.
    pub(crate) fn migrate_legacy_sfp_nic(&mut self) {
        const LEGACY: &str = "dual_sfpplus_10g";
        const CURRENT: &str = "intel_x520_da2";
        if let Some(count) = self.server_parts.remove(LEGACY) {
            *self.server_parts.entry(CURRENT.into()).or_default() += count;
        }
        for device in self.devices.values_mut() {
            if let crate::DeviceKind::Server(server) = &mut device.kind
                && let Some(hardware) = &mut server.hardware
            {
                for part in hardware.pcie.iter_mut().flatten() {
                    if part == LEGACY {
                        *part = CURRENT.into();
                    }
                }
            }
        }
    }
}
