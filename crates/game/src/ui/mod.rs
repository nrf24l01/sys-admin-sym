use crate::app::*;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use cloud_provider_sim::*;
use std::collections::HashMap;

pub fn main_ui(
    mut contexts: EguiContexts,
    snapshot: Res<SimSnapshot>,
    mut state: ResMut<UiState>,
    mut drafts: ResMut<EditorDrafts>,
    mut actions: MessageWriter<UiAction>,
) -> Result {
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
    workspace(&mut viewport_ui, &snapshot.0, &state, &mut actions);
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
                for (workspace, label) in [
                    (Workspace::Room, "3D ROOM"),
                    (Workspace::Rack, "RACK"),
                    (Workspace::Topology, "TOPOLOGY"),
                ] {
                    if ui
                        .selectable_label(state.workspace == workspace, label)
                        .clicked()
                    {
                        state.workspace = workspace;
                    }
                }
                ui.separator();
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
                (DeviceTemplate::Router, "Router R1 — $500", "WAN + 3 × LAN"),
                (
                    DeviceTemplate::Switch,
                    "Switch S24 — $500",
                    "24 × managed Ethernet",
                ),
                (
                    DeviceTemplate::Server,
                    "Server S1 — $1000",
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
                if ui.selectable_label(false, &port.name).clicked() {
                    actions.write(UiAction::SelectPort(*port_id));
                }
                if ui
                    .small_button(if state.pending_cable == Some(*port_id) {
                        "cancel"
                    } else {
                        "cable"
                    })
                    .clicked()
                {
                    actions.write(UiAction::CablePort(*port_id));
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
    if ui.button("Use for cable").clicked() {
        actions.write(UiAction::CablePort(id));
    }
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
    actions: &mut MessageWriter<UiAction>,
) {
    match state.workspace {
        Workspace::Room => {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(egui::Color32::TRANSPARENT))
                .show(viewport, |ui| {
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                        ui.label("3D projection · use Rack view for installation and cabling");
                    });
                });
        }
        Workspace::Rack => rack_view(viewport, sim, state, actions),
        Workspace::Topology => topology_view(viewport, sim, actions),
    }
}

fn rack_view(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    egui::CentralPanel::default().show(viewport, |ui| {
        let Some(rack) = sim.racks().min_by_key(|r| r.id) else {
            return;
        };
        ui.vertical_centered(|ui| {
            ui.heading(&rack.name);
            ui.weak(
                "Click equipment to inspect it; select inventory and click an empty U to install.",
            );
        });
        let selected_inventory = match state.selected {
            Selection::Device(id) if sim.device(id).is_some_and(|d| d.rack.is_none()) => Some(id),
            _ => None,
        };
        egui::Frame::group(ui.style())
            .fill(egui::Color32::from_rgb(22, 25, 29))
            .inner_margin(12.0)
            .show(ui, |ui| {
                for unit in (1..=rack.units).rev() {
                    ui.horizontal(|ui| {
                        ui.monospace(format!("U{unit:02}"));
                        if let Some(device_id) = rack.occupies(unit) {
                            let device = sim.device(device_id).expect("rack device exists");
                            let fill = match device.kind {
                                DeviceKind::Server(_) => egui::Color32::from_rgb(53, 65, 77),
                                DeviceKind::Switch(_) => egui::Color32::from_rgb(32, 73, 68),
                                DeviceKind::Router(_) => egui::Color32::from_rgb(83, 59, 36),
                            };
                            if ui
                                .add_sized(
                                    [ui.available_width(), 34.0],
                                    egui::Button::new(format!(
                                        "{}    {}",
                                        if device.powered { "●" } else { "○" },
                                        device.name
                                    ))
                                    .fill(fill),
                                )
                                .clicked()
                            {
                                actions.write(UiAction::SelectDevice(device_id));
                            }
                        } else {
                            let label = selected_inventory
                                .map(|_| "+ INSTALL HERE")
                                .unwrap_or("EMPTY");
                            if ui
                                .add_sized(
                                    [ui.available_width(), 34.0],
                                    egui::Button::new(label)
                                        .fill(egui::Color32::from_rgb(27, 30, 34)),
                                )
                                .clicked()
                                && let Some(device) = selected_inventory
                            {
                                actions.write(UiAction::Place {
                                    device,
                                    rack: rack.id,
                                    unit,
                                });
                            }
                        }
                    });
                }
            });
    });
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
