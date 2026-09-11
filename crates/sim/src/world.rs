use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSim {
    pub(crate) devices: HashMap<DeviceId, Device>,
    pub(crate) ports: HashMap<PortId, Port>,
    pub(crate) links: HashMap<LinkId, Link>,
    pub(crate) racks: HashMap<RackId, Rack>,
    pub money: i64,
    #[serde(default)]
    pub(crate) cable_inventory: CableInventory,
    pub topology_revision: u64,
    pub routing_revision: u64,
    #[serde(default)]
    pub(crate) ios_configs: HashMap<DeviceId, IosDeviceConfig>,
    #[serde(default)]
    pub(crate) startup_configs: HashMap<DeviceId, IosStartupConfig>,
    #[serde(skip)]
    pub(crate) console_modes: HashMap<DeviceId, IosMode>,
    next_device_id: u64,
    next_port_id: u64,
    next_link_id: u64,
    next_rack_id: u64,
    #[serde(skip)]
    port_links: HashMap<PortId, LinkId>,
    #[serde(skip)]
    pub(crate) runtime: NetworkRuntime,
}

impl Default for NetworkSim {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkSim {
    pub fn new() -> Self {
        let mut sim = Self {
            devices: HashMap::new(),
            ports: HashMap::new(),
            links: HashMap::new(),
            racks: HashMap::new(),
            money: 6_000,
            cable_inventory: CableInventory::default(),
            topology_revision: 0,
            routing_revision: 0,
            ios_configs: HashMap::new(),
            startup_configs: HashMap::new(),
            console_modes: HashMap::new(),
            next_device_id: 1,
            next_port_id: 1,
            next_link_id: 1,
            next_rack_id: 1,
            port_links: HashMap::new(),
            runtime: NetworkRuntime::default(),
        };
        sim.add_rack("Rack 01", 12);
        sim
    }

    pub fn rebuild_indexes(&mut self) {
        // Runtime state is deliberately not persisted across a loaded or
        // replaced topology: learned MAC/ARP entries and port LEDs refer to
        // the old physical graph.
        self.runtime.reset();
        // Saves created before connector types existed named SFP ports explicitly.
        // Restore that physical distinction before accepting their links.
        for port in self.ports.values_mut() {
            if port.name.starts_with("SFP ") {
                port.connector = PortConnector::Sfp;
            }
        }
        self.links.retain(|_, link| {
            [link.a, link.b].iter().all(|id| {
                self.ports
                    .get(id)
                    .is_some_and(|port| port.connector.supports_cabling())
            })
        });
        // Keep the parallel color vector aligned after loading older saves.
        self.normalize_patch_cable_colors();
        self.port_links.clear();
        let cuts: Vec<_> = self
            .links
            .values()
            .filter(|link| link.auto_length)
            .filter_map(|link| {
                self.minimum_cable_length(link.a, link.b)
                    .ok()
                    .map(|cm| (link.id, cm))
            })
            .collect();
        for (id, cm) in cuts {
            // Trim legacy automatic leads; offcuts do not return to the spool.
            if let Some(link) = self.links.get_mut(&id) {
                link.length_cm = link.length_cm.min(cm);
            }
        }
        for (id, link) in &self.links {
            self.port_links.insert(link.a, *id);
            self.port_links.insert(link.b, *id);
        }
        self.next_device_id = self.devices.keys().map(|v| v.0).max().unwrap_or(0) + 1;
        self.next_port_id = self.ports.keys().map(|v| v.0).max().unwrap_or(0) + 1;
        self.next_link_id = self.links.keys().map(|v| v.0).max().unwrap_or(0) + 1;
        self.next_rack_id = self.racks.keys().map(|v| v.0).max().unwrap_or(0) + 1;
    }

