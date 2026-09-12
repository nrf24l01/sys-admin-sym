use crate::app::*;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiTextureHandle, egui};
use cloud_provider_sim::*;
use std::collections::HashMap;
mod cables;
mod rack;
use cables::{CableScene, CableView};
use rack::RackLayout;

fn selected_link_id(state: &UiState, sim: &NetworkSim) -> Option<LinkId> {
    match state.selected {
        Selection::Link(id) => Some(id),
        Selection::Port(id) => sim.link_for_port(id).map(|link| link.id),
        _ => None,
    }
}

fn console_scroll_height(available_height: f32, controls_height: f32) -> f32 {
    (available_height - controls_height).max(0.0)
}

fn console_output_scroll(ui: &egui::Ui, controls_height: f32) -> egui::ScrollArea {
    let height = console_scroll_height(ui.available_height(), controls_height);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .min_scrolled_height(height)
        .max_height(height)
}

fn inventory_model_name(device: &Device) -> String {
    device
        .name
        .split_once(" #")
        .map_or_else(|| device.name.clone(), |(model, _)| model.to_owned())
}

fn source_label(sim: &NetworkSim, source: SourceId) -> String {
    match source {
        SourceId::Rack(id) => sim
            .rack(id)
            .map_or_else(|| format!("Rack {id}"), |r| format!("{} mains", r.name)),
        SourceId::Ups(id) => sim
            .devices()
            .find_map(|d| match d.kind {
                DeviceKind::Ups(ref u) if u.source == Some(source) => Some(d.name.clone()),
                _ => None,
            })
            .unwrap_or_else(|| format!("UPS {id}")),
        SourceId::Pdu(id) => sim
            .devices()
            .find_map(|d| match d.kind {
                DeviceKind::Pdu(ref p) if p.source == Some(source) => Some(d.name.clone()),
                _ => None,
            })
            .unwrap_or_else(|| format!("PDU {id}")),
    }
}

fn power_sources(sim: &NetworkSim) -> Vec<SourceId> {
    sim.racks()
        .map(|r| SourceId::Rack(r.id))
        .chain(sim.power.ups.keys().copied().map(SourceId::Ups))
        .chain(sim.power.pdus.keys().copied().map(SourceId::Pdu))
        .collect()
}

fn device_power_endpoint(device: &Device) -> Option<PowerEndpoint> {
    match device.kind {
        DeviceKind::Ups(ref x) => x.source.map(PowerEndpoint::Source),
        DeviceKind::Pdu(ref x) => x.source.map(PowerEndpoint::Source),
        _ => Some(PowerEndpoint::Device(device.id)),
    }
}

#[derive(Resource, Default)]
pub struct EquipmentImages {
    server_front: Handle<Image>,
    server_rear: Handle<Image>,
    switch_front: Handle<Image>,
    switch_rear: Handle<Image>,
    router_front: Handle<Image>,
    router_rear: Handle<Image>,
    jacket: Handle<Image>,
    plug: Handle<Image>,
    cables: CableScene,
    textures: Option<EquipmentTextures>,
}

#[derive(Clone, Copy)]
struct EquipmentTextures {
    server_front: egui::TextureId,
    server_rear: egui::TextureId,
    switch_front: egui::TextureId,
    switch_rear: egui::TextureId,
    router_front: egui::TextureId,
    router_rear: egui::TextureId,
    jacket: egui::TextureId,
    plug: egui::TextureId,
}

pub fn load_equipment_images(mut images: ResMut<EquipmentImages>, assets: Res<AssetServer>) {
    images.server_front = assets.load("equipment/server_front.png");
    images.server_rear = assets.load("equipment/server_rear_clean.png");
    images.switch_front = assets.load("equipment/switch_front_clean.png");
    images.switch_rear = assets.load("equipment/switch_rear.png");
    images.router_front = assets.load("equipment/router_rear_clean.png");
    images.router_rear = assets.load("equipment/router_front_clean.png");
    images.jacket = assets.load("cables/pvc_jacket.png");
    images.plug = assets.load("cables/rj45_plug.png");
}

pub fn main_ui(
    mut contexts: EguiContexts,
    snapshot: Res<SimSnapshot>,
    mut state: ResMut<UiState>,
    mut drafts: ResMut<EditorDrafts>,
    mut images: ResMut<EquipmentImages>,
    mut actions: MessageWriter<UiAction>,
) -> Result {
    if images.textures.is_none() {
        images.textures = Some(EquipmentTextures {
            server_front: contexts
                .add_image(EguiTextureHandle::Strong(images.server_front.clone())),
            server_rear: contexts.add_image(EguiTextureHandle::Strong(images.server_rear.clone())),
            switch_front: contexts
                .add_image(EguiTextureHandle::Strong(images.switch_front.clone())),
            switch_rear: contexts.add_image(EguiTextureHandle::Strong(images.switch_rear.clone())),
            router_front: contexts
                .add_image(EguiTextureHandle::Strong(images.router_front.clone())),
            router_rear: contexts.add_image(EguiTextureHandle::Strong(images.router_rear.clone())),
            jacket: contexts.add_image(EguiTextureHandle::Strong(images.jacket.clone())),
            plug: contexts.add_image(EguiTextureHandle::Strong(images.plug.clone())),
        });
    }
    let ctx = contexts.ctx_mut()?;
    configure_style(ctx);
    let mut viewport_ui = egui::Ui::new(
        ctx.clone(),
        "viewport".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );
    top_bar(&mut viewport_ui, &snapshot.0, &mut state, &mut actions);
    if let Some(error) = state.error_dialog.clone() {
        let mut open = true;
        egui::Window::new("Error")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(egui::vec2(420.0, 150.0))
            .show(&viewport_ui, |ui| {
                ui.colored_label(egui::Color32::LIGHT_RED, "Operation failed");
                ui.separator();
                ui.label(error);
                if ui.button("OK").clicked() {
                    state.error_dialog = None;
                }
            });
        if !open {
            state.error_dialog = None;
        }
    }
    shop_panel(&mut viewport_ui, &snapshot.0, &mut state, &mut actions);
    inspector_panel(
        &mut viewport_ui,
        &snapshot.0,
        &mut state,
        &mut drafts,
        &mut actions,
    );
    terminal_panel(&mut viewport_ui, &snapshot.0, &mut state, &mut actions);
    let textures = images.textures.expect("equipment textures registered");
    workspace(
        &mut viewport_ui,
        &snapshot.0,
        &mut state,
        &textures,
        &mut images.cables,
        &mut actions,
    );
    Ok(())
}

fn configure_style(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(13, 17, 23);
    visuals.window_fill = egui::Color32::from_rgb(17, 23, 31);
    visuals.selection.bg_fill = egui::Color32::from_rgb(31, 111, 91);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(40, 135, 111);
    ctx.set_visuals(visuals);
}

fn top_bar(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    egui::Panel::top("top_bar")
        .default_size(48.0)
        .show(viewport, |ui| {
            ui.horizontal_centered(|ui| {
                ui.heading("CLOUD PROVIDER // ROOM 01");
                ui.separator();
                ui.strong(format!("${}", sim.money));
                ui.separator();
                for (workspace, label) in
                    [(Workspace::Rack, "RACK"), (Workspace::Topology, "TOPOLOGY")]
                {
                    if ui
                        .selectable_label(state.workspace == workspace, label)
                        .clicked()
                    {
                        state.workspace = workspace;
                    }
                }
                ui.separator();
                if state.pending_cable.is_some() {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 196, 64),
                        format!(
                            "RJ45 CABLE: {} ANCHOR{} → SELECT PORT",
                            state.pending_cable_route.len(),
                            if state.pending_cable_route.len() == 1 {
                                ""
                            } else {
                                "S"
                            },
                        ),
                    );
                    if ui.small_button("Cancel cable").clicked() {
                        state.pending_cable = None;
                        state.pending_cable_route.clear();
                    }
                    ui.separator();
                }
                if ui.button("Save").clicked() {
                    actions.write(UiAction::Save);
                }
                if ui.button("Load").clicked() {
                    actions.write(UiAction::Load);
                }
                if ui.button("New").clicked() {
                    actions.write(UiAction::NewGame);
                }
                if let Some((notice, ok)) = &state.notice {
                    ui.separator();
                    ui.colored_label(
                        if *ok {
                            egui::Color32::LIGHT_GREEN
                        } else {
                            egui::Color32::LIGHT_RED
                        },
                        notice,
                    );
                }
            });
        });
}

