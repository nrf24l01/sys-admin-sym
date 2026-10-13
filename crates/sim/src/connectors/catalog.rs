use super::*;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorFamily {
    Network,
    Power,
    Routing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorRole {
    Data,
    Module,
    Inlet,
    Outlet,
    Anchor,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectorDefinition {
    pub id: ConnectorType,
    pub family: ConnectorFamily,
    pub role: ConnectorRole,
    pub mates_with: Vec<ConnectorType>,
    /// Socket size relative to its equipment face, used for dynamic NIC ports.
    pub default_size: [f32; 2],
    #[serde(default)]
    pub module: Option<ModuleConnectorDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModuleConnectorDefinition {
    pub label: String,
    pub accepts_modules: Vec<ConnectorType>,
    pub default_profile: crate::CageProfile,
}

#[derive(Debug, Deserialize)]
pub struct ConnectorCatalog {
    pub types: Vec<ConnectorDefinition>,
    pub equipment_maps: Vec<String>,
    pub atlases: std::collections::HashMap<String, String>,
    pub sprites: std::collections::HashMap<String, ConnectorSpriteDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectorSpriteDefinition {
    pub atlas: String,
    pub uv: [f32; 4],
    pub cable_exit: [f32; 2],
}
impl ConnectorCatalog {
    pub fn sprite(&self, name: &str) -> &ConnectorSpriteDefinition {
        self.sprites
            .get(name)
            .expect("validated connector sprite catalog")
    }
    pub fn atlas(&self, name: &str) -> &str {
        self.atlases
            .get(name)
            .expect("validated connector atlas catalog")
    }
    pub fn definition(&self, id: ConnectorType) -> &ConnectorDefinition {
        self.types
            .iter()
            .find(|d| d.id == id)
            .expect("validated connector catalog")
    }
    fn validate(&self) -> Result<(), String> {
        for name in [
            "rj45-plug",
            "iec-plug",
            "cisco-four-pin-plug",
            "cisco-six-pin-plug",
            "iec-socket",
            "empty-cage",
            "duplex-module",
            "simplex-module",
            "copper-module",
            "duplex-plug",
            "simplex-plug",
            "dac-plug",
            "aoc-plug",
        ] {
            let sprite = self
                .sprites
                .get(name)
                .ok_or_else(|| format!("missing connector sprite {name}"))?;
            let [l, t, r, b] = sprite.uv;
            if !self.atlases.contains_key(&sprite.atlas)
                || sprite
                    .uv
                    .iter()
                    .chain(sprite.cable_exit.iter())
                    .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
                || l >= r
                || t >= b
            {
                return Err(format!("invalid connector sprite {name}"));
            }
        }
        for name in ["rj45", "optical", "power", "power-plugs"] {
            let path = std::path::Path::new(
                self.atlases
                    .get(name)
                    .ok_or_else(|| format!("missing connector atlas {name}"))?,
            );
            if path.as_os_str().is_empty()
                || path.is_absolute()
                || path
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err(format!("invalid connector atlas path for {name}"));
            }
        }
        let expected = [
            ConnectorType::Rj45,
            ConnectorType::Sfp,
            ConnectorType::SfpPlus,
            ConnectorType::Sfp28,
            ConnectorType::QsfpPlus,
            ConnectorType::Qsfp28,
            ConnectorType::Lc,
            ConnectorType::IecC14,
            ConnectorType::IecC13,
            ConnectorType::CiscoFourPin,
            ConnectorType::CiscoSixPin,
            ConnectorType::Anchor,
        ];
        for id in expected {
            if self.types.iter().filter(|d| d.id == id).count() != 1 {
                return Err(format!("connector {id:?} must be defined exactly once"));
            }
        }
        for d in &self.types {
            if let Some(module) = &d.module {
                if d.role != ConnectorRole::Module
                    || d.family != ConnectorFamily::Network
                    || module.label.trim().is_empty()
                    || module.default_profile.kind.connector_type() != d.id
                    || module.default_profile.modes.is_empty()
                    || module.default_profile.max_power_mw == 0
                    || !module.accepts_modules.contains(&d.id)
                    || module
                        .accepts_modules
                        .iter()
                        .any(|id| id.module_kind().is_none())
                {
                    return Err(format!("invalid module connector {:?}", d.id));
                }
            } else if d.id.module_kind().is_some() || d.role == ConnectorRole::Module {
                return Err(format!(
                    "missing module connector definition for {:?}",
                    d.id
                ));
            }
            if d.default_size
                .iter()
                .any(|v| !v.is_finite() || *v <= 0.0 || *v > 1.0)
            {
                return Err(format!("invalid socket size for {:?}", d.id));
            }
            for mate in &d.mates_with {
                let other = self.definition(*mate);
                if other.family != d.family || !other.mates_with.contains(&d.id) {
                    return Err(format!(
                        "asymmetric/cross-family mating rule for {:?}",
                        d.id
                    ));
                }
            }
        }
        if self.equipment_maps.is_empty() {
            return Err("equipment_maps cannot be empty".into());
        }
        let mut filenames = std::collections::HashSet::new();
        for filename in &self.equipment_maps {
            if !filenames.insert(filename)
                || std::path::Path::new(filename).components().count() != 1
                || !filename.ends_with(".json")
            {
                return Err(format!(
                    "invalid or duplicate equipment map filename: {filename}"
                ));
            }
        }
        Ok(())
    }
}

pub fn connector_catalog() -> &'static ConnectorCatalog {
    static CATALOG: OnceLock<ConnectorCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let catalog: ConnectorCatalog = crate::equipment_config::equipment_catalog(
            "connectors.json",
            include_str!("../../../../assets/equipment/connectors.json"),
        );
        catalog
            .validate()
            .unwrap_or_else(|error| panic!("invalid connectors.json: {error}"));
        catalog
    })
}
