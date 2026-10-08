//! One presentation catalog adapts the authoritative domain models and prices.
use crate::app::{ShopSection, ShopTarget};
use crate::localization::{item_description, item_name, tr};
use cloud_provider_sim::*;
use std::sync::OnceLock;
mod components;
mod hardware;
mod optics;

#[derive(Debug, Clone)]
pub(super) struct Attribute {
    pub key: &'static str,
    pub value: String,
    pub number: Option<u64>,
}
impl Attribute {
    fn text(key: &'static str, value: impl Into<String>) -> Self {
        Self {
            key,
            value: value.into(),
            number: None,
        }
    }
    fn number(key: &'static str, value: impl Into<u64>) -> Self {
        let value = value.into();
        Self {
            key,
            value: value.to_string(),
            number: Some(value),
        }
    }
    pub fn label(&self) -> String {
        tr(&format!("shop.spec.{}", self.key))
    }
    pub fn display(&self) -> String {
        display_value(self.key, &self.value)
    }
}

pub(super) fn display_value(key: &str, value: &str) -> String {
    let suffix = match key {
        "rack" => " U",
        "capacity" => " GB",
        "speed" => " Mb/s",
        "length" | "reach" => " cm",
        "watts" | "psu-watts" => " W",
        "va" => " VA",
        "battery" => " Wh",
        "frequency" => " MHz",
        "read" | "write" => " MB/s",
        "read-iops" | "write-iops" => " IOPS",
        "power" => " mW",
        "tx" | "rx" => " nm",
        _ => "",
    };
    if matches!(key, "length" | "reach")
        && let Ok(cm) = value.parse::<u64>()
    {
        return format!("{} m", cm as f64 / 100.0);
    }
    if key == "speed"
        && let Ok(speed) = value.parse::<u64>()
        && speed >= 1000
    {
        return format!("{}G", speed as f64 / 1000.0);
    }
    if matches!(key, "type" | "strands" | "dom" | "configuration") {
        return tr(&format!("shop.value.{value}"));
    }
    format!("{value}{suffix}")
}