fn shop_panel(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    egui::Panel::left("shop")
        .resizable(false)
        .default_size(220.0)
        .show(viewport, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("shop-scroll")
                .show(ui, |ui| {
                    ui.heading("Equipment shop");
                    ui.label("Buy real rack hardware, then install it into an empty U.");
                    ui.add_space(8.0);
                    for (template, label, detail) in [
                        (
                            DeviceTemplate::Router,
                            "Cisco ISR C1111-8P — $500",
                            "2 × WAN RJ45 + 8 × GE LAN RJ45",
                        ),
                        (
                            DeviceTemplate::Switch,
                            "Cisco C1000-24T-4G-L — $500",
                            "24 × RJ45 active · 4 × SFP coming later",
                        ),
                        (
                            DeviceTemplate::Server,
                            "Dell PowerEdge R360 — $1000",
                            "3 × rear RJ45 Ethernet: primary, secondary, management · 1U",
                        ),
                        (
                            DeviceTemplate::PatchPanel,
                            "24-port RJ45 patch panel — $150",
                            "24 passive RJ45 passthrough ports · front/rear paired · 1U",
                        ),
                        (
                            DeviceTemplate::CableManager,
                            "1U horizontal cable manager — $75",
                            "Front routing anchors · no network logic · 1U",
                        ),
                        (
                            DeviceTemplate::Ups,
                            "APC Smart-UPS SMT1500RMI2U — $1200",
                            "1000 W / 1500 VA · 4 C13 · battery backup",
                        ),
                        (
                            DeviceTemplate::Pdu,
                            "Rack PDU 8×C13 — $250",
                            "3680 W / 16 A · feed from UPS or mains",
                        ),
                    ] {
                        ui.group(|ui| {
                            ui.label(detail);
                            if ui
                                .add_enabled(
                                    sim.money >= template.price(),
                                    egui::Button::new(label),
                                )
                                .clicked()
                            {
                                actions.write(UiAction::Buy(template));
                            }
                        });
                    }
                    ui.separator();
                    ui.heading("Cable supplies");
                    let stock = sim.cable_inventory();
                    ui.label(format!(
                        "Bulk cable: {:.2} m",
                        stock.cable_cm as f32 / 100.0
                    ));
                    ui.label(format!("RJ45 connectors: {}", stock.connectors));
                    for (supply, label) in [
                        (CableSupply::CableBox305m, "305 m Ethernet cable box"),
                        (CableSupply::Rj45Pack20, "20 × RJ45 connectors"),
                    ] {
                        if ui
                            .add_enabled(
                                sim.money >= supply.price(),
                                egui::Button::new(format!("{label} — ${}", supply.price())),
                            )
                            .clicked()
                        {
                            actions.write(UiAction::BuyCableSupply(supply));
                        }
                    }
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
                            let fill = cable_color_value(color);
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
                            .entry(inventory_model_name(device))
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

fn inspector_panel(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    drafts: &mut EditorDrafts,
    actions: &mut MessageWriter<UiAction>,
) {
    let passive_selection = match state.selected {
        Selection::Device(id) => sim.device(id).is_some_and(|device| {
            matches!(
                device.kind,
                DeviceKind::PatchPanel(_) | DeviceKind::CableManager(_)
            )
        }),
        Selection::Port(id) => sim
            .port(id)
            .and_then(|port| sim.device(port.device))
            .is_some_and(|device| {
                matches!(
                    device.kind,
                    DeviceKind::PatchPanel(_) | DeviceKind::CableManager(_)
                )
            }),
        _ => false,
    };
    if passive_selection {
        return;
    }
    egui::Panel::right("inspector")
        .default_size(360.0)
        .show(viewport, |ui| {
            ui.heading("Inspector");
            ui.separator();
            match state.selected {
                Selection::None => {
                    ui.label("Select a device, port, or cable.");
                }
                Selection::Device(id) => device_inspector(ui, sim, id, state, actions),
                Selection::Port(id) => port_inspector(ui, sim, id, drafts, actions),
                Selection::Link(id) => link_inspector(ui, sim, id, actions),
            }
        });
}

fn device_inspector(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    id: DeviceId,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    let Some(device) = sim.device(id) else {
        ui.label("Device no longer exists");
        return;
    };
    ui.heading(&device.name);
    power_controls(ui, sim, device, state, actions);
    let actual_powered = device.powered
        || match device.kind {
            DeviceKind::Ups(ref x) => x
                .source
                .and_then(|s| sim.power.source_telemetry(s))
                .is_some_and(|t| t.available),
            DeviceKind::Pdu(ref x) => x
                .source
                .and_then(|s| sim.power.source_telemetry(s))
                .is_some_and(|t| t.available),
            _ => false,
        };
    let passive = matches!(
        device.kind,
        DeviceKind::PatchPanel(_) | DeviceKind::CableManager(_)
    );
    if passive {
        if device.rack.is_some()
            && ui.button("Eject from rack").on_hover_text(
                "Uninstall this device; connected cables are unplugged and returned to inventory.",
            ).clicked()
        {
            actions.write(UiAction::Remove(id));
        }
        ui.separator();
        ui.weak("Passive rack hardware has no power, status, terminal, or configuration controls. Use its rack sockets to connect and unplug cables.");
        return;
    }
    ui.colored_label(
        if actual_powered {
            egui::Color32::GREEN
        } else {
            egui::Color32::DARK_GRAY
        },
        if actual_powered {
            "● POWER ON"
        } else {
            "○ POWER OFF"
        },
    );
    if device.rack.is_some()
        && ui
            .button("Eject from rack")
            .on_hover_text(
                "Uninstall this device; connected cables are unplugged and returned to inventory.",
            )
            .clicked()
    {
        actions.write(UiAction::Remove(id));
    }
    if device.rack.is_none() {
        ui.label("Select an empty rack unit in the Rack view to install this device.");
        state.workspace = Workspace::Rack;
    }
    ui.separator();
    ui.strong("Ports");
    egui::Grid::new("device_ports")
        .num_columns(3)
        .striped(true)
        .show(ui, |ui| {
            for port_id in device.ports() {
                let port = sim.port(*port_id).expect("device port exists");
                let link = sim.link_for_port(*port_id);
                ui.colored_label(
                    if link.is_some() && device.powered {
                        egui::Color32::GREEN
                    } else {
                        egui::Color32::GRAY
                    },
                    if link.is_some() { "●" } else { "○" },
                );
                if ui
                    .add_enabled(
                        port.connector.supports_cabling(),
                        egui::Button::selectable(false, &port.name),
                    )
                    .on_hover_text(if port.connector.supports_cabling() {
                        "Select port"
                    } else {
                        "SFP cabling is not implemented yet"
                    })
                    .clicked()
                {
                    actions.write(UiAction::SelectPort(*port_id));
                }
                ui.weak(connector_label(port.connector));
                ui.end_row();
            }
        });
    if let DeviceKind::Switch(sw) = &device.kind {
        ui.separator();
        ui.strong("VLAN database");
        for vlan in &sw.vlans {
            ui.label(format!("{} — {}", vlan.id.0, vlan.name));
        }
        ui.horizontal(|ui| {
            ui.label("ID");
            ui.text_edit_singleline(&mut state.new_vlan_id);
        });
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut state.new_vlan_name);
        });
        if ui.button("Create VLAN").clicked() {
            actions.write(UiAction::CreateVlan(id));
        }
    }
}

fn power_controls(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    device: &Device,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    let source = match device.kind {
        DeviceKind::Ups(ref x) => x.source,
        DeviceKind::Pdu(ref x) => x.source,
        _ => None,
    };
    if let Some(power) = sim.power.device_status(device.id) {
        ui.label(format!(
            "Load: {} W / {} VA / {:.2} A",
            power.load.watts,
            power.load.va,
            power.load.current_ma as f32 / 1000.0
        ));
        ui.colored_label(
            if power.effective {
                egui::Color32::LIGHT_GREEN
            } else {
                egui::Color32::YELLOW
            },
            if power.effective {
                "Effective power: ON"
            } else if power.requested {
                "Requested ON · source unavailable"
            } else {
                "Requested OFF"
            },
        );
        if ui
            .button(if power.requested {
                "Turn device off"
            } else {
                "Turn device on"
            })
            .clicked()
        {
            actions.write(UiAction::TogglePower(device.id, !power.requested));
        }
    }
    if let Some(source) = source {
        let telemetry = sim.power.source_telemetry(source);
        ui.separator();
        ui.strong(if matches!(device.kind, DeviceKind::Ups(_)) {
            "UPS status"
        } else {
            "PDU status"
        });
        let enabled = sim.power.source_enabled(source);
        if ui
            .button(if enabled {
                "Turn source off"
            } else {
                "Turn source on"
            })
            .clicked()
        {
            actions.write(UiAction::TogglePower(device.id, !enabled));
        }
        if let Some(t) = telemetry {
            let state = if !t.available {
                "OFF / TRIPPED / NO INPUT"
            } else if matches!(source, SourceId::Ups(_)) && !t.input_available {
                "ON BATTERY"
            } else {
                "ON MAINS"
            };
            ui.label(format!(
                "{state} · output {} W / {} VA / {:.2} A",
                t.output.watts,
                t.output.va,
                t.output.current_ma as f32 / 1000.0
            ));
            ui.label(format!(
                "input {} W / {} VA / {:.2} A",
                t.input.watts,
                t.input.va,
                t.input.current_ma as f32 / 1000.0
            ));
            if let SourceId::Ups(id) = source {
                let cap = sim.power.ups.get(&id).map_or(0, |u| u.spec.battery_wh);
                ui.label(format!(
                    "Battery: {} / {} Wh ({:.0}%){}",
                    t.battery_mwh / 1000,
                    cap,
                    if cap == 0 {
                        0.0
                    } else {
                        t.battery_mwh as f32 / (cap as f32 * 1000.0) * 100.0
                    },
                    t.runtime_seconds.map_or(String::new(), |s| format!(
                        " · runtime {}m {}s",
                        s / 60,
                        s % 60
                    ))
                ));
                if sim.power.ups.get(&id).is_some_and(|u| u.tripped)
                    && ui.button("Reset UPS breaker").clicked()
                {
                    actions.write(UiAction::ResetPower(source));
                }
            } else if sim
                .power
                .pdus
                .get(&match source {
                    SourceId::Pdu(id) => id,
                    _ => 0,
                })
                .is_some_and(|p| p.tripped)
                && ui.button("Reset PDU breaker").clicked()
            {
                actions.write(UiAction::ResetPower(source));
            }
        }
        ui.label("Input: connect this device's C14 inlet to a rack or upstream C13 outlet.");
    }
    let endpoint = device_power_endpoint(device);
    let connected = endpoint.and_then(|e| {
        sim.power
            .connections
            .iter()
            .find(|(_, x)| **x == e)
            .map(|(o, _)| *o)
    });
    if let Some(outlet) = connected {
        ui.horizontal(|ui| {
            ui.label(format!(
                "Fed by {} C13-{}",
                source_label(sim, outlet.source),
                outlet.index + 1
            ));
            if ui.small_button("Unplug").clicked() {
                actions.write(UiAction::DisconnectPower(outlet));
            }
        });
    } else if endpoint.is_some() {
        let sources = power_sources(sim);
        if state.power_source.is_none() || !sources.contains(&state.power_source.unwrap()) {
            state.power_source = sources.first().copied();
            state.power_outlet = 0;
        }
        egui::ComboBox::from_id_salt(("power-source", device.id.0))
            .selected_text(
                state
                    .power_source
                    .map_or_else(|| "No power sources".into(), |s| source_label(sim, s)),
            )
            .show_ui(ui, |ui| {
                for s in &sources {
                    if ui
                        .selectable_value(&mut state.power_source, Some(*s), source_label(sim, *s))
                        .changed()
                    {
                        state.power_outlet = 0;
                    }
                }
            });
        if let Some(s) = state.power_source {
            let max = sim.power.outlets(s) as u8;
            if state.power_outlet >= max {
                state.power_outlet = 0;
            }
            egui::ComboBox::from_id_salt(("power-outlet", device.id.0))
                .selected_text(format!("C13-{}", state.power_outlet + 1))
                .show_ui(ui, |ui| {
                    for n in 0..max {
                        let occupied = sim.power.connections.contains_key(&OutletId {
                            source: s,
                            index: n,
                        });
                        ui.add_enabled_ui(!occupied, |ui| {
                            ui.selectable_value(
                                &mut state.power_outlet,
                                n,
                                format!(
                                    "C13-{}{}",
                                    n + 1,
                                    if occupied { " (occupied)" } else { "" }
                                ),
                            );
                        });
                    }
                });
            if ui.button("Connect power").clicked()
                && !sim.power.connections.contains_key(&OutletId {
                    source: s,
                    index: state.power_outlet,
                })
                && let Some(endpoint) = endpoint
            {
                actions.write(UiAction::ConnectPower(
                    OutletId {
                        source: s,
                        index: state.power_outlet,
                    },
                    endpoint,
                ));
            }
        }
    }
}

fn port_inspector(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    id: PortId,
    drafts: &mut EditorDrafts,
    actions: &mut MessageWriter<UiAction>,
) {
    let Some(port) = sim.port(id) else {
        ui.label("Port no longer exists");
        return;
    };
    let owner = sim.device(port.device).expect("port owner exists");
    ui.heading(format!("{} / {}", owner.name, port.name));
    ui.label(format!("Connector: {}", connector_label(port.connector)));
    if !port.connector.supports_cabling() {
        ui.colored_label(
            egui::Color32::from_rgb(255, 196, 64),
            "SFP is visible on the physical device but is not implemented in this MVP.",
        );
        return;
    }
    let link = sim.link_for_port(id);
    ui.label(match link {
        Some(link) => format!("LINK UP · cable {}", link.id.0),
        None => "LINK DOWN".into(),
    });
    if link.is_none() && ui.button("Connect RJ45 cable").clicked() {
        actions.write(UiAction::CablePort(id));
    }
    ui.weak("Select the other socket in the rack, or its Connect RJ45 cable button.");
    if let Some(link) = link
        && ui.button("Disconnect cable").clicked()
    {
        actions.write(UiAction::Disconnect(link.id));
    }
    if matches!(
        owner.kind,
        DeviceKind::PatchPanel(_) | DeviceKind::CableManager(_)
    ) {
        ui.weak("Passive port: cable connections are managed from the rack view.");
        return;
    }
    ui.separator();
    match &port.config {
        PortConfig::Server(config) => {
            let draft = drafts.servers.entry(id).or_insert_with(|| {
                let ip = config.ipv4.as_ref();
                ServerDraft {
                    address: ip.map(|v| v.address.to_string()).unwrap_or_default(),
                    prefix: ip
                        .map(|v| v.prefix.to_string())
                        .unwrap_or_else(|| "24".into()),
                    gateway: ip
                        .and_then(|v| v.gateway)
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    vlan: ip
                        .and_then(|v| v.vlan)
                        .map(|v| v.0.to_string())
                        .unwrap_or_default(),
                    hostname: match &owner.kind {
                        DeviceKind::Server(v) => v.hostname.clone(),
                        _ => String::new(),
                    },
                }
            });
            ui.label("Hostname");
            ui.text_edit_singleline(&mut draft.hostname);
            ui.label("IPv4 address");
            ui.text_edit_singleline(&mut draft.address);
            ui.horizontal(|ui| {
                ui.label("Prefix");
                ui.text_edit_singleline(&mut draft.prefix);
            });
            ui.label("Access VLAN (optional; blank = untagged)");
            ui.text_edit_singleline(&mut draft.vlan);
            ui.label("Default gateway");
            ui.text_edit_singleline(&mut draft.gateway);
            if ui.button("Apply server config").clicked() {
                actions.write(UiAction::ApplyServer(id));
            }
        }
        PortConfig::Switch(config) => {
            let draft = drafts
                .switches
                .entry(id)
                .or_insert_with(|| match &config.mode {
                    SwitchPortMode::Access { vlan } => SwitchPortDraft {
                        vlan: vlan.map(|v| v.0.to_string()).unwrap_or_default(),
                        allowed: String::new(),
                        trunk: false,
                    },
                    SwitchPortMode::Trunk { allowed, .. } => SwitchPortDraft {
                        vlan: String::new(),
                        allowed: allowed
                            .iter()
                            .map(|v| v.0.to_string())
                            .collect::<Vec<_>>()
                            .join(","),
                        trunk: true,
                    },
                });
            ui.checkbox(&mut draft.trunk, "Trunk mode");
            if draft.trunk {
                ui.label("Allowed VLANs (comma-separated)");
                ui.text_edit_singleline(&mut draft.allowed);
            } else {
                ui.label("Access VLAN (optional; blank = untagged)");
                ui.text_edit_singleline(&mut draft.vlan);
            }
            if ui.button("Apply switch port").clicked() {
                actions.write(UiAction::ApplySwitch(id));
            }
        }
        PortConfig::Router(config) => {
            let first = config.interfaces.first();
            let draft = drafts.routers.entry(id).or_insert_with(|| RouterDraft {
                name: first
                    .map(|v| v.name.clone())
                    .unwrap_or_else(|| port.name.clone()),
                address: first
                    .and_then(|v| v.address)
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                prefix: first
                    .map(|v| v.prefix.to_string())
                    .unwrap_or_else(|| "24".into()),
                vlan: first
                    .and_then(|v| v.vlan)
                    .map(|v| v.0.to_string())
                    .unwrap_or_default(),
                internet: first.is_some_and(|v| v.internet_connected),
            });
            ui.label("Interface name");
            ui.text_edit_singleline(&mut draft.name);
            ui.label("IPv4 (blank = DHCP)");
            ui.text_edit_singleline(&mut draft.address);
            ui.horizontal(|ui| {
                ui.label("Prefix");
                ui.text_edit_singleline(&mut draft.prefix);
                ui.label("VLAN");
                ui.text_edit_singleline(&mut draft.vlan);
            });
            ui.checkbox(&mut draft.internet, "Internet connected");
            if ui.button("Apply router interface").clicked() {
                actions.write(UiAction::ApplyRouter(id));
            }
        }
        PortConfig::PatchPanel | PortConfig::CableManager => {
            ui.label("Passive physical interface; paired ports follow the rack side.");
        }
    }
    ui.separator();
    if ui
        .button("Flush configuration")
        .on_hover_text(
            "Reset this interface to its device template defaults; link and power stay unchanged.",
        )
        .clicked()
    {
        actions.write(UiAction::FlushPortConfig(id));
    }
}

fn link_inspector(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    id: LinkId,
    actions: &mut MessageWriter<UiAction>,
) {
    let Some(link) = sim.link(id) else {
        ui.label("Cable no longer exists");
        return;
    };
    let endpoint = |id| {
        sim.port(id)
            .map(|p| {
                format!(
                    "{} / {}",
                    sim.device(p.device).map(|d| d.name.as_str()).unwrap_or("?"),
                    p.name
                )
            })
            .unwrap_or_default()
    };
    ui.heading(format!("Cable {}", id.0));
    ui.label(format!(
        "{:.2} m {:?} Ethernet lead · 2 RJ45 plugs",
        link.length_cm as f32 / 100.0,
        link.color,
    ));
    ui.weak("Drag its jacket in the rack to move the slack. Unplugging returns the finished lead for reuse.");
    ui.label(endpoint(link.a));
    ui.label("↕");
    ui.label(endpoint(link.b));
    ui.separator();
    ui.strong("Manual route");
    ui.weak("Ordered rack anchors shape the physical cable path.");
    let route = link.route.clone();
    for (index, point) in route.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.monospace(format!(
                "{}: U{} {:?} {} cm",
                index + 1,
                point.unit,
                point.side,
                point.offset_cm
            ));
            if ui.small_button("×").clicked() {
                actions.write(UiAction::RemoveCableRoutePoint { link: id, index });
            }
            if index > 0 && ui.small_button("↑").clicked() {
                let mut reordered = route.clone();
                reordered.swap(index - 1, index);
                actions.write(UiAction::RerouteCable {
                    link: id,
                    route: reordered,
                });
            }
            if index + 1 < route.len() && ui.small_button("↓").clicked() {
                let mut reordered = route.clone();
                reordered.swap(index, index + 1);
                actions.write(UiAction::RerouteCable {
                    link: id,
                    route: reordered,
                });
            }
            if ui.small_button("Left").clicked() {
                let mut moved = *point;
                moved.offset_cm = 0;
                actions.write(UiAction::MoveCableRoutePoint {
                    link: id,
                    index,
                    point: moved,
                });
            }
            if ui.small_button("Right").clicked() {
                let mut moved = *point;
                moved.offset_cm = 48;
                actions.write(UiAction::MoveCableRoutePoint {
                    link: id,
                    index,
                    point: moved,
                });
            }
        });
    }
    if route.is_empty() {
        ui.label("No anchors; click a rack anchor to add one.");
    }
    if ui.button("Disconnect").clicked() {
        actions.write(UiAction::Disconnect(id));
    }
}

