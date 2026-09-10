use crate::app::*;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiTextureHandle, egui};
use cloud_provider_sim::*;
use std::collections::HashMap;

#[derive(Resource, Default)]
pub struct EquipmentImages {
    server: Handle<Image>,
    switch: Handle<Image>,
    router: Handle<Image>,
    textures: Option<EquipmentTextures>,
}

#[derive(Clone, Copy)]
struct EquipmentTextures {
    server: egui::TextureId,
    switch: egui::TextureId,
    router: egui::TextureId,
}

pub fn load_equipment_images(mut images: ResMut<EquipmentImages>, assets: Res<AssetServer>) {
    images.server = assets.load("equipment/server_rear_clean.png");
    images.switch = assets.load("equipment/switch_front_clean.png");
    images.router = assets.load("equipment/router_rear_clean.png");
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
            server: contexts.add_image(EguiTextureHandle::Strong(images.server.clone())),
            switch: contexts.add_image(EguiTextureHandle::Strong(images.switch.clone())),
            router: contexts.add_image(EguiTextureHandle::Strong(images.router.clone())),
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
    shop_panel(&mut viewport_ui, &snapshot.0, &state, &mut actions);
    inspector_panel(
        &mut viewport_ui,
        &snapshot.0,
        &mut state,
        &mut drafts,
        &mut actions,
    );
    terminal_panel(&mut viewport_ui, &snapshot.0, &mut state, &mut actions);
    workspace(
        &mut viewport_ui,
        &snapshot.0,
        &state,
        images
            .textures
            .as_ref()
            .expect("equipment textures registered"),
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
                    ui.colored_label(egui::Color32::from_rgb(255, 196, 64), "CABLE: SELECT PORT");
                    if ui.small_button("Cancel cable").clicked() {
                        state.pending_cable = None;
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
    state: &UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    egui::Panel::left("shop")
        .resizable(false)
        .default_size(220.0)
        .show(viewport, |ui| {
            ui.heading("Equipment shop");
            ui.label("Buy real rack hardware, then install it into an empty U.");
            ui.add_space(8.0);
            for (template, label, detail) in [
                (
                    DeviceTemplate::Router,
                    "Cisco ISR C1111-8P — $500",
                    "1 × WAN + 8 × GE LAN",
                ),
                (
                    DeviceTemplate::Switch,
                    "Cisco C1000-24T-4G-L — $500",
                    "24 × 1GE + 4 × SFP",
                ),
                (
                    DeviceTemplate::Server,
                    "Dell PowerEdge R360 — $1000",
                    "2 × Ethernet, 1U",
                ),
            ] {
                ui.group(|ui| {
                    ui.label(detail);
                    if ui
                        .add_enabled(sim.money >= template.price(), egui::Button::new(label))
                        .clicked()
                    {
                        actions.write(UiAction::Buy(template));
                    }
                });
            }
            ui.separator();
            ui.heading("Inventory");
            let mut inventory: Vec<_> = sim.devices().filter(|d| d.rack.is_none()).collect();
            inventory.sort_by_key(|d| d.id);
            if inventory.is_empty() {
                ui.weak("No uninstalled devices");
            }
            for device in inventory {
                if ui
                    .selectable_label(state.selected == Selection::Device(device.id), &device.name)
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
                        format!("Cable {:02}", link.id.0),
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
}

fn inspector_panel(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    drafts: &mut EditorDrafts,
    actions: &mut MessageWriter<UiAction>,
) {
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
    ui.horizontal(|ui| {
        let color = if device.powered {
            egui::Color32::GREEN
        } else {
            egui::Color32::DARK_GRAY
        };
        ui.colored_label(
            color,
            if device.powered {
                "● POWER ON"
            } else {
                "○ POWER OFF"
            },
        );
        if ui
            .button(if device.powered {
                "Power off"
            } else {
                "Power on"
            })
            .clicked()
        {
            actions.write(UiAction::TogglePower(id, !device.powered));
        }
    });
    if device.rack.is_some() && ui.button("Remove from rack").clicked() {
        actions.write(UiAction::Remove(id));
    }
    if device.rack.is_none() {
        ui.label("Select an empty rack unit in the Rack view to install this device.");
        state.workspace = Workspace::Rack;
    }
    ui.separator();
    ui.strong("Ports");
    egui::Grid::new("device_ports")
        .num_columns(2)
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
                if ui.selectable_label(false, &port.name).clicked() {
                    actions.write(UiAction::SelectPort(*port_id));
                }
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
    let link = sim.link_for_port(id);
    ui.label(match link {
        Some(link) => format!("LINK UP · cable {}", link.id.0),
        None => "LINK DOWN".into(),
    });
    ui.weak("Connect cables by clicking ports directly in the Rack view.");
    if let Some(link) = link
        && ui.button("Disconnect cable").clicked()
    {
        actions.write(UiAction::Disconnect(link.id));
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
                        .map(|v| v.vlan.0.to_string())
                        .unwrap_or_else(|| "1".into()),
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
                ui.label("VLAN");
                ui.text_edit_singleline(&mut draft.vlan);
            });
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
                        vlan: vlan.0.to_string(),
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
                ui.label("Access VLAN");
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
    ui.label(endpoint(link.a));
    ui.label("↕");
    ui.label(endpoint(link.b));
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
    let device = match state.selected {
        Selection::Device(id)
            if sim
                .device(id)
                .is_some_and(|d| matches!(d.kind, DeviceKind::Server(_))) =>
        {
            Some(id)
        }
        Selection::Port(id) => sim
            .port(id)
            .and_then(|p| sim.device(p.device))
            .filter(|d| matches!(d.kind, DeviceKind::Server(_)))
            .map(|d| d.id),
        _ => None,
    };
    let Some(device) = device else {
        return;
    };
    egui::Panel::bottom("terminal")
        .resizable(true)
        .default_size(180.0)
        .show(viewport, |ui| {
            ui.strong(format!("{} terminal", sim.device(device).unwrap().name));
            egui::ScrollArea::vertical()
                .max_height(100.0)
                .show(ui, |ui| {
                    for line in &state.terminal_lines {
                        ui.monospace(line);
                    }
                });
            ui.horizontal(|ui| {
                ui.monospace("$ ");
                let enter = ui
                    .text_edit_singleline(&mut state.terminal_input)
                    .lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button("Run").clicked() || enter) && !state.terminal_input.trim().is_empty()
                {
                    actions.write(UiAction::RunTerminal(
                        device,
                        std::mem::take(&mut state.terminal_input),
                    ));
                }
            });
        });
}

fn workspace(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &UiState,
    textures: &EquipmentTextures,
    actions: &mut MessageWriter<UiAction>,
) {
    match state.workspace {
        Workspace::Rack => rack_view(viewport, sim, state, textures, actions),
        Workspace::Topology => topology_view(viewport, sim, actions),
    }
}

fn rack_view(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &UiState,
    textures: &EquipmentTextures,
    actions: &mut MessageWriter<UiAction>,
) {
    egui::CentralPanel::default().show(viewport, |ui| {
        let Some(rack) = sim.racks().min_by_key(|r| r.id) else {
            return;
        };
        ui.vertical_centered(|ui| {
            ui.heading(format!("{} · NETWORK SIDE", rack.name));
            ui.weak("Left-click one port, then another to patch them. Right-click a port to configure it.");
        });
        let selected_inventory = match state.selected {
            Selection::Device(id) if sim.device(id).is_some_and(|d| d.rack.is_none()) => Some(id),
            _ => None,
        };
        let row_height = 62.0;
        let rack_width = ui.available_width().min(820.0);
        let mut port_visuals = Vec::new();
        egui::Frame::group(ui.style())
            .fill(egui::Color32::from_rgb(8, 10, 13))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(rack_width);
                for unit in (1..=rack.units).rev() {
                    let (row_rect, row_response) = ui.allocate_exact_size(
                        egui::vec2(rack_width, row_height),
                        egui::Sense::click(),
                    );
                    let panel_rect = egui::Rect::from_min_max(
                        egui::pos2(row_rect.left() + 170.0, row_rect.top() + 3.0),
                        egui::pos2(row_rect.right() - 5.0, row_rect.bottom() - 3.0),
                    );
                    ui.painter().text(
                        egui::pos2(row_rect.left() + 21.0, row_rect.center().y),
                        egui::Align2::CENTER_CENTER,
                        format!("U{unit:02}"),
                        egui::FontId::monospace(12.0),
                        egui::Color32::from_gray(135),
                    );
                    if let Some(device_id) = rack.occupies(unit) {
                        let device = sim.device(device_id).expect("rack device exists");
                        let texture = match device.kind {
                            DeviceKind::Server(_) => textures.server,
                            DeviceKind::Switch(_) => textures.switch,
                            DeviceKind::Router(_) => textures.router,
                        };
                        ui.painter().rect_filled(panel_rect, 2.0, egui::Color32::BLACK);
                        ui.painter().image(
                            texture,
                            panel_rect,
                            egui::Rect::from_min_max(
                                egui::Pos2::ZERO,
                                egui::pos2(1.0, 1.0),
                            ),
                            if device.powered {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::from_gray(90)
                            },
                        );
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
                        let label_rect = egui::Rect::from_min_max(
                            egui::pos2(row_rect.left() + 43.0, panel_rect.top()),
                            egui::pos2(panel_rect.left() - 7.0, panel_rect.bottom()),
                        );
                        ui.painter().rect_filled(
                            label_rect,
                            2.0,
                            egui::Color32::from_rgb(17, 21, 26),
                        );
                        ui.painter().text(
                            label_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            rack_device_label(device),
                            egui::FontId::monospace(10.0),
                            egui::Color32::from_gray(210),
                        );
                        ui.painter().circle_filled(
                            egui::pos2(panel_rect.right() - 10.0, panel_rect.top() + 10.0),
                            4.0,
                            if device.powered {
                                egui::Color32::GREEN
                            } else {
                                egui::Color32::from_gray(45)
                            },
                        );
                        for (index, port_id) in device.ports().iter().enumerate() {
                            let position = rack_port_position(&device.kind, panel_rect, index);
                            port_visuals.push((*port_id, position));
                        }
                        if row_response.clicked() {
                            actions.write(UiAction::SelectDevice(device_id));
                        }
                    } else {
                        let installing = selected_inventory.is_some();
                        ui.painter().rect_filled(
                            panel_rect,
                            2.0,
                            if installing {
                                egui::Color32::from_rgb(24, 45, 40)
                            } else {
                                egui::Color32::from_rgb(20, 23, 27)
                            },
                        );
                        ui.painter().rect_stroke(
                            panel_rect,
                            2.0,
                            egui::Stroke::new(1.0, egui::Color32::from_gray(48)),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().text(
                            panel_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            if installing { "+ INSTALL SELECTED DEVICE" } else { "EMPTY" },
                            egui::FontId::monospace(11.0),
                            egui::Color32::from_gray(110),
                        );
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
            });

        let positions: HashMap<_, _> = port_visuals.iter().copied().collect();
        for link in sim.links().filter(|link| link.enabled) {
            let (Some(a), Some(b)) = (positions.get(&link.a), positions.get(&link.b)) else {
                continue;
            };
            let bend = 18.0 + (link.id.0 % 7) as f32 * 5.0;
            let middle_y = a.y.min(b.y) - bend;
            let path = vec![
                *a,
                egui::pos2(a.x, middle_y),
                egui::pos2(b.x, middle_y),
                *b,
            ];
            ui.painter().add(egui::Shape::line(
                path.clone(),
                egui::Stroke::new(6.0, egui::Color32::from_black_alpha(190)),
            ));
            ui.painter().add(egui::Shape::line(
                path,
                egui::Stroke::new(3.0, cable_color(link.id)),
            ));
        }
        for (port_id, position) in port_visuals {
            let port = sim.port(port_id).expect("visualized port exists");
            let link = sim.link_for_port(port_id);
            let is_pending = state.pending_cable == Some(port_id);
            let port_rect = egui::Rect::from_center_size(position, egui::vec2(14.0, 13.0));
            let response = ui.interact(
                port_rect,
                egui::Id::new(("rack-port", port_id.0)),
                egui::Sense::click(),
            );
            ui.painter().rect_filled(port_rect, 1.5, egui::Color32::from_rgb(9, 12, 13));
            ui.painter().rect_stroke(
                port_rect,
                1.5,
                egui::Stroke::new(
                    if is_pending { 2.5 } else { 1.5 },
                    if is_pending {
                        egui::Color32::from_rgb(255, 196, 64)
                    } else if link.is_some() {
                        egui::Color32::GREEN
                    } else {
                        egui::Color32::from_gray(130)
                    },
                ),
                egui::StrokeKind::Inside,
            );
            let response = response.on_hover_text(format!(
                "{} / {}\nLeft click: connect cable\nRight click: configure",
                sim.device(port.device).map(|d| d.name.as_str()).unwrap_or("?"),
                port.name
            ));
            if response.clicked() {
                actions.write(UiAction::SelectPort(port_id));
                actions.write(UiAction::CablePort(port_id));
            } else if response.secondary_clicked() {
                actions.write(UiAction::SelectPort(port_id));
            }
        }
    });
}

fn rack_port_position(kind: &DeviceKind, rect: egui::Rect, index: usize) -> egui::Pos2 {
    let normalized = match kind {
        DeviceKind::Switch(_) if index < 24 => {
            let bank = index / 12;
            let bank_port = index % 12;
            let column = bank_port / 2;
            let row = bank_port % 2;
            egui::pos2(
                0.25 + bank as f32 * 0.266 + column as f32 * 0.0335,
                0.255 + row as f32 * 0.315,
            )
        }
        DeviceKind::Switch(_) => egui::pos2(0.758 + (index - 24) as f32 * 0.036, 0.575),
        DeviceKind::Server(_) => egui::pos2(0.15 + index as f32 * 0.037, 0.40),
        DeviceKind::Router(_) if index == 0 => egui::pos2(0.38, 0.55),
        DeviceKind::Router(_) => {
            let lan = index - 1;
            egui::pos2(
                0.182 + (lan % 4) as f32 * 0.03,
                0.27 + (lan / 4) as f32 * 0.29,
            )
        }
    };
    egui::pos2(
        rect.left() + rect.width() * normalized.x,
        rect.top() + rect.height() * normalized.y,
    )
}

fn rack_device_label(device: &Device) -> String {
    device
        .name
        .replace("Dell PowerEdge ", "Dell ")
        .replace("Cisco Catalyst ", "Cisco ")
        .replace("Cisco ISR ", "Cisco ")
}

fn cable_color(id: LinkId) -> egui::Color32 {
    const COLORS: [egui::Color32; 6] = [
        egui::Color32::from_rgb(45, 210, 130),
        egui::Color32::from_rgb(65, 155, 255),
        egui::Color32::from_rgb(255, 185, 45),
        egui::Color32::from_rgb(220, 80, 105),
        egui::Color32::from_rgb(155, 105, 245),
        egui::Color32::from_rgb(55, 205, 215),
    ];
    COLORS[id.0 as usize % COLORS.len()]
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
                DeviceKind::Switch(_) => center.y,
                DeviceKind::Server(_) => center.y + 190.0,
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
                        egui::Color32::from_rgb(35, 190, 125)
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