    pub fn devices(&self) -> impl Iterator<Item = &Device> {
        self.devices.values()
    }
    pub fn ports(&self) -> impl Iterator<Item = &Port> {
        self.ports.values()
    }
    pub fn links(&self) -> impl Iterator<Item = &Link> {
        self.links.values()
    }
    pub fn racks(&self) -> impl Iterator<Item = &Rack> {
        self.racks.values()
    }
    pub fn device(&self, id: DeviceId) -> Option<&Device> {
        self.devices.get(&id)
    }
    pub fn port(&self, id: PortId) -> Option<&Port> {
        self.ports.get(&id)
    }
    pub fn link(&self, id: LinkId) -> Option<&Link> {
        self.links.get(&id)
    }
    pub fn rack(&self, id: RackId) -> Option<&Rack> {
        self.racks.get(&id)
    }
    pub fn link_for_port(&self, id: PortId) -> Option<&Link> {
        self.port_links.get(&id).and_then(|v| self.links.get(v))
    }

    pub fn port_link_up(&self, id: PortId) -> bool {
        self.link_for_port(id).is_some_and(|link| {
            link.enabled
                && link.length_cm <= 10_000
                && [link.a, link.b].iter().all(|endpoint| {
                    self.port(*endpoint).is_some_and(|port| {
                        port.connector.supports_cabling()
                            && port.enabled
                            && self
                                .device(port.device)
                                .is_some_and(|device| device.powered && device.rack.is_some())
                    })
                })
        })
    }

    /// Returns the negotiated physical rate when this port has an active link.
    pub fn port_link_speed(&self, id: PortId) -> Option<LinkSpeed> {
        let link = self.link_for_port(id)?;
        if !self.port_link_up(id) {
            return None;
        }
        let a = self.port(link.a)?;
        let b = self.port(link.b)?;
        Some(
            a.advertised_speed
                .min(a.max_speed)
                .min(b.advertised_speed)
                .min(b.max_speed),
        )
    }

    /// Advance the deterministic packet clock used for port activity LEDs.
    pub fn advance_time(&mut self, ms: u64) {
        self.runtime.advance_time(ms);
    }

    pub fn simulation_time_ms(&self) -> u64 {
        self.runtime.simulation_time_ms()
    }

    pub fn port_activity(&self, id: PortId, window_ms: u64) -> bool {
        self.port_link_up(id) && self.runtime.activity(id, window_ms)
    }

    /// Current layer-1 counters and activity state for a port.
    pub fn port_telemetry(&self, id: PortId) -> PortTelemetry {
        let mut telemetry = self.runtime.port_telemetry(id);
        telemetry.link_up = self.port_link_up(id);
        telemetry
    }

    /// Execute a diagnostic while recording the frames generated by its
    /// source interface. The legacy immutable `ping` remains available for
    /// callers that only need a result; new packet-driving code should use
    /// this method so runtime state is updated.
    pub fn ping_mut(&mut self, source: PortId, destination: Ipv4Addr) -> ReachabilityResult {
        self.transmit_icmp(source, destination)
    }

    pub fn arp_entries(&self, port: PortId) -> Vec<(Ipv4Addr, MacAddress)> {
        self.runtime.arp_entries(port).collect()
    }

    pub fn add_rack(&mut self, name: impl Into<String>, units: u8) -> RackId {
        let id = RackId(self.next_rack_id);
        self.next_rack_id += 1;
        self.racks.insert(
            id,
            Rack {
                id,
                name: name.into(),
                units,
                placements: Vec::new(),
            },
        );
        id
    }

