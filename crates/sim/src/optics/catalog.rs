use super::{CableAssemblyModel, CageProfile, TransceiverModel};
use crate::LocalizedText;
use serde::Deserialize;
use std::sync::OnceLock;
#[derive(Debug, Deserialize)]
pub struct OpticalHardwareModel {
    pub id: String,
    pub display_name: LocalizedText,
    pub desc: LocalizedText,
    pub price: i64,
    #[serde(flatten)]
    pub profile: OpticalHardwareProfile,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OpticalHardwareProfile {
    Switch { cage: CageProfile },
    FiberPanel,
}
#[derive(Debug, Deserialize)]
pub struct OpticsCatalog {
    pub modules: Vec<TransceiverModel>,
    pub cables: Vec<CableAssemblyModel>,
    pub hardware: Vec<OpticalHardwareModel>,
}
pub fn optics_catalog() -> &'static OpticsCatalog {
    static CATALOG: OnceLock<OpticsCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../assets/equipment/optics.json"))
            .expect("valid optical equipment catalog")
    })
}
impl OpticsCatalog {
    pub fn hardware(&self, id: &str) -> Option<&OpticalHardwareModel> {
        self.hardware.iter().find(|m| m.id == id)
    }
    pub fn module(&self, id: &str) -> Option<&TransceiverModel> {
        self.modules.iter().find(|m| m.id == id)
    }
    pub fn cable(&self, id: &str) -> Option<&CableAssemblyModel> {
        self.cables.iter().find(|m| m.id == id)
    }
}
