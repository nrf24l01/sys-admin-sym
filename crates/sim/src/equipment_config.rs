//! Catalogs are loaded once by their callers; installed assets override defaults.
use serde::de::DeserializeOwned;
use std::{fs, io::ErrorKind, path::Path};

pub(crate) fn equipment_catalog<T: DeserializeOwned>(filename: &str, embedded: &str) -> T {
    let path = Path::new("assets/equipment").join(filename);
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == ErrorKind::NotFound => embedded.to_owned(),
        Err(error) => panic!("cannot read equipment config {}: {error}", path.display()),
    };
    serde_json::from_str(&source)
        .unwrap_or_else(|error| panic!("invalid equipment config {}: {error}", path.display()))
}