    pub fn execute(&mut self, command: Command) -> Result<Vec<SimEvent>, SimError> {
        let mut events = match command {
            Command::BuyCableSupply { supply } => {
                self.buy_cable_supply(supply)?;
                vec![SimEvent::CableSuppliesPurchased(supply)]
            }
            Command::ConnectCable { a, b, length_cm } => {
                let id = self.connect(a, b, Some(length_cm), CableColor::White)?;
                vec![SimEvent::LinkCreated(id)]
            }
            Command::ConnectColoredCable {
                a,
                b,
                length_cm,
                color,
            } => {
                let id = self.connect(a, b, length_cm, color)?;
                vec![SimEvent::LinkCreated(id)]
            }
            Command::BuyDevice { kind } => {
                let id = self.buy_device(kind)?;
                vec![SimEvent::DeviceAdded(id)]
            }
            Command::SellDevice { device } => {
                self.sell_device(device)?;
                vec![SimEvent::DeviceRemoved(device)]
            }
            Command::PlaceDevice { device, rack, unit } => {
                self.place_device(device, rack, unit)?;
                vec![SimEvent::DeviceMoved { device }]
            }
            Command::RemoveDevice { device } => {
                self.remove_device(device)?;
                vec![SimEvent::DeviceMoved { device }]
            }
            Command::Connect { a, b } => {
                let id = self.connect(a, b, None, CableColor::White)?;
                vec![SimEvent::LinkCreated(id)]
            }
            Command::Disconnect { link } => {
                self.disconnect(link)?;
                vec![SimEvent::LinkRemoved(link)]
            }
            Command::SetSwitchPortMode { port, mode } => {
                self.set_switch_mode(port, mode)?;
                vec![SimEvent::PortConfigChanged(port)]
            }
            Command::SetPortSpeed { port, speed } => {
                self.set_port_speed(port, speed)?;
                vec![SimEvent::PortConfigChanged(port)]
            }
            Command::CreateVlan { switch, vlan } => {
                self.create_vlan(switch, vlan)?;
                vec![SimEvent::ConnectivityChanged]
            }
            Command::SetIpv4 { port, config } => {
                self.set_ipv4(port, config)?;
                vec![SimEvent::PortConfigChanged(port)]
            }
            Command::ConfigureRouterInterface {
                port,
                name,
                vlan,
                address,
                prefix,
                internet_connected,
            } => {
                self.configure_router_interface(
                    port,
                    name,
                    vlan,
                    address,
                    prefix,
                    internet_connected,
                )?;
                vec![SimEvent::PortConfigChanged(port)]
            }
            Command::SetStaticRoute { router, route } => {
                self.set_static_route(router, route)?;
                vec![SimEvent::ConnectivityChanged]
            }
            Command::RemoveStaticRoute { router, route } => {
                self.remove_static_route(router, route)?;
                vec![SimEvent::ConnectivityChanged]
            }
            Command::SetHostname { device, hostname } => {
                self.set_hostname(device, hostname)?;
                vec![SimEvent::ConnectivityChanged]
            }
            Command::SetPower { device, powered } => {
                self.set_power(device, powered)?;
                vec![SimEvent::DevicePowerChanged { device, powered }]
            }
            Command::ResetPortConfig { port } => {
                self.reset_port_config(port)?;
                vec![SimEvent::PortConfigChanged(port)]
            }
        };
        if events.iter().any(|e| {
            matches!(
                e,
                SimEvent::LinkCreated(_)
                    | SimEvent::LinkRemoved(_)
                    | SimEvent::PortConfigChanged(_)
                    | SimEvent::DevicePowerChanged { .. }
            )
        }) {
            self.topology_revision += 1;
            events.push(SimEvent::TopologyChanged {
                revision: self.topology_revision,
            });
            events.push(SimEvent::ConnectivityChanged);
        }
        Ok(events)
    }

    fn alloc_port(
        &mut self,
        device: DeviceId,
        name: String,
        connector: PortConnector,
        config: PortConfig,
    ) -> PortId {
        let id = PortId(self.next_port_id);
        self.next_port_id += 1;
        self.ports.insert(
            id,
            Port {
                id,
                device,
                name,
                enabled: true,
                connector,
                advertised_speed: LinkSpeed::default(),
                max_speed: LinkSpeed::default(),
                config,
            },
        );
        id
    }

