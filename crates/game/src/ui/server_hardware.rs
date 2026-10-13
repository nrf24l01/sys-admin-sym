use super::*;
mod picker;
pub(super) use picker::picker;

pub(super) fn server_hardware_inspector(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    device: DeviceId,
    hardware: &cloud_provider_sim::ServerHardware,
    state: &mut UiState,
) {
    use cloud_provider_sim::{PciCard, ServerPartKind, server_catalog};
    let catalog = server_catalog();
    let DeviceKind::Server(server) = &sim.device(device).unwrap().kind else {
        return;
    };
    let model = server.model();
    let chassis = &model.chassis;

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
    ui.separator();
    ui.strong(tr("hardware.cpu-sockets"));
    ui.horizontal_wrapped(|ui| {
        for index in 0..chassis.cpu_sockets {
            socket_tile(ui, sim, device, ServerSocket::Cpu(index), state);
        }
    });
    egui::CollapsingHeader::new(tr("hardware.dimm-slots")).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for index in 0..chassis.dimm_slots {
                socket_tile(ui, sim, device, ServerSocket::Dimm(index), state);
            }
        });
    });
    egui::CollapsingHeader::new("PCIe").show(ui, |ui| {
        for index in 0..chassis.pcie_slots.len() {
            socket_tile(ui, sim, device, ServerSocket::Pcie(index), state);
        }
    });
    egui::CollapsingHeader::new(tr("ui.drive-bays")).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for index in 0..chassis.drive_bays.len() {
                socket_tile(ui, sim, device, ServerSocket::Drive(index), state);
            }
        });
    });
}

pub(super) fn section(socket: ServerSocket) -> ShopSection {
    match socket {
        ServerSocket::Cpu(_) => ShopSection::Cpu,
        ServerSocket::Dimm(_) => ShopSection::Ram,
        ServerSocket::Pcie(_) => ShopSection::PciCards,
        ServerSocket::Drive(_) => ShopSection::Storage,
    }
}

pub(super) fn socket_name(sim: &NetworkSim, device: DeviceId, socket: ServerSocket) -> String {
    let Some(device) = sim.device(device) else {
        return String::new();
    };
    let DeviceKind::Server(server) = &device.kind else {
        return String::new();
    };
    let chassis = &server.model().chassis;
    match socket {
        ServerSocket::Cpu(index) => format!("CPU {} · {}", index + 1, chassis.cpu_socket),
        ServerSocket::Dimm(index) => {
            chassis
                .memory
                .slots
                .get(index)
                .map_or_else(String::new, |slot| {
                    format!(
                        "{} · CPU {} · {}",
                        slot.name,
                        slot.cpu_socket + 1,
                        chassis.memory_type
                    )
                })
        }
        ServerSocket::Pcie(index) => {
            chassis
                .pcie_slots
                .get(index)
                .map_or_else(String::new, |slot| {
                    format!(
                        "{} · x{} · Gen{} · CPU {}",
                        slot.name,
                        slot.width,
                        slot.generation,
                        slot.cpu_socket + 1
                    )
                })
        }
        ServerSocket::Drive(index) => chassis
            .drive_bays
            .get(index)
            .map_or_else(String::new, |slot| {
                format!("{} · {}", slot.name, slot.interface)
            }),
    }
}

fn socket_tile(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    device: DeviceId,
    socket: ServerSocket,
    state: &mut UiState,
) {
    let name = socket_name(sim, device, socket);
    let installed = sim.server_socket_item(device, socket);
    let item = installed
        .map(|id| crate::localization::item_name(id, id))
        .unwrap_or_else(|| tr("ui.empty"));
    let response = ui
        .add(egui::Button::new(format!("{name}\n{item}")).min_size(egui::vec2(145.0, 48.0)))
        .on_hover_text(tr("hardware.select-socket"));
    if response.clicked() {
        state.hardware_picker = Some((device, socket));
    }
}
