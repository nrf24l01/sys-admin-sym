//! JSON-backed presentation text. Simulation and terminal data remain untranslated.
mod items;
mod labels;
mod message;
pub use labels::{device_name, rack_name, room_name};
mod template;
pub use message::UiMessage;
use serde::Deserialize;
use std::{cell::RefCell, collections::BTreeMap, path::Path, sync::Arc};
use template::{fields, render};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    language: String,
    name: String,
    messages: BTreeMap<String, String>,
}

struct Translator {
    language: String,
    items: BTreeMap<String, items::Item>,
    messages: BTreeMap<String, String>,
}
impl Translator {
    fn new(english: &Catalog, catalog: &Catalog) -> Result<Self, String> {
        let mut messages = english.messages.clone();
        for (key, translation) in &catalog.messages {
            if key.is_empty()
                || !key.chars().all(|c| {
                    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_')
                })
            {
                return Err(format!("Invalid message ID: {key}"));
            }
            let translated = fields(translation)?;
            if let Some(source) = english.messages.get(key)
                && translated != fields(source)?
            {
                return Err(format!("Translation changes placeholders for {key}"));
            }
            messages.insert(key.clone(), translation.clone());
        }
        Ok(Self {
            language: catalog.language.clone(),
            items: items::load(None),
            messages,
        })
    }
    fn translate(&self, key: &str, values: &[String]) -> String {
        self.messages
            .get(key)
            .map_or_else(|| format!("[{key}]"), |template| render(template, values))
    }
}

pub struct Localization {
    catalogs: BTreeMap<String, Catalog>,
    current: Arc<Translator>,
    items: BTreeMap<String, items::Item>,
}
impl Default for Localization {
    fn default() -> Self {
        let en: Catalog = serde_json::from_str(include_str!("../../../../assets/locales/en.json"))
            .expect("bundled English catalog");
        let ru: Catalog = serde_json::from_str(include_str!("../../../../assets/locales/ru.json"))
            .expect("bundled Russian catalog");
        let current = Arc::new(Translator::new(&en, &en).expect("valid English placeholders"));
        Self {
            catalogs: [(en.language.clone(), en), (ru.language.clone(), ru)].into(),
            items: items::load(None),
            current,
        }
    }
}
impl Localization {
    pub fn load(directory: &Path) -> Self {
        let mut result = Self::default();
        if let Ok(files) = std::fs::read_dir(directory) {
            let mut files: Vec<_> = files.flatten().map(|entry| entry.path()).collect();
            files.sort();
            for path in files
                .into_iter()
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
            {
                match std::fs::read(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|data| {
                        serde_json::from_slice::<Catalog>(&data).map_err(|e| e.to_string())
                    }) {
                    Ok(mut catalog) if !catalog.language.is_empty() && !catalog.name.is_empty() => {
                        let english = &result.catalogs["en"];
                        if catalog.language == "en" {
                            let mut merged = english.messages.clone();
                            merged.extend(catalog.messages);
                            catalog.messages = merged;
                        }
                        match Translator::new(english, &catalog) {
                            Ok(_) => {
                                result.catalogs.insert(catalog.language.clone(), catalog);
                            }
                            Err(error) => {
                                bevy::log::warn!("Invalid locale {}: {error}", path.display())
                            }
                        }
                    }
                    Ok(_) => bevy::log::warn!("Missing locale metadata: {}", path.display()),
                    Err(error) => {
                        bevy::log::warn!("Could not load locale {}: {error}", path.display())
                    }
                }
            }
        }
        result.items = items::load(directory.parent().map(|p| p.join("equipment")).as_deref());
        result.select("en").expect("English fallback exists");
        result
    }
    pub fn validate_language(&self, language: &str) -> Result<(), String> {
        let catalog = self
            .catalogs
            .get(language)
            .ok_or_else(|| format!("Unknown language: {language}"))?;
        Translator::new(&self.catalogs["en"], catalog).map(|_| ())
    }
    pub fn select(&mut self, language: &str) -> Result<(), String> {
        let catalog = self
            .catalogs
            .get(language)
            .ok_or_else(|| format!("Unknown language: {language}"))?;
        let mut translator = Translator::new(&self.catalogs["en"], catalog)?;
        translator.items = self.items.clone();
        self.current = Arc::new(translator);
        Ok(())
    }
    pub fn languages(&self) -> impl Iterator<Item = (&str, &str)> {
        self.catalogs
            .values()
            .map(|c| (c.language.as_str(), c.name.as_str()))
    }
    pub fn enter(&self) -> LanguageScope {
        let previous = ACTIVE.with(|active| active.replace(self.current.clone()));
        LanguageScope(previous)
    }
    pub fn text(&self, key: &str) -> String {
        self.current.translate(key, &[])
    }
}