    fn buy_device(&mut self, template: DeviceTemplate) -> Result<DeviceId, SimError> {
        let price = template.price();
        if self.money < price {
            return Err(SimError::InsufficientFunds {
                needed: price,
                available: self.money,
            });
        }
        self.money -= price;
        let id = DeviceId(self.next_device_id);
        self.next_device_id += 1;
        let index = self
            .devices
            .values()
            .filter(|d| d.template() == template)
            .count()
            + 1;
        let (name, kind) = match template {
            DeviceTemplate::Server => {
                let ports = (0..2)
                    .map(|n| {
                        self.alloc_port(
                            id,
                            format!("eth{n}"),
                            PortConnector::Rj45,
                            PortConfig::Server(ServerPortConfig::default()),
                        )
                    })
                    .collect();
                (
                    format!("Dell PowerEdge R360 #{index:02}"),
                    DeviceKind::Server(Server {
                        hostname: format!("server{index:02}"),
                        ports,
                    }),
                )
            }
            DeviceTemplate::Switch => {
                let mut ports: Vec<_> = (1..=24)
                    .map(|n| {
                        self.alloc_port(
                            id,
                            format!("Gi1/0/{n:02}"),
                            PortConnector::Rj45,
                            PortConfig::Switch(SwitchPortConfig {
                                mode: SwitchPortMode::Access { vlan: None },
                            }),
                        )
                    })
                    .collect();
                ports.extend((25..=28).map(|n| {
                    self.alloc_port(
                        id,
                        format!("SFP Gi1/0/{n:02}"),
                        PortConnector::Sfp,
                        PortConfig::Switch(SwitchPortConfig {
                            mode: SwitchPortMode::Trunk {
                                native_vlan: Some(VlanId(1)),
                                allowed: vec![VlanId(1)],
                            },
                        }),
                    )
                }));
                (
                    format!("Cisco Catalyst C1000-24T-4G-L #{index:02}"),
                    DeviceKind::Switch(Switch {
                        ports,
                        vlans: vec![Vlan {
                            id: VlanId(1),
                            name: "Default".into(),
                        }],
                    }),
                )
            }
            DeviceTemplate::Router => {
                let mut ports = Vec::new();
                for name in [
                    "WAN1", "WAN2", "LAN1", "LAN2", "LAN3", "LAN4", "LAN5", "LAN6", "LAN7", "LAN8",
                ] {
                    ports.push(self.alloc_port(
                        id,
                        name.into(),
                        PortConnector::Rj45,
                        PortConfig::Router(RouterPortConfig::default()),
                    ));
                }
                let wan = ports[0];
                let interface = RouterInterface::wan("WAN1", wan);
                if let Some(Port {
                    config: PortConfig::Router(config),
                    ..
                }) = self.ports.get_mut(&wan)
                {
                    config.interfaces.push(interface.clone());
                }
                (
                    format!("Cisco ISR C1111-8P #{index:02}"),
                    DeviceKind::Router(Router {
                        ports,
                        interfaces: vec![interface],
                        routes: Vec::new(),
                    }),
                )
            }
        };
        self.devices.insert(
            id,
            Device {
                id,
                name,
                powered: false,
                rack: None,
                kind,
            },
        );
        Ok(id)
    }

    fn sell_device(&mut self, id: DeviceId) -> Result<(), SimError> {
        let device = self.devices.get(&id).ok_or(SimError::DeviceNotFound(id))?;
        if device.rack.is_some() {
            return Err(SimError::DeviceInstalled);
        }
        let ports = device.ports().to_vec();
        for port in &ports {
            if let Some(link) = self.port_links.get(port).copied() {
                self.disconnect(link)?;
            }
        }
        let device = self.devices.remove(&id).expect("checked");
        self.ios_configs.remove(&id);
        self.startup_configs.remove(&id);
        self.console_modes.remove(&id);
        for port in ports {
            self.ports.remove(&port);
        }
        self.money += device.template().price() / 2;
        Ok(())
    }

    fn place_device(&mut self, id: DeviceId, rack_id: RackId, unit: u8) -> Result<(), SimError> {
        let height = self
            .devices
            .get(&id)
            .ok_or(SimError::DeviceNotFound(id))?
            .template()
            .rack_units();
        let rack = self
            .racks
            .get(&rack_id)
            .ok_or(SimError::RackNotFound(rack_id))?;
        if unit == 0 || unit.saturating_add(height - 1) > rack.units {
            return Err(SimError::RackPlacementOutOfBounds);
        }
        for target in unit..unit + height {
            if rack.occupies(target).is_some_and(|v| v != id) {
                return Err(SimError::RackUnitOccupied {
                    rack: rack_id,
                    unit: target,
                });
            }
        }
        self.remove_device(id)?;
        let placement = RackPlacement {
            rack: rack_id,
            unit,
            height,
        };
        self.racks
            .get_mut(&rack_id)
            .expect("checked")
            .placements
            .push((id, placement));
        self.devices.get_mut(&id).expect("checked").rack = Some(placement);
        Ok(())
    }

