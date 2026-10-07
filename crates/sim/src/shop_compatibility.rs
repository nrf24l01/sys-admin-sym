//! Read-only hardware installation validation shared by shop previews and commands.
use crate::*;

impl NetworkSim {
    /// Validate installation without requiring an owned instance or mutating inventory.
    pub fn server_part_installation_slot(
        &self,
        device: DeviceId,
        part_id: &str,
        slot: Option<usize>,
    ) -> Result<Option<usize>, SimError> {
        let catalog = server_catalog();
        let part = catalog
            .parts
            .iter()
            .find(|p| p.id == part_id)
            .ok_or_else(|| SimError::UnknownServerPart(part_id.into()))?;
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
        let chassis = &catalog.chassis;
        let selected_slot = match &part.kind {
            ServerPartKind::Cpu { socket, .. } => {
                if socket != &chassis.cpu_socket || hardware.cpus.len() >= chassis.cpu_sockets {
                    return Err(SimError::ServerHardware(
                        "CPU socket is incompatible or full".into(),
                    ));
                }
                None
            }
            ServerPartKind::Ram { memory_type, .. } => {
                if memory_type != &chassis.memory_type || hardware.ram.len() >= chassis.dimm_slots {
                    return Err(SimError::ServerHardware(
                        "DIMM type is incompatible or slots are full".into(),
                    ));
                }
                None
            }
            ServerPartKind::PowerSupply { .. } => {
                if hardware.power_supplies.len() >= chassis.psu_bays {
                    return Err(SimError::ServerHardware(
                        "power supply bays are full".into(),
                    ));
                }
                None
            }
            ServerPartKind::PciCard {
                card:
                    PciCard::Ethernet {
                        lanes,
                        generation,
                        width,
                        speed_mbps,
                        ..
                    },
            } => {
                if LinkSpeed::from_mbps(*speed_mbps).is_none() {
                    return Err(SimError::ServerHardware(
                        "NIC speed is unsupported by the network simulator".into(),
                    ));
                }
                let free = |index: usize| {
                    chassis.pcie_slots.get(index).is_some_and(|s| {
                        hardware.pcie.get(index).is_some_and(Option::is_none)
                            && s.lanes >= *lanes
                            && s.width >= *width
                            && s.generation >= *generation
                    })
                };
                let index = slot
                    .or_else(|| (0..chassis.pcie_slots.len()).find(|&i| free(i)))
                    .ok_or_else(|| {
                        SimError::ServerHardware("no compatible free PCIe slot".into())
                    })?;
                if !free(index) {
                    return Err(SimError::ServerHardware(
                        "PCIe slot is occupied or incompatible".into(),
                    ));
                }
                let available: u16 = hardware
                    .cpus
                    .iter()
                    .filter_map(|id| catalog.parts.iter().find(|p| &p.id == id))
                    .filter_map(|p| match p.kind {
                        ServerPartKind::Cpu { pcie_lanes, .. } => Some(u16::from(pcie_lanes)),
                        _ => None,
                    })
                    .sum();
                let used: u16 = hardware
                    .pcie
                    .iter()
                    .flatten()
                    .filter_map(|id| catalog.parts.iter().find(|p| &p.id == id))
                    .filter_map(|p| match &p.kind {
                        ServerPartKind::PciCard {
                            card: PciCard::Ethernet { lanes, .. },
                        } => Some(u16::from(*lanes)),
                        _ => None,
                    })
                    .sum();
                if used + u16::from(*lanes) > available {
                    return Err(SimError::ServerHardware(format!(
                        "PCIe lane budget exceeded: need {}, available {available}",
                        used + u16::from(*lanes)
                    )));
                }
                Some(index)
            }
        };
        Ok(selected_slot)
    }

    /// Select a free compatible bay using the same rules as drive installation.
    pub fn drive_installation_bay(
        &self,
        device: DeviceId,
        drive_id: &str,
        bay: Option<usize>,
    ) -> Result<usize, SimError> {
        let drive = crate::drive_catalog()
            .drives
            .iter()
            .find(|drive| drive.id == drive_id)
            .ok_or_else(|| SimError::UnknownDrive(drive_id.into()))?;
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
        let hardware = server.hardware.as_ref().ok_or_else(|| {
            SimError::ServerHardware("legacy server has no configurable drive bays".into())
        })?;
        let chassis = &server_catalog().chassis;
        let compatible = |index: usize| {
            chassis.drive_bays.get(index).is_some_and(|slot| {
                slot.interface == drive.interface
                    && hardware.drives.get(index).is_some_and(Option::is_none)
            })
        };
        let index = bay
            .or_else(|| (0..chassis.drive_bays.len()).find(|&i| compatible(i)))
            .ok_or_else(|| SimError::ServerHardware("no compatible free drive bay".into()))?;
        if !compatible(index) {
            return Err(SimError::ServerHardware(
                "drive bay is occupied or incompatible".into(),
            ));
        }
        Ok(index)
    }
}
