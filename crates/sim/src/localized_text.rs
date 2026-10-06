//! Language-tagged asset metadata. The simulation never selects a UI language.
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(transparent)]
pub struct LocalizedText(BTreeMap<String, String>);

impl LocalizedText {
    pub fn get(&self, language: &str) -> &str {
        self.0
            .get(language)
            .or_else(|| self.0.get("en"))
            .map_or("", String::as_str)
    }
}
