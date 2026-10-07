use crate::app::*;
use crate::localization::tr;
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
                    ui.heading(tr("cable.setup"));
                    let stock = sim.cable_inventory();
                    ui.label(crate::localization::tr_args(
                        "ui.bulk-cable-m",
                        &[format!("{:.2}", stock.cable_cm as f32 / 100.0)],
                    ));
                    ui.label(crate::localization::tr_args(
                        "ui.rj45-connectors",
                        &[(stock.connectors).to_string()],
                    ));
                    let mut automatic = state.cable_length_cm.is_none();
                    if ui
                        .checkbox(&mut automatic, tr("ui.auto-shortest-path-5"))
                        .changed()
                    {
                        state.cable_length_cm = if automatic { None } else { Some(100) };
                    }
                    if let Some(cm) = &mut state.cable_length_cm {
                        ui.horizontal(|ui| {
                            ui.label(tr("ui.cut-length"));
                            ui.add(
                                egui::DragValue::new(cm)
                                    .range(1..=10000)
                                    .speed(1)
                                    .suffix(tr("ui.cm")),
                            );
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label(tr("ui.jacket-color"));
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
                                .on_hover_text(crate::localization::cable_color(color))
                                .clicked()
                            {
                                state.cable_color = color;
                            }
                        }
                    });
                    ui.weak(tr("ui.a-new-lead-uses-its-length-2"));
                    if !stock.patch_cables_cm.is_empty() {
                        ui.label(crate::localization::tr_args(
                            "ui.reusable-leads",
                            &[(stock.patch_cables_cm.len()).to_string()],
                        ));
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
                                .small_button(crate::localization::tr_args(
                                    "ui.m-use",
                                    &[
                                        format!("{:.2}", cm as f32 / 100.0),
                                        crate::localization::cable_color(color),
                                        (count).to_string(),
                                    ],
                                ))
                                .clicked()
                            {
                                state.cable_length_cm = Some(cm);
                                state.cable_color = color;
                            }
                        }
                    }
                    ui.separator();
                    super::optics::inventory(ui, sim);
                    ui.separator();
                    ui.heading(tr("ui.inventory"));
                    let inventory: Vec<_> = sim.devices().filter(|d| d.rack.is_none()).collect();
                    if inventory.is_empty() {
                        ui.weak(tr("ui.no-uninstalled-devices"));
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
                                crate::localization::tr_args(
                                    "inventory.device-group",
                                    &[
                                        (row + 1).to_string(),
                                        (label).to_string(),
                                        (matching.len()).to_string(),
                                    ],
                                ),
                            )
                            .clicked()
                        {
                            actions.write(UiAction::SelectDevice(device.id));
                        }
                    }
                    ui.separator();
                    ui.heading(tr("ui.cables"));
                    let mut links: Vec<_> = sim.links().collect();
                    links.sort_by_key(|link| link.id);
                    if links.is_empty() {
                        ui.weak(tr("ui.no-cables-connected"));
                    }
                    for link in links {
                        if ui
                            .selectable_label(
                                state.selected == Selection::Link(link.id),
                                crate::localization::tr_args(
                                    "ui.cable-m",
                                    &[
                                        format!("{:02}", link.id.0),
                                        crate::localization::cable_color(link.color),
                                        format!("{:.2}", link.length_cm as f32 / 100.0),
                                    ],
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
                        ui.colored_label(egui::Color32::LIGHT_RED, tr("ui.ip-conflict"));
                        for (address, ports) in conflicts {
                            ui.label(crate::localization::tr_args(
                                "ui.on-interfaces",
                                &[(address).to_string(), (ports.len()).to_string()],
                            ));
                        }
                    }
                });
        });
}
