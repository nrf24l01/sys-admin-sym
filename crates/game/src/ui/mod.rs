use crate::app::*;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiTextureHandle, egui};
use cloud_provider_sim::*;
use std::collections::HashMap;
mod cables;
use cables::{CableScene, CableView};

const RACK_FACE_ASPECT: f32 = 19.0 / 1.75;
const RACK_METADATA_WIDTH: f32 = 175.0;
const RACK_MANAGER_WIDTH: f32 = 22.0;
const RACK_RAIL_WIDTH: f32 = 8.0;

fn selected_link_id(state: &UiState) -> Option<LinkId> {
    match state.selected {
        Selection::Link(id) => Some(id),
        _ => None,
    }
}

fn inventory_model_name(device: &Device) -> String {
    device
        .name
        .split_once(" #")
        .map_or_else(|| device.name.clone(), |(model, _)| model.to_owned())
}

#[derive(Resource, Default)]
pub struct EquipmentImages {
    server: Handle<Image>,
    switch: Handle<Image>,
    router: Handle<Image>,
    jacket: Handle<Image>,
    plug: Handle<Image>,
    cables: CableScene,
    textures: Option<EquipmentTextures>,
}

#[derive(Clone, Copy)]
struct EquipmentTextures {
    server: egui::TextureId,
    switch: egui::TextureId,
    router: egui::TextureId,
    jacket: egui::TextureId,
    plug: egui::TextureId,
}

