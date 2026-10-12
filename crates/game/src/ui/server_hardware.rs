use super::*;

pub(super) fn server_hardware_inspector(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    device: DeviceId,
    hardware: &cloud_provider_sim::ServerHardware,
    actions: &mut MessageWriter<UiAction>,
) {
    use cloud_provider_sim::{PciCard, ServerPartKind, server_catalog};
    let catalog = server_catalog();
    let DeviceKind::Server(server) = &sim.device(device).unwrap().kind else {
        return;
    };
    let model = server.model();
    let chassis = &model.chassis;
    let parts_editable = !sim.device(device).unwrap().powered || chassis.parts_hot_swappable;
    ui.separator();
    ui.strong(tr("ui.server-hardware"));
    ui.label(crate::localization::tr_args(
        "ui.cpu-ram-integrated-psu-w",
        &[
            (hardware.cpus.len()).to_string(),
            (chassis.cpu_sockets).to_string(),
            (hardware.ram.len()).to_string(),
            (chassis.dimm_slots).to_string(),
            (model.power.psu.capacity_watts).to_string(),
            chassis.psu_bays.to_string(),
        ],
    ));
    match hardware.memory_status_for_model(model) {
        Ok(memory) => {
            ui.label(crate::localization::tr_args(
                "hardware.memory-capacity",
                &[
                    memory.capacity_gb.to_string(),
                    memory.max_capacity_gb.to_string(),
                ],
            ));
            ui.label(crate::localization::tr_args(
                "hardware.memory-channels-speed",
                &[
                    memory.populated_channels.to_string(),
                    memory.available_channels.to_string(),
                    memory.speed_mt_s.to_string(),
                ],
            ));
            if !hardware.ram.is_empty() && !memory.balanced {
                ui.weak(tr("hardware.memory-unbalanced"));
            }
        }
        Err(reason) => {
            ui.colored_label(
                egui::Color32::YELLOW,
                format!("{}: {reason}", tr("hardware.invalid-memory")),
            );
        }
    }
    let dimm_indices = hardware.dimm_slot_indices_for_model(model);
    egui::CollapsingHeader::new(tr("hardware.dimm-slots")).show(ui, |ui| {
        for (index, slot) in chassis.memory.slots.iter().enumerate() {
            let installed = dimm_indices
                .iter()
                .position(|i| *i == index)
                .and_then(|i| hardware.ram.get(i));
            ui.horizontal_wrapped(|ui| {
                ui.label(crate::localization::tr_args(
                    "hardware.dimm-slot",
                    &[
                        slot.name.clone(),
                        (slot.cpu_socket + 1).to_string(),
                        (slot.channel + 1).to_string(),
                    ],
                ));
                if let Some(id) = installed {
                    ui.label(crate::localization::item_name(id, id));
                    if ui
                        .add_enabled(parts_editable, egui::Button::new(tr("ui.remove")))
                        .clicked()
                    {
                        actions.write(UiAction::RemoveServerPart {
                            device,
                            part_id: id.clone(),
                            slot: Some(index),
                        });
                    }
                } else {
                    ui.weak(tr("ui.empty"));
                }
            });
        }
    });
    let cpu_lanes: u16 = hardware
        .cpus
        .iter()
        .filter_map(|id| catalog.parts.iter().find(|p| &p.id == id))
        .filter_map(|part| match part.kind {
            ServerPartKind::Cpu { pcie_lanes, .. } => Some(u16::from(pcie_lanes)),
            _ => None,
        })
        .sum();
    let used_lanes: u16 = hardware
        .pcie
        .iter()
        .flatten()
        .filter_map(|id| catalog.parts.iter().find(|p| &p.id == id))
        .filter_map(|part| match &part.kind {
            ServerPartKind::PciCard {
                card: PciCard::Ethernet { lanes, .. },
            } => Some(u16::from(*lanes)),
            _ => None,
        })
        .sum();
    ui.label(crate::localization::tr_args(
        "ui.pcie-lanes",
        &[(used_lanes).to_string(), (cpu_lanes).to_string()],
    ));
    let active = sim.server_resources(device);
    ui.label(crate::localization::tr_args(
        "ui.compute-core-mhz-memory-weighted-gb-live",
        &[
            (hardware.compute_mhz()).to_string(),
            (hardware.memory_score_gb()).to_string(),
            (active.network_mbps).to_string(),
        ],
    ));
    if hardware.ready_for_model(model) {
        ui.colored_label(egui::Color32::GREEN, tr("ui.required-hardware-installed"));
    } else {
        ui.weak(tr("ui.install-a-cpu-and-ram-to-complete"));
    }
    for (index, slot) in chassis.pcie_slots.iter().enumerate() {
        let installed = hardware.pcie.get(index).and_then(Option::as_deref);
        ui.horizontal(|ui| {
            ui.label(crate::localization::tr_args(
                "ui.x-gen",
                &[
                    (slot.name).to_string(),
                    (slot.lanes).to_string(),
                    (slot.generation).to_string(),
                ],
            ));
            if let Some(id) = installed {
                let name = catalog
                    .parts
                    .iter()
                    .find(|part| part.id == id)
                    .map_or(id, |part| part.name.as_str());
                ui.label(crate::localization::item_name(id, name));
                if let Some(generation) = hardware.pcie_link_generation_for_model(model, index) {
                    ui.weak(crate::localization::tr_args(
                        "hardware.pcie-negotiated",
                        &[generation.to_string()],
                    ));
                }
                if ui
                    .add_enabled(parts_editable, egui::Button::new(tr("ui.remove")))
                    .clicked()
                {
                    actions.write(UiAction::RemoveServerPart {
                        device,
                        part_id: id.into(),
                        slot: Some(index),
                    });
                }
            } else {
                ui.weak(tr("ui.empty"));
                for part in &catalog.parts {
                    let ServerPartKind::PciCard { .. } = &part.kind else {
                        continue;
                    };
                    if sim.server_parts.get(&part.id).copied().unwrap_or(0) > 0
                        && sim
                            .server_part_installation_slot(device, &part.id, Some(index))
                            .is_ok()
                        && ui
                            .add_enabled(
                                parts_editable,
                                egui::Button::new(crate::localization::tr_args(
                                    "hardware.install-item",
                                    &[crate::localization::item_name(&part.id, &part.name)],
                                )),
                            )
                            .clicked()
                    {
                        actions.write(UiAction::InstallServerPart {
                            device,
                            part_id: part.id.clone(),
                            slot: Some(index),
                        });
                    }
                }
            }
        });
    }
    for part in &catalog.parts {
        let owned = sim.server_parts.get(&part.id).copied().unwrap_or(0);
        let installed = match &part.kind {
            ServerPartKind::Cpu { .. } => hardware.cpus.iter().filter(|id| *id == &part.id).count(),
            ServerPartKind::Ram { .. } => hardware.ram.iter().filter(|id| *id == &part.id).count(),
            ServerPartKind::PowerSupply { .. } => hardware
                .power_supplies
                .iter()
                .filter(|id| *id == &part.id)
                .count(),
            ServerPartKind::PciCard { .. } => hardware
                .pcie
                .iter()
                .filter(|id| id.as_deref() == Some(part.id.as_str()))
                .count(),
        };
        ui.horizontal(|ui| {
            ui.label(crate::localization::tr_args(
                "ui.inventory-installed",
                &[
                    crate::localization::item_name(&part.id, &part.name),
                    (owned).to_string(),
                    (installed).to_string(),
                ],
            ));
            let compatibility = sim.server_part_installation_slot(device, &part.id, None);
            if ui
                .add_enabled(
                    owned > 0 && parts_editable && compatibility.is_ok(),
                    egui::Button::new(tr("ui.install")),
                )
                .on_disabled_hover_text(if !parts_editable {
                    tr("hardware.power-off-parts")
                } else {
                    compatibility
                        .err()
                        .map(|error| error.to_string())
                        .unwrap_or_else(|| tr("ui.empty"))
                })
                .clicked()
            {
                actions.write(UiAction::InstallServerPart {
                    device,
                    part_id: part.id.clone(),
                    slot: None,
                });
            }
            if installed > 0
                && !matches!(part.kind, ServerPartKind::PciCard { .. })
                && ui
                    .add_enabled(parts_editable, egui::Button::new(tr("ui.remove")))
                    .clicked()
            {
                actions.write(UiAction::RemoveServerPart {
                    device,
                    part_id: part.id.clone(),
                    slot: None,
                });
            }
        });
    }
    ui.separator();
    ui.strong(tr("ui.drive-bays"));
    ui.weak(tr("ui.buy-drives-in-compute-storage-drives-then"));
    for (bay, slot) in chassis.drive_bays.iter().enumerate() {
        ui.horizontal_wrapped(|ui| {
            ui.label(crate::localization::tr_args(
                "hardware.slot",
                &[(slot.name).to_string(), (slot.interface).to_string()],
            ));
            if let Some(id) = hardware.drives.get(bay).and_then(Option::as_deref) {
                if let Some(drive) = cloud_provider_sim::drive_catalog()
                    .drives
                    .iter()
                    .find(|d| d.id == id)
                {
                    ui.label(crate::localization::tr_args(
                        "ui.read-mb-s-iops-write-mb-s",
                        &[
                            crate::localization::item_name(&drive.id, &drive.name),
                            (drive.read_mb_s).to_string(),
                            (drive.read_iops).to_string(),
                            (drive.write_mb_s).to_string(),
                            (drive.write_iops).to_string(),
                        ],
                    ));
                } else {
                    ui.label(id);
                }
                if ui.button(tr("ui.remove-drive")).clicked() {
                    actions.write(UiAction::RemoveDrive { device, bay });
                }
            } else {
                ui.weak(tr("ui.empty"));
                for drive in &cloud_provider_sim::drive_catalog().drives {
                    if drive.interface == slot.interface
                        && sim.drive_inventory.get(&drive.id).copied().unwrap_or(0) > 0
                        && ui
                            .button(crate::localization::tr_args(
                                "hardware.install-item",
                                &[crate::localization::item_name(&drive.id, &drive.name)],
                            ))
                            .clicked()
                    {
                        actions.write(UiAction::InstallDrive {
                            device,
                            drive_id: drive.id.clone(),
                            bay: Some(bay),
                        });
                    }
                }
            }
        });
    }
}
