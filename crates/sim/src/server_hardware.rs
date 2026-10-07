use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

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
    pub psu_bays: usize,
    #[serde(default = "ServerChassis::default_integrated_psu_watts")]
    pub integrated_psu_watts: u16,
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
    },
    Ram {
        memory_type: String,
        capacity_gb: u16,
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
        power_w: u16,
    },
}

pub fn server_catalog() -> &'static ServerCatalog {
    static CATALOG: OnceLock<ServerCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../../../assets/equipment/server_parts.json"))
            .expect("valid server hardware catalog")
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

    pub fn load_watts(&self) -> u32 {
        let catalog = server_catalog();
        let cpu_w: u32 = self
            .cpus
            .iter()
            .filter_map(|id| catalog.parts.iter().find(|part| &part.id == id))
            .filter_map(|part| match part.kind {
                ServerPartKind::Cpu { tdp_w, .. } => Some(u32::from(tdp_w)),
                _ => None,
            })
            .sum();
        let card_w: u32 = self
            .pcie
            .iter()
            .flatten()
            .filter_map(|id| catalog.parts.iter().find(|part| &part.id == id))
            .filter_map(|part| match &part.kind {
                ServerPartKind::PciCard {
                    card: PciCard::Ethernet { power_w, .. },
                } => Some(u32::from(*power_w)),
                _ => None,
            })
            .sum();
        let drive_w: u32 = self
            .drives
            .iter()
            .flatten()
            .filter_map(|id| {
                crate::drive_catalog()
                    .drives
                    .iter()
                    .find(|drive| &drive.id == id)
            })
            .map(|drive| u32::from(drive.power_w))
            .sum();
        100 + cpu_w + self.ram.len() as u32 * 5 + card_w + drive_w
    }

    pub fn ready(&self) -> bool {
        if self.cpus.is_empty() || self.ram.is_empty() {
            return false;
        }
        u32::from(server_catalog().chassis.integrated_psu_watts) >= self.load_watts()
    }
}

impl ServerChassis {
    fn default_integrated_psu_watts() -> u16 {
        600
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