    pub(crate) fn set_port_speed(
        &mut self,
        port: PortId,
        speed: LinkSpeed,
    ) -> Result<(), SimError> {
        let target = self
            .ports
            .get_mut(&port)
            .ok_or(SimError::PortNotFound(port))?;
        target.advertised_speed = speed;
        Ok(())
    }

    fn remove_device(&mut self, id: DeviceId) -> Result<(), SimError> {
        let old = self
            .devices
            .get(&id)
            .ok_or(SimError::DeviceNotFound(id))?
            .rack;
        if let Some(old) = old {
            self.racks
                .get_mut(&old.rack)
                .ok_or(SimError::RackNotFound(old.rack))?
                .placements
                .retain(|(v, _)| *v != id);
        }
        self.devices.get_mut(&id).expect("checked").rack = None;
        let links: Vec<_> = self.devices[&id]
            .ports()
            .iter()
            .filter_map(|p| self.link_for_port(*p).map(|l| l.id))
            .collect();
        for link in links {
            if self.links.contains_key(&link) {
                self.disconnect(link)?;
            }
        }
        Ok(())
    }

    pub(crate) fn validate_cable_endpoints(&self, a: PortId, b: PortId) -> Result<(), SimError> {
        if a == b {
            return Err(SimError::SamePort);
        }
        let pa = self.ports.get(&a).ok_or(SimError::PortNotFound(a))?;
        let pb = self.ports.get(&b).ok_or(SimError::PortNotFound(b))?;
        if self.port_links.contains_key(&a) {
            return Err(SimError::PortAlreadyConnected(a));
        }
        if self.port_links.contains_key(&b) {
            return Err(SimError::PortAlreadyConnected(b));
        }
        for port in [pa, pb] {
            if !port.connector.supports_cabling() {
                return Err(SimError::UnsupportedConnector {
                    port: port.id,
                    connector: port.connector,
                });
            }
        }
        Ok(())
    }

    fn connect(
        &mut self,
        a: PortId,
        b: PortId,
        length_cm: Option<u32>,
        color: CableColor,
    ) -> Result<LinkId, SimError> {
        let quote = self.quote_colored_cable(a, b, length_cm, color)?;
        self.consume_cable(quote)?;
        let id = LinkId(self.next_link_id);
        self.next_link_id += 1;
        self.links.insert(
            id,
            Link {
                id,
                a,
                b,
                enabled: true,
                length_cm: quote.length_cm,
                auto_length: length_cm.is_none(),
                color,
            },
        );
        self.port_links.insert(a, id);
        self.port_links.insert(b, id);
        Ok(id)
    }

    fn disconnect(&mut self, id: LinkId) -> Result<(), SimError> {
        let link = self.links.remove(&id).ok_or(SimError::LinkNotFound)?;
        self.port_links.remove(&link.a);
        self.port_links.remove(&link.b);
        self.normalize_patch_cable_colors();
        self.cable_inventory.patch_cables_cm.push(link.length_cm);
        self.cable_inventory.patch_cable_colors.push(link.color);
        let mut leads: Vec<_> = self
            .cable_inventory
            .patch_cables_cm
            .iter()
            .copied()
            .zip(self.cable_inventory.patch_cable_colors.iter().copied())
            .collect();
        leads.sort_unstable_by_key(|(length, _)| *length);
        self.cable_inventory.patch_cables_cm = leads.iter().map(|(length, _)| *length).collect();
        self.cable_inventory.patch_cable_colors =
            leads.into_iter().map(|(_, color)| color).collect();
        Ok(())
    }

    fn set_power(&mut self, id: DeviceId, powered: bool) -> Result<(), SimError> {
        self.devices
            .get_mut(&id)
            .ok_or(SimError::DeviceNotFound(id))?
            .powered = powered;
        Ok(())
    }