#[derive(Debug, Clone)]
pub(super) struct Offer {
    pub id: String,
    pub display_id: String,
    pub family: String,
    pub section: ShopSection,
    pub item: PurchaseItem,
    pub attributes: Vec<Attribute>,
    pub guidance: Vec<&'static str>,
}
impl Offer {
    fn new(id: impl Into<String>, section: ShopSection, item: PurchaseItem) -> Self {
        let id = id.into();
        Self {
            display_id: id.clone(),
            family: id.clone(),
            id,
            section,
            item,
            attributes: Vec::new(),
            guidance: Vec::new(),
        }
    }
    pub fn name(&self) -> String {
        if matches!(self.item, PurchaseItem::PublicIpv4Pool) {
            return tr("ui.public-ipv4-29-address-pool");
        }
        if matches!(self.item, PurchaseItem::ServerFullPack) {
            return tr("shop.server-full-pack");
        }
        item_name(&self.display_id, &self.display_id)
    }
    pub fn description(&self) -> String {
        if matches!(self.item, PurchaseItem::PublicIpv4Pool) {
            return tr("ui.open-ip-ranges-to-select-its-uplink");
        }
        if matches!(self.item, PurchaseItem::ServerFullPack) {
            return tr("ui.full-pack-cpu-16-gb-ram-4");
        }
        item_description(&self.display_id)
    }
    pub fn price(&self) -> i64 {
        self.item
            .unit_price()
            .expect("catalog offer has a valid domain price")
    }
    pub fn can_compare_with(&self, other: &Self) -> bool {
        self.section == other.section && self.comparison_kind() == other.comparison_kind()
    }
    fn comparison_kind(&self) -> &'static str {
        match (&self.section, &self.item) {
            (ShopSection::Transceivers, PurchaseItem::Transceiver(_)) => "module",
            (ShopSection::FiberCables | ShopSection::DirectAttach, PurchaseItem::Assembly(_)) => {
                "assembly"
            }
            (ShopSection::PatchPanels, PurchaseItem::Device(DeviceTemplate::PatchPanel))
            | (ShopSection::PatchPanels, PurchaseItem::OpticalHardware(_)) => "panel",
            (ShopSection::CableManagers, PurchaseItem::Device(DeviceTemplate::CableManager)) => {
                "manager"
            }
            (ShopSection::CopperSupplies, PurchaseItem::Supply(CableSupply::CableBox305m)) => {
                "bulk"
            }
            (ShopSection::CopperSupplies, PurchaseItem::Supply(CableSupply::Rj45Pack20)) => "plugs",
            _ => "equipment",
        }
    }
    pub fn numeric(&self, key: &str) -> u64 {
        self.attributes
            .iter()
            .filter(|a| a.key == key)
            .filter_map(|a| a.number)
            .max()
            .unwrap_or(0)
    }
    pub fn values(&self, key: &str) -> impl Iterator<Item = &str> {
        self.attributes
            .iter()
            .filter(move |a| a.key == key)
            .map(|a| a.value.as_str())
    }
    pub fn search_text(&self) -> String {
        let mut text = format!("{} {} {}", self.id, self.name(), self.description());
        for attribute in &self.attributes {
            text.push_str(&format!(
                " {} {} {}",
                attribute.label(),
                attribute.value,
                attribute.display()
            ));
        }
        text.to_lowercase()
    }
    pub fn compatibility(
        &self,
        sim: &NetworkSim,
        target: ShopTarget,
    ) -> Option<Result<(), SimError>> {
        match (&self.item, target) {
            (PurchaseItem::ServerPart(id), ShopTarget::Server(device)) => Some(
                sim.server_part_installation_slot(device, id, None)
                    .map(|_| ()),
            ),
            (PurchaseItem::Drive(id), ShopTarget::Server(device)) => {
                Some(sim.drive_installation_bay(device, id, None).map(|_| ()))
            }
            (PurchaseItem::Transceiver(id), ShopTarget::Port(port)) => Some(
                sim.transceiver_model_supported_at_port(id, port)
                    .map_err(Into::into),
            ),
            (PurchaseItem::Assembly(id), ShopTarget::Port(port)) => Some(
                if sim.assembly_model_supported_at_port(optics_catalog().cable(id)?, port) {
                    Ok(())
                } else {
                    Err(OpticsError::ConnectorMismatch.into())
                },
            ),
            _ => None,
        }
    }
    pub fn ownership(&self, sim: &NetworkSim) -> (u64, u64) {
        match &self.item {
            PurchaseItem::Transceiver(id) => {
                let all: Vec<_> = sim
                    .optics
                    .transceivers
                    .values()
                    .filter(|m| &m.model_id == id)
                    .collect();
                (
                    all.iter().filter(|m| m.port.is_none()).count() as u64,
                    all.iter().filter(|m| m.port.is_some()).count() as u64,
                )
            }
            PurchaseItem::Assembly(id) => {
                let all: Vec<_> = sim
                    .optics
                    .assemblies
                    .values()
                    .filter(|m| &m.model_id == id)
                    .collect();
                (
                    all.iter().filter(|m| m.link.is_none()).count() as u64,
                    all.iter().filter(|m| m.link.is_some()).count() as u64,
                )
            }
            PurchaseItem::ServerPart(id) => {
                let installed = sim
                    .devices()
                    .filter_map(|d| match &d.kind {
                        DeviceKind::Server(s) => s.hardware.as_ref(),
                        _ => None,
                    })
                    .map(|h| {
                        h.cpus
                            .iter()
                            .chain(h.ram.iter())
                            .chain(h.pcie.iter().flatten())
                            .filter(|model| *model == id)
                            .count() as u64
                    })
                    .sum();
                (
                    u64::from(sim.server_parts.get(id).copied().unwrap_or(0)),
                    installed,
                )
            }
            PurchaseItem::Drive(id) => {
                let installed = sim
                    .devices()
                    .filter_map(|d| match &d.kind {
                        DeviceKind::Server(s) => s.hardware.as_ref(),
                        _ => None,
                    })
                    .map(|h| {
                        h.drives
                            .iter()
                            .flatten()
                            .filter(|model| *model == id)
                            .count() as u64
                    })
                    .sum();
                (
                    u64::from(sim.drive_inventory.get(id).copied().unwrap_or(0)),
                    installed,
                )
            }
            PurchaseItem::PublicIpv4Pool => (sim.public_ipv4_blocks().len() as u64, 0),
            PurchaseItem::Supply(_) => (0, 0),
            _ => {
                let matching = |d: &&Device| match &self.item {
                    PurchaseItem::Device(template) => {
                        d.template() == *template && !sim.optics.device_models.contains_key(&d.id)
                    }
                    PurchaseItem::OpticalHardware(id) => {
                        sim.optics.device_models.get(&d.id) == Some(id)
                    }
                    PurchaseItem::ServerChassis => matches!(d.kind, DeviceKind::Server(_)),
                    PurchaseItem::ServerFullPack => {
                        matches!(&d.kind, DeviceKind::Server(s) if s.hardware.as_ref().is_some_and(|h| h.cpus.contains(&ServerFullPack::CPU.to_string()) && h.ram.contains(&ServerFullPack::RAM.to_string()) && h.pcie.iter().flatten().any(|id| id == ServerFullPack::NIC) && h.drives.iter().flatten().any(|id| id == ServerFullPack::DRIVE)))
                    }
                    _ => false,
                };
                let devices: Vec<_> = sim.devices().filter(matching).collect();
                (
                    devices.iter().filter(|d| d.rack.is_none()).count() as u64,
                    devices.iter().filter(|d| d.rack.is_some()).count() as u64,
                )
            }
        }
    }
}

pub(super) fn catalog() -> &'static [Offer] {
    static CATALOG: OnceLock<Vec<Offer>> = OnceLock::new();
    CATALOG.get_or_init(build_catalog)
}

fn cage_name(cage: CageKind) -> &'static str {
    match cage {
        CageKind::Sfp => "SFP",
        CageKind::SfpPlus => "SFP+",
        CageKind::Sfp28 => "SFP28",
    }
}
fn fiber_name(fiber: FiberClass) -> &'static str {
    match fiber {
        FiberClass::Om3 => "OM3",
        FiberClass::Om4 => "OM4",
        FiberClass::Os2 => "OS2",
    }
}

fn build_catalog() -> Vec<Offer> {
    hardware::offers()
        .into_iter()
        .chain(components::offers())
        .chain(optics::offers())
        .collect()
}
