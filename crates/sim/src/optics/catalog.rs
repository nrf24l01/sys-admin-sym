use super::{CableAssemblyModel, ModuleConnection, TransceiverModel};
use crate::{LocalizedText, SwitchModel};
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
    Switch { model: SwitchModel },
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
        let catalog: OpticsCatalog = crate::equipment_config::equipment_catalog(
            "optics.json",
            include_str!("../../../../assets/equipment/optics.json"),
        );
        catalog
            .validate()
            .unwrap_or_else(|error| panic!("invalid optics.json: {error}"));
        catalog
    })
}
impl OpticsCatalog {
    pub fn validate(&self) -> Result<(), String> {
        for cable in &self.cables {
            if let Some(id) = cable.medium.direct_connect_model() {
                let module = self
                    .module(id)
                    .ok_or_else(|| format!("unknown cable end {id} for {}", cable.id))?;
                if module.connection() != ModuleConnection::DirectConnect {
                    return Err(format!("{} requires a direct-connect module end", cable.id));
                }
            }
        }
        Ok(())
    }
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
