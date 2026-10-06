//! Presentation metadata belongs to the item config, keyed by the item identity.
use cloud_provider_sim::LocalizedText;
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Deserialize)]
pub(super) struct Item {
    id: String,
    display_name: LocalizedText,
    desc: LocalizedText,
}
#[derive(Deserialize)]
struct ItemFile {
    #[serde(default, alias = "drives", alias = "parts")]
    items: Vec<Item>,
}
pub(super) fn load(directory: Option<&Path>) -> BTreeMap<String, Item> {
    let mut items = BTreeMap::new();
    for (file, bundled) in [
        (
            "supplies.json",
            include_str!("../../../../assets/equipment/supplies.json"),
        ),
        (
            "drives.json",
            include_str!("../../../../assets/equipment/drives.json"),
        ),
        (
            "server_parts.json",
            include_str!("../../../../assets/equipment/server_parts.json"),
        ),
    ] {
        let fallback: ItemFile = serde_json::from_str(bundled).expect("bundled item translations");
        items.extend(
            fallback
                .items
                .into_iter()
                .map(|item| (item.id.clone(), item)),
        );
        if let Some(directory) = directory {
            let path = directory.join(file);
            match std::fs::read(&path) {
                Ok(bytes) => match serde_json::from_slice::<ItemFile>(&bytes) {
                    Ok(config) => {
                        items.extend(config.items.into_iter().map(|item| (item.id.clone(), item)))
                    }
                    Err(error) => {
                        bevy::log::warn!("Invalid item translations {}: {error}", path.display())
                    }
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => bevy::log::warn!(
                    "Could not load item translations {}: {error}",
                    path.display()
                ),
            }
        }
    }
    for (file, bundled) in [
        (
            "router_config.json",
            include_str!("../../../../assets/equipment/router_config.json"),
        ),
        (
            "switch_config.json",
            include_str!("../../../../assets/equipment/switch_config.json"),
        ),
        (
            "server_config.json",
            include_str!("../../../../assets/equipment/server_config.json"),
        ),
        (
            "ups_config.json",
            include_str!("../../../../assets/equipment/ups_config.json"),
        ),
        (
            "pdu_config.json",
            include_str!("../../../../assets/equipment/pdu_config.json"),
        ),
        (
            "patch_panel_config.json",
            include_str!("../../../../assets/equipment/patch_panel_config.json"),
        ),
        (
            "cable_manager_config.json",
            include_str!("../../../../assets/equipment/cable_manager_config.json"),
        ),
    ] {
        let fallback: Item = serde_json::from_str(bundled).expect("bundled equipment translations");
        items.insert(fallback.id.clone(), fallback);
        if let Some(directory) = directory {
            let path = directory.join(file);
            match std::fs::read(&path) {
                Ok(bytes) => match serde_json::from_slice::<Item>(&bytes) {
                    Ok(item) => {
                        items.insert(item.id.clone(), item);
                    }
                    Err(error) => bevy::log::warn!(
                        "Invalid equipment translations {}: {error}",
                        path.display()
                    ),
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => bevy::log::warn!(
                    "Could not load equipment translations {}: {error}",
                    path.display()
                ),
            }
        }
    }
    items
}
impl Item {
    pub(super) fn name(&self, language: &str) -> &str {
        self.display_name.get(language)
    }
    pub(super) fn description(&self, language: &str) -> &str {
        self.desc.get(language)
    }
}
