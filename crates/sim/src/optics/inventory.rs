use super::*;
use crate::*;

impl NetworkSim {
    pub fn cage_profile(&self, port: PortId) -> Option<CageProfile> {
        (self.port(port)?.connector == PortConnector::Sfp).then(|| {
            self.optics
                .cages
                .get(&port)
                .cloned()
                .unwrap_or_else(CageProfile::sfp)
        })
    }
    pub fn installed_transceiver(&self, port: PortId) -> Option<&TransceiverInstance> {
        self.optics
            .transceivers
            .values()
            .find(|m| m.port == Some(port))
    }
    pub fn connected_assembly(&self, link: LinkId) -> Option<&CableAssemblyInstance> {
        self.optics
            .assemblies
            .values()
            .find(|c| c.link == Some(link))
    }
    pub fn endpoint_module(&self, port: PortId) -> Option<&'static TransceiverModel> {
        if let Some(module) = self.installed_transceiver(port) {
            return optics_catalog().module(&module.model_id);
        }
        let assembly = self.connected_assembly(self.link_for_port(port)?.id)?;
        match &optics_catalog().cable(&assembly.model_id)?.medium {
            AssemblyMedium::Dac { transceiver } | AssemblyMedium::Aoc { transceiver } => {
                optics_catalog().module(transceiver)
            }
            AssemblyMedium::Fiber { .. } => None,
        }
    }
    pub fn port_is_copper(&self, port: PortId) -> bool {
        self.port(port)
            .is_some_and(|p| p.connector == PortConnector::Rj45)
            || self
                .endpoint_module(port)
                .is_some_and(|m| matches!(m.medium, ModuleMedium::Copper))
    }
    /// Inventory choices supported by this endpoint. Unlike physical connector
    /// validation, this excludes fiber that fits an LC socket but cannot carry
    /// the installed optic's signal. The other endpoint and route are checked
    /// by `quote_assembly` when the player finishes the connection.
    pub fn assembly_supported_at_port(&self, assembly: CableAssemblyId, port: PortId) -> bool {
        let Some(p) = self.port(port) else {
            return false;
        };
        if self.link_for_port(port).is_some() {
            return false;
        }
        let Some(spec) = self
            .optics
            .assemblies
            .get(&assembly)
            .filter(|c| c.link.is_none())
            .and_then(|c| optics_catalog().cable(&c.model_id))
        else {
            return false;
        };
        match &spec.medium {
            AssemblyMedium::Fiber { fiber, strands, .. } => {
                if p.connector == PortConnector::Lc {
                    return true;
                }
                if p.connector != PortConnector::Sfp {
                    return false;
                }
                self.endpoint_module(port).is_some_and(|module| {
                    matches!(&module.medium, ModuleMedium::Optical { strands: count, reaches, .. }
                        if count == strands && reaches.iter().any(|reach| reach.fiber == *fiber
                            && spec.length_cm <= reach.max_length_cm))
                })
            }
            AssemblyMedium::Dac { transceiver } | AssemblyMedium::Aoc { transceiver } => {
                self.endpoint_module(port).is_none()
                    && self.cage_profile(port).is_some_and(|cage| {
                        optics_catalog()
                            .module(transceiver)
                            .is_some_and(|module| cage.supports(module))
                    })
            }
        }
    }
    pub(crate) fn detach_module(&mut self, port: PortId) {
        for module in self.optics.transceivers.values_mut() {
            if module.port == Some(port) {
                module.port = None;
            }
        }
        self.optics.cages.remove(&port);
    }
    pub(crate) fn return_assembly(&mut self, link: LinkId) -> bool {
        let Some(assembly) = self
            .optics
            .assemblies
            .values_mut()
            .find(|c| c.link == Some(link))
        else {
            return false;
        };
        assembly.link = None;
        self.refresh_module_loads();
        self.sync_effective_power();
        true
    }
    pub(crate) fn module_load_watts(&self, device: DeviceId) -> u32 {
        let mw: u32 = self.device(device).map_or(0, |d| {
            d.ports()
                .iter()
                .filter_map(|port| self.endpoint_module(*port))
                .map(|m| m.power_mw)
                .sum()
        });
        mw.div_ceil(1000)
    }
    pub(crate) fn refresh_module_loads(&mut self) {
        let loads: Vec<_> = self
            .devices
            .values()
            .filter_map(|device| {
                let base = match &device.kind {
                    DeviceKind::Server(server) => server
                        .hardware
                        .as_ref()
                        .map_or(180, ServerHardware::load_watts),
                    DeviceKind::Switch(_) => 80,
                    DeviceKind::Router(_) => 60,
                    _ => return None,
                };
                Some((
                    device.id,
                    base.saturating_add(self.module_load_watts(device.id)),
                ))
            })
            .collect();
        for (device, watts) in loads {
            if let Some(power) = self.power.devices.get_mut(&device) {
                power.load = ElectricalLoad::from_watts_pf(watts, 90);
            }
        }
        self.power.recompute_now();
    }
    pub(crate) fn configure_optics(
        &mut self,
        command: OpticsCommand,
    ) -> Result<Vec<SimEvent>, SimError> {
        let events = match command {
            OpticsCommand::BuyTransceiver { model } => {
                let spec = optics_catalog()
                    .module(&model)
                    .ok_or(OpticsError::UnknownModel)?;
                if matches!(spec.medium, ModuleMedium::DirectAttach) {
                    return Err(OpticsError::AttachedCable.into());
                }
                self.spend_optics(spec.price)?;
                self.optics.next_transceiver += 1;
                let id = TransceiverId(self.optics.next_transceiver);
                self.optics.transceivers.insert(
                    id,
                    TransceiverInstance {
                        id,
                        model_id: model,
                        port: None,
                    },
                );
                vec![SimEvent::ConnectivityChanged]
            }
            OpticsCommand::InstallTransceiver { port, module } => {
                let cage = self.cage_profile(port).ok_or(OpticsError::NotCage)?;
                if self.endpoint_module(port).is_some() {
                    return Err(OpticsError::CageOccupied.into());
                }
                let instance = self
                    .optics
                    .transceivers
                    .get(&module)
                    .filter(|m| m.port.is_none())
                    .ok_or(OpticsError::NotOwned)?;
                let spec = optics_catalog()
                    .module(&instance.model_id)
                    .ok_or(OpticsError::UnknownModel)?;
                if !cage.supports(spec) {
                    return Err(OpticsError::IncompatibleHost.into());
                }
                if let Some(link) = self.link_for_port(port) {
                    let fiber = self.connected_assembly(link.id).is_some();
                    if fiber != matches!(spec.medium, ModuleMedium::Optical { .. }) {
                        return Err(OpticsError::ConnectorMismatch.into());
                    }
                }
                self.optics.transceivers.get_mut(&module).unwrap().port = Some(port);
                vec![SimEvent::PortConfigChanged(port)]
            }
            OpticsCommand::RemoveTransceiver { port } => {
                let instance = self
                    .optics
                    .transceivers
                    .values_mut()
                    .find(|m| m.port == Some(port))
                    .ok_or(OpticsError::NotOwned)?;
                // The fiber remains plugged into this physical port; replacing the optic preserves interface configuration.
                instance.port = None;
                vec![SimEvent::PortConfigChanged(port)]
            }
            OpticsCommand::BuyAssembly { model } => {
                let spec = optics_catalog()
                    .cable(&model)
                    .ok_or(OpticsError::UnknownModel)?;
                self.spend_optics(spec.price)?;
                self.optics.next_assembly += 1;
                let id = CableAssemblyId(self.optics.next_assembly);
                self.optics.assemblies.insert(
                    id,
                    CableAssemblyInstance {
                        id,
                        model_id: model,
                        link: None,
                        crossed: true,
                    },
                );
                vec![SimEvent::ConnectivityChanged]
            }
            OpticsCommand::ConnectAssembly {
                assembly,
                a,
                b,
                route,
            } => {
                let spec = self.quote_assembly(assembly, a, b, &route)?;
                let id = LinkId(self.next_link_id);
                self.next_link_id += 1;
                self.links.insert(
                    id,
                    Link {
                        id,
                        a,
                        b,
                        enabled: true,
                        length_cm: spec.length_cm,
                        auto_length: false,
                        color: spec.color,
                        route,
                    },
                );
                self.port_links.insert(a, id);
                self.port_links.insert(b, id);
                self.optics.assemblies.get_mut(&assembly).unwrap().link = Some(id);
                vec![SimEvent::LinkCreated(id)]
            }
            OpticsCommand::FlipPolarity { assembly } => {
                let cable = self
                    .optics
                    .assemblies
                    .get_mut(&assembly)
                    .ok_or(OpticsError::NotOwned)?;
                if !optics_catalog()
                    .cable(&cable.model_id)
                    .is_some_and(|c| matches!(c.medium, AssemblyMedium::Fiber { strands: 2, .. }))
                {
                    return Err(OpticsError::NotFiber.into());
                }
                cable.crossed = !cable.crossed;
                vec![SimEvent::ConnectivityChanged]
            }
            OpticsCommand::BuyHardware { model } => {
                let model = optics_catalog()
                    .hardware(&model)
                    .ok_or(OpticsError::UnknownModel)?;
                let switch = matches!(model.profile, OpticalHardwareProfile::Switch { .. });
                let template = if switch {
                    DeviceTemplate::Switch
                } else {
                    DeviceTemplate::PatchPanel
                };
                if self.money < model.price {
                    return Err(SimError::InsufficientFunds {
                        needed: model.price,
                        available: self.money,
                    });
                }
                // The existing device implementation owns switching/panel behavior; the profile supplies hardware capabilities.
                let id = self.buy_device_at_price(template, model.price)?;
                self.devices.get_mut(&id).unwrap().name =
                    format!("{} #{:02}", model.display_name.get("en"), id.0);
                self.optics.device_models.insert(id, model.id.clone());
                for port in self.devices[&id].ports().to_vec() {
                    let p = self.ports.get_mut(&port).unwrap();
                    if switch && p.connector == PortConnector::Sfp {
                        let OpticalHardwareProfile::Switch { cage } = &model.profile else {
                            unreachable!()
                        };
                        let speed = cage
                            .modes
                            .iter()
                            .map(|m| m.speed)
                            .max()
                            .ok_or(OpticsError::IncompatibleHost)?;
                        p.max_speed = speed;
                        p.advertised_speed = speed;
                        p.name = p.name.replace("Gi1/0/", "Te1/0/").replace("SFP ", "");
                        self.optics.cages.insert(port, cage.clone());
                    } else if !switch {
                        p.connector = PortConnector::Lc;
                    }
                }
                vec![SimEvent::DeviceAdded(id)]
            }
        };
        self.refresh_module_loads();
        self.sync_effective_power();
        Ok(events)
    }
    /// Read-only assembly validation shared by GUI previews and the mutation command.
    pub fn quote_assembly(
        &self,
        assembly: CableAssemblyId,
        a: PortId,
        b: PortId,
        route: &[CableRoutePoint],
    ) -> Result<&'static CableAssemblyModel, SimError> {
        let spec = self
            .optics
            .assemblies
            .get(&assembly)
            .filter(|c| c.link.is_none())
            .and_then(|c| optics_catalog().cable(&c.model_id))
            .ok_or(OpticsError::NotOwned)?;
        if a == b {
            return Err(SimError::SamePort);
        }
        for port in [a, b] {
            let p = self.port(port).ok_or(SimError::PortNotFound(port))?;
            if self.link_for_port(port).is_some() {
                return Err(SimError::PortAlreadyConnected(port));
            }
            match &spec.medium {
                AssemblyMedium::Fiber { .. }
                    if matches!(p.connector, PortConnector::Sfp | PortConnector::Lc) =>
                {
                    if p.connector == PortConnector::Sfp {
                        let module = self
                            .endpoint_module(port)
                            .ok_or(OpticsError::MissingTransceiver)?;
                        if !matches!(module.medium, ModuleMedium::Optical { .. }) {
                            return Err(OpticsError::ConnectorMismatch.into());
                        }
                    }
                }
                AssemblyMedium::Dac { transceiver } | AssemblyMedium::Aoc { transceiver } => {
                    if self.installed_transceiver(port).is_some() {
                        return Err(OpticsError::CageOccupied.into());
                    }
                    let module = optics_catalog()
                        .module(transceiver)
                        .ok_or(OpticsError::UnknownModel)?;
                    if !self
                        .cage_profile(port)
                        .is_some_and(|cage| cage.supports(module))
                    {
                        return Err(OpticsError::IncompatibleHost.into());
                    }
                }
                _ => return Err(OpticsError::ConnectorMismatch.into()),
            }
        }
        for point in route {
            self.validate_route_point(point)?;
        }
        let minimum_cm = self.minimum_routed_cable_length(a, b, route)?;
        if spec.length_cm < minimum_cm {
            return Err(SimError::CableTooShort { minimum_cm });
        }
        Ok(spec)
    }
    fn spend_optics(&mut self, price: i64) -> Result<(), SimError> {
        if self.money < price {
            return Err(SimError::InsufficientFunds {
                needed: price,
                available: self.money,
            });
        }
        self.money -= price;
        Ok(())
    }
}

