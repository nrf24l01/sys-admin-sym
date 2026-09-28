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
    pub cooling_bays: usize,
    pub required_fans: usize,
    pub pcie_slots: Vec<PcieSlot>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PcieSlot {
    pub name: String,
    pub lanes: u8,
    pub generation: u8,
    pub width: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerPart {
    pub id: String,
    pub name: String,
    pub price: i64,
    #[serde(flatten)]
    pub kind: ServerPartKind,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServerPartKind {
    Cpu {
        socket: String,
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
    Cooling {
        cooling_w: u16,
    },
    PciCard {
        card: PciCard,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PciCard {
    Ethernet {
        lanes: u8,
        generation: u8,
        width: u8,
        rj45_ports: u8,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ServerHardware {
    pub cpus: Vec<String>,
    pub ram: Vec<String>,
    pub power_supplies: Vec<String>,
    pub cooling: Vec<String>,
    /// One entry per physical chassis slot. `None` means the slot is empty.
    pub pcie: Vec<Option<String>>,
    #[serde(default)]
    pub card_ports: Vec<Vec<crate::PortId>>,
}

impl ServerHardware {
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
        100 + cpu_w + self.ram.len() as u32 * 5 + card_w + self.cooling.len() as u32 * 5
    }

    pub fn ready(&self) -> bool {
        if self.cpus.is_empty()
            || self.ram.is_empty()
            || self.power_supplies.is_empty()
            || self.cooling.len() < server_catalog().chassis.required_fans
        {
            return false;
        }
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
        let cooling_w: u32 = self
            .cooling
            .iter()
            .filter_map(|id| catalog.parts.iter().find(|part| &part.id == id))
            .filter_map(|part| match part.kind {
                ServerPartKind::Cooling { cooling_w } => Some(u32::from(cooling_w)),
                _ => None,
            })
            .sum();
        let psu_w: u32 = self
            .power_supplies
            .iter()
            .filter_map(|id| catalog.parts.iter().find(|part| &part.id == id))
            .filter_map(|part| match part.kind {
                ServerPartKind::PowerSupply { capacity_w } => Some(u32::from(capacity_w)),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        cooling_w >= cpu_w && psu_w >= self.load_watts()
    }
}
