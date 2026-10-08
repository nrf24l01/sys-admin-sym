use crate::CageProfile;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwitchModel {
    #[default]
    #[serde(rename = "c1000_24t_4g_l")]
    Catalyst24T4G,
    #[serde(rename = "c1000_24t_4x_l")]
    Catalyst24T4X,
}

#[derive(Debug, Deserialize)]
pub struct SwitchSpec {
    pub id: String,
    pub name: String,
    pub copper_ports: usize,
    pub uplinks: usize,
    pub cage: CageProfile,
    pub power: crate::PowerProfile,
    pub switching_mbps: u32,
    pub forwarding_kpps: u32,
    pub weight_grams: u32,
    pub cpu_mhz: u32,
    pub dram_mb: u32,
    pub flash_mb: u32,
}

impl SwitchModel {
    pub fn spec(self) -> &'static SwitchSpec {
        static SPECS: OnceLock<Vec<SwitchSpec>> = OnceLock::new();
        let id = match self {
            Self::Catalyst24T4G => "c1000_24t_4g_l",
            Self::Catalyst24T4X => "c1000_24t_4x_l",
        };
        SPECS
            .get_or_init(|| {
                crate::equipment_config::equipment_catalog(
                    "switches.json",
                    include_str!("../../../../assets/equipment/switches.json"),
                )
            })
            .iter()
            .find(|spec| spec.id == id)
            .expect("catalog includes both Catalyst models")
    }
}
