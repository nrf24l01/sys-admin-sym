use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
pub struct DriveCatalog {
    pub drives: Vec<DriveModel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriveKind {
    Hdd,
    Ssd,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DriveModel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub display_name: crate::LocalizedText,
    #[serde(default)]
    pub desc: crate::LocalizedText,
    pub kind: DriveKind,
    pub interface: String,
    pub capacity_gb: u32,
    pub read_mb_s: u32,
    pub write_mb_s: u32,
    pub read_iops: u32,
    pub write_iops: u32,
    pub power: crate::PowerProfile,
    pub price: i64,
}

pub fn drive_catalog() -> &'static DriveCatalog {
    static CATALOG: OnceLock<DriveCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        crate::equipment_config::equipment_catalog(
            "drives.json",
            include_str!("../../../assets/equipment/drives.json"),
        )
    })
}

pub fn linux_drive_name(ordinal: usize) -> String {
    // Linux's sd naming continues as aa, ab, ... after z.
    let mut n = ordinal;
    let mut suffix = String::new();
    loop {
        suffix.insert(0, (b'a' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    format!("sd{suffix}")
}