thread_local! {
    // A UI frame selects its immutable catalog snapshot. Separate egui/test threads
    // can use different languages without a process-wide mutable language setting.
    static ACTIVE: RefCell<Arc<Translator>> = RefCell::new(Localization::default().current);
}
pub struct LanguageScope(Arc<Translator>);
impl Drop for LanguageScope {
    fn drop(&mut self) {
        ACTIVE.with(|active| {
            active.replace(self.0.clone());
        });
    }
}
/// Look up a stable message ID. Values and player-entered text are never translated.
pub fn tr(key: &str) -> String {
    tr_args(key, &[])
}
pub fn tr_args(key: &str, values: &[String]) -> String {
    ACTIVE.with(|active| active.borrow().translate(key, values))
}
/// Item-owned translations use the same selected language and English fallback.
pub fn item_name(id: &str, fallback: &str) -> String {
    ACTIVE.with(|active| {
        let active = active.borrow();
        active
            .items
            .get(id)
            .map_or(fallback, |item| {
                let name = item.name(&active.language);
                if name.is_empty() { fallback } else { name }
            })
            .to_owned()
    })
}
pub fn item_description(id: &str) -> String {
    ACTIVE.with(|active| {
        let active = active.borrow();
        active
            .items
            .get(id)
            .map_or("", |item| item.description(&active.language))
            .to_owned()
    })
}

#[cfg(test)]
mod tests;

pub fn cable_color(color: cloud_provider_sim::CableColor) -> String {
    use cloud_provider_sim::CableColor::*;
    tr(match color {
        White => "cable.color.white",
        Gray => "cable.color.gray",
        Blue => "cable.color.blue",
        Orange => "cable.color.orange",
        Red => "cable.color.red",
        Aqua => "cable.color.aqua",
        Yellow => "cable.color.yellow",
    })
}
pub fn rack_side(side: cloud_provider_sim::RackSide) -> String {
    tr(match side {
        cloud_provider_sim::RackSide::Front => "rack.side.front",
        cloud_provider_sim::RackSide::Rear => "rack.side.rear",
    })
}
pub fn cable_visibility(visibility: crate::app::CableVisibility) -> String {
    use crate::app::CableVisibility::*;
    tr(match visibility {
        All => "cable.visibility.all",
        Selected => "cable.visibility.selected",
        Hidden => "cable.visibility.hidden",
    })
}

pub fn link_fault(fault: cloud_provider_sim::LinkFault) -> String {
    use cloud_provider_sim::LinkFault::*;
    tr(match fault {
        NoCable => "optics.fault.no-cable",
        EmptyCage => "optics.fault.empty-cage",
        Disabled => "optics.fault.disabled",
        Unpowered => "optics.fault.unpowered",
        NotInstalled => "optics.fault.not-installed",
        UnsupportedModule => "optics.fault.unsupported-module",
        ConnectorMismatch => "optics.fault.connector-mismatch",
        ModeMismatch => "optics.fault.mode-mismatch",
        FiberMismatch => "optics.fault.fiber-mismatch",
        WavelengthMismatch => "optics.fault.wavelength-mismatch",
        PolarityMismatch => "optics.fault.polarity-mismatch",
        TooLong => "optics.fault.too-long",
        LowLight => "optics.fault.low-light",
        ReceiverOverload => "optics.fault.receiver-overload",
    })
}
pub fn optics_error_id(error: cloud_provider_sim::OpticsError) -> &'static str {
    use cloud_provider_sim::OpticsError::*;
    match error {
        MissingTransceiver => "optics.error.missing-transceiver",
        UnknownModel => "optics.error.unknown-model",
        NotOwned => "optics.error.not-owned",
        NotCage => "optics.error.not-cage",
        CageOccupied => "optics.error.cage-occupied",
        IncompatibleHost => "optics.error.incompatible-host",
        ConnectorMismatch => "optics.error.connector-mismatch",
        AttachedCable => "optics.error.attached-cable",
        NotFiber => "optics.error.not-fiber",
    }
}