fn terminal_panel(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    // Existing terminal windows remain visible even when the rack selection is
    // a passive device. Passive hardware itself has no console dock or action.
    let windows = state.terminal_windows.iter().copied().collect::<Vec<_>>();
    for window_device in windows.iter().copied() {
        terminal_window(viewport, sim, state, window_device, actions);
    }
    let device = match state.selected {
        Selection::Device(id) => sim.device(id).map(|d| d.id),
        Selection::Port(id) => sim.port(id).map(|p| p.device),
        _ => None,
    };
    let Some(device) = device.filter(|id| sim.device(*id).is_some_and(is_console_device)) else {
        return;
    };
    let dev = sim.device(device).unwrap();
    let server = matches!(dev.kind, DeviceKind::Server(_));
    if state.terminal_windows.contains(&device) {
        return;
    }
    let console = state.terminals.entry(device).or_default();
    egui::Panel::bottom("terminal")
        .resizable(true)
        .default_size(240.0)
        .show(viewport, |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("{} — Console", dev.name));
                if ui.small_button("Open terminal window").clicked() {
                    actions.write(UiAction::LaunchExternalTerminal(device));
                }
                if ui.small_button("Clear output").clicked() { console.lines.clear(); }
                if !server { ui.checkbox(&mut console.script_mode, "Paste configuration"); }
            });
            if !server { ui.weak("? help · Up/Down history · Tab completion · Ctrl-Z end · Scripts stop at the first error"); }
            if !dev.powered { ui.colored_label(egui::Color32::YELLOW, "Power on this device in the Inspector to use its console."); }
            console_output_scroll(ui, if console.script_mode { 120.0 } else { 70.0 })
                .id_salt(("console-output", device.0))
                .max_height(console_scroll_height(
                    ui.available_height(),
                    if console.script_mode { 120.0 } else { 70.0 },
                ).max(60.0))
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if console.lines.is_empty() {
                        ui.monospace(if server { "Type help for server commands." } else { "IOS-style simulator console. Type enable, then configure terminal. Type ? to see supported commands." });
                    }
                    for line in &console.lines { ui.monospace(line); }
                });
            ui.horizontal(|ui| {
                ui.monospace(sim.terminal_prompt(device));
                let input_id = egui::Id::new(("console-input", device.0));
                let focused = ui.memory(|m| m.has_focus(input_id));
                if focused && !console.script_mode {
                    let (up, down, tab, end) = ui.input_mut(|i| (
                        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                        i.consume_key(egui::Modifiers::NONE, egui::Key::Tab),
                        i.consume_key(egui::Modifiers::CTRL, egui::Key::Z),
                    ));
                    if up && !console.history.is_empty() {
                        let pos = console.history_position.unwrap_or(console.history.len()).saturating_sub(1);
                        console.history_position = Some(pos);
                        console.input = console.history[pos].clone();
                    }
                    if down && let Some(pos) = console.history_position {
                        let next = pos + 1;
                        console.history_position = (next < console.history.len()).then_some(next);
                        console.input = console.history.get(next).cloned().unwrap_or_default();
                    }
                    if tab && !server {
                        let completions = sim.console_help(device, &console.input);
                        if completions.len() == 1 && !completions[0].contains('<') { console.input = completions[0].clone(); }
                        else { console.lines.extend(completions); }
                    }
                    if end && !server { actions.write(UiAction::RunTerminal(device, "end".into())); }
                }
                let editor = if console.script_mode {
                    egui::TextEdit::multiline(&mut console.input).desired_rows(3)
                } else { egui::TextEdit::singleline(&mut console.input) };
                let response = ui.add_enabled(dev.powered, editor
                    .id(input_id).font(egui::TextStyle::Monospace)
                    .desired_width((ui.available_width() - 65.0).max(80.0))
                    .hint_text(if server { "help" } else { "Enter command" }));
                let enter = !console.script_mode && response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.add_enabled(dev.powered, egui::Button::new("Run")).clicked() || (dev.powered && enter) {
                    let input = std::mem::take(&mut console.input);
                    if !input.trim().is_empty() {
                        console.history.push(input.clone());
                        if console.history.len() > 100 { console.history.remove(0); }
                    }
                    console.history_position = None;
                    actions.write(UiAction::RunTerminal(device, input));
                    response.request_focus();
                }
            });
        });
}

