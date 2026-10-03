#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShopCategory {
    #[default]
    Network,
    Compute,
    Power,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopSection {
    Routers,
    Switches,
    Cabling,
    PublicIp,
    DellServers,
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
            Self::Routers | Self::Switches | Self::Cabling | Self::PublicIp => {
                ShopCategory::Network
            }
            Self::DellServers | Self::Cpu | Self::Ram | Self::PciCards | Self::Storage => {
                ShopCategory::Compute
            }
            Self::Ups | Self::Pdu => ShopCategory::Power,
        }
    }
}

#[derive(Default)]
pub struct ShopState {
    pub open: bool,
    pub category: ShopCategory,
    pub section: Option<ShopSection>,
    pub search: String,
    pub affordable_only: bool,
    pub max_price: Option<i64>,
    pub rack_units: Option<u8>,
    pub ports: Option<u16>,
    pub outlets: Option<u16>,
}

impl ShopState {
    pub fn select(&mut self, category: ShopCategory, section: Option<ShopSection>) {
        if self.category != category {
            self.ports = None;
            self.outlets = None;
        }
        self.category = category;
        self.section = section;
    }

    pub fn clear_filters(&mut self) {
        self.search.clear();
        self.affordable_only = false;
        self.max_price = None;
        self.rack_units = None;
        self.ports = None;
        self.outlets = None;
    }
}