    fn reset_port_config(&mut self, port: PortId) -> Result<(), SimError> {
        let device = self
            .ports
            .get(&port)
            .ok_or(SimError::PortNotFound(port))?
            .device;
        let is_wan = matches!(self.devices.get(&device).map(|d| &d.kind), Some(DeviceKind::Router(r)) if r.ports.first() == Some(&port));
        match &mut self.ports.get_mut(&port).expect("checked").config {
            PortConfig::Server(config) => config.ipv4 = None,
            PortConfig::Switch(config) => config.mode = SwitchPortMode::Access { vlan: None },
            PortConfig::Router(config) => {
                config.interfaces = if is_wan {
                    vec![RouterInterface::wan("WAN1", port)]
                } else {
                    vec![]
                };
            }
        }
        if let DeviceKind::Router(router) =
            &mut self.devices.get_mut(&device).expect("checked").kind
        {
            router.interfaces.retain(|interface| interface.port != port);
            router.routes.retain(|route| route.egress != port);
            if is_wan {
                router.interfaces.push(RouterInterface::wan("WAN1", port));
            }
        }
        self.routing_revision += 1;
        Ok(())
    }

    fn set_hostname(&mut self, id: DeviceId, hostname: String) -> Result<(), SimError> {
        if hostname.is_empty()
            || !hostname
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(SimError::InvalidHostname);
        }
        match &mut self
            .devices
            .get_mut(&id)
            .ok_or(SimError::DeviceNotFound(id))?
            .kind
        {
            DeviceKind::Server(server) => {
                server.hostname = hostname;
                Ok(())
            }
            _ => Err(SimError::WrongPortType),
        }
    }

    fn create_vlan(&mut self, id: DeviceId, vlan: Vlan) -> Result<(), SimError> {
        if vlan.id.0 == 0 || vlan.id.0 >= 4095 {
            return Err(SimError::VlanNotFound(vlan.id));
        }
        match &mut self
            .devices
            .get_mut(&id)
            .ok_or(SimError::DeviceNotFound(id))?
            .kind
        {
            DeviceKind::Switch(sw) => {
                if let Some(old) = sw.vlans.iter_mut().find(|v| v.id == vlan.id) {
                    *old = vlan;
                } else {
                    sw.vlans.push(vlan);
                }
                Ok(())
            }
            _ => Err(SimError::WrongPortType),
        }
    }

    fn set_switch_mode(&mut self, id: PortId, mode: SwitchPortMode) -> Result<(), SimError> {
        let port = self.ports.get(&id).ok_or(SimError::PortNotFound(id))?;
        if !port.connector.supports_cabling() {
            return Err(SimError::UnsupportedConnector {
                port: id,
                connector: port.connector,
            });
        }
        let device_id = port.device;
        let switch = match &self
            .devices
            .get(&device_id)
            .ok_or(SimError::DeviceNotFound(device_id))?
            .kind
        {
            DeviceKind::Switch(v) => v,
            _ => return Err(SimError::WrongPortType),
        };
        let vlans: Vec<_> = match &mode {
            SwitchPortMode::Access { vlan } => vlan.iter().copied().collect(),
            SwitchPortMode::Trunk {
                allowed,
                native_vlan,
            } => allowed
                .iter()
                .copied()
                .chain(native_vlan.iter().copied())
                .collect(),
        };
        if let Some(invalid) = vlans.iter().find(|v| v.0 == 0 || v.0 >= 4095) {
            return Err(SimError::VlanNotFound(*invalid));
        }
        // Trunks may allow VLANs before those VLANs exist in the local database.
        if matches!(mode, SwitchPortMode::Access { .. })
            && let Some(missing) = vlans
                .into_iter()
                .find(|v| !switch.vlans.iter().any(|known| known.id == *v))
        {
            return Err(SimError::VlanNotFound(missing));
        }
        match &mut self.ports.get_mut(&id).expect("checked").config {
            PortConfig::Switch(config) => {
                config.mode = mode;
                Ok(())
            }
            _ => Err(SimError::WrongPortType),
        }
    }

    fn set_ipv4(&mut self, id: PortId, config: Ipv4InterfaceConfig) -> Result<(), SimError> {
        if config.prefix > 32 {
            return Err(SimError::InvalidIpv4Prefix);
        }
        if config.address.is_unspecified() {
            return Err(SimError::InvalidIpv4Address);
        }
        if config.vlan.is_some_and(|v| v.0 == 0 || v.0 >= 4095) {
            return Err(SimError::InvalidIpv4Vlan);
        }
        match &mut self
            .ports
            .get_mut(&id)
            .ok_or(SimError::PortNotFound(id))?
            .config
        {
            PortConfig::Server(v) => {
                v.ipv4 = Some(config);
                self.routing_revision += 1;
                Ok(())
            }
            _ => Err(SimError::WrongPortType),
        }
    }