fn is_console_device(device: &Device) -> bool {
    matches!(
        device.kind,
        DeviceKind::Server(_) | DeviceKind::Switch(_) | DeviceKind::Router(_)
    )
}

fn terminal_window(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    device: DeviceId,
    actions: &mut MessageWriter<UiAction>,
) {
    let Some(dev) = sim.device(device) else {
        state.terminal_windows.remove(&device);
        state.terminal_window_focus.remove(&device);
        return;
    };
    if !is_console_device(dev) {
        state.terminal_windows.remove(&device);
        state.terminal_window_focus.remove(&device);
        return;
    }
    let powered = dev.powered;
    let name = dev.name.clone();
    let server = matches!(dev.kind, DeviceKind::Server(_));
    let console = state.terminals.entry(device).or_default();
    let mut open = true;
    egui::Window::new(format!("Terminal — {name}"))
        .open(&mut open)
        .resizable(true)
        .default_size(egui::vec2(760.0, 480.0))
        .min_size(egui::vec2(420.0, 260.0))
        .frame(egui::Frame::window(viewport.style()).fill(egui::Color32::from_rgb(8, 10, 12)))
        .show(viewport, |ui| {
            ui.colored_label(
                egui::Color32::from_rgb(125, 190, 145),
                if server {
                    "SERVER CONSOLE • LIVE"
                } else {
                    "IOS CONSOLE • LIVE"
                },
            );
            console_output_scroll(ui, 58.0)
                .max_height((ui.available_height() - 58.0).max(0.0))
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if console.lines.is_empty() {
                        ui.colored_label(
                            egui::Color32::from_rgb(145, 180, 150),
                            if server {
                                "Linux terminal · type help to begin"
                            } else {
                                "IOS-style simulator console · type ? for help"
                            },
                        );
                    }
                    for line in &console.lines {
                        ui.colored_label(
                            egui::Color32::from_rgb(190, 205, 195),
                            egui::RichText::new(line).monospace(),
                        );
                    }
                });
            ui.separator();
            ui.horizontal(|ui| {
                ui.monospace(sim.terminal_prompt(device));
                let response = ui.add_enabled(
                    powered,
                    egui::TextEdit::singleline(&mut console.input)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(ui.available_width() - 55.0),
                );
                if state.terminal_window_focus.contains(&device) {
                    response.request_focus();
                }
                if (ui.button("Run").clicked()
                    || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))))
                    && powered
                {
                    let input = std::mem::take(&mut console.input);
                    if !input.trim().is_empty() {
                        console.history.push(input.clone());
                    }
                    actions.write(UiAction::RunTerminal(device, input));
                    state.terminal_window_focus.insert(device);
                    response.request_focus();
                }
            });
        });
    state.terminal_window_focus.remove(&device);
    if !open {
        state.terminal_windows.remove(&device);
    }
}

fn workspace(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    textures: &EquipmentTextures,
    cables: &mut CableScene,
    actions: &mut MessageWriter<UiAction>,
) {
    match state.workspace {
        Workspace::Rack => rack_view(viewport, sim, state, textures, cables, actions),
        Workspace::Topology => topology_view(viewport, sim, actions),
    }
}

