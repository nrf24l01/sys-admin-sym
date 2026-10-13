//! Startup validation also covers externally added JSON models.
use super::*;
use std::collections::BTreeSet;

fn rect_valid(r: [f32; 4]) -> bool {
    r.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)) && r[0] < r[2] && r[1] < r[3]
}

impl ServerCatalog {
    pub fn validate_models(&self) -> Result<(), String> {
        if self.model(DEFAULT_SERVER_MODEL).is_none() {
            return Err("missing default server dell_r360".into());
        }
        let mut ids = BTreeSet::new();
        for model in &self.models {
            let mut validate = || -> Result<(), String> {
                if model.id.is_empty() || !ids.insert(&model.id) {
                    return Err("duplicate or empty model ID".into());
                }
                if model.price <= 0
                    || !(1..=42).contains(&model.rack_units)
                    || model.manufacturer.is_empty()
                    || model.display_name.get("en").is_empty()
                    || model.power_inlets.len() != model.chassis.psu_bays
                {
                    return Err("invalid price, rack_units, name or power_inlets".into());
                }
                // Validate explicit topology against shared parts, without initializing global catalogs.
                let profile = ServerCatalog {
                    chassis: model.chassis.clone(),
                    parts: self.parts.clone(),
                    models: Vec::new(),
                };
                profile.validate()?;
                if model.power_inlets.iter().any(|p| {
                    !rect_valid(p.rect)
                        || !matches!(p.side.as_deref(), None | Some("front" | "back"))
                        || !matches!(
                            p.connector,
                            None | Some(
                                crate::ConnectorType::IecC14 | crate::ConnectorType::CiscoFourPin
                            )
                        )
                }) || model
                    .chassis
                    .pcie_slots
                    .iter()
                    .any(|p| !rect_valid(p.face_rect))
                    || model
                        .chassis
                        .drive_bays
                        .iter()
                        .any(|p| !rect_valid(p.face_rect))
                {
                    return Err("invalid socket, PCIe or drive geometry".into());
                }
                let mut names = BTreeSet::new();
                for port in &model.onboard_ports {
                    if !names.insert(&port.name)
                        || port.name.is_empty()
                        || crate::LinkSpeed::from_mbps(port.speed_mbps).is_none()
                        || !rect_valid(port.socket.rect)
                        || !matches!(port.socket.side.as_deref(), None | Some("front" | "back"))
                        || !matches!(
                            port.socket.connector,
                            None | Some(crate::ConnectorType::Rj45)
                        )
                    {
                        return Err("invalid onboard_ports name, speed or geometry".into());
                    }
                }
                for face in [&model.artwork.front, &model.artwork.rear] {
                    let path = std::path::Path::new(&face.path);
                    if face.path.is_empty()
                        || path.is_absolute()
                        || path
                            .components()
                            .any(|c| matches!(c, std::path::Component::ParentDir))
                        || !rect_valid(face.uv)
                        || face.source_size.contains(&0)
                    {
                        return Err("invalid artwork path, UV or source_size".into());
                    }
                }
                self.validate_recipe(model, &profile)?;
                model.full_pack_price(self)?;
                Ok(())
            };
            validate().map_err(|e| format!("server model {}: {e}", model.id))?;
        }
        Ok(())
    }

    fn validate_recipe(&self, model: &ServerModel, profile: &ServerCatalog) -> Result<(), String> {
        let mut h = ServerHardware {
            pcie: vec![None; model.chassis.pcie_slots.len()],
            drives: vec![None; model.chassis.drive_bays.len()],
            ..Default::default()
        };
        for p in &model.full_pack.parts {
            let part = self
                .part(&p.part_id)
                .ok_or_else(|| format!("unknown recipe part {}", p.part_id))?;
            if part.price < 0 {
                return Err("negative component price".into());
            }
            match &part.kind {
                ServerPartKind::Cpu { .. } => h.cpus.push(p.part_id.clone()),
                ServerPartKind::Ram { .. } => {
                    h.ram.push(p.part_id.clone());
                    h.ram_slot_indices
                        .push(p.slot.ok_or("recipe DIMM needs explicit slot")?);
                }
                ServerPartKind::PciCard { .. } => {
                    let slot = h
                        .pcie
                        .get_mut(p.slot.ok_or("recipe PCIe card needs explicit slot")?)
                        .ok_or("recipe PCIe slot out of range")?;
                    if slot.is_some() {
                        return Err("duplicate recipe PCIe slot".into());
                    }
                    *slot = Some(p.part_id.clone());
                }
                ServerPartKind::PowerSupply { .. } => {
                    return Err("PSUs are integrated into chassis".into());
                }
            }
        }
        for drive in &model.full_pack.drives {
            let bay = h
                .drives
                .get_mut(drive.bay)
                .ok_or("recipe drive bay out of range")?;
            if bay.is_some() {
                return Err("duplicate recipe drive bay".into());
            }
            *bay = Some(drive.drive_id.clone());
        }
        if h.cpus.is_empty() || h.ram.is_empty() {
            return Err("recipe needs CPU and RAM".into());
        }
        h.validate_limits(profile)?;
        let mut peak = model.power.board.peak_mw + model.power.fans.peak_mw;
        for id in h.cpus.iter().chain(&h.ram).chain(h.pcie.iter().flatten()) {
            peak = peak
                .checked_add(self.part(id).unwrap().power.peak_mw)
                .ok_or("power overflow")?;
        }
        for id in h.drives.iter().flatten() {
            peak = peak
                .checked_add(
                    crate::drive_catalog()
                        .drives
                        .iter()
                        .find(|d| &d.id == id)
                        .unwrap()
                        .power
                        .peak_mw,
                )
                .ok_or("power overflow")?;
        }
        if peak.div_ceil(1000) > model.power.psu.capacity_watts {
            return Err("recipe exceeds single PSU capacity".into());
        }
        Ok(())
    }
}
