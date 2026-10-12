//! Model identity and physical metadata shared by simulation and presentation.
use super::*;

pub const DEFAULT_SERVER_MODEL: &str = "dell_r360";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ServerModelId(String);

impl Default for ServerModelId {
    fn default() -> Self {
        Self(DEFAULT_SERVER_MODEL.into())
    }
}
impl ServerModelId {
    pub fn parse(id: &str) -> Result<Self, String> {
        server_catalog()
            .model(id)
            .ok_or_else(|| format!("unknown server model: {id}"))?;
        Ok(Self(id.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl<'de> Deserialize<'de> for ServerModelId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerConfiguration {
    Chassis,
    FullPack,
}

#[derive(Debug, Deserialize)]
pub(super) struct ServerModels {
    pub models: Vec<ServerModel>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerModel {
    pub id: String,
    pub manufacturer: String,
    pub display_name: crate::LocalizedText,
    pub desc: crate::LocalizedText,
    pub price: i64,
    pub rack_units: u8,
    pub chassis: ServerChassis,
    pub power: crate::ServerPowerProfile,
    pub onboard_ports: Vec<ServerOnboardPort>,
    pub power_inlets: Vec<ServerSocketRect>,
    pub artwork: ServerArtwork,
    pub full_pack: ServerPackRecipe,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerOnboardPort {
    pub name: String,
    pub speed_mbps: u32,
    pub socket: ServerSocketRect,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSocketRect {
    pub rect: [f32; 4],
}
impl ServerSocketRect {
    pub fn center(&self) -> (f32, f32) {
        let [l, t, r, b] = self.rect;
        ((l + r) * 0.5, (t + b) * 0.5)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerArtwork {
    pub front: ServerFaceArtwork,
    pub rear: ServerFaceArtwork,
}
#[derive(Debug, Clone, Deserialize)]
pub struct ServerFaceArtwork {
    pub path: String,
    pub uv: [f32; 4],
    pub source_size: [u32; 2],
}
#[derive(Debug, Clone, Deserialize)]
pub struct ServerPackRecipe {
    pub parts: Vec<ServerPackPart>,
    pub drives: Vec<ServerPackDrive>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct ServerPackPart {
    pub part_id: String,
    #[serde(default)]
    pub slot: Option<usize>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct ServerPackDrive {
    pub drive_id: String,
    pub bay: usize,
}

impl ServerCatalog {
    pub fn model(&self, id: &str) -> Option<&ServerModel> {
        self.models.iter().find(|m| m.id == id)
    }
    pub fn default_model(&self) -> &ServerModel {
        self.model(DEFAULT_SERVER_MODEL)
            .expect("server catalog must include dell_r360")
    }
    pub fn part(&self, id: &str) -> Option<&ServerPart> {
        self.parts.iter().find(|p| p.id == id)
    }
}
impl ServerChassis {
    pub fn supports_memory_type(&self, kind: &str) -> bool {
        self.memory_type == kind || self.additional_memory_types.iter().any(|t| t == kind)
    }
}
impl ServerModel {
    /// An owned full pack must still contain every recipe ingredient. Extra
    /// parts are allowed, and repeated ingredients consume separate instances.
    pub fn contains_full_pack(&self, hardware: &ServerHardware) -> bool {
        let mut parts: Vec<_> = hardware
            .cpus
            .iter()
            .chain(&hardware.ram)
            .chain(hardware.pcie.iter().flatten())
            .chain(&hardware.power_supplies)
            .collect();
        let mut drives: Vec<_> = hardware.drives.iter().flatten().collect();
        self.full_pack.parts.iter().all(|p| {
            parts
                .iter()
                .position(|id| id.as_str() == p.part_id)
                .map(|i| parts.swap_remove(i))
                .is_some()
        }) && self.full_pack.drives.iter().all(|d| {
            drives
                .iter()
                .position(|id| id.as_str() == d.drive_id)
                .map(|i| drives.swap_remove(i))
                .is_some()
        })
    }

    /// Recipes describe inventory, independent of authoring order. CPUs precede
    /// primary/secondary DIMMs, which precede PCIe cards.
    pub(crate) fn ordered_pack_parts<'a>(
        &'a self,
        catalog: &ServerCatalog,
    ) -> Vec<&'a ServerPackPart> {
        let mut parts: Vec<_> = self.full_pack.parts.iter().collect();
        parts.sort_by_key(|p| {
            let priority = match catalog.part(&p.part_id).map(|p| &p.kind) {
                Some(ServerPartKind::Cpu { .. }) => 0,
                Some(ServerPartKind::Ram { .. }) => 1,
                _ => 2,
            };
            let position = if priority == 1 {
                p.slot
                    .and_then(|i| self.chassis.memory.slots.get(i))
                    .map_or(0, |s| s.position)
            } else {
                0
            };
            (priority, position, p.slot.unwrap_or(0))
        });
        parts
    }
    pub fn full_pack_price(&self, catalog: &ServerCatalog) -> Result<i64, String> {
        let mut price = self.price;
        for part in &self.full_pack.parts {
            let p = catalog
                .part(&part.part_id)
                .ok_or_else(|| format!("unknown full-pack part {}", part.part_id))?;
            price = price
                .checked_add(p.price)
                .ok_or("full-pack price overflow")?;
        }
        for drive in &self.full_pack.drives {
            let p = crate::drive_catalog()
                .drives
                .iter()
                .find(|d| d.id == drive.drive_id)
                .ok_or_else(|| format!("unknown full-pack drive {}", drive.drive_id))?;
            price = price
                .checked_add(p.price)
                .ok_or("full-pack price overflow")?;
        }
        Ok(price)
    }
    pub fn port_position(&self, index: usize) -> Option<(f32, f32)> {
        self.onboard_ports.get(index).map(|p| p.socket.center())
    }
    pub fn power_inlet_position(&self, index: usize) -> Option<(f32, f32)> {
        self.power_inlets.get(index).map(ServerSocketRect::center)
    }
}