    fn configure_router_interface(
        &mut self,
        id: PortId,
        name: String,
        vlan: Option<VlanId>,
        address: Option<Ipv4Addr>,
        prefix: u8,
        internet_connected: bool,
    ) -> Result<(), SimError> {
        if prefix > 32 {
            return Err(SimError::InvalidIpv4Prefix);
        }
        if address.is_some_and(|value| value.is_unspecified()) {
            return Err(SimError::InvalidIpv4Address);
        }
        if vlan.is_some_and(|value| value.0 == 0 || value.0 >= 4095) {
            return Err(SimError::InvalidIpv4Vlan);
        }
        let device_id = self
            .ports
            .get(&id)
            .ok_or(SimError::PortNotFound(id))?
            .device;
        let interface = RouterInterface {
            name,
            port: id,
            vlan,
            address,
            prefix,
            dhcp: address.is_none(),
            internet_connected,
        };
        match &mut self.ports.get_mut(&id).expect("checked").config {
            PortConfig::Router(v) => {
                v.interfaces.retain(|old| old.vlan != vlan);
                v.interfaces.push(interface.clone());
            }
            _ => return Err(SimError::WrongPortType),
        }
        match &mut self
            .devices
            .get_mut(&device_id)
            .expect("port owner exists")
            .kind
        {
            DeviceKind::Router(router) => {
                router
                    .interfaces
                    .retain(|old| !(old.port == id && old.vlan == vlan));
                router.interfaces.push(interface);
            }
            _ => return Err(SimError::WrongPortType),
        }
        self.routing_revision += 1;
        Ok(())
    }

    fn set_static_route(&mut self, id: DeviceId, mut route: Route) -> Result<(), SimError> {
        if route.prefix > 32 {
            return Err(SimError::InvalidIpv4Prefix);
        }
        if route.via.is_some_and(|ip| {
            ip.is_unspecified() || ip.is_multicast() || ip.is_loopback() || ip.is_broadcast()
        }) {
            return Err(SimError::InvalidRouteNextHop);
        }
        let device = self.devices.get(&id).ok_or(SimError::DeviceNotFound(id))?;
        let DeviceKind::Router(config) = &device.kind else {
            return Err(SimError::WrongPortType);
        };
        if !config
            .interfaces
            .iter()
            .any(|interface| interface.port == route.egress)
        {
            return Err(SimError::WrongPortType);
        }
        route.network = canonical_network(route.network, route.prefix);
        let DeviceKind::Router(config) = &mut self.devices.get_mut(&id).expect("checked").kind
        else {
            unreachable!()
        };
        config
            .routes
            .retain(|old| old.network != route.network || old.prefix != route.prefix);
        config.routes.push(route);
        self.routing_revision += 1;
        Ok(())
    }

    fn remove_static_route(&mut self, id: DeviceId, mut route: Route) -> Result<(), SimError> {
        if route.prefix > 32 {
            return Err(SimError::InvalidIpv4Prefix);
        }
        let device = self
            .devices
            .get_mut(&id)
            .ok_or(SimError::DeviceNotFound(id))?;
        let DeviceKind::Router(config) = &mut device.kind else {
            return Err(SimError::WrongPortType);
        };
        route.network = canonical_network(route.network, route.prefix);
        let before = config.routes.len();
        config.routes.retain(|old| {
            !(old.network == route.network
                && old.prefix == route.prefix
                && old.egress == route.egress
                && old.via == route.via)
        });
        if config.routes.len() != before {
            self.routing_revision += 1;
        }
        Ok(())
    }
}

fn canonical_network(address: Ipv4Addr, prefix: u8) -> Ipv4Addr {
    if prefix == 0 {
        return Ipv4Addr::UNSPECIFIED;
    }
    Ipv4Addr::from(u32::from(address) & (u32::MAX << (32 - prefix)))
}
