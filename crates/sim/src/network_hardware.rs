//! Validated model data shared by network simulation, shop and renderer.
use crate::*;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::HashSet, sync::OnceLock};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct NetworkModelId(Cow<'static, str>);
impl Default for NetworkModelId {
    fn default() -> Self {
        Self(Cow::Owned(network_device_catalog().default_router.clone()))
    }
}
impl NetworkModelId {
    pub fn parse(id: &str) -> Result<Self, String> {
        network_device_catalog()
            .model(id)
            .ok_or_else(|| format!("unknown network device model: {id}"))?;
        Ok(Self(Cow::Owned(id.into())))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn spec(&self) -> &'static NetworkDeviceModel {
        network_device_catalog()
            .model(self.as_str())
            .expect("validated network model identity")
    }
}
impl<'de> Deserialize<'de> for NetworkModelId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkDeviceCategory {
    Router,
    Switch,
}
impl NetworkDeviceCategory {
    pub fn template(self) -> DeviceTemplate {
        match self {
            Self::Router => DeviceTemplate::Router,
            Self::Switch => DeviceTemplate::Switch,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPortRole {
    Routed,
    Switch,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkPortLeds {
    pub status: [f32; 2],
    pub activity: [f32; 2],
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkPortSpec {
    pub name: String,
    pub ios_name: String,
    pub connector: PortConnector,
    pub role: NetworkPortRole,
    pub speed_mbps: u32,
    #[serde(default)]
    pub cage: Option<CageProfile>,
    #[serde(default)]
    pub flex: bool,
    #[serde(default)]
    pub trunk: bool,
    #[serde(default)]
    pub default_wan: bool,
    pub side: RackSide,
    pub socket: ServerSocketRect,
    #[serde(default)]
    pub optical_socket: Option<ServerSocketRect>,
    #[serde(default)]
    pub leds: Option<NetworkPortLeds>,
}
impl NetworkPortSpec {
    pub fn position(&self, connector: PortConnector) -> (f32, f32) {
        if connector == PortConnector::Sfp {
            self.optical_socket
                .as_ref()
                .unwrap_or(&self.socket)
                .center()
        } else {
            self.socket.center()
        }
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkPowerInlet {
    pub connector: PowerInletConnector,
    pub side: RackSide,
    pub socket: ServerSocketRect,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkModeBlock {
    pub trigger_port: usize,
    pub min_speed_mbps: u32,
    pub blocked_ports: Vec<usize>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkDeviceModel {
    pub id: String,
    pub shop_id: String,
    pub name: String,
    pub manufacturer: String,
    pub family: String,
    pub category: NetworkDeviceCategory,
    pub display_name: LocalizedText,
    pub desc: LocalizedText,
    pub price: i64,
    pub rack_units: u8,
    pub routing_enabled: bool,
    pub power: PowerProfile,
    pub adapter: Option<AdapterPowerProfile>,
    pub psu: PsuPowerProfile,
    pub ports: Vec<NetworkPortSpec>,
    pub power_inlets: Vec<NetworkPowerInlet>,
    pub artwork: ServerArtwork,
    #[serde(default)]
    pub switching_mbps: u32,
    #[serde(default)]
    pub forwarding_kpps: u32,
    #[serde(default)]
    pub weight_grams: u32,
    #[serde(default)]
    pub cpu_mhz: u32,
    #[serde(default)]
    pub dram_mb: u32,
    #[serde(default)]
    pub flash_mb: u32,
    #[serde(default)]
    pub mode_blocks: Vec<NetworkModeBlock>,
    pub source: String,
    pub notes: String,
}
impl NetworkDeviceModel {
    pub fn network_profile(&self) -> RouterNetworkProfile {
        RouterNetworkProfile {
            wan_ports: self
                .ports
                .iter()
                .enumerate()
                .filter_map(|(i, p)| (p.role == NetworkPortRole::Routed).then_some(i))
                .collect(),
            lan_ports: self
                .ports
                .iter()
                .enumerate()
                .filter_map(|(i, p)| (p.role == NetworkPortRole::Switch).then_some(i))
                .collect(),
            flex_ports: self
                .ports
                .iter()
                .enumerate()
                .filter_map(|(i, p)| p.flex.then_some(i))
                .collect(),
            routing_enabled: self.routing_enabled,
        }
    }
    pub fn power_inlet(&self) -> Option<PowerInlet> {
        self.power_inlets.first().map(|p| PowerInlet {
            connector: p.connector,
            side: p.side,
            position: p.socket.center(),
        })
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkDeviceCatalog {
    pub default_router: String,
    pub default_switch: String,
    pub models: Vec<NetworkDeviceModel>,
}
pub fn network_device_catalog() -> &'static NetworkDeviceCatalog {
    static CATALOG: OnceLock<NetworkDeviceCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let catalog: NetworkDeviceCatalog = crate::equipment_config::equipment_catalog(
            "network_devices.json",
            include_str!("../../../assets/equipment/network_devices.json"),
        );
        catalog
            .validate()
            .expect("invalid assets/equipment/network_devices.json");
        catalog
    })
}
impl NetworkDeviceCatalog {
    pub fn model(&self, id: &str) -> Option<&NetworkDeviceModel> {
        self.models.iter().find(|m| m.id == id)
    }
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = HashSet::new();
        let mut offers = HashSet::new();
        for model in &self.models {
            let validate = || -> Result<(), String> {
                if model.id.is_empty()
                    || !model
                        .id
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                    || !ids.insert(&model.id)
                    || model.shop_id.is_empty()
                    || !offers.insert(&model.shop_id)
                {
                    return Err("duplicate or empty model/shop ID".into());
                }
                if model.price <= 0
                    || !(1..=42).contains(&model.rack_units)
                    || model.ports.is_empty()
                    || model.power_inlets.is_empty()
                    || model.power_inlets.len() > 8
                    || model.psu.capacity_watts == 0
                    || model.power.peak_watts() > model.psu.capacity_watts
                {
                    return Err("invalid price, rack height, ports or power capacity".into());
                }
                if model.name.is_empty()
                    || model.display_name.get("en").is_empty()
                    || model.display_name.get("ru").is_empty()
                    || model.desc.get("en").is_empty()
                    || model.desc.get("ru").is_empty()
                {
                    return Err("missing model text".into());
                }
                let rect = |r: [f32; 4]| {
                    r.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                        && r[0] < r[2]
                        && r[1] < r[3]
                };
                let mut names = HashSet::new();
                let mut console = HashSet::new();
                for p in &model.ports {
                    if p.name.is_empty()
                        || !names.insert(&p.name)
                        || p.ios_name.is_empty()
                        || !console.insert(&p.ios_name)
                        || LinkSpeed::from_mbps(p.speed_mbps).is_none()
                        || !rect(p.socket.rect)
                        || p.optical_socket.as_ref().is_some_and(|s| !rect(s.rect))
                    {
                        return Err(format!("invalid port {} name, speed or geometry", p.name));
                    }
                    if p.connector == PortConnector::Lc
                        || (p.connector == PortConnector::Sfp && p.cage.is_none())
                        || (p.optical_socket.is_some()
                            && (p.connector != PortConnector::Rj45 || p.cage.is_none()))
                    {
                        return Err(format!("invalid connector/cage for {}", p.name));
                    }
                    if (p.role == NetworkPortRole::Routed || p.flex) && !model.routing_enabled {
                        return Err(format!("routing disabled for routed/flex port {}", p.name));
                    }
                    if let Some(c) = &p.cage
                        && (c.modes.is_empty()
                            || c.max_power_mw == 0
                            || c.modes
                                .iter()
                                .any(|m| m.lanes == 0 || m.speed.mbps() > p.speed_mbps)
                            || !c.modes.iter().any(|m| m.speed.mbps() == p.speed_mbps))
                    {
                        return Err(format!("invalid cage modes for {}", p.name));
                    }
                }
                let mut sockets = Vec::new();
                for port in &model.ports {
                    for socket in std::iter::once(&port.socket).chain(port.optical_socket.as_ref())
                    {
                        if sockets.contains(&(port.side, socket.rect)) {
                            return Err(format!("duplicate socket geometry for {}", port.name));
                        }
                        sockets.push((port.side, socket.rect));
                    }
                }
                for p in &model.ports {
                    if p.leds.as_ref().is_some_and(|l| {
                        l.status
                            .iter()
                            .chain(&l.activity)
                            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
                    }) {
                        return Err(format!("invalid LED geometry for {}", p.name));
                    }
                }
                for rule in &model.mode_blocks {
                    if rule.trigger_port >= model.ports.len()
                        || LinkSpeed::from_mbps(rule.min_speed_mbps).is_none()
                        || rule.blocked_ports.is_empty()
                        || rule
                            .blocked_ports
                            .iter()
                            .any(|i| *i >= model.ports.len() || *i == rule.trigger_port)
                    {
                        return Err("invalid shared-bay mode block".into());
                    }
                }
                for inlet in &model.power_inlets {
                    if !rect(inlet.socket.rect)
                        || (inlet.connector == PowerInletConnector::CiscoFourPin)
                            != model.adapter.is_some()
                    {
                        return Err("invalid inlet geometry/adapter".into());
                    }
                }
                if let Some(a) = &model.adapter {
                    if model.power_inlets.len() != 1 {
                        return Err("DC adapters require one inlet".into());
                    }
                    if u64::from(a.output_volts) * u64::from(a.output_current_ma)
                        < u64::from(a.output_watts) * 1000
                    {
                        return Err("adapter output ratings disagree".into());
                    }
                }
                for face in [&model.artwork.front, &model.artwork.rear] {
                    if face.path.is_empty()
                        || face.path.starts_with('/')
                        || face.path.split('/').any(|part| part == "..")
                        || !rect(face.uv)
                        || face.source_size.contains(&0)
                    {
                        return Err("invalid artwork path, UV or size".into());
                    }
                }
                Ok(())
            };
            // ID sets are deliberately captured mutably for duplicate detection.
            let mut validate = validate;
            validate().map_err(|e| format!("network model {}: {e}", model.id))?;
        }
        for (id, category) in [
            (&self.default_router, NetworkDeviceCategory::Router),
            (&self.default_switch, NetworkDeviceCategory::Switch),
        ] {
            if !self.model(id).is_some_and(|m| m.category == category) {
                return Err(format!("invalid default model {id}"));
            }
        }
        Ok(())
    }
}

impl NetworkSim {
    pub(crate) fn network_mode_available(&self, port: PortId) -> bool {
        let Some(p) = self.port(port) else {
            return false;
        };
        let Some(d) = self.device(p.device) else {
            return true;
        };
        let Some(model) = d.kind.network_model() else {
            return true;
        };
        let Some(index) = d.ports().iter().position(|id| *id == port) else {
            return true;
        };
        !model.mode_blocks.iter().any(|rule| {
            rule.blocked_ports.contains(&index)
                && d.ports().get(rule.trigger_port).is_some_and(|trigger| {
                    self.port(*trigger).is_some_and(|p| {
                        p.enabled
                            && self.endpoint_module(*trigger).is_some_and(|m| {
                                let ceiling = p.max_speed.min(p.advertised_speed);
                                m.modes
                                    .iter()
                                    .filter(|mode| {
                                        mode.speed <= ceiling
                                            && model.ports[rule.trigger_port]
                                                .cage
                                                .as_ref()
                                                .is_some_and(|c| c.modes.contains(mode))
                                    })
                                    .map(|mode| mode.speed.mbps())
                                    .max()
                                    .is_some_and(|speed| speed >= rule.min_speed_mbps)
                            })
                    })
                })
        })
    }
}
