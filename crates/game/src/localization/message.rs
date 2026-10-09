//! Deferred UI messages retain IDs and arguments when the language changes.
#[derive(Debug, Clone)]
pub enum UiMessage {
    Catalog {
        id: &'static str,
        arguments: Vec<String>,
    },
    Diagnostic(String),
}
impl UiMessage {
    pub fn new(id: &'static str, arguments: Vec<String>) -> Self {
        Self::Catalog { id, arguments }
    }
    pub fn render(&self) -> String {
        match self {
            Self::Catalog { id, arguments } => super::tr_args(id, arguments),
            Self::Diagnostic(text) => text.clone(),
        }
    }
}
impl From<&'static str> for UiMessage {
    fn from(id: &'static str) -> Self {
        Self::new(id, Vec::new())
    }
}
impl From<String> for UiMessage {
    fn from(text: String) -> Self {
        Self::Diagnostic(text)
    }
}

impl From<cloud_provider_sim::SimError> for UiMessage {
    fn from(error: cloud_provider_sim::SimError) -> Self {
        use cloud_provider_sim::SimError::*;
        match error {
            InvalidPurchaseQuantity => Self::new("shop.invalid-quantity", vec![]),
            Optics(error) => Self::new(super::optics_error_id(error), vec![]),
            InsufficientCable {
                needed_cm,
                available_cm,
            } => Self::new(
                "error.insufficient-cable",
                vec![needed_cm.to_string(), available_cm.to_string()],
            ),
            UnsupportedConnector { port, connector } => Self::new(
                "error.unsupported-connector",
                vec![port.to_string(), format!("{connector:?}")],
            ),
            RackUnitOccupied { rack, unit } => Self::new(
                "error.rack-unit-occupied",
                vec![unit.to_string(), rack.to_string()],
            ),
            InsufficientFunds { needed, available } => Self::new(
                "error.insufficient-funds",
                vec![needed.to_string(), available.to_string()],
            ),

            Provider(value) => Self::new("error.provider", vec![(value).to_string()]),
            CableDevicesNotInstalled => Self::new("error.cable-devices-not-installed", vec![]),
            CableTooShort { minimum_cm } => {
                Self::new("error.cable-too-short", vec![(minimum_cm).to_string()])
            }
            CableTooLong => Self::new("error.cable-too-long", vec![]),
            InvalidCableSettings => Self::new("error.invalid-cable-settings", vec![]),
            InsufficientConnectors { available } => Self::new(
                "error.insufficient-connectors",
                vec![(available).to_string()],
            ),
            CableInventoryFull => Self::new("error.cable-inventory-full", vec![]),
            DeviceNotFound(value) => Self::new("error.device-not-found", vec![(value).to_string()]),
            PortNotFound(value) => Self::new("error.port-not-found", vec![(value).to_string()]),
            RackNotFound(value) => Self::new("error.rack-not-found", vec![(value).to_string()]),
            RoomAnchorNotFound(value) => {
                Self::new("error.room-anchor-not-found", vec![(value).to_string()])
            }
            PortAlreadyConnected(value) => {
                Self::new("error.port-already-connected", vec![(value).to_string()])
            }
            UnsupportedConnection => Self::new("error.unsupported-connection", vec![]),
            SamePort => Self::new("error.same-port", vec![]),
            RackPlacementOutOfBounds => Self::new("error.rack-placement-out-of-bounds", vec![]),
            VlanNotFound(value) => Self::new("error.vlan-not-found", vec![(value).to_string()]),
            InvalidIpv4 => Self::new("error.invalid-ipv4", vec![]),
            InvalidIpv4Prefix => Self::new("error.invalid-ipv4-prefix", vec![]),
            InvalidIpv4Address => Self::new("error.invalid-ipv4-address", vec![]),
            InvalidIpv4Vlan => Self::new("error.invalid-ipv4-vlan", vec![]),
            InvalidIpv4Gateway => Self::new("error.invalid-ipv4-gateway", vec![]),
            InvalidPublicUplink => Self::new("error.invalid-public-uplink", vec![]),
            PublicIpv4BlockNotOwned => Self::new("error.public-ipv4-block-not-owned", vec![]),
            PublicIpv4Exhausted => Self::new("error.public-ipv4-exhausted", vec![]),
            LanIpv4Exhausted => Self::new("error.lan-ipv4-exhausted", vec![]),
            InvalidRouteNextHop => Self::new("error.invalid-route-next-hop", vec![]),
            WrongPortType => Self::new("error.wrong-port-type", vec![]),
            UnknownServerPart(value) => {
                Self::new("error.unknown-server-part", vec![(value).to_string()])
            }
            ServerPartNotOwned(value) => {
                Self::new("error.server-part-not-owned", vec![(value).to_string()])
            }
            ServerHardware(value) => Self::new("error.server-hardware", vec![(value).to_string()]),
            UnknownDrive(value) => Self::new("error.unknown-drive", vec![(value).to_string()]),
            DriveNotOwned(value) => Self::new("error.drive-not-owned", vec![(value).to_string()]),
            LinkNotFound => Self::new("error.link-not-found", vec![]),
            DeviceInstalled => Self::new("error.device-installed", vec![]),
            InvalidHostname => Self::new("error.invalid-hostname", vec![]),
            Power(value) => Self::new("error.power", vec![(value).to_string()]),
        }
    }
}
