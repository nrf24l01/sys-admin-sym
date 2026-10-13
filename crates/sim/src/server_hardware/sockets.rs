use crate::*;

/// A physical socket on the selected server model. Indices are zero based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServerSocket {
    Cpu(usize),
    Dimm(usize),
    Pcie(usize),
    Drive(usize),
}

impl NetworkSim {
    /// Return the installed item in a physical socket, including legacy DIMM layouts.
    pub fn server_socket_item(&self, device: DeviceId, socket: ServerSocket) -> Option<&str> {
        let DeviceKind::Server(server) = &self.devices.get(&device)?.kind else {
            return None;
        };
        let hardware = server.hardware.as_ref()?;
        match socket {
            ServerSocket::Cpu(index) => hardware.cpus.get(index).map(String::as_str),
            ServerSocket::Dimm(index) => hardware
                .dimm_slot_indices_for_model(server.model())
                .iter()
                .position(|slot| *slot == index)
                .and_then(|position| hardware.ram.get(position))
                .map(String::as_str),
            ServerSocket::Pcie(index) => hardware.pcie.get(index).and_then(Option::as_deref),
            ServerSocket::Drive(index) => hardware.drives.get(index).and_then(Option::as_deref),
        }
    }

    /// Validate a prospective socket item without changing hardware or inventory.
    /// Occupied sockets are evaluated with their current item replaced.
    pub fn server_socket_compatibility(
        &self,
        device: DeviceId,
        socket: ServerSocket,
        item_id: &str,
    ) -> Result<(), SimError> {
        let DeviceKind::Server(server) = &self
            .devices
            .get(&device)
            .ok_or(SimError::DeviceNotFound(device))?
            .kind
        else {
            return Err(SimError::ServerHardware(
                "selected device is not a server".into(),
            ));
        };
        let mut candidate = server.hardware.clone().ok_or_else(|| {
            SimError::ServerHardware("legacy server has no configurable chassis".into())
        })?;
        let model = server.model();
        let chassis = &model.chassis;
        let catalog = server_catalog();
        if let ServerSocket::Drive(index) = socket {
            let drive = crate::drive_catalog()
                .drives
                .iter()
                .find(|d| d.id == item_id)
                .ok_or_else(|| SimError::UnknownDrive(item_id.into()))?;
            let bay = chassis
                .drive_bays
                .get(index)
                .ok_or_else(|| SimError::ServerHardware("drive bay does not exist".into()))?;
            if bay.interface != drive.interface {
                return Err(SimError::ServerHardware("drive bay is incompatible".into()));
            }
            candidate
                .drives
                .get(index)
                .ok_or_else(|| SimError::ServerHardware("drive bay does not exist".into()))?;
            // Drive installation does not depend on CPU, memory or PCIe population.
            return Ok(());
        } else {
            let part = catalog
                .parts
                .iter()
                .find(|part| part.id == item_id)
                .ok_or_else(|| SimError::UnknownServerPart(item_id.into()))?;
            match (socket, &part.kind) {
                (ServerSocket::Cpu(index), ServerPartKind::Cpu { .. }) => {
                    if index >= chassis.cpu_sockets || index > candidate.cpus.len() {
                        return Err(SimError::ServerHardware(
                            "CPU socket does not exist or requires the preceding CPU".into(),
                        ));
                    }
                    if index == candidate.cpus.len() {
                        candidate.cpus.push(item_id.into());
                    } else {
                        candidate.cpus[index] = item_id.into();
                    }
                }
                (ServerSocket::Dimm(index), ServerPartKind::Ram { .. }) => {
                    if index >= chassis.dimm_slots {
                        return Err(SimError::ServerHardware("DIMM slot does not exist".into()));
                    }
                    candidate.normalize_dimm_slots_for_model(model);
                    if let Some(position) = candidate
                        .ram_slot_indices
                        .iter()
                        .position(|slot| *slot == index)
                    {
                        candidate.ram[position] = item_id.into();
                    } else {
                        candidate.ram.push(item_id.into());
                        candidate.ram_slot_indices.push(index);
                    }
                }
                (
                    ServerSocket::Pcie(index),
                    ServerPartKind::PciCard {
                        card: PciCard::Ethernet { speed_mbps, .. },
                    },
                ) => {
                    if LinkSpeed::from_mbps(*speed_mbps).is_none() {
                        return Err(SimError::ServerHardware(
                            "NIC speed is unsupported by the network simulator".into(),
                        ));
                    }
                    if chassis.pcie_slots.get(index).is_none() {
                        return Err(SimError::ServerHardware("PCIe slot does not exist".into()));
                    }
                    let item = candidate.pcie.get_mut(index).ok_or_else(|| {
                        SimError::ServerHardware("PCIe slot does not exist".into())
                    })?;
                    *item = Some(item_id.into());
                }
                _ => {
                    return Err(SimError::ServerHardware(
                        "item kind does not match the selected socket".into(),
                    ));
                }
            }
        }
        candidate
            .validate_installation_for_model(model, catalog)
            .map_err(SimError::ServerHardware)
    }
}