impl NetworkSim {
    pub(crate) fn normalize_optics(&mut self) {
        let ports = &self.ports;
        let links = &self.links;
        let mut occupied = std::collections::BTreeSet::new();
        for module in self.optics.transceivers.values_mut() {
            if module
                .port
                .is_some_and(|port| !ports.contains_key(&port) || !occupied.insert(port))
            {
                module.port = None;
            }
        }
        let mut occupied_links = std::collections::BTreeSet::new();
        for cable in self.optics.assemblies.values_mut() {
            if cable
                .link
                .is_some_and(|link| !links.contains_key(&link) || !occupied_links.insert(link))
            {
                cable.link = None;
            }
        }
        self.optics.cages.retain(|port, _| ports.contains_key(port));
        self.optics
            .device_models
            .retain(|device, _| self.devices.contains_key(device));
        self.optics.next_transceiver = self.optics.next_transceiver.max(
            self.optics
                .transceivers
                .keys()
                .map(|id| id.0)
                .max()
                .unwrap_or(0),
        );
        self.optics.next_assembly = self.optics.next_assembly.max(
            self.optics
                .assemblies
                .keys()
                .map(|id| id.0)
                .max()
                .unwrap_or(0),
        );
        self.refresh_module_loads();
        self.sync_effective_power();
    }
}
