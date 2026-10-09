use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSim {
    #[serde(default)]
    pub optics: OpticsState,
    #[serde(default)]
    pub(crate) provider: ProviderNetwork,
    pub(crate) devices: HashMap<DeviceId, Device>,
    pub(crate) ports: HashMap<PortId, Port>,
    pub(crate) links: HashMap<LinkId, Link>,
    pub(crate) racks: HashMap<RackId, Rack>,
    #[serde(default)]
    pub room: DataCenterRoom,
    #[serde(default)]
    pub(crate) network_outlets: Vec<NetworkOutlet>,
    #[serde(default)]
    pub(crate) public_ipv4_blocks: Vec<PublicIpv4Block>,
    #[serde(default)]
    pub(crate) server_operating_systems: HashMap<DeviceId, ServerOs>,
    #[serde(default)]
    pub power: PowerSystem,
    #[serde(default)]
    pub(crate) device_workloads: HashMap<DeviceId, DeviceWorkload>,
    pub money: i64,
    #[serde(default)]
    pub(crate) cable_inventory: CableInventory,
    #[serde(default)]
    pub server_parts: HashMap<String, u32>,
    #[serde(default)]
    pub drive_inventory: HashMap<String, u32>,
    pub topology_revision: u64,
    pub routing_revision: u64,
    #[serde(default)]
    pub(crate) ios_configs: HashMap<DeviceId, IosDeviceConfig>,
    #[serde(default)]
    pub(crate) startup_configs: HashMap<DeviceId, IosStartupConfig>,
    #[serde(skip)]
    pub(crate) console_modes: HashMap<DeviceId, IosMode>,
    #[serde(skip)]
    pub(crate) ssh_sessions: HashMap<DeviceId, DeviceId>,
    next_device_id: u64,
    next_port_id: u64,
    pub(crate) next_link_id: u64,
    next_rack_id: u64,
    #[serde(skip)]
    pub(crate) port_links: HashMap<PortId, LinkId>,
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
            optics: OpticsState::default(),
            provider: ProviderNetwork::default(),
            devices: HashMap::new(),
            ports: HashMap::new(),
            links: HashMap::new(),
            racks: HashMap::new(),
            room: DataCenterRoom::default(),
            network_outlets: Vec::new(),
            public_ipv4_blocks: Vec::new(),
            server_operating_systems: HashMap::new(),
            power: PowerSystem::new(),
            device_workloads: HashMap::new(),
            money: 6_000,
            cable_inventory: CableInventory::default(),
            server_parts: HashMap::new(),
            drive_inventory: HashMap::new(),
            topology_revision: 0,
            routing_revision: 0,
            ios_configs: HashMap::new(),
            startup_configs: HashMap::new(),
            console_modes: HashMap::new(),
            ssh_sessions: HashMap::new(),
            next_device_id: 1,
            next_port_id: 1,
            next_link_id: 1,
            next_rack_id: 1,
            port_links: HashMap::new(),
            runtime: NetworkRuntime::default(),
        };
        sim.ensure_predefined_room();
        sim.ensure_network_outlets();
        sim.migrate_provider_inventory();
        sim
    }

    pub fn rebuild_indexes(&mut self) {
        self.device_workloads
            .retain(|id, _| self.devices.contains_key(id));
        for workload in self.device_workloads.values_mut() {
            workload.cpu = workload.cpu.min(1000);
            workload.memory = workload.memory.min(1000);
            workload.storage = workload.storage.min(1000);
        }
        self.normalize_switch_models();
        self.migrate_legacy_sfp_nic();
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
        self.power
            .cord_routes
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
                self.power.add_device(*id, 0, self.device_power_factor(*id));
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
            match &self.devices[&id].kind {
                DeviceKind::Ups(ups) => {
                    if let Some(SourceId::Ups(source)) = ups.source
                        && let Some(state) = self.power.ups.get_mut(&source)
                    {
                        state.spec = UpsSpec::default();
                        state.battery_mwh = state
                            .battery_mwh
                            .min(u64::from(state.spec.battery_wh) * 1000);
                        state.battery_wh = (state.battery_mwh / 1000) as u32;
                    }
                }
                DeviceKind::Pdu(pdu) => {
                    if let Some(SourceId::Pdu(source)) = pdu.source
                        && let Some(state) = self.power.pdus.get_mut(&source)
                    {
                        *state = PduState {
                            enabled: state.enabled,
                            tripped: state.tripped,
                            ..PduState::default()
                        };
                    }
                }
                _ => {}
            }
        }
        self.refresh_device_loads();
        self.power.recompute_now();
        self.sync_effective_power();
        // Migrate legacy servers that predate the dedicated management NIC.
        let legacy_servers: Vec<_> =
            self.devices
                .iter()
                .filter_map(|(id, device)| match &device.kind {
                    DeviceKind::Server(server)
                        if !server.ports.iter().any(|id| {
                            self.ports.get(id).is_some_and(|port| port.name == "mgmt0")
                        }) =>
                    {
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
            while current < 2 {
                let name = if !existing.contains("eth1") {
                    "eth1"
                } else {
                    "eth0"
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
            let port = self.alloc_port(
                device,
                "mgmt0".into(),
                PortConnector::Rj45,
                PortConfig::Server(ServerPortConfig::default()),
            );
            if let DeviceKind::Server(server) = &mut self.devices.get_mut(&device).unwrap().kind {
                server.ports.push(port);
            }
        }
        for device in self.devices.values_mut() {
            if let DeviceKind::Server(server) = &mut device.kind
                && let Some(hardware) = &mut server.hardware
            {
                hardware.normalize_dimm_slots();
                let slots = server_catalog().chassis.pcie_slots.len();
                if hardware.pcie.len() < slots {
                    hardware.pcie.resize(slots, None);
                }
                if hardware.card_ports.len() < hardware.pcie.len() {
                    hardware
                        .card_ports
                        .resize_with(hardware.pcie.len(), Vec::new);
                }
                if hardware.drives.len() < server_catalog().chassis.drive_bays.len() {
                    hardware
                        .drives
                        .resize(server_catalog().chassis.drive_bays.len(), None);
                }
            }
        }
        // Fans were sold as parts in older saves. Return their full purchase
        // price once; the chassis now includes cooling by default.
        let mut retired_fans = self.server_parts.remove("r360_fan").unwrap_or(0) as usize;
        for device in self.devices.values_mut() {
            if let DeviceKind::Server(server) = &mut device.kind
                && let Some(hardware) = &mut server.hardware
            {
                retired_fans += hardware.legacy_fans.len();
                hardware.legacy_fans.clear();
            }
        }
        self.money += retired_fans as i64 * 45;
        // The chassis now includes its PSU. Refund stand-alone PSUs from old saves once.
        let mut retired_psus = self.server_parts.remove("r360_psu_600w").unwrap_or(0) as usize;
        for device in self.devices.values_mut() {
            if let DeviceKind::Server(server) = &mut device.kind
                && let Some(hardware) = &mut server.hardware
            {
                retired_psus += hardware.power_supplies.len();
                hardware.power_supplies.clear();
            }
        }
        self.money += retired_psus as i64 * 180;
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
        for port in self.ports.values_mut() {
            if let PortConfig::Router(config) = &mut port.config {
                crate::normalize_router_interfaces(&mut config.interfaces);
            }
        }
        for device in self.devices.values_mut() {
            if let DeviceKind::Router(router) = &mut device.kind {
                crate::normalize_router_interfaces(&mut router.interfaces);
            }
        }
        for saved in self.startup_configs.values_mut() {
            saved.normalize_interfaces();
        }
        self.ensure_predefined_room();
        self.ensure_network_outlets();
        self.reconcile_provider();
        self.migrate_provider_inventory();
        self.normalize_optics();
    }

    fn ensure_network_outlets(&mut self) {
        self.network_outlets.retain(|outlet| {
            self.ports
                .get(&outlet.port)
                .is_some_and(|port| matches!(port.config, PortConfig::Infrastructure))
        });
        for (number, position) in [
            RoomPosition { x_cm: 75, y_cm: 90 },
            RoomPosition {
                x_cm: 145,
                y_cm: 90,
            },
        ]
        .into_iter()
        .enumerate()
        {
            if !self.network_outlets.iter().any(|outlet| {
                matches!(outlet.kind, NetworkOutletKind::Uplink { .. })
                    && self.port(outlet.port).is_some()
                    && self
                        .port(outlet.port)
                        .is_some_and(|p| p.name == format!("UPLINK {}", number + 1))
            }) {
                self.add_network_outlet(
                    NetworkOutletKind::Uplink { position },
                    format!("UPLINK {}", number + 1),
                );
            }
        }
        let racks: Vec<_> = self.racks.keys().copied().collect();
        for rack in racks {
            if !self.network_outlets.iter().any(|outlet| {
                outlet.kind == NetworkOutletKind::Lan { rack } && self.port(outlet.port).is_some()
            }) {
                self.add_network_outlet(
                    NetworkOutletKind::Lan { rack },
                    format!("R{} LAN", rack.0),
                );
            }
        }
    }

    fn add_network_outlet(&mut self, kind: NetworkOutletKind, name: String) {
        let port = self.alloc_port(
            NetworkOutlet::OWNER,
            name,
            PortConnector::Rj45,
            PortConfig::Infrastructure,
        );
        self.network_outlets.push(NetworkOutlet { port, kind });
    }

    pub fn network_outlet(&self, port: PortId) -> Option<&NetworkOutlet> {
        self.network_outlets
            .iter()
            .find(|outlet| outlet.port == port)
    }
    pub fn network_outlets(&self) -> impl Iterator<Item = &NetworkOutlet> {
        self.network_outlets.iter()
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
        self.link_status(id).speed.is_some()
    }
    /// Returns the resolved physical rate from the shared L1 evaluator.
    pub fn port_link_speed(&self, id: PortId) -> Option<LinkSpeed> {
        self.link_status(id).speed
    }

    /// Advance the deterministic packet clock used for port activity LEDs.
    pub fn advance_time(&mut self, ms: u64) {
        let mut remaining = ms;
        while remaining > 0 {
            self.refresh_device_loads();
            self.sync_effective_power();
            let step = if self.runtime.power_activity.pending() {
                remaining.min(100 - self.simulation_time_ms() % 100)
            } else {
                remaining
            };
            self.power.tick_ms(step);
            self.runtime.advance_time(step);
            self.sync_effective_power();
            remaining -= step;
        }
        self.refresh_device_loads();
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
        self.room
            .rack_positions
            .insert(id, predefined_rack_position(id));
        id
    }

    fn ensure_predefined_room(&mut self) {
        self.room.width_cm = 1600;
        self.room.depth_cm = 2800;
        for number in 1..=DATACENTER_RACK_COUNT {
            let id = RackId(number);
            let rack = self.racks.entry(id).or_insert_with(|| Rack {
                id,
                name: format!("Rack {number:02}"),
                units: 42,
                placements: Vec::new(),
            });
            rack.units = 42;
            self.room
                .rack_positions
                .insert(id, predefined_rack_position(id));
            if !self.power.racks.contains_key(&id) {
                self.power.add_rack(id);
            }
        }
        for fixed_anchor in RoomCableLayout::anchors() {
            if let Some(anchor) = self
                .room
                .cable_anchors
                .iter_mut()
                .find(|anchor| anchor.id == fixed_anchor.id)
            {
                anchor.position = fixed_anchor.position;
            } else {
                self.room.cable_anchors.push(fixed_anchor);
            }
        }
        self.next_rack_id = self.next_rack_id.max(DATACENTER_RACK_COUNT + 1);
    }

    pub fn rack_room_position(&self, id: RackId) -> RoomPosition {
        if id.0 <= DATACENTER_RACK_COUNT {
            predefined_rack_position(id)
        } else {
            self.room
                .rack_positions
                .get(&id)
                .copied()
                .unwrap_or(predefined_rack_position(id))
        }
    }

    pub fn execute(&mut self, command: Command) -> Result<Vec<SimEvent>, SimError> {
        let mut events = match command {
            Command::Purchase { item, quantity } => return self.purchase_quantity(item, quantity),
            Command::Optics(command) => self.configure_optics(command)?,
            Command::Provider(command) => {
                let router = match &command {
                    crate::ProviderCommand::SetDomainRoute(route)
                    | crate::ProviderCommand::RemoveDomainRoute(route)
                    | crate::ProviderCommand::ReplaceDomainRoute { route, .. } => {
                        Some(route.router)
                    }
                    _ => None,
                };
                self.configure_provider(command)?;
                router.map_or_else(
                    || vec![SimEvent::ConnectivityChanged],
                    |id| {
                        vec![
                            SimEvent::RouterRoutesChanged(id),
                            SimEvent::ConnectivityChanged,
                        ]
                    },
                )
            }
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
            Command::BuyServerChassis => {
                let id = self.buy_server_chassis()?;
                vec![SimEvent::DeviceAdded(id)]
            }
            Command::BuyServerFullPack => {
                let id = self.buy_server_full_pack()?;
                vec![SimEvent::DeviceAdded(id)]
            }
            Command::BuyPublicIpv4Pool => {
                self.buy_public_ipv4_pool()?;
                vec![SimEvent::ConnectivityChanged]
            }
            Command::BuyPublicIpv4Block { uplink } => {
                self.buy_public_ipv4_block(uplink)?;
                vec![SimEvent::ConnectivityChanged]
            }
            Command::AssignPublicIpv4 { port, network } => {
                self.assign_public_ipv4(port, network)?;
                vec![SimEvent::PortConfigChanged(port)]
            }
            Command::AssignLanIpv4 { port } => {
                self.assign_lan_ipv4(port)?;
                vec![SimEvent::PortConfigChanged(port)]
            }
            Command::BuyServerPart { part_id } => {
                self.buy_server_part(&part_id)?;
                vec![SimEvent::ConnectivityChanged]
            }
            Command::BuyDrive { drive_id } => {
                self.buy_drive(&drive_id)?;
                vec![SimEvent::ConnectivityChanged]
            }
            Command::InstallDrive {
                device,
                drive_id,
                bay,
            } => {
                self.install_drive(device, &drive_id, bay)?;
                vec![SimEvent::ConnectivityChanged]
            }
            Command::RemoveDrive { device, bay } => {
                self.remove_drive(device, bay)?;
                vec![SimEvent::ConnectivityChanged]
            }
            Command::InstallServerPart {
                device,
                part_id,
                slot,
            } => {
                self.install_server_part(device, &part_id, slot)?;
                vec![SimEvent::ConnectivityChanged]
            }
            Command::RemoveServerPart {
                device,
                part_id,
                slot,
            } => {
                self.remove_server_part(device, &part_id, slot)?;
                vec![SimEvent::ConnectivityChanged]
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
            Command::SetRouterSwitchport { port, switchport } => {
                let device = self
                    .ports
                    .get(&port)
                    .ok_or(SimError::PortNotFound(port))?
                    .device;
                self.ios_set_switchport(device, port, switchport)
                    .map_err(|e| SimError::Provider(e.trim_start_matches("% ").into()))?;
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
            Command::SetDeviceWorkload { device, workload } => {
                self.set_device_workload(device, workload)?;
                vec![SimEvent::PowerChanged]
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
            Command::ConnectPowerRouted {
                outlet,
                endpoint,
                route,
            } => {
                for point in &route {
                    self.validate_route_point(point)?;
                }
                let kind = self.inferred_power_cord(endpoint);
                self.validate_power_cord(endpoint, kind)?;
                self.validate_power_endpoint(outlet.source, endpoint)?;
                self.power
                    .connect_with_kind(outlet, endpoint, kind)
                    .map_err(|e| SimError::Power(e.to_string()))?;
                if !route.is_empty() {
                    self.power.cord_routes.insert(outlet, route);
                }
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
            Command::ReroutePowerCable { outlet, route } => {
                if !self.power.connections.contains_key(&outlet) {
                    return Err(SimError::Power("power cable is not connected".into()));
                }
                for point in &route {
                    self.validate_route_point(point)?;
                }
                self.power.cord_routes.insert(outlet, route);
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
                    | SimEvent::ConnectivityChanged
            )
        }) {
            self.topology_revision += 1;
            events.push(SimEvent::TopologyChanged {
                revision: self.topology_revision,
            });
            if !events
                .iter()
                .any(|e| matches!(e, SimEvent::ConnectivityChanged))
            {
                events.push(SimEvent::ConnectivityChanged);
            }
        }
        self.reconcile_provider();
        self.refresh_device_loads();
        self.sync_effective_power();
        Ok(events)
    }

    pub(crate) fn alloc_port(
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

    pub(crate) fn buy_device(&mut self, template: DeviceTemplate) -> Result<DeviceId, SimError> {
        self.buy_device_at_price(template, template.price())
    }

    pub(crate) fn buy_device_at_price(
        &mut self,
        template: DeviceTemplate,
        price: i64,
    ) -> Result<DeviceId, SimError> {
        self.buy_device_with_switch_model(template, price, SwitchModel::default())
    }

    pub(crate) fn buy_device_with_switch_model(
        &mut self,
        template: DeviceTemplate,
        price: i64,
        switch_model: SwitchModel,
    ) -> Result<DeviceId, SimError> {
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
                let ports = ["eth0", "eth1", "mgmt0"]
                    .into_iter()
                    .map(|name| {
                        self.alloc_port(
                            id,
                            name.into(),
                            PortConnector::Rj45,
                            PortConfig::Server(ServerPortConfig::default()),
                        )
                    })
                    .collect();
                (
                    format!("{} #{index:02}", server_catalog().chassis.model),
                    DeviceKind::Server(Server {
                        hostname: format!("server{index:02}"),
                        ports,
                        hardware: None,
                    }),
                )
            }
            DeviceTemplate::Switch => {
                let spec = switch_model.spec();
                let mut ports: Vec<_> = (1..=spec.copper_ports)
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
                ports.extend((1..=spec.uplinks).map(|n| {
                    let speed = spec.cage.modes.iter().map(|mode| mode.speed).max().unwrap();
                    let name = if speed == LinkSpeed::Gbps10 {
                        format!("Te1/0/{n:02}")
                    } else {
                        format!("Gi1/0/{:02}", spec.copper_ports + n)
                    };
                    let port = self.alloc_port(
                        id,
                        name,
                        PortConnector::Sfp,
                        PortConfig::Switch(SwitchPortConfig {
                            mode: SwitchPortMode::Trunk {
                                native_vlan: Some(VlanId(1)),
                                allowed: vec![VlanId(1)],
                            },
                        }),
                    );
                    let p = self.ports.get_mut(&port).unwrap();
                    p.max_speed = speed;
                    p.advertised_speed = speed;
                    p.side = RackSide::Front;
                    self.optics.cages.insert(port, spec.cage.clone());
                    port
                }));
                (
                    format!("{} #{index:02}", spec.name),
                    DeviceKind::Switch(Switch {
                        model: switch_model,
                        services: SwitchServices::default(),
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
                let profile = router_network_profile();
                for index in 0..profile.wan_ports.len() + profile.lan_ports.len() {
                    let wan = profile.wan_ports.iter().position(|i| *i == index);
                    let name = wan.map(|n| format!("WAN{}", n + 1)).unwrap_or_else(|| {
                        format!(
                            "LAN{}",
                            profile.lan_ports.iter().position(|i| *i == index).unwrap() + 1
                        )
                    });
                    ports.push(self.alloc_port(
                        id,
                        name,
                        PortConnector::Rj45,
                        if wan.is_some() {
                            PortConfig::Router(RouterPortConfig::default())
                        } else {
                            PortConfig::Switch(SwitchPortConfig {
                                mode: SwitchPortMode::Access { vlan: None },
                            })
                        },
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
                        domain_routes: Vec::new(),
                        vlans: crate::device::default_router_vlans(),
                        svi_ports: Vec::new(),
                        routing_enabled: profile.routing_enabled,
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
                self.power.add_device(id, 0, self.device_power_factor(id));
            }
        }
        Ok(id)
    }

    fn buy_server_chassis(&mut self) -> Result<DeviceId, SimError> {
        let id = self.buy_device(DeviceTemplate::Server)?;
        let DeviceKind::Server(server) = &mut self.devices.get_mut(&id).unwrap().kind else {
            unreachable!()
        };
        server.hardware = Some(ServerHardware {
            pcie: vec![None; server_catalog().chassis.pcie_slots.len()],
            card_ports: vec![Vec::new(); server_catalog().chassis.pcie_slots.len()],
            drives: vec![None; server_catalog().chassis.drive_bays.len()],
            ..Default::default()
        });
        self.update_server_load(id);
        Ok(id)
    }

    fn buy_server_full_pack(&mut self) -> Result<DeviceId, SimError> {
        let price = ServerFullPack::price();
        if self.money < price {
            return Err(SimError::InsufficientFunds {
                needed: price,
                available: self.money,
            });
        }
        let mut purchase = self.clone();
        let id = purchase.buy_server_chassis()?;
        for part in [
            ServerFullPack::CPU,
            ServerFullPack::RAM,
            ServerFullPack::NIC,
        ] {
            purchase.buy_server_part(part)?;
            purchase.install_server_part(id, part, None)?;
        }
        purchase.buy_drive(ServerFullPack::DRIVE)?;
        purchase.install_drive(id, ServerFullPack::DRIVE, None)?;
        *self = purchase;
        Ok(id)
    }

    fn buy_server_part(&mut self, part_id: &str) -> Result<(), SimError> {
        let part = server_catalog()
            .parts
            .iter()
            .find(|p| p.id == part_id)
            .ok_or_else(|| SimError::UnknownServerPart(part_id.into()))?;
        if self.money < part.price {
            return Err(SimError::InsufficientFunds {
                needed: part.price,
                available: self.money,
            });
        }
        self.money -= part.price;
        *self.server_parts.entry(part_id.into()).or_default() += 1;
        Ok(())
    }

    fn buy_drive(&mut self, drive_id: &str) -> Result<(), SimError> {
        let drive = drive_catalog()
            .drives
            .iter()
            .find(|d| d.id == drive_id)
            .ok_or_else(|| SimError::UnknownDrive(drive_id.into()))?;
        if self.money < drive.price {
            return Err(SimError::InsufficientFunds {
                needed: drive.price,
                available: self.money,
            });
        }
        self.money -= drive.price;
        *self.drive_inventory.entry(drive_id.into()).or_default() += 1;
        Ok(())
    }

    fn install_drive(
        &mut self,
        device: DeviceId,
        drive_id: &str,
        bay: Option<usize>,
    ) -> Result<(), SimError> {
        drive_catalog()
            .drives
            .iter()
            .find(|d| d.id == drive_id)
            .ok_or_else(|| SimError::UnknownDrive(drive_id.into()))?;
        if self.drive_inventory.get(drive_id).copied().unwrap_or(0) == 0 {
            return Err(SimError::DriveNotOwned(drive_id.into()));
        }
        let index = self.drive_installation_bay(device, drive_id, bay)?;
        let DeviceKind::Server(server) = &mut self.devices.get_mut(&device).unwrap().kind else {
            unreachable!()
        };
        server.hardware.as_mut().unwrap().drives[index] = Some(drive_id.into());
        *self.drive_inventory.get_mut(drive_id).unwrap() -= 1;
        self.update_server_load(device);
        self.sync_effective_power();
        Ok(())
    }

    fn remove_drive(&mut self, device: DeviceId, bay: usize) -> Result<(), SimError> {
        let DeviceKind::Server(server) = &mut self
            .devices
            .get_mut(&device)
            .ok_or(SimError::DeviceNotFound(device))?
            .kind
        else {
            return Err(SimError::ServerHardware(
                "selected device is not a server".into(),
            ));
        };
        let drive = server
            .hardware
            .as_mut()
            .and_then(|h| h.drives.get_mut(bay))
            .and_then(Option::take)
            .ok_or_else(|| SimError::ServerHardware("drive bay is empty".into()))?;
        *self.drive_inventory.entry(drive).or_default() += 1;
        self.update_server_load(device);
        self.sync_effective_power();
        Ok(())
    }

    fn install_server_part(
        &mut self,
        device: DeviceId,
        part_id: &str,
        slot: Option<usize>,
    ) -> Result<(), SimError> {
        if self.devices.get(&device).is_some_and(|d| d.powered)
            && !server_catalog().chassis.parts_hot_swappable
        {
            return Err(SimError::ServerHardware(
                "power off the server before changing CPU, RAM or PCIe cards".into(),
            ));
        }
        let catalog = server_catalog();
        let part = catalog
            .parts
            .iter()
            .find(|p| p.id == part_id)
            .ok_or_else(|| SimError::UnknownServerPart(part_id.into()))?;
        if self.server_parts.get(part_id).copied().unwrap_or(0) == 0 {
            return Err(SimError::ServerPartNotOwned(part_id.into()));
        }
        let selected_slot = self.server_part_installation_slot(device, part_id, slot)?;
        let DeviceKind::Server(server) = &self.devices[&device].kind else {
            unreachable!()
        };
        let mut new_ports = Vec::new();
        if let ServerPartKind::PciCard {
            card:
                PciCard::Ethernet {
                    ports,
                    speed_mbps,
                    connector,
                    cage,
                    ..
                },
        } = &part.kind
        {
            let start = server
                .ports
                .iter()
                .filter_map(|id| self.ports.get(id))
                .filter_map(|port| port.name.strip_prefix("eth")?.parse::<usize>().ok())
                .max()
                .map_or(0, |index| index + 1);
            let speed = LinkSpeed::from_mbps(*speed_mbps).expect("validated NIC speed");
            for index in 0..*ports {
                let port = self.alloc_port(
                    device,
                    format!("eth{}", start + usize::from(index)),
                    *connector,
                    PortConfig::Server(ServerPortConfig::default()),
                );
                self.ports.get_mut(&port).unwrap().max_speed = speed;
                self.ports.get_mut(&port).unwrap().advertised_speed = speed;
                if *connector == PortConnector::Sfp {
                    self.optics.cages.insert(
                        port,
                        cage.clone()
                            .unwrap_or_else(|| CageProfile::for_speed(speed)),
                    );
                }
                new_ports.push(port);
            }
        }
        let DeviceKind::Server(server) = &mut self.devices.get_mut(&device).unwrap().kind else {
            unreachable!()
        };
        let hardware = server.hardware.as_mut().unwrap();
        match &part.kind {
            ServerPartKind::Cpu { .. } => hardware.cpus.push(part_id.into()),
            ServerPartKind::Ram { .. } => {
                hardware.normalize_dimm_slots();
                hardware.ram.push(part_id.into());
                hardware.ram_slot_indices.push(selected_slot.unwrap());
            }
            ServerPartKind::PowerSupply { .. } => hardware.power_supplies.push(part_id.into()),
            ServerPartKind::PciCard { .. } => {
                let index = selected_slot.unwrap();
                hardware.pcie[index] = Some(part_id.into());
                hardware.card_ports[index] = new_ports.clone();
                server.ports.extend(new_ports);
            }
        }
        *self.server_parts.get_mut(part_id).unwrap() -= 1;
        self.topology_revision += 1;
        self.update_server_load(device);
        self.sync_effective_power();
        Ok(())
    }

    fn remove_server_part(
        &mut self,
        device: DeviceId,
        part_id: &str,
        slot: Option<usize>,
    ) -> Result<(), SimError> {
        if self.devices.get(&device).is_some_and(|d| d.powered)
            && !server_catalog().chassis.parts_hot_swappable
        {
            return Err(SimError::ServerHardware(
                "power off the server before changing CPU, RAM or PCIe cards".into(),
            ));
        }
        let server = match &self
            .devices
            .get(&device)
            .ok_or(SimError::DeviceNotFound(device))?
            .kind
        {
            DeviceKind::Server(server) => server,
            _ => {
                return Err(SimError::ServerHardware(
                    "selected device is not a server".into(),
                ));
            }
        };
        let hardware = server.hardware.as_ref().ok_or_else(|| {
            SimError::ServerHardware("legacy server has no configurable chassis".into())
        })?;
        let part = server_catalog()
            .parts
            .iter()
            .find(|p| p.id == part_id)
            .ok_or_else(|| SimError::UnknownServerPart(part_id.into()))?;
        let pci_slot = if matches!(part.kind, ServerPartKind::PciCard { .. }) {
            Some(
                slot.or_else(|| {
                    hardware
                        .pcie
                        .iter()
                        .position(|p| p.as_deref() == Some(part_id))
                })
                .filter(|&i| hardware.pcie.get(i).and_then(Option::as_deref) == Some(part_id))
                .ok_or_else(|| {
                    SimError::ServerHardware("card is not installed in that slot".into())
                })?,
            )
        } else {
            None
        };
        if matches!(part.kind, ServerPartKind::Cpu { .. })
            && hardware.pcie.iter().any(Option::is_some)
        {
            return Err(SimError::ServerHardware(
                "remove PCIe cards before removing the CPU".into(),
            ));
        }
        let ports = pci_slot
            .map(|i| hardware.card_ports[i].clone())
            .unwrap_or_default();
        for port in &ports {
            self.detach_module(*port);
            if let Some(link) = self.port_links.get(port).copied() {
                self.disconnect(link)?;
            }
        }
        let DeviceKind::Server(server) = &mut self.devices.get_mut(&device).unwrap().kind else {
            unreachable!()
        };
        let hardware = server.hardware.as_mut().unwrap();
        hardware.normalize_dimm_slots();
        let ram_index = if matches!(part.kind, ServerPartKind::Ram { .. }) {
            Some(
                hardware
                    .ram
                    .iter()
                    .enumerate()
                    .position(|(i, id)| {
                        id == part_id && slot.is_none_or(|s| hardware.ram_slot_indices[i] == s)
                    })
                    .ok_or_else(|| {
                        SimError::ServerHardware("DIMM is not installed in that slot".into())
                    })?,
            )
        } else {
            None
        };
        let list = match part.kind {
            ServerPartKind::Cpu { .. } => Some(&mut hardware.cpus),
            ServerPartKind::Ram { .. } => Some(&mut hardware.ram),
            ServerPartKind::PowerSupply { .. } => Some(&mut hardware.power_supplies),
            ServerPartKind::PciCard { .. } => None,
        };
        if let Some(list) = list {
            let index = ram_index
                .or_else(|| list.iter().position(|p| p == part_id))
                .ok_or_else(|| SimError::ServerHardware("part is not installed".into()))?;
            list.remove(index);
            if ram_index.is_some() {
                hardware.ram_slot_indices.remove(index);
            }
        } else {
            let index = pci_slot.unwrap();
            hardware.pcie[index] = None;
            hardware.card_ports[index].clear();
            server.ports.retain(|p| !ports.contains(p));
            for port in ports {
                self.ports.remove(&port);
                self.port_links.remove(&port);
            }
        }
        *self.server_parts.entry(part_id.into()).or_default() += 1;
        self.topology_revision += 1;
        self.update_server_load(device);
        self.sync_effective_power();
        Ok(())
    }

    fn update_server_load(&mut self, _device: DeviceId) {
        self.refresh_device_loads();
    }

    fn sell_device(&mut self, id: DeviceId) -> Result<(), SimError> {
        let device = self.devices.get(&id).ok_or(SimError::DeviceNotFound(id))?;
        if device.rack.is_some() {
            return Err(SimError::DeviceInstalled);
        }
        let ports = self.ios_ports(id);
        let source = match &device.kind {
            DeviceKind::Ups(x) => x.source,
            DeviceKind::Pdu(x) => x.source,
            _ => None,
        };
        for port in &ports {
            self.detach_module(*port);
            if let Some(link) = self.port_links.get(port).copied() {
                self.disconnect(link)?;
            }
        }
        let device = self.devices.remove(&id).expect("checked");
        if let DeviceKind::Server(server) = &device.kind
            && let Some(hardware) = &server.hardware
        {
            for part in hardware
                .cpus
                .iter()
                .chain(&hardware.ram)
                .chain(&hardware.power_supplies)
                .chain(hardware.pcie.iter().flatten())
            {
                *self.server_parts.entry(part.clone()).or_default() += 1;
            }
            for drive in hardware.drives.iter().flatten() {
                *self.drive_inventory.entry(drive.clone()).or_default() += 1;
            }
        }
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
        self.power
            .cord_routes
            .retain(|outlet, _| self.power.connections.contains_key(outlet));
        self.server_operating_systems.remove(&id);
        self.ssh_sessions
            .retain(|source, target| *source != id && *target != id);
        self.ios_configs.remove(&id);
        self.startup_configs.remove(&id);
        self.device_workloads.remove(&id);
        self.console_modes.remove(&id);
        for port in ports {
            self.ports.remove(&port);
        }
        let price = self
            .optics
            .device_models
            .remove(&id)
            .and_then(|model| optics_catalog().hardware(&model).map(|m| m.price))
            .unwrap_or_else(|| device.template().price());
        self.money += price / 2;
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
        if self.router_svi(a).is_some() || self.router_svi(b).is_some() {
            return Err(SimError::WrongPortType);
        }
        if self.port_links.contains_key(&a) {
            return Err(SimError::PortAlreadyConnected(a));
        }
        if self.port_links.contains_key(&b) {
            return Err(SimError::PortAlreadyConnected(b));
        }
        for port in [pa, pb] {
            if !self.port_is_copper(port.id) {
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
        if self.return_assembly(id) {
            return Ok(());
        }
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
        let device = self.devices.get(&id).ok_or(SimError::DeviceNotFound(id))?;
        if powered
            && let DeviceKind::Server(server) = &device.kind
            && let Some(hardware) = &server.hardware
        {
            if hardware.cpus.is_empty() || hardware.ram.is_empty() {
                return Err(SimError::ServerHardware(
                    "install a CPU and RAM before powering on".into(),
                ));
            }
            hardware
                .validate_limits(server_catalog())
                .map_err(SimError::ServerHardware)?;
            if !hardware.ready() {
                return Err(SimError::ServerHardware(
                    "component peak demand exceeds configured PSU capacity".into(),
                ));
            }
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

    pub(crate) fn sync_effective_power(&mut self) {
        self.power
            .cord_routes
            .retain(|outlet, _| self.power.connections.contains_key(outlet));
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
                let assembled = !matches!(&device.kind, DeviceKind::Server(server) if server.hardware.as_ref().is_some_and(|hardware| !hardware.ready()));
                let next = device.rack.is_some() && status.effective && assembled;
                if next {
                    self.runtime
                        .device_started
                        .entry(*id)
                        .or_insert(self.runtime.now_ms);
                } else {
                    self.runtime.device_started.remove(id);
                }
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
            PortConfig::PatchPanel | PortConfig::CableManager | PortConfig::Infrastructure => {}
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
        if !self.devices.contains_key(&id) {
            return Err(SimError::DeviceNotFound(id));
        }
        match self.device_vlans_mut(id) {
            Some(vlans) => {
                if let Some(old) = vlans.iter_mut().find(|v| v.id == vlan.id) {
                    *old = vlan;
                } else {
                    vlans.push(vlan);
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
        let known_vlans = self
            .device_vlans(device_id)
            .ok_or(SimError::WrongPortType)?;
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
                .find(|v| !known_vlans.iter().any(|known| known.id == *v))
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
        if !matches!(
            port.config,
            PortConfig::Server(_)
                | PortConfig::Router(_)
                | PortConfig::Switch(_)
                | PortConfig::Infrastructure
        ) {
            return Err(SimError::WrongPortType);
        }
        port.enabled = enabled;
        self.routing_revision += 1;
        Ok(())
    }

    pub(crate) fn set_ipv4(
        &mut self,
        id: PortId,
        config: Ipv4InterfaceConfig,
    ) -> Result<(), SimError> {
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
                v.interfaces
                    .retain(|old| old.vlan.unwrap_or(VlanId(1)) != vlan.unwrap_or(VlanId(1)));
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
                router.interfaces.retain(|old| {
                    !(old.port == id && old.vlan.unwrap_or(VlanId(1)) == vlan.unwrap_or(VlanId(1)))
                });
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
        config.routes.retain(|old| {
            old.network != route.network
                || old.prefix != route.prefix
                || old.egress != route.egress
                || old.via != route.via
        });
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
    pub(crate) fn validate_route_point(&self, point: &CableRoutePoint) -> Result<(), SimError> {
        if let Some(id) = point.room_anchor_id() {
            return if self.room.cable_anchors.iter().any(|anchor| anchor.id == id) {
                Ok(())
            } else {
                Err(SimError::RoomAnchorNotFound(id))
            };
        }
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
        let mut route = self
            .links
            .get(&link)
            .ok_or(SimError::LinkNotFound)?
            .route
            .clone();
        route.push(point);
        self.reroute_cable(link, route)
    }
    fn remove_cable_route_point(&mut self, link: LinkId, index: usize) -> Result<(), SimError> {
        let mut route = self
            .links
            .get(&link)
            .ok_or(SimError::LinkNotFound)?
            .route
            .clone();
        if index >= route.len() {
            return Err(SimError::LinkNotFound);
        }
        route.remove(index);
        self.reroute_cable(link, route)
    }
    fn move_cable_route_point(
        &mut self,
        link: LinkId,
        index: usize,
        point: CableRoutePoint,
    ) -> Result<(), SimError> {
        let mut route = self
            .links
            .get(&link)
            .ok_or(SimError::LinkNotFound)?
            .route
            .clone();
        if index >= route.len() {
            return Err(SimError::LinkNotFound);
        }
        route[index] = point;
        self.reroute_cable(link, route)
    }
    fn reroute_cable(&mut self, link: LinkId, route: Vec<CableRoutePoint>) -> Result<(), SimError> {
        for point in &route {
            self.validate_route_point(point)?;
        }
        let cable = self.links.get(&link).ok_or(SimError::LinkNotFound)?;
        if self.connected_assembly(link).is_some() {
            let minimum_cm = self.minimum_routed_cable_length(cable.a, cable.b, &route)?;
            if minimum_cm > cable.length_cm {
                return Err(SimError::CableTooShort { minimum_cm });
            }
        }
        self.links.get_mut(&link).unwrap().route = route;
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
    fn older_single_rack_save_expands_to_predefined_room() {
        let mut sim = NetworkSim::new();
        sim.racks.retain(|id, _| *id == RackId(1));
        sim.racks.get_mut(&RackId(1)).unwrap().units = 12;
        sim.room.rack_positions.retain(|id, _| *id == RackId(1));
        sim.room.cable_anchors.retain(|anchor| anchor.id == 1);
        sim.room.cable_anchors[0].position = RoomPosition {
            x_cm: 240,
            y_cm: 160,
        };
        sim.power.racks.retain(|id, _| *id == RackId(1));
        sim.next_rack_id = 2;
        let mut loaded: NetworkSim = ron::from_str(&ron::to_string(&sim).unwrap()).unwrap();
        loaded.rebuild_indexes();
        assert_eq!(loaded.racks().count(), 50);
        assert_eq!(loaded.rack(RackId(1)).unwrap().units, 42);
        assert_eq!(
            loaded.rack_room_position(RackId(50)),
            predefined_rack_position(RackId(50))
        );
        assert!(loaded.power.racks.contains_key(&RackId(50)));
        assert_eq!(
            loaded.room.cable_anchors.len(),
            RoomCableLayout::ANCHOR_COUNT
        );
        assert_eq!(
            loaded.room.cable_anchors[0].position,
            RoomPosition {
                x_cm: 630,
                y_cm: 180
            }
        );
    }
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