fn rack_view(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    textures: &EquipmentTextures,
    cables: &mut CableScene,
    actions: &mut MessageWriter<UiAction>,
) {
    egui::CentralPanel::default().show(viewport, |ui| {
        let Some(rack) = sim.racks().min_by_key(|r| r.id) else {
            return;
        };
        ui.vertical_centered(|ui| {
            ui.heading(format!("{} · {:?} SIDE", rack.name, state.rack_side));
            ui.horizontal(|ui| {
                for side in [RackSide::Front, RackSide::Rear] {
                    if ui
                        .selectable_label(state.rack_side == side, format!("{side:?}"))
                        .clicked()
                    {
                        state.rack_side = side;
                    }
                }
                ui.separator();
                ui.label("Cables:");
                for visibility in [CableVisibility::All, CableVisibility::Selected, CableVisibility::Hidden] {
                    if ui.selectable_label(state.cable_visibility == visibility, format!("{visibility:?}")).clicked() {
                        state.cable_visibility = visibility;
                    }
                }
            });
            ui.weak(
                "Buy cable + RJ45 plugs, then click two sockets. Select a cable, then click the visible left/right rail anchors to add or remove route points. Drag a wire to move its slack.",
            );
            if let Some(rp) = sim.power.racks.get(&rack.id) {
                ui.horizontal(|ui| {
                    let reading = sim.power.source_telemetry(SourceId::Rack(rack.id));
                    let watts = reading.map_or(0, |x| x.output.watts);
                    let amps = reading.map_or(0.0, |x| x.output.current_ma as f32 / 1000.0);
                    ui.label(format!("Power: mains {} · breaker {} · {} W / {:.2} A · 4 C13 outlets", if rp.mains_on { "ON" } else { "OFF" }, if rp.breaker_on { "OK" } else { "TRIPPED" }, watts, amps));
                    if ui.button(if rp.mains_on { "Mains off" } else { "Mains on" }).clicked() { actions.write(UiAction::RackMains(rack.id, !rp.mains_on)); }
                    if !rp.breaker_on && ui.button("Reset breaker").clicked() { actions.write(UiAction::ResetPower(SourceId::Rack(rack.id))); }
                });
                for source in power_sources(sim).into_iter().filter(|s| !matches!(s, SourceId::Rack(_))) {
                    if let Some(t) = sim.power.source_telemetry(source) { ui.label(format!("{}: {} W / {} VA · {}", source_label(sim, source), t.output.watts, t.output.va, if t.available { "ONLINE" } else { "OFFLINE" })); }
                }
                ui.horizontal(|ui| {
                    for n in 0..4u8 {
                        let outlet = OutletId { source: SourceId::Rack(rack.id), index: n };
                        let occupied = sim.power.connections.contains_key(&outlet);
                        if ui.add_enabled(!occupied, egui::Button::new(format!("C13-{} {}", n + 1, if occupied { "●" } else { "○" }))).on_hover_text(if occupied { "Occupied · use Unplug in the device inspector" } else { "Select this outlet, then click a rear C14 inlet" }).clicked() { state.pending_power_outlet = Some(outlet); state.pending_cable = None; state.pending_cable_route.clear(); }
                        if occupied && ui.small_button("×").clicked() { actions.write(UiAction::DisconnectPower(outlet)); }
                    }
                });
                if let Some(outlet) = state.pending_power_outlet { ui.colored_label(egui::Color32::LIGHT_YELLOW, format!("Selected {} C13-{} · click a rear C14 inlet", source_label(sim, outlet.source), outlet.index + 1)); }
                if state.pending_power_outlet.is_some() && (ui.button("Cancel power lead").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape))) { state.pending_power_outlet = None; }
            }
        });
            egui::ScrollArea::both()
                .id_salt("rack-scroll")
                .show(ui, |ui| {
                let selected_inventory = match state.selected {
                    Selection::Device(id) if sim.device(id).is_some_and(|d| d.rack.is_none()) => {
                        Some(id)
                    }
                    _ => None,
                };
                let layout = RackLayout::new((ui.available_width() - 20.0).min(960.0));
                let panel_width = layout.face_width;
                let rack_width = layout.width;
                let row_height = layout.row_height;
                let mut port_visuals = Vec::new();
                let mut led_visuals = Vec::new();
                let mut power_endpoints: Vec<(PowerEndpoint, egui::Pos2)> = Vec::new();
                let mut power_outlets: HashMap<OutletId, egui::Pos2> = HashMap::new();
                let mut route_anchors: Vec<(RackId, u8, RackSide, u16, egui::Pos2)> = Vec::new();
                let rack_frame = egui::Frame::group(ui.style())
                    .fill(egui::Color32::from_rgb(8, 10, 13))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width(rack_width);
                        ui.spacing_mut().item_spacing.y = 0.0;
                        layout.paint_crossbar(ui, &format!("{} U  /  {:?}", rack.units, state.rack_side).to_uppercase());
                        ui.horizontal(|ui| {
                            ui.label("RACK C13:");
                            for n in 0..4u8 {
                                let outlet = OutletId { source: SourceId::Rack(rack.id), index: n };
                                let occupied = sim.power.connections.contains_key(&outlet);
                                let response = ui.add_enabled(!occupied, egui::Button::new(format!("C13-{}", n + 1))).on_hover_text("Select rack outlet, then click a rear C14 inlet");
                                power_outlets.insert(outlet, response.rect.center());
                                if response.clicked() { state.pending_power_outlet = Some(outlet); state.pending_cable = None; state.pending_cable_route.clear(); }
                            }
                        });
                        for unit in (1..=rack.units).rev() {
                            let (row_rect, row_response) = ui.allocate_exact_size(
                                egui::vec2(rack_width, row_height),
                                egui::Sense::click(),
                            );
                            let row = layout.row(row_rect);
                            let panel_rect = row.face;
                            row.paint(ui.painter(), unit, rack.occupies(unit).is_some());
                            for (offset_cm, position) in [0, 48].into_iter().zip(row.anchors) {
                                route_anchors.push((rack.id, unit, state.rack_side, offset_cm, position));
                            }
                            if let Some(device_id) = rack.occupies(unit) {
                                let device = sim.device(device_id).expect("rack device exists");
                                row_response.context_menu(|menu| {
                                    menu.label(device.name.as_str());
                                    if let Some(endpoint) = device_power_endpoint(device) && let Some((outlet, _)) = sim.power.connections.iter().find(|(_, target)| **target == endpoint) && menu.button(format!("Unplug power ({} C13-{})", source_label(sim, outlet.source), outlet.index + 1)).clicked() {
                                                actions.write(UiAction::DisconnectPower(*outlet));
                                                menu.close();
                                    }
                                    if menu.button("Eject from rack").clicked() {
                                        actions.write(UiAction::Remove(device_id));
                                        menu.close();
                                    }
                                });
                                if matches!(device.kind, DeviceKind::CableManager(_))
                                    && state.rack_side == RackSide::Front
                                {
                                    // The end slots share route identities with the rail anchors.
                                    for slot in 1..5u16 {
                                        let offset_cm = slot * 48 / 5;
                                        route_anchors.push((
                                            rack.id,
                                            unit,
                                            RackSide::Front,
                                            offset_cm,
                                            egui::pos2(
                                                panel_rect.left() + panel_rect.width() * offset_cm as f32 / 48.0,
                                                panel_rect.center().y,
                                            ),
                                        ));
                                    }
                                }
                                ui.painter()
                                    .rect_filled(panel_rect, 2.0, egui::Color32::BLACK);
                                match device.kind {
                                    DeviceKind::PatchPanel(_) => {
                                        ui.painter().rect_filled(
                                            panel_rect.shrink(3.0),
                                            1.0,
                                            egui::Color32::from_rgb(43, 48, 54),
                                        );
                                        let sides: Vec<_> = device.ports().iter().map(|id| sim.port(*id).expect("patch port exists").side).collect();
                                        for (slot, index) in visible_patch_port_indices(&sides, state.rack_side).into_iter().enumerate() {
                                            let port_id = &device.ports()[index];
                                            let port = sim.port(*port_id).expect("patch port exists");
                                            let socket = rack_port_rect(&device.kind, panel_rect, index, port.connector);
                                            ui.painter().rect_filled(
                                                socket,
                                                1.0,
                                                egui::Color32::from_rgb(12, 15, 18),
                                            );
                                            ui.painter().text(
                                                egui::pos2(socket.center().x, socket.top() - 1.0),
                                                egui::Align2::CENTER_BOTTOM,
                                                format!("{:02}", slot + 1),
                                                egui::FontId::monospace(6.0),
                                                egui::Color32::from_gray(170),
                                            );
                                        }
                                    }
                                    DeviceKind::CableManager(_) => {
                                        ui.painter().rect_filled(
                                            panel_rect.shrink(3.0),
                                            1.0,
                                            egui::Color32::from_rgb(31, 37, 42),
                                        );
                                        for slot in 1..5 {
                                            let offset_cm = slot * 48 / 5;
                                            let center = egui::pos2(
                                                panel_rect.left() + panel_rect.width() * offset_cm as f32 / 48.0,
                                                panel_rect.center().y,
                                            );
                                            ui.painter().circle_stroke(
                                                center,
                                                5.0,
                                                egui::Stroke::new(2.0, egui::Color32::from_rgb(82, 92, 99)),
                                            );
                                        }
                                    }
                                    _ => {
                                        let textured = matches!(device.kind, DeviceKind::Server(_)
                                            | DeviceKind::Switch(_) | DeviceKind::Router(_));
                                        if textured {
                                            let texture = match device.kind {
                                                DeviceKind::Server(_) if state.rack_side == RackSide::Front => textures.server_front,
                                                DeviceKind::Server(_) => textures.server_rear,
                                                DeviceKind::Switch(_) if state.rack_side == RackSide::Front => textures.switch_front,
                                                DeviceKind::Switch(_) => textures.switch_rear,
                                                DeviceKind::Router(_) if state.rack_side == RackSide::Front => textures.router_front,
                                                DeviceKind::Router(_) => textures.router_rear,
                                                _ => unreachable!(),
                                            };
                                            ui.painter().image(texture, panel_rect, equipment_uv(&device.kind, state.rack_side), if device.powered { egui::Color32::WHITE } else { egui::Color32::from_gray(90) });
                                        } else {
                                            ui.painter().rect_filled(panel_rect.shrink(3.0), 1.0, egui::Color32::from_rgb(38, 44, 50));
                                            ui.painter().rect_stroke(panel_rect.shrink(8.0), 1.0, egui::Stroke::new(1.0, egui::Color32::from_gray(75)), egui::StrokeKind::Inside);
                                            ui.painter().circle_filled(egui::pos2(panel_rect.left() + 14.0, panel_rect.center().y), 4.0, if device.powered { egui::Color32::GREEN } else { egui::Color32::from_gray(35) });
                                            if matches!(device.kind, DeviceKind::Switch(_) | DeviceKind::Router(_)) {
                                                for fan in 0..3 { ui.painter().circle_stroke(egui::pos2(panel_rect.center().x + (fan as f32 - 1.0) * 18.0, panel_rect.center().y), 7.0, egui::Stroke::new(1.0, egui::Color32::from_gray(85))); }
                                            }
                                        }
                                    }
                                }
                                if state.rack_side == RackSide::Rear && device.rack.is_some_and(|p| p.unit == unit) && !matches!(device.kind, DeviceKind::PatchPanel(_) | DeviceKind::CableManager(_)) {
                                    let inlet = egui::Rect::from_center_size(egui::pos2(panel_rect.right() - 13.0, panel_rect.center().y), egui::vec2(16.0, 22.0));
                                    let endpoint = device_power_endpoint(device);
                                    let fed = endpoint.and_then(|e| sim.power.connections.iter().find(|(_, x)| **x == e).map(|(o, _)| *o));
                                    if let Some(endpoint) = endpoint { power_endpoints.push((endpoint, inlet.center())); }
                                    ui.painter().rect_filled(inlet, 2.0, if fed.is_some() { egui::Color32::from_rgb(35, 105, 77) } else { egui::Color32::from_rgb(30, 35, 40) });
                                    ui.painter().rect_stroke(inlet, 2.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(170, 180, 185)), egui::StrokeKind::Inside);
                                    ui.painter().text(inlet.center(), egui::Align2::CENTER_CENTER, "C14", egui::FontId::monospace(6.0), egui::Color32::WHITE);
                                    let response = ui.interact(inlet, egui::Id::new(("power-inlet", device_id.0)), egui::Sense::click()).on_hover_text(if fed.is_some() { "Powered · select in inspector to unplug" } else { "C14 inlet · click to connect selected C13 source" });
                                    if response.clicked() && let Some(outlet) = state.pending_power_outlet.take() && let Some(endpoint) = endpoint { actions.write(UiAction::ConnectPower(outlet, endpoint)); state.pending_cable = None; state.pending_cable_route.clear(); }
                                }
                                if state.rack_side == RackSide::Rear && device.rack.is_some_and(|p| p.unit == unit) {
                                    let source = match device.kind { DeviceKind::Ups(ref x) => x.source, DeviceKind::Pdu(ref x) => x.source, _ => None };
                                    if let Some(source) = source {
                                        let count = sim.power.outlets(source);
                                        for n in 0..count {
                                            let socket = egui::Rect::from_center_size(egui::pos2(panel_rect.left() + panel_rect.width() * (0.18 + 0.64 * (n as f32 / count.max(1) as f32)), panel_rect.bottom() - 10.0), egui::vec2(12.0, 12.0));
                                            let occupied = sim.power.connections.contains_key(&OutletId { source, index: n as u8 });
                                            ui.painter().rect_filled(socket, 2.0, if occupied { egui::Color32::from_rgb(110, 74, 30) } else { egui::Color32::from_rgb(25, 30, 35) });
                                            ui.painter().text(socket.center(), egui::Align2::CENTER_CENTER, format!("{}", n + 1), egui::FontId::monospace(6.0), egui::Color32::WHITE);
                                            let response = ui.interact(socket, egui::Id::new(("power-outlet", device_id.0, n)), egui::Sense::click()).on_hover_text(if occupied { "C13 occupied" } else { "C13 outlet · select as source" });
                                            power_outlets.insert(OutletId { source, index: n as u8 }, socket.center());
                                            if response.clicked() && !occupied { state.pending_power_outlet = Some(OutletId { source, index: n as u8 }); state.pending_cable = None; state.pending_cable_route.clear(); }
                                        }
                                    }
                                }
                                ui.painter().rect_stroke(
                                    panel_rect,
                                    2.0,
                                    egui::Stroke::new(
                                        1.5,
                                        if state.selected == Selection::Device(device_id) {
                                            egui::Color32::from_rgb(255, 196, 64)
                                        } else {
                                            egui::Color32::from_gray(75)
                                        },
                                    ),
                                    egui::StrokeKind::Outside,
                                );
                                if !matches!(device.kind, DeviceKind::PatchPanel(_) | DeviceKind::CableManager(_)) {
                                    ui.painter().circle_filled(
                                        egui::pos2(panel_rect.right() - 10.0, panel_rect.top() + 10.0),
                                        4.0,
                                        if device.powered { egui::Color32::GREEN } else { egui::Color32::from_gray(45) },
                                    );
                                }
                                if matches!(device.kind, DeviceKind::Server(_)) && state.rack_side == RackSide::Front {
                                    let power_rect = egui::Rect::from_center_size(
                                        egui::pos2(panel_rect.right() - panel_rect.width() * 0.04, panel_rect.top() + panel_rect.height() * 0.14),
                                        egui::vec2(18.0, 18.0),
                                    );
                                    if ui.interact(power_rect, egui::Id::new(("rack-power", device_id.0)), egui::Sense::click()).on_hover_text("Power on/off").clicked() {
                                        actions.write(UiAction::TogglePower(device_id, !sim.power.device_status(device_id).is_some_and(|p| p.requested)));
                                    }
                                }
                                for (index, port_id) in device.ports().iter().enumerate() {
                                    let port = sim.port(*port_id).expect("device port exists");
                                    if port.side != state.rack_side {
                                        continue;
                                    }
                                    let port_rect = rack_port_rect(
                                        &device.kind,
                                        panel_rect,
                                        index,
                                        port.connector,
                                    );
                                    port_visuals.push((*port_id, port_rect));
                                    if port.connector.supports_cabling()
                                        && !matches!(device.kind, DeviceKind::PatchPanel(_) | DeviceKind::CableManager(_))
                                    {
                                        let led_size = egui::vec2(2.0, 2.0);
                                        // Switch LEDs are printed beside each jack on the
                                        // generated front panel. Keep those calibrated positions;
                                        // other equipment uses the socket-relative fallback.
                                        let (status_center, activity_center) =
                                            if matches!(device.kind, DeviceKind::Switch(_))
                                                && index < 24
                                            {
                                                let (x, _) =
                                                    device.kind.port_position_normalized(index);
                                                let y = if index % 2 == 0 { 0.26 } else { 0.742 };
                                                (
                                                    egui::pos2(
                                                        panel_rect.left()
                                                            + panel_rect.width() * (x - 0.013),
                                                        panel_rect.top() + panel_rect.height() * y,
                                                    ),
                                                    egui::pos2(
                                                        panel_rect.left()
                                                            + panel_rect.width() * (x + 0.011),
                                                        panel_rect.top() + panel_rect.height() * y,
                                                    ),
                                                )
                                            } else {
                                                let center = port_rect.center();
                                                (
                                                    center
                                                        + egui::vec2(
                                                            -3.0,
                                                            -port_rect.height() * 0.34,
                                                        ),
                                                    center
                                                        + egui::vec2(
                                                            3.0,
                                                            -port_rect.height() * 0.34,
                                                        ),
                                                )
                                            };
                                        led_visuals.push((
                                            *port_id,
                                            false,
                                            egui::Rect::from_center_size(status_center, led_size),
                                        ));
                                        led_visuals.push((
                                            *port_id,
                                            true,
                                            egui::Rect::from_center_size(activity_center, led_size),
                                        ));
                                    }
                                }
                                if row_response.clicked() {
                                    actions.write(UiAction::SelectDevice(device_id));
                                }
                            } else {
                                let installing = selected_inventory.is_some();
                                if installing {
                                    let hovered = row_response.hovered();
                                    ui.painter().rect_filled(
                                        panel_rect, 1.0,
                                        if hovered { egui::Color32::from_rgb(28, 52, 48) }
                                        else { egui::Color32::from_rgb(15, 25, 26) },
                                    );
                                    ui.painter().rect_stroke(panel_rect, 1.0,
                                        egui::Stroke::new(1.0, egui::Color32::from_rgb(67, 115, 105)),
                                        egui::StrokeKind::Inside);
                                    ui.painter().text(panel_rect.center(), egui::Align2::CENTER_CENTER,
                                        format!("+ INSTALL AT U{unit:02}"), egui::FontId::monospace(11.0),
                                        egui::Color32::from_rgb(146, 186, 175));
                                }
                                if row_response.clicked()
                                    && let Some(device) = selected_inventory
                                {
                                    actions.write(UiAction::Place {
                                        device,
                                        rack: rack.id,
                                        unit,
                                    });
                                }
                            }
                        }
                        layout.paint_crossbar(ui, "CABLE SERVICE SPACE");
                    });

                // Power leads follow the same physical rack view as network cables.  Rack
                // outlets are drawn on the power bar; device and UPS/PDU inlets are on rear faces.
                for (outlet, endpoint) in &sim.power.connections {
                    let Some((_, target)) = power_endpoints.iter().find(|(e, _)| e == endpoint) else { continue };
                    let Some(source_pos) = power_outlets.get(outlet).copied() else { continue };
                    ui.painter().line_segment([source_pos, *target], egui::Stroke::new(3.0, egui::Color32::from_rgb(211, 150, 48)));
                    ui.painter().circle_filled(*target, 3.0, egui::Color32::from_rgb(255, 196, 64));
                }

                let positions: HashMap<_, _> = port_visuals.iter().copied().collect();
                let selected_route = selected_link_id(state, sim).and_then(|link_id| sim.link(link_id).map(|link| (link_id, link.route.clone())));
                let anchors: Vec<_> = route_anchors.iter().map(|&(rack, unit, side, offset_cm, pos)| {
                    (CableRoutePoint { rack, unit, side, offset_cm }, pos)
                }).collect();
                let mut preview = None;
                for (rack_id, unit, side, offset_cm, position) in route_anchors {
                    let point = CableRoutePoint {
                        rack: rack_id,
                        unit,
                        side,
                        offset_cm,
                    };
                    if let Some(source) = state.pending_cable {
                        let anchor_rect =
                            egui::Rect::from_center_size(position, egui::vec2(14.0, 14.0));
                        let response = ui.interact(
                            anchor_rect,
                            egui::Id::new((
                                "pending-route-anchor",
                                rack_id.0,
                                unit,
                                side == RackSide::Front,
                                offset_cm,
                            )),
                            egui::Sense::click(),
                        )
                        .on_hover_text("Click to route the pending cable through this rail fixing point");
                        let selected = state.pending_cable_route.contains(&point);
                        ui.painter().circle_filled(
                            position,
                            5.0,
                            if selected || response.hovered() {
                                egui::Color32::from_rgb(255, 196, 64)
                            } else {
                                egui::Color32::from_rgb(92, 112, 125)
                            },
                        );
                        if response.hovered()
                            && let Some(source_rect) = positions.get(&source)
                        {
                            let mut proposed = state.pending_cable_route.clone();
                            proposed.push(point);
                            let path = std::iter::once(source_rect.center())
                                .chain(proposed.iter().filter_map(|route| {
                                    cables::anchor_position(route, &anchors)
                                }))
                                .collect();
                            preview = Some((path, cable_color_value(state.cable_color)));
                        }
                        if response.clicked() {
                            actions.write(UiAction::AddPendingCableRoutePoint(point));
                        }
                        continue;
                    }
                    let Some((link_id, route)) = selected_route.as_ref() else {
                        let anchor_rect = egui::Rect::from_center_size(position, egui::vec2(14.0, 14.0));
                        ui.interact(anchor_rect, egui::Id::new(("route-anchor", rack_id.0, unit, side == RackSide::Front, offset_cm)), egui::Sense::hover())
                            .on_hover_text("Cable route anchor · select a cable to add or remove this anchor");
                        ui.painter().circle_filled(position, 5.0, egui::Color32::from_rgb(76, 104, 115));
                        continue;
                    };
                        let used = route.iter().position(|point| {
                            point.rack == rack_id
                                && point.unit == unit
                                && point.side == side
                                && point.offset_cm == offset_cm
                        });
                        let anchor_rect =
                            egui::Rect::from_center_size(position, egui::vec2(14.0, 14.0));
                        let response = ui.interact(
                            anchor_rect,
                            egui::Id::new((
                                "route-anchor",
                                rack_id.0,
                                unit,
                                side == RackSide::Front,
                                offset_cm,
                            )),
                            egui::Sense::click(),
                        ).on_hover_text("Click to add or remove this anchor for the selected cable");
                        ui.painter().circle_filled(
                            position,
                            4.0,
                            if used.is_some() || response.hovered() {
                                egui::Color32::from_rgb(255, 196, 64)
                            } else {
                                egui::Color32::from_rgb(92, 112, 125)
                            },
                        );
                        if response.hovered() {
                            let link = sim.link(*link_id).expect("selected cable exists");
                            if let (Some(a), Some(b)) = (positions.get(&link.a), positions.get(&link.b)) {
                                let mut proposed = route.clone();
                                if let Some(index) = used {
                                    proposed.remove(index);
                                } else {
                                    proposed.push(CableRoutePoint { rack: rack_id, unit, side, offset_cm });
                                }
                                let path = std::iter::once(a.center())
                                    .chain(proposed.iter().filter_map(|point| cables::anchor_position(point, &anchors)))
                                    .chain(std::iter::once(b.center())).collect();
                                preview = Some((path, cable_color_value(link.color)));
                            }
                        }
                        if response.clicked() {
                            if let Some(index) = used {
                                actions.write(UiAction::RemoveCableRoutePoint {
                                    link: *link_id,
                                    index,
                                });
                            } else {
                                actions.write(UiAction::AddCableRoutePoint {
                                    link: *link_id,
                                    point: CableRoutePoint {
                                        rack: rack_id,
                                        unit,
                                        side,
                                        offset_cm,
                                    },
                                });
                            }
                        }
                }
                // Space below the rack is the floor for hanging service loops.
                ui.allocate_space(egui::vec2(rack_width, 120.0));
                if let Some(link) = cables.show(
                    ui,
                    sim,
                    &positions,
                    CableView {
                        origin: rack_frame.response.rect.left_top(),
                        pixels_per_cm: panel_width / 48.26,
                        floor_y: rack_frame.response.rect.bottom() + 110.0,
                        jacket: textures.jacket,
                        plug: textures.plug,
                        selected: selected_link_id(state, sim),
                        visibility: state.cable_visibility,
                        anchors: anchors.clone(),
                        preview,
                    },
                ) {
                    actions.write(UiAction::SelectLink(link));
                }
                for (port_id, activity_led, rect) in led_visuals {
                    let activity = sim.port_activity(port_id, 180);
                    let status = sim
                        .port_link_speed(port_id)
                        .map(|speed| match speed {
                            LinkSpeed::Gbps1 => egui::Color32::from_rgb(70, 235, 85),
                            LinkSpeed::Mbps100 | LinkSpeed::Mbps10 => {
                                egui::Color32::from_rgb(235, 170, 45)
                            }
                        })
                        .unwrap_or(egui::Color32::from_gray(24));
                    ui.painter().rect_filled(
                        rect,
                        0.5,
                        if activity_led {
                            if activity {
                                egui::Color32::from_rgb(245, 170, 42)
                            } else {
                                egui::Color32::from_gray(24)
                            }
                        } else {
                            status
                        },
                    );
                }
                for (port_id, port_rect) in port_visuals {
                    let port = sim.port(port_id).expect("visualized port exists");
                    let link = sim.link_for_port(port_id);
                    let paired_selected = sim
                        .port(port_id)
                        .and_then(|port| port.paired_port)
                        .is_some_and(|paired| state.selected == Selection::Port(paired));
                    let is_pending = state.pending_cable == Some(port_id);
                    let supported = port.connector.supports_cabling();
                    let response = ui.interact(
                        port_rect,
                        egui::Id::new(("rack-port", port_id.0)),
                        if supported {
                            egui::Sense::click()
                        } else {
                            egui::Sense::hover()
                        },
                    );
                    let hovered = response.hovered();
                    if hovered && supported
                        && let Some(first) = state.pending_cable.filter(|first| *first != port_id)
                        && let Some(start) = positions.get(&first)
                    {
                        let valid = sim.quote_colored_cable(first, port_id, state.cable_length_cm, state.cable_color).is_ok();
                        let path = std::iter::once(start.center())
                            .chain(state.pending_cable_route.iter().filter_map(|route| {
                                cables::anchor_position(route, &anchors)
                            }))
                            .chain(std::iter::once(port_rect.center()))
                            .collect();
                        cables::paint_preview(ui, path,
                            if valid { cable_color_value(state.cable_color) } else { egui::Color32::from_rgb(240, 65, 65) });
                    }
                    if is_pending {
                        ui.painter().rect_filled(
                            port_rect,
                            1.5,
                            egui::Color32::from_rgba_premultiplied(255, 196, 64, 42),
                        );
                }
                    if is_pending
                        || hovered
                        || paired_selected
                        || state.selected == Selection::Port(port_id)
                    {
                        ui.painter().rect_stroke(
                            port_rect,
                            1.5,
                            egui::Stroke::new(
                                if is_pending
                                    || paired_selected
                                    || state.selected == Selection::Port(port_id)
                                {
                                    2.0
                                } else {
                                    1.0
                                },
                                if is_pending || !supported {
                                    egui::Color32::from_rgb(255, 196, 64)
                                } else if hovered {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::from_white_alpha(45)
                                },
                            ),
                            egui::StrokeKind::Inside,
                        );
                    }
                    let quote = state
                        .pending_cable
                        .filter(|first| *first != port_id)
                        .map(|first| {
                            match sim.quote_colored_cable(
                                first,
                                port_id,
                                state.cable_length_cm,
                                state.cable_color,
                            ) {
                                Ok(q) if q.reused => format!(
                                    "\nReuse {:.2} m finished lead (no materials used)",
                                    q.length_cm as f32 / 100.0
                                ),
                                Ok(q) => format!(
                                    "\nCut {:.2} m cable + 2 RJ45 connectors",
                                    q.length_cm as f32 / 100.0
                                ),
                                Err(error) => format!("\n{error}"),
                            }
                        })
                        .unwrap_or_default();
                    let response = response.on_hover_text(format!(
                        "{} / {} · {}\n{}{quote}",
                        sim.device(port.device)
                            .map(|d| d.name.as_str())
                            .unwrap_or("?"),
                        port.name,
                        connector_label(port.connector),
                        if supported {
                            if link.is_some() {
                                "RJ45 plug connected · Right click: configure / unplug"
                            } else {
                                "Left click: connect RJ45 cable\nRight click: configure"
                            }
                        } else {
                            "SFP cabling is not implemented yet"
                        }
                    ));
                    response.context_menu(|menu| {
                        menu.label("Cable actions");
                        if let Some(link) = link {
                            if menu.button("Unplug cable").clicked() {
                                actions.write(UiAction::Disconnect(link.id));
                                menu.close();
                            }
                        } else if supported && menu.button("Connect RJ45 cable").clicked() {
                            actions.write(UiAction::CablePort(port_id));
                            menu.close();
                        }
                    });
                    if supported && link.is_some() && response.clicked() {
                        actions.write(UiAction::SelectPort(port_id));
                    } else if supported && response.clicked() {
                        actions.write(UiAction::SelectPort(port_id));
                        actions.write(UiAction::CablePort(port_id));
                    } else if supported && response.secondary_clicked() {
                        actions.write(UiAction::SelectPort(port_id));
                    }
                }
            });
    });
}

