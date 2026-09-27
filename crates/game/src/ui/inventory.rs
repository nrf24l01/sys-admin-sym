use crate::app::*;
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::{CableColor, NetworkSim};

pub(super) fn show(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    egui::Panel::left("inventory")
        .resizable(false)
        .default_size(220.0)
        .show(viewport, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("inventory-scroll")
                .show(ui, |ui| {
                    ui.heading("Cable setup");
                    let stock = sim.cable_inventory();
                    ui.label(format!(
                        "Bulk cable: {:.2} m",
                        stock.cable_cm as f32 / 100.0
                    ));
                    ui.label(format!("RJ45 connectors: {}", stock.connectors));
                    let mut automatic = state.cable_length_cm.is_none();
                    if ui
                        .checkbox(&mut automatic, "Auto: shortest path + 5%")
                        .changed()
                    {
                        state.cable_length_cm = if automatic { None } else { Some(100) };
                    }
                    if let Some(cm) = &mut state.cable_length_cm {
                        ui.horizontal(|ui| {
                            ui.label("Cut length");
                            ui.add(
                                egui::DragValue::new(cm)
                                    .range(1..=10000)
                                    .speed(1)
                                    .suffix(" cm"),
                            );
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label("Jacket color");
                        for color in [
                            CableColor::White,
                            CableColor::Gray,
                            CableColor::Blue,
                            CableColor::Orange,
                            CableColor::Red,
                        ] {
                            let selected = state.cable_color == color;
                            let fill = super::cable_color_value(color);
                            if ui
                                .add(egui::Button::new("   ").fill(fill).selected(selected))
                                .on_hover_text(format!("{color:?}"))
                                .clicked()
                            {
                                state.cable_color = color;
                            }
                        }
                    });
                    ui.weak("A new lead uses its length + 2 plugs. Unplugged leads can be reused.");
                    if !stock.patch_cables_cm.is_empty() {
                        ui.label(format!("Reusable leads: {}", stock.patch_cables_cm.len()));
                        let mut leads: Vec<_> = stock
                            .patch_cables_cm
                            .iter()
                            .enumerate()
                            .map(|(index, cm)| {
                                (
                                    *cm,
                                    stock
                                        .patch_cable_colors
                                        .get(index)
                                        .copied()
                                        .unwrap_or_default(),
                                )
                            })
                            .collect();
                        leads.sort_by_key(|(cm, color)| (*cm, *color as u8));
                        leads.dedup();
                        for (cm, color) in leads {
                            let count = stock
                                .patch_cables_cm
                                .iter()
                                .enumerate()
                                .filter(|(index, length)| {
                                    **length == cm
                                        && stock
                                            .patch_cable_colors
                                            .get(*index)
                                            .copied()
                                            .unwrap_or_default()
                                            == color
                                })
                                .count();
                            if ui
                                .small_button(format!(
                                    "{:.2} m {:?} × {count} — use",
                                    cm as f32 / 100.0,
                                    color
                                ))
                                .clicked()
                            {
                                state.cable_length_cm = Some(cm);
                                state.cable_color = color;
                            }
                        }
                    }
                    ui.separator();
                    ui.heading("Inventory");
                    let inventory: Vec<_> = sim.devices().filter(|d| d.rack.is_none()).collect();
                    if inventory.is_empty() {
                        ui.weak("No uninstalled devices");
                    }
                    let mut groups: std::collections::BTreeMap<String, Vec<_>> =
                        std::collections::BTreeMap::new();
                    for device in &inventory {
                        groups
                            .entry(super::inventory_model_name(device))
                            .or_default()
                            .push(*device);
                    }
                    for (row, (label, matching)) in groups.into_iter().enumerate() {
                        if row >= 9 {
                            break;
                        }
                        let Some(device) = matching.first() else {
                            continue;
                        };
                        let key = [
                            egui::Key::Num1,
                            egui::Key::Num2,
                            egui::Key::Num3,
                            egui::Key::Num4,
                            egui::Key::Num5,
                            egui::Key::Num6,
                            egui::Key::Num7,
                            egui::Key::Num8,
                            egui::Key::Num9,
                        ][row];
                        let shortcut = !ui.ctx().egui_wants_keyboard_input()
                            && ui.input(|input| input.key_pressed(key));
                        if shortcut {
                            actions.write(UiAction::SelectDevice(device.id));
                        }
                        if ui
                            .selectable_label(
                                state.selected == Selection::Device(device.id),
                                format!("{}  {label} × {}", row + 1, matching.len()),
                            )
                            .clicked()
                        {
                            actions.write(UiAction::SelectDevice(device.id));
                        }
                    }
                    ui.separator();
                    ui.heading("Cables");
                    let mut links: Vec<_> = sim.links().collect();
                    links.sort_by_key(|link| link.id);
                    if links.is_empty() {
                        ui.weak("No cables connected");
                    }
                    for link in links {
                        if ui
                            .selectable_label(
                                state.selected == Selection::Link(link.id),
                                format!(
                                    "Cable {:02} · {:?} · {:.2} m",
                                    link.id.0,
                                    link.color,
                                    link.length_cm as f32 / 100.0
                                ),
                            )
                            .clicked()
                        {
                            actions.write(UiAction::SelectLink(link.id));
                        }
                    }
                    ui.separator();
                    let conflicts = sim.duplicate_addresses();
                    if !conflicts.is_empty() {
                        ui.colored_label(egui::Color32::LIGHT_RED, "IP CONFLICT");
                        for (address, ports) in conflicts {
                            ui.label(format!("{address} on {} interfaces", ports.len()));
                        }
                    }
                });
        });
}