pub fn load_equipment_images(mut images: ResMut<EquipmentImages>, assets: Res<AssetServer>) {
    images.server = assets.load("equipment/server_rear_clean.png");
    images.switch = assets.load("equipment/switch_front_clean.png");
    images.router = assets.load("equipment/router_rear_clean.png");
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
            server: contexts.add_image(EguiTextureHandle::Strong(images.server.clone())),
            switch: contexts.add_image(EguiTextureHandle::Strong(images.switch.clone())),
            router: contexts.add_image(EguiTextureHandle::Strong(images.router.clone())),
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
                        "RJ45 CABLE: SELECT PORT",
                    );
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
                        .checkbox(&mut automatic, "Auto: shortest path + 10%")
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
    let device = match state.selected {
        Selection::Device(id) => sim.device(id).map(|d| d.id),
        Selection::Port(id) => sim.port(id).map(|p| p.device),
        _ => state
            .terminal_windows
            .iter()
            .copied()
            .find(|id| sim.device(*id).is_some()),
    };
    let Some(device) = device else {
        return;
    };
    let dev = sim.device(device).unwrap();
    let server = matches!(dev.kind, DeviceKind::Server(_));
    if state.terminal_windows.contains(&device) {
        for window_device in state.terminal_windows.iter().copied().collect::<Vec<_>>() {
            terminal_window(viewport, sim, state, window_device, actions);
        }
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
            egui::ScrollArea::vertical()
                .id_salt(("console-output", device.0))
                .max_height((ui.available_height() - if console.script_mode { 120.0 } else { 70.0 }).max(60.0))
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
    for window_device in state.terminal_windows.iter().copied().collect::<Vec<_>>() {
        if window_device != device {
            terminal_window(viewport, sim, state, window_device, actions);
        }
    }
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
            egui::ScrollArea::vertical()
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
                "Buy cable + RJ45 plugs, then click two sockets. Drag a wire to move its slack.",
            );
        });
        egui::ScrollArea::vertical()
            .id_salt("rack-scroll")
            .show(ui, |ui| {
                let selected_inventory = match state.selected {
                    Selection::Device(id) if sim.device(id).is_some_and(|d| d.rack.is_none()) => {
                        Some(id)
                    }
                    _ => None,
                };
                let available_rack_width = ui.available_width().min(820.0);
                let panel_height = ((available_rack_width - RACK_METADATA_WIDTH)
                    / RACK_FACE_ASPECT)
                    .clamp(30.0, 56.0);
                let panel_width = panel_height * RACK_FACE_ASPECT;
                let rack_width = panel_width + RACK_METADATA_WIDTH;
                let gap = RACK_GAP_CM * panel_width / RACK_FACE_WIDTH_CM;
                let row_height = panel_height + gap;
                let mut port_visuals = Vec::new();
                let mut led_visuals = Vec::new();
                let mut route_anchors: Vec<(RackId, u8, RackSide, u16, egui::Pos2)> = Vec::new();
                let rack_frame = egui::Frame::group(ui.style())
                    .fill(egui::Color32::from_rgb(8, 10, 13))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width(rack_width);
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for unit in (1..=rack.units).rev() {
                            let (row_rect, row_response) = ui.allocate_exact_size(
                                egui::vec2(rack_width, row_height),
                                egui::Sense::click(),
                            );
                            let panel_rect = egui::Rect::from_min_max(
                                egui::pos2(
                                    row_rect.right() - 5.0 - panel_width,
                                    row_rect.top() + gap * 0.5,
                                ),
                                egui::pos2(row_rect.right() - 5.0, row_rect.bottom() - gap * 0.5),
                            );
                            if selected_link_id(state).is_some() {
                                route_anchors.push((
                                    rack.id,
                                    unit,
                                    state.rack_side,
                                    0,
                                    egui::pos2(panel_rect.left() - 8.0, panel_rect.center().y),
                                ));
                                route_anchors.push((
                                    rack.id,
                                    unit,
                                    state.rack_side,
                                    48,
                                    egui::pos2(panel_rect.right() + 8.0, panel_rect.center().y),
                                ));
                            }
                            ui.painter().text(
                                egui::pos2(row_rect.left() + 21.0, row_rect.center().y),
                                egui::Align2::CENTER_CENTER,
                                format!("U{unit:02}"),
                                egui::FontId::monospace(12.0),
                                egui::Color32::from_gray(135),
                            );
                            if let Some(device_id) = rack.occupies(unit) {
                                let device = sim.device(device_id).expect("rack device exists");
                                if matches!(device.kind, DeviceKind::CableManager(_))
                                    && state.rack_side == RackSide::Front
                                    && selected_link_id(state).is_some()
                                {
                                    for slot in 0..6u16 {
                                        let offset_cm = slot * 48 / 5;
                                        route_anchors.push((
                                            rack.id,
                                            unit,
                                            RackSide::Front,
                                            offset_cm,
                                            egui::pos2(
                                                panel_rect.left() + panel_rect.width() * slot as f32 / 5.0,
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
                                        for slot in 0..6 {
                                            let center = egui::pos2(
                                                panel_rect.left() + panel_rect.width() * slot as f32 / 5.0,
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
                                        let textured = matches!(device.kind, DeviceKind::Server(_) if state.rack_side == RackSide::Rear)
                                            || matches!(device.kind, DeviceKind::Switch(_) | DeviceKind::Router(_) if state.rack_side == RackSide::Front);
                                        if textured {
                                            let texture = match device.kind {
                                                DeviceKind::Server(_) => textures.server,
                                                DeviceKind::Switch(_) => textures.switch,
                                                DeviceKind::Router(_) => textures.router,
                                                _ => unreachable!(),
                                            };
                                            ui.painter().image(texture, panel_rect, equipment_uv(&device.kind), if device.powered { egui::Color32::WHITE } else { egui::Color32::from_gray(90) });
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
                                ui.painter().circle_filled(
                                    egui::pos2(panel_rect.right() - 10.0, panel_rect.top() + 10.0),
                                    4.0,
                                    if device.powered {
                                        egui::Color32::GREEN
                                    } else {
                                        egui::Color32::from_gray(45)
                                    },
                                );
                                if matches!(device.kind, DeviceKind::Server(_)) && state.rack_side == RackSide::Front {
                                    let power_rect = egui::Rect::from_center_size(
                                        egui::pos2(panel_rect.left() + 14.0, panel_rect.center().y),
                                        egui::vec2(18.0, 18.0),
                                    );
                                    if ui.interact(power_rect, egui::Id::new(("rack-power", device_id.0)), egui::Sense::click()).clicked() {
                                        actions.write(UiAction::TogglePower(device_id, !device.powered));
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
                                    if port.connector.supports_cabling() {
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
                                ui.painter().rect_filled(
                                    panel_rect,
                                    2.0,
                                    if installing {
                                        egui::Color32::from_rgb(24, 45, 40)
                                    } else {
                                        egui::Color32::from_rgba_premultiplied(20, 23, 27, 38)
                                    },
                                );
                                ui.painter().rect_stroke(
                                    panel_rect,
                                    2.0,
                                    egui::Stroke::new(1.0, egui::Color32::from_gray(48)),
                                    egui::StrokeKind::Inside,
                                );
                                if installing {
                                    ui.painter().text(
                                        panel_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "+ INSTALL SELECTED DEVICE",
                                        egui::FontId::monospace(11.0),
                                        egui::Color32::from_gray(110),
                                    );
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
                    });

                // A neutral scalable cabinet face: heavy vertical mounting rails,
                // repeating cage-nut holes, and cable-management channels beside
                // the equipment face. These are presentation anchors for the
                // flexible leads below and remain independent of device sprites.
                let frame = rack_frame.response.rect;
                let cabinet = egui::Rect::from_min_max(
                    egui::pos2(frame.left() + RACK_METADATA_WIDTH, frame.top()),
                    frame.right_bottom(),
                );
                let panel_right = cabinet.right() - RACK_MANAGER_WIDTH - RACK_RAIL_WIDTH;
                let panel_left = panel_right - panel_width;
                for manager in [
                    egui::Rect::from_min_max(
                        egui::pos2(panel_left - RACK_MANAGER_WIDTH, cabinet.top()),
                        egui::pos2(panel_left, cabinet.bottom()),
                    ),
                    egui::Rect::from_min_max(
                        egui::pos2(panel_right, cabinet.top()),
                        egui::pos2(panel_right + RACK_MANAGER_WIDTH, cabinet.bottom()),
                    ),
                ] {
                    let x = manager.center().x;
                    ui.painter().line_segment(
                        [
                            egui::pos2(x, cabinet.top() + 4.0),
                            egui::pos2(x, cabinet.bottom() - 4.0),
                        ],
                        egui::Stroke::new(3.0, egui::Color32::from_rgb(24, 29, 34)),
                    );
                    for unit in 0..rack.units {
                        let y = cabinet.top() + (unit as f32 + 0.5) * row_height;
                        ui.painter().circle_filled(
                            egui::pos2(x, y),
                            3.0,
                            egui::Color32::from_rgb(67, 76, 83),
                        );
                    }
                }
                let positions: HashMap<_, _> = port_visuals.iter().copied().collect();
                if let Some(link_id) = selected_link_id(state) {
                    let route = sim
                        .link(link_id)
                        .map(|link| link.route.clone())
                        .unwrap_or_default();
                    for (rack_id, unit, side, offset_cm, position) in route_anchors {
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
                        );
                        ui.painter().circle_filled(
                            position,
                            4.0,
                            if used.is_some() || response.hovered() {
                                egui::Color32::from_rgb(255, 196, 64)
                            } else {
                                egui::Color32::from_rgb(92, 112, 125)
                            },
                        );
                        if response.clicked() {
                            if let Some(index) = used {
                                actions.write(UiAction::RemoveCableRoutePoint {
                                    link: link_id,
                                    index,
                                });
                            } else {
                                actions.write(UiAction::AddCableRoutePoint {
                                    link: link_id,
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
                        selected: match state.selected {
                            Selection::Link(id) => Some(id),
                            Selection::Port(port) => sim.link_for_port(port).map(|link| link.id),
                            _ => None,
                        },
                        visibility: state.cable_visibility,
                        route_left_px: panel_left,
                        route_top_px: frame.top() + 10.0 + gap * 0.5,
                        route_width_px: panel_width,
                        route_row_height_px: row_height,
                        rack_units: rack.units,
                        route_side: state.rack_side,
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

fn equipment_uv(kind: &DeviceKind) -> egui::Rect {
    // Crop the generated photographs to their front panels at render time.
    let (top, bottom) = match kind {
        DeviceKind::Switch(_) => (210.0 / 666.0, 434.0 / 666.0),
        DeviceKind::Router(_) => (193.0 / 683.0, 480.0 / 683.0),
        _ => (0.0, 1.0),
    };
    egui::Rect::from_min_max(egui::pos2(0.0, top), egui::pos2(1.0, bottom))
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
        let available_rack_width = 820.0_f32;
        let panel_height =
            ((available_rack_width - RACK_METADATA_WIDTH) / RACK_FACE_ASPECT).clamp(30.0, 56.0);
        let panel_width = panel_height * RACK_FACE_ASPECT;

        assert!((panel_width / panel_height - 19.0 / 1.75).abs() < f32::EPSILON);
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
}