fn equipment_uv(kind: &DeviceKind, side: RackSide) -> egui::Rect {
    // Crop the generated photographs to their front panels at render time.
    let (left, top, right, bottom) = match kind {
        DeviceKind::Server(_) if side == RackSide::Front => (0.009, 0.48, 0.995, 0.84),
        DeviceKind::Switch(_) if side == RackSide::Rear => (0.002, 0.45, 0.997, 0.755),
        DeviceKind::Switch(_) => (0.0, 210.0 / 666.0, 1.0, 434.0 / 666.0),
        DeviceKind::Router(_) if side == RackSide::Rear => (0.0, 0.31, 1.0, 0.70),
        DeviceKind::Router(_) => (0.0, 193.0 / 683.0, 1.0, 480.0 / 683.0),
        _ => (0.0, 0.0, 1.0, 1.0),
    };
    egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, bottom))
}

fn rack_port_position(kind: &DeviceKind, rect: egui::Rect, index: usize) -> egui::Pos2 {
    let (x, y) = kind.port_position_normalized(index);
    egui::pos2(
        rect.left() + rect.width() * x,
        rect.top() + rect.height() * y,
    )
}

fn visible_patch_port_indices(sides: &[RackSide], side: RackSide) -> Vec<usize> {
    sides
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| (*candidate == side).then_some(index))
        .collect()
}

