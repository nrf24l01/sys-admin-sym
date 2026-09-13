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
    #[serde(default)]
    pub power: PowerSystem,
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
            power: PowerSystem::new(),
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
        self.power
            .devices
            .retain(|id, _| self.devices.contains_key(id));
        self.power.connections.retain(|_, endpoint| match endpoint {
            PowerEndpoint::Device(id) => self.devices.contains_key(id),
            PowerEndpoint::Source(_) => true,
        });
        // Saves written before typed cords existed used IEC implicitly. Routers
        // are the sole device supplied with the Cisco DC adapter.
        self.power
            .cord_kinds
            .retain(|outlet, _| self.power.connections.contains_key(outlet));
        let missing_cords: Vec<_> = self
            .power
            .connections
            .iter()
            .filter_map(|(outlet, endpoint)| {
                (!self.power.cord_kinds.contains_key(outlet)).then_some((*outlet, *endpoint))
            })
            .collect();
        for (outlet, endpoint) in missing_cords {
            let kind = match endpoint {
                PowerEndpoint::Device(id)
                    if self
                        .devices
                        .get(&id)
                        .is_some_and(|d| matches!(d.kind, DeviceKind::Router(_))) =>
                {
                    PowerCordKind::Cisco66WAdapter
                }
                _ => PowerCordKind::IecC13C14,
            };
            self.power.cord_kinds.insert(outlet, kind);
        }
        for rack in self.racks.keys().copied().collect::<Vec<_>>() {
            self.power.add_rack(rack);
        }
        for (id, device) in &self.devices {
            if matches!(
                device.kind,
                DeviceKind::Ups(_)
                    | DeviceKind::Pdu(_)
                    | DeviceKind::PatchPanel(_)
                    | DeviceKind::CableManager(_)
            ) {
                continue;
            }
            if !self.power.devices.contains_key(id) {
                let watts = match device.kind {
                    DeviceKind::Server(_) => 180,
                    DeviceKind::Switch(_) => 80,
                    DeviceKind::Router(_) => 60,
                    _ => 0,
                };
                self.power.add_device(*id, watts, 90);
                if let Some(p) = self.power.devices.get_mut(id) {
                    p.requested = device.powered;
                }
            }
        }
        let source_devices: Vec<_> = self
            .devices
            .iter()
            .filter_map(|(id, d)| match d.kind {
                DeviceKind::Ups(_) | DeviceKind::Pdu(_) => Some(*id),
                _ => None,
            })
            .collect();
        for id in source_devices {
            let missing = match &self.devices[&id].kind {
                DeviceKind::Ups(x) => x.source.is_none(),
                DeviceKind::Pdu(x) => x.source.is_none(),
                _ => false,
            };
            if missing {
                let source = if matches!(self.devices[&id].kind, DeviceKind::Ups(_)) {
                    SourceId::Ups(self.power.add_ups(UpsSpec::default()))
                } else {
                    SourceId::Pdu(self.power.add_pdu(PduState::default()))
                };
                match &mut self.devices.get_mut(&id).unwrap().kind {
                    DeviceKind::Ups(x) => x.source = Some(source),
                    DeviceKind::Pdu(x) => x.source = Some(source),
                    _ => {}
                }
            }
        }
        self.power.recompute_now();
        self.sync_effective_power();
        // Migrate legacy servers that predate the dedicated management NIC.
        let legacy_servers: Vec<_> = self
            .devices
            .iter()
            .filter_map(|(id, device)| match &device.kind {
                DeviceKind::Server(server) if server.ports.len() < 3 => {
                    Some((*id, server.ports.len()))
                }
                _ => None,
            })
            .collect();
        for (device, count) in legacy_servers {
            let mut existing: std::collections::HashSet<_> = self.devices[&device]
                .ports()
                .iter()
                .filter_map(|port| self.ports.get(port).map(|port| port.name.clone()))
                .collect();
            let mut current = count;
            while current < 3 {
                let name = if !existing.contains("eth1") {
                    "eth1"
                } else if !existing.contains("mgmt0") {
                    "mgmt0"
                } else {
                    "mgmt1"
                };
                let port = self.alloc_port(
                    device,
                    name.into(),
                    PortConnector::Rj45,
                    PortConfig::Server(ServerPortConfig::default()),
                );
                if let DeviceKind::Server(server) = &mut self.devices.get_mut(&device).unwrap().kind
                {
                    server.ports.push(port);
                }
                existing.insert(name.to_string());
                current += 1;
            }
        }
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
        // Port face metadata was added after the original save format. Restore
        // deterministic faces for legacy equipment, while preserving panel
        // front/rear assignments and rebuilding missing reciprocal pairs.
        let port_devices: Vec<_> = self.ports.iter().map(|(id, p)| (*id, p.device)).collect();
        for (id, device) in port_devices {
            let side = match self.devices.get(&device).map(|d| &d.kind) {
                Some(DeviceKind::Server(_)) => RackSide::Rear,
                Some(DeviceKind::Switch(_)) | Some(DeviceKind::Router(_)) => RackSide::Front,
                Some(DeviceKind::PatchPanel(_)) => self.ports[&id].side,
                _ => RackSide::Rear,
            };
            self.ports.get_mut(&id).unwrap().side = side;
        }
        let panel_ports: Vec<Vec<PortId>> = self
            .devices
            .values()
            .filter_map(|d| {
                if let DeviceKind::PatchPanel(panel) = &d.kind {
                    Some(panel.ports.clone())
                } else {
                    None
                }
            })
            .collect();
        for ports in panel_ports {
            for pair in ports.chunks(2).filter(|pair| pair.len() == 2) {
                self.ports.get_mut(&pair[0]).unwrap().paired_port = Some(pair[1]);
                self.ports.get_mut(&pair[1]).unwrap().paired_port = Some(pair[0]);
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
                let minimum = if link.route.is_empty() {
                    self.minimum_cable_length(link.a, link.b)
                } else {
                    self.minimum_routed_cable_length(link.a, link.b, &link.route)
                };
                minimum.ok().map(|cm| (link.id, cm))
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
        if self.port(id).and_then(|p| p.paired_port).is_some()
            || self
                .link_for_port(id)
                .and_then(|l| l.other(id))
                .and_then(|p| self.port(p))
                .and_then(|p| p.paired_port)
                .is_some()
        {
            return self.physical_link_up(id);
        }
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
        if self.port(id).and_then(|p| p.paired_port).is_some()
            || self
                .link_for_port(id)
                .and_then(|l| l.other(id))
                .and_then(|p| self.port(p))
                .and_then(|p| p.paired_port)
                .is_some()
        {
            return self.physical_link_speed(id);
        }
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
        self.power.tick_ms(ms);
        self.sync_effective_power();
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
        self.power.add_rack(id);
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
            Command::ConnectRoutedColoredCable {
                a,
                b,
                length_cm,
                color,
                route,
            } => {
                // Validate first so a bad route never consumes cable stock.
                for point in &route {
                    self.validate_route_point(point)?;
                }
                let routed_minimum = self.minimum_routed_cable_length(a, b, &route)?;
                if let Some(length) = length_cm
                    && length < routed_minimum
                {
                    return Err(SimError::CableTooShort {
                        minimum_cm: routed_minimum,
                    });
                }
                let automatic = length_cm.is_none();
                let effective_length = length_cm.or(Some(routed_minimum));
                let id = self.connect(a, b, effective_length, color)?;
                let link = self.links.get_mut(&id).expect("new link exists");
                link.route = route;
                link.auto_length = automatic;
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
            Command::AddCableRoutePoint { link, point } => {
                self.add_cable_route_point(link, point)?;
                vec![SimEvent::ConnectivityChanged]
            }
            Command::RemoveCableRoutePoint { link, index } => {
                self.remove_cable_route_point(link, index)?;
                vec![SimEvent::ConnectivityChanged]
            }
            Command::MoveCableRoutePoint { link, index, point } => {
                self.move_cable_route_point(link, index, point)?;
                vec![SimEvent::ConnectivityChanged]
            }
            Command::RerouteCable { link, route } => {
                self.reroute_cable(link, route)?;
                vec![SimEvent::ConnectivityChanged]
            }
            Command::SetSwitchPortMode { port, mode } => {
                self.set_switch_mode(port, mode)?;
                vec![SimEvent::PortConfigChanged(port)]
            }
            Command::SetPortSpeed { port, speed } => {
                self.set_port_speed(port, speed)?;
                vec![SimEvent::PortConfigChanged(port)]
            }
            Command::SetPortEnabled { port, enabled } => {
                self.set_port_enabled(port, enabled)?;
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
                let effective = self.device(device).is_some_and(|d| d.powered);
                vec![SimEvent::DevicePowerChanged {
                    device,
                    powered: effective,
                }]
            }
            Command::ConnectPower { outlet, endpoint } => {
                let kind = self.inferred_power_cord(endpoint);
                self.validate_power_cord(endpoint, kind)?;
                self.validate_power_endpoint(outlet.source, endpoint)?;
                self.power
                    .connect_with_kind(outlet, endpoint, kind)
                    .map_err(|e| SimError::Power(e.to_string()))?;
                self.sync_effective_power();
                vec![SimEvent::PowerChanged]
            }
            Command::ConnectPowerCord {
                outlet,
                endpoint,
                kind,
            } => {
                self.validate_power_cord(endpoint, kind)?;
                self.validate_power_endpoint(outlet.source, endpoint)?;
                self.power
                    .connect_with_kind(outlet, endpoint, kind)
                    .map_err(|e| SimError::Power(e.to_string()))?;
                self.sync_effective_power();
                vec![SimEvent::PowerChanged]
            }
            Command::DisconnectPower { outlet } => {
                self.power.disconnect(outlet);
                self.sync_effective_power();
                vec![SimEvent::PowerChanged]
            }
            Command::ResetPowerBreaker { source } => {
                self.power.reset_breaker(source);
                self.sync_effective_power();
                vec![SimEvent::PowerChanged]
            }
            Command::SetRackMains { rack, on } => {
                self.power.set_rack_mains(rack, on);
                self.sync_effective_power();
                vec![SimEvent::PowerChanged]
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
                side: RackSide::Rear,
                paired_port: None,
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
                let ports = (0..3)
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
                for port in &ports {
                    self.ports.get_mut(port).unwrap().side = RackSide::Front;
                }
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
                for port in &ports {
                    self.ports.get_mut(port).unwrap().side = RackSide::Front;
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
            DeviceTemplate::PatchPanel => {
                let mut ports = Vec::new();
                for n in 1..=24 {
                    let rear = self.alloc_port(
                        id,
                        format!("Rear {n:02}"),
                        PortConnector::Rj45,
                        PortConfig::PatchPanel,
                    );
                    let front = self.alloc_port(
                        id,
                        format!("Front {n:02}"),
                        PortConnector::Rj45,
                        PortConfig::PatchPanel,
                    );
                    self.ports.get_mut(&rear).unwrap().paired_port = Some(front);
                    self.ports.get_mut(&front).unwrap().paired_port = Some(rear);
                    self.ports.get_mut(&front).unwrap().side = RackSide::Front;
                    ports.extend([rear, front]);
                }
                (
                    format!("24-port Patch Panel #{index:02}"),
                    DeviceKind::PatchPanel(PatchPanel { ports }),
                )
            }
            DeviceTemplate::CableManager => (
                format!("1U Horizontal Cable Manager #{index:02}"),
                DeviceKind::CableManager(CableManager { ports: Vec::new() }),
            ),
            DeviceTemplate::Ups => (
                format!("APC Smart-UPS SMT1500RMI2U #{index:02}"),
                DeviceKind::Ups(Ups {
                    ports: Vec::new(),
                    source: None,
                }),
            ),
            DeviceTemplate::Pdu => (
                format!("Rack PDU 8x C13 #{index:02}"),
                DeviceKind::Pdu(Pdu {
                    ports: Vec::new(),
                    source: None,
                }),
            ),
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
        let watts = match template {
            DeviceTemplate::Server => 180,
            DeviceTemplate::Switch => 80,
            DeviceTemplate::Router => 60,
            _ => 0,
        };
        match template {
            DeviceTemplate::Ups => {
                let source = self.power.add_ups(UpsSpec::default());
                if let DeviceKind::Ups(x) = &mut self.devices.get_mut(&id).unwrap().kind {
                    x.source = Some(SourceId::Ups(source));
                }
            }
            DeviceTemplate::Pdu => {
                let source = self.power.add_pdu(PduState::default());
                if let DeviceKind::Pdu(x) = &mut self.devices.get_mut(&id).unwrap().kind {
                    x.source = Some(SourceId::Pdu(source));
                }
            }
            _ => {
                self.power.add_device(id, watts, 90);
            }
        }
        Ok(id)
    }

    fn sell_device(&mut self, id: DeviceId) -> Result<(), SimError> {
        let device = self.devices.get(&id).ok_or(SimError::DeviceNotFound(id))?;
        if device.rack.is_some() {
            return Err(SimError::DeviceInstalled);
        }
        let ports = device.ports().to_vec();
        let source = match &device.kind {
            DeviceKind::Ups(x) => x.source,
            DeviceKind::Pdu(x) => x.source,
            _ => None,
        };
        for port in &ports {
            if let Some(link) = self.port_links.get(port).copied() {
                self.disconnect(link)?;
            }
        }
        let device = self.devices.remove(&id).expect("checked");
        self.power.devices.remove(&id);
        self.power
            .connections
            .retain(|_, endpoint| !matches!(endpoint, PowerEndpoint::Device(d) if *d == id));
        self.power
            .cord_kinds
            .retain(|outlet, _| self.power.connections.contains_key(outlet));
        if let Some(source) = source {
            self.power.connections.retain(|outlet, endpoint| {
                outlet.source != source
                    && !matches!(endpoint, PowerEndpoint::Source(s) if *s == source)
            });
            match source {
                SourceId::Ups(s) => {
                    self.power.ups.remove(&s);
                }
                SourceId::Pdu(s) => {
                    self.power.pdus.remove(&s);
                }
                SourceId::Rack(_) => {}
            }
        }
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
        self.disconnect_device_power(id);
        Ok(())
    }

    fn disconnect_device_power(&mut self, id: DeviceId) {
        let source = match &self.devices[&id].kind {
            DeviceKind::Ups(x) => x.source,
            DeviceKind::Pdu(x) => x.source,
            _ => None,
        };
        self.power.connections.retain(|outlet, endpoint| {
            outlet.source != source.unwrap_or(SourceId::Rack(RackId(0)))
                && !matches!(endpoint, PowerEndpoint::Device(d) if *d == id)
                && !matches!(endpoint, PowerEndpoint::Source(s) if Some(*s) == source)
        });
        self.power
            .cord_kinds
            .retain(|outlet, _| self.power.connections.contains_key(outlet));
        self.power.recompute_now();
        self.sync_effective_power();
    }

    fn validate_power_endpoint(
        &self,
        outlet_source: SourceId,
        endpoint: PowerEndpoint,
    ) -> Result<(), SimError> {
        if let SourceId::Ups(_) | SourceId::Pdu(_) = outlet_source {
            let owner = self.devices.values().find(|d| match &d.kind {
                DeviceKind::Ups(x) => x.source == Some(outlet_source),
                DeviceKind::Pdu(x) => x.source == Some(outlet_source),
                _ => false,
            });
            if owner.is_some_and(|d| d.rack.is_none()) {
                return Err(SimError::Power("power source must be installed".into()));
            }
        }
        match endpoint {
            PowerEndpoint::Device(id) => {
                let d = self.devices.get(&id).ok_or(SimError::DeviceNotFound(id))?;
                if d.rack.is_none()
                    || matches!(
                        d.kind,
                        DeviceKind::PatchPanel(_)
                            | DeviceKind::CableManager(_)
                            | DeviceKind::Ups(_)
                            | DeviceKind::Pdu(_)
                    )
                {
                    return Err(SimError::Power(
                        "power endpoint must be an installed active device".into(),
                    ));
                }
            }
            PowerEndpoint::Source(source) => {
                if !self.power.source_exists_public(source) {
                    return Err(SimError::Power("power source does not exist".into()));
                }
                let owner = self.devices.values().find(|d| match &d.kind {
                    DeviceKind::Ups(x) => x.source == Some(source),
                    DeviceKind::Pdu(x) => x.source == Some(source),
                    _ => false,
                });
                if let Some(owner) = owner
                    && owner.rack.is_none()
                {
                    return Err(SimError::Power("power source must be installed".into()));
                }
            }
        }
        Ok(())
    }

    fn inferred_power_cord(&self, endpoint: PowerEndpoint) -> PowerCordKind {
        match endpoint {
            PowerEndpoint::Device(id)
                if self
                    .devices
                    .get(&id)
                    .is_some_and(|d| matches!(d.kind, DeviceKind::Router(_))) =>
            {
                PowerCordKind::Cisco66WAdapter
            }
            _ => PowerCordKind::IecC13C14,
        }
    }

    fn validate_power_cord(
        &self,
        endpoint: PowerEndpoint,
        kind: PowerCordKind,
    ) -> Result<(), SimError> {
        if matches!(endpoint, PowerEndpoint::Source(_))
            && matches!(kind, PowerCordKind::Cisco66WAdapter)
        {
            return Err(SimError::Power(
                "Cisco adapter cords terminate at a device, not a power source".into(),
            ));
        }
        if let PowerEndpoint::Device(id) = endpoint {
            let d = self.devices.get(&id).ok_or(SimError::DeviceNotFound(id))?;
            let router = matches!(d.kind, DeviceKind::Router(_));
            if router != matches!(kind, PowerCordKind::Cisco66WAdapter) {
                return Err(SimError::Power("Cisco routers require the supplied 66 W DC adapter; other devices require an IEC C13/C14 cord".into()));
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
                route: Vec::new(),
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
        if !self.devices.contains_key(&id) {
            return Err(SimError::DeviceNotFound(id));
        }
        let source = match &self.devices[&id].kind {
            DeviceKind::Ups(x) => x.source,
            DeviceKind::Pdu(x) => x.source,
            _ => None,
        };
        if let Some(source) = source {
            self.power.set_source_enabled(source, powered);
            self.sync_effective_power();
            return Ok(());
        }
        self.power
            .set_requested(id, powered)
            .map_err(|e| SimError::Power(e.to_string()))?;
        self.sync_effective_power();
        Ok(())
    }

    fn sync_effective_power(&mut self) {
        let mut changed = false;
        for (id, device) in &mut self.devices {
            let source = match &device.kind {
                DeviceKind::Ups(x) => x.source,
                DeviceKind::Pdu(x) => x.source,
                _ => None,
            };
            if let Some(source) = source {
                let next = device.rack.is_some()
                    && self
                        .power
                        .source_telemetry(source)
                        .is_some_and(|t| t.available);
                changed |= device.powered != next;
                device.powered = next;
                continue;
            }
            if let Some(status) = self.power.device_status(*id) {
                let next = device.rack.is_some() && status.effective;
                changed |= device.powered != next;
                device.powered = next;
            }
        }
        if changed {
            self.topology_revision = self.topology_revision.saturating_add(1);
        }
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
            PortConfig::PatchPanel | PortConfig::CableManager => {}
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

    fn set_port_enabled(&mut self, id: PortId, enabled: bool) -> Result<(), SimError> {
        let port = self.ports.get_mut(&id).ok_or(SimError::PortNotFound(id))?;
        if !matches!(port.config, PortConfig::Server(_)) {
            return Err(SimError::WrongPortType);
        }
        port.enabled = enabled;
        self.routing_revision += 1;
        Ok(())
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
    fn validate_route_point(&self, point: &CableRoutePoint) -> Result<(), SimError> {
        let rack = self
            .racks
            .get(&point.rack)
            .ok_or(SimError::RackNotFound(point.rack))?;
        if point.unit == 0 || point.unit > rack.units || point.offset_cm > 48 {
            return Err(SimError::RackPlacementOutOfBounds);
        }
        Ok(())
    }
    fn add_cable_route_point(
        &mut self,
        link: LinkId,
        point: CableRoutePoint,
    ) -> Result<(), SimError> {
        self.validate_route_point(&point)?;
        self.links
            .get_mut(&link)
            .ok_or(SimError::LinkNotFound)?
            .route
            .push(point);
        Ok(())
    }
    fn remove_cable_route_point(&mut self, link: LinkId, index: usize) -> Result<(), SimError> {
        let r = &mut self
            .links
            .get_mut(&link)
            .ok_or(SimError::LinkNotFound)?
            .route;
        if index >= r.len() {
            return Err(SimError::LinkNotFound);
        }
        r.remove(index);
        Ok(())
    }
    fn move_cable_route_point(
        &mut self,
        link: LinkId,
        index: usize,
        point: CableRoutePoint,
    ) -> Result<(), SimError> {
        self.validate_route_point(&point)?;
        let r = &mut self
            .links
            .get_mut(&link)
            .ok_or(SimError::LinkNotFound)?
            .route;
        if index >= r.len() {
            return Err(SimError::LinkNotFound);
        }
        r[index] = point;
        Ok(())
    }
    fn reroute_cable(&mut self, link: LinkId, route: Vec<CableRoutePoint>) -> Result<(), SimError> {
        for p in &route {
            self.validate_route_point(p)?;
        }
        self.links
            .get_mut(&link)
            .ok_or(SimError::LinkNotFound)?
            .route = route;
        Ok(())
    }
}

fn canonical_network(address: Ipv4Addr, prefix: u8) -> Ipv4Addr {
    if prefix == 0 {
        return Ipv4Addr::UNSPECIFIED;
    }
    Ipv4Addr::from(u32::from(address) & (u32::MAX << (32 - prefix)))
}

#[cfg(test)]
mod route_tests {
    use super::*;
    #[test]
    fn route_commands_preserve_link_endpoints() {
        let mut sim = NetworkSim::new();
        let id = LinkId(77);
        sim.links.insert(
            id,
            Link {
                id,
                a: PortId(1),
                b: PortId(2),
                enabled: true,
                length_cm: 1,
                auto_length: false,
                color: CableColor::White,
                route: vec![],
            },
        );
        let point = CableRoutePoint {
            rack: RackId(1),
            unit: 2,
            side: RackSide::Front,
            offset_cm: 48,
        };
        sim.execute(Command::AddCableRoutePoint { link: id, point })
            .unwrap();
        sim.execute(Command::MoveCableRoutePoint {
            link: id,
            index: 0,
            point,
        })
        .unwrap();
        sim.execute(Command::RerouteCable {
            link: id,
            route: vec![point],
        })
        .unwrap();
        sim.execute(Command::RemoveCableRoutePoint { link: id, index: 0 })
            .unwrap();
        assert_eq!((sim.links[&id].a, sim.links[&id].b), (PortId(1), PortId(2)));
        assert!(
            sim.execute(Command::AddCableRoutePoint {
                link: id,
                point: CableRoutePoint {
                    offset_cm: 49,
                    ..point
                }
            })
            .is_err()
        );
    }

    #[test]
    fn routed_connection_persists_its_route_when_created() {
        let mut sim = NetworkSim::new();
        let rack = RackId(1);
        let first = sim.buy_device(DeviceTemplate::Switch).unwrap();
        let second = sim.buy_device(DeviceTemplate::Switch).unwrap();
        sim.place_device(first, rack, 1).unwrap();
        sim.place_device(second, rack, 2).unwrap();
        sim.buy_cable_supply(CableSupply::CableBox305m).unwrap();
        sim.buy_cable_supply(CableSupply::Rj45Pack20).unwrap();
        let a = sim.device(first).unwrap().ports()[0];
        let b = sim.device(second).unwrap().ports()[0];
        let route = vec![CableRoutePoint {
            rack,
            unit: 1,
            side: RackSide::Front,
            offset_cm: 0,
        }];
        sim.execute(Command::ConnectRoutedColoredCable {
            a,
            b,
            length_cm: None,
            color: CableColor::Blue,
            route: route.clone(),
        })
        .unwrap();
        let link = sim.link_for_port(a).unwrap();
        assert_eq!(link.route, route);
        assert_eq!(link.color, CableColor::Blue);
        assert_eq!(
            link.length_cm,
            sim.minimum_routed_cable_length(a, b, &route).unwrap()
        );
    }
}
