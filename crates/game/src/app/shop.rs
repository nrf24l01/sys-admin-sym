use cloud_provider_sim::{DeviceId, PortId, PurchaseReceipt, SimError};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum ShopCategory {
    #[default]
    Network,
    Compute,
    Connectivity,
    Rack,
    Power,
    Services,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShopSection {
    Routers,
    Switches,
    CopperSupplies,
    Transceivers,
    FiberCables,
    DirectAttach,
    PatchPanels,
    CableManagers,
    PublicIp,
    Servers,
    Cpu,
    Ram,
    PciCards,
    Storage,
    Ups,
    Pdu,
}

impl ShopSection {
    pub fn category(self) -> ShopCategory {
        match self {
            Self::Routers | Self::Switches => ShopCategory::Network,
            Self::Servers | Self::Cpu | Self::Ram | Self::PciCards | Self::Storage => {
                ShopCategory::Compute
            }
            Self::CopperSupplies | Self::Transceivers | Self::FiberCables | Self::DirectAttach => {
                ShopCategory::Connectivity
            }
            Self::PatchPanels | Self::CableManagers => ShopCategory::Rack,
            Self::Ups | Self::Pdu => ShopCategory::Power,
            Self::PublicIp => ShopCategory::Services,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShopSort {
    #[default]
    Category,
    Name,
    PriceAscending,
    PriceDescending,
    CapacityDescending,
    SpeedDescending,
    LengthAscending,
    CpuSocket,
    MemoryType,
    RamSlotsDescending,
    CpuSocketsDescending,
    DriveBaysDescending,
    PcieSlotsDescending,
}

impl ShopSort {
    pub fn applies_to(self, section: Option<ShopSection>) -> bool {
        use ShopSection::*;
        match self {
            Self::CapacityDescending => matches!(section, Some(Ram | Storage)),
            Self::SpeedDescending => {
                matches!(
                    section,
                    Some(Routers | Switches | PciCards | Transceivers | DirectAttach)
                )
            }
            Self::LengthAscending => matches!(section, Some(FiberCables | DirectAttach)),
            Self::CpuSocket => matches!(section, Some(Servers | Cpu)),
            Self::MemoryType => matches!(section, Some(Servers | Ram)),
            Self::RamSlotsDescending
            | Self::CpuSocketsDescending
            | Self::DriveBaysDescending
            | Self::PcieSlotsDescending => section == Some(Servers),
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopTarget {
    Server(DeviceId),
    Port(PortId),
}

#[derive(Debug, Clone)]
pub struct PendingPurchase {
    pub request_id: u64,
    pub offer_id: String,
    pub quantity: u32,
}

#[derive(Debug, Clone)]
pub struct ShopFeedback {
    pub offer_id: String,
    pub result: Result<PurchaseReceipt, SimError>,
}

#[derive(Default)]
pub struct ShopState {
    pub open: bool,
    pub category: ShopCategory,
    pub section: Option<ShopSection>,
    pub all_categories: bool,
    pub search: String,
    pub affordable_only: bool,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
    pub rack_units: Option<u8>,
    pub ports: Option<u16>,
    pub outlets: Option<u16>,
    pub facets: BTreeMap<String, BTreeSet<String>>,
    pub saved_facets:
        BTreeMap<(ShopCategory, Option<ShopSection>), BTreeMap<String, BTreeSet<String>>>,
    pub sort: ShopSort,
    pub list_view: bool,
    pub selected_offer: Option<String>,
    pub details_open: bool,
    pub compared: Vec<String>,
    pub comparison_open: bool,
    pub variants: BTreeMap<String, String>,
    pub quantities: BTreeMap<String, u32>,
    pub target: Option<ShopTarget>,
    pub compatible_only: bool,
    pub pending: Option<PendingPurchase>,
    pub feedback: Option<ShopFeedback>,
    pub next_request_id: u64,
}

impl ShopState {
    pub fn select(&mut self, category: ShopCategory, section: Option<ShopSection>) {
        let category = section.map_or(category, ShopSection::category);
        if self.category != category || self.section != section || self.all_categories {
            if !self.all_categories {
                self.saved_facets.insert(
                    (self.category, self.section),
                    std::mem::take(&mut self.facets),
                );
            }
            self.facets = self
                .saved_facets
                .get(&(category, section))
                .cloned()
                .unwrap_or_default();
            self.rack_units = None;
            self.ports = None;
            self.outlets = None;
            self.compatible_only = false;
        }
        self.all_categories = false;
        self.category = category;
        self.section = section;
        if !self.sort.applies_to(section) {
            self.sort = ShopSort::Category;
        }
    }

    pub fn select_all(&mut self) {
        if !self.all_categories {
            self.saved_facets.insert(
                (self.category, self.section),
                std::mem::take(&mut self.facets),
            );
        }
        self.all_categories = true;
        self.facets.clear();
        self.rack_units = None;
        self.ports = None;
        self.outlets = None;
        self.compatible_only = false;
        if !self.sort.applies_to(None) {
            self.sort = ShopSort::Category;
        }
    }

    pub fn clear_filters(&mut self) {
        self.search.clear();
        self.affordable_only = false;
        self.min_price = None;
        self.max_price = None;
        self.rack_units = None;
        self.ports = None;
        self.outlets = None;
        self.facets.clear();
        self.saved_facets.remove(&(self.category, self.section));
        self.compatible_only = false;
    }

    pub fn finish_purchase(&mut self, request_id: u64, result: Result<PurchaseReceipt, SimError>) {
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.request_id == request_id)
        {
            let purchase = self.pending.take().unwrap();
            self.feedback = Some(ShopFeedback {
                offer_id: purchase.offer_id,
                result,
            });
        }
    }
}