fn rack_port_rect(
    kind: &DeviceKind,
    panel: egui::Rect,
    index: usize,
    connector: PortConnector,
) -> egui::Rect {
    let center = rack_port_position(kind, panel, index);
    let normalized_size = match (&kind, connector) {
        (DeviceKind::Router(_), PortConnector::Rj45) => egui::vec2(0.041, 0.27),
        (DeviceKind::Server(_), PortConnector::Rj45) => egui::vec2(0.036, 0.25),
        (_, PortConnector::Rj45) => egui::vec2(0.029, 0.27),
        (_, PortConnector::Sfp) => egui::vec2(0.034, 0.36),
    };
    egui::Rect::from_center_size(
        center,
        egui::vec2(
            panel.width() * normalized_size.x,
            panel.height() * normalized_size.y,
        ),
    )
}

fn connector_label(connector: PortConnector) -> &'static str {
    match connector {
        PortConnector::Rj45 => "RJ45",
        PortConnector::Sfp => "SFP (not implemented)",
    }
}

fn cable_color_value(color: CableColor) -> egui::Color32 {
    match color {
        CableColor::White => egui::Color32::from_rgb(235, 238, 236),
        CableColor::Gray => egui::Color32::from_rgb(125, 132, 138),
        CableColor::Blue => egui::Color32::from_rgb(45, 125, 225),
        CableColor::Orange => egui::Color32::from_rgb(232, 125, 35),
        CableColor::Red => egui::Color32::from_rgb(205, 48, 52),
    }
}

fn topology_view(viewport: &mut egui::Ui, sim: &NetworkSim, actions: &mut MessageWriter<UiAction>) {
    egui::CentralPanel::default().show(viewport, |ui| {
        ui.heading("Logical topology");
        let rect = ui.available_rect_before_wrap();
        let painter = ui.painter_at(rect);
        let mut devices: Vec<_> = sim.devices().filter(|d| d.rack.is_some()).collect();
        devices.sort_by_key(|d| d.id);
        let mut positions = HashMap::new();
        let center = rect.center();
        for (i, device) in devices.iter().enumerate() {
            let kind_y = match device.kind {
                DeviceKind::Router(_) => center.y - 180.0,
                DeviceKind::PatchPanel(_) | DeviceKind::CableManager(_) => center.y,
                DeviceKind::Switch(_) => center.y,
                DeviceKind::Server(_) => center.y + 190.0,
                DeviceKind::Ups(_) | DeviceKind::Pdu(_) => center.y,
            };
            let same_kind: Vec<_> = devices
                .iter()
                .filter(|d| std::mem::discriminant(&d.kind) == std::mem::discriminant(&device.kind))
                .collect();
            let index = same_kind
                .iter()
                .position(|d| d.id == device.id)
                .unwrap_or(i);
            let x = center.x
                + (index as f32 - (same_kind.len().saturating_sub(1) as f32 / 2.0)) * 150.0;
            positions.insert(device.id, egui::pos2(x, kind_y));
        }
        for link in sim.links() {
            let Some(a) = sim.port(link.a).and_then(|p| positions.get(&p.device)) else {
                continue;
            };
            let Some(b) = sim.port(link.b).and_then(|p| positions.get(&p.device)) else {
                continue;
            };
            painter.line_segment(
                [*a, *b],
                egui::Stroke::new(
                    3.0,
                    if link.enabled {
                        cable_color_value(link.color)
                    } else {
                        egui::Color32::DARK_GRAY
                    },
                ),
            );
        }
        for device in devices {
            let pos = positions[&device.id];
            let node = egui::Rect::from_center_size(pos, egui::vec2(120.0, 52.0));
            let response = ui.interact(
                node,
                egui::Id::new(("node", device.id.0)),
                egui::Sense::click(),
            );
            painter.rect_filled(
                node,
                7.0,
                if device.powered {
                    egui::Color32::from_rgb(32, 86, 76)
                } else {
                    egui::Color32::from_rgb(55, 57, 61)
                },
            );
            painter.rect_stroke(
                node,
                7.0,
                egui::Stroke::new(1.0, egui::Color32::GRAY),
                egui::StrokeKind::Outside,
            );
            painter.text(
                pos,
                egui::Align2::CENTER_CENTER,
                &device.name,
                egui::FontId::proportional(16.0),
                egui::Color32::WHITE,
            );
            if response.clicked() {
                actions.write(UiAction::SelectDevice(device.id));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rack_face_uses_nineteen_inch_one_u_proportions() {
        let layout = RackLayout::new(820.0);
        let row = layout.row(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(layout.width, layout.row_height),
        ));
        assert!((row.face.width() / row.face.height() - 19.0 / 1.75).abs() < 0.001);
    }

    #[test]
    fn patch_panel_has_24_unique_slots_per_side() {
        let sides: Vec<_> = (0..24)
            .flat_map(|_| [RackSide::Front, RackSide::Rear])
            .collect();
        let front = visible_patch_port_indices(&sides, RackSide::Front);
        let rear = visible_patch_port_indices(&sides, RackSide::Rear);
        assert_eq!(front.len(), 24);
        assert_eq!(rear.len(), 24);
        assert_eq!(
            front
                .iter()
                .map(|index| index / 2)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            24
        );
        assert_eq!(
            rear.iter()
                .map(|index| index / 2)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            24
        );
    }

    #[test]
    fn console_scroll_area_grows_with_window_and_keeps_input_visible() {
        fn layout(height: f32, line_count: usize) -> (egui::Rect, egui::Rect, egui::Rect) {
            let ctx = egui::Context::default();
            let mut output = egui::Rect::NOTHING;
            let mut input = egui::Rect::NOTHING;
            let mut parent = egui::Rect::NOTHING;
            let mut full_output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(700.0, height),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        parent = ui.max_rect();
                        output = console_output_scroll(ui, 58.0)
                            .show(ui, |ui| {
                                for _ in 0..line_count {
                                    ui.label("long console output");
                                }
                            })
                            .inner_rect;
                        let mut input_text = String::new();
                        input = ui.add(egui::TextEdit::singleline(&mut input_text)).rect;
                    });
                },
            );
            full_output.textures_delta.clear();
            (parent, output, input)
        }

        let (small_parent, small_output, small_input) = layout(240.0, 0);
        let (large_parent, large_output, large_input) = layout(480.0, 0);
        let (_, long_small_output, _) = layout(240.0, 100);
        let (_, long_large_output, _) = layout(480.0, 100);
        assert!(large_output.height() > small_output.height());
        assert!(long_small_output.height() > 0.0);
        assert!(long_large_output.height() > long_small_output.height());
        assert!(small_input.bottom() <= small_parent.bottom());
        assert!(large_input.bottom() <= large_parent.bottom());
    }
}
