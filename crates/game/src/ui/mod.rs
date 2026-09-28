use crate::app::*;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiTextureHandle, egui};
use cloud_provider_sim::*;
use std::collections::HashMap;
mod cables;
mod equipment;
mod inventory;
mod rack;
mod room;
mod shop;
use cables::{CableScene, CableView, PowerCableView};
use equipment::equipment_power_port_position;
use rack::RackLayout;

fn selected_link_id(state: &UiState, sim: &NetworkSim) -> Option<LinkId> {
    match state.selected {
        Selection::Link(id) => Some(id),
        Selection::Port(id) => sim.link_for_port(id).map(|link| link.id),
        _ => None,
    }
}

/// A routed cable's UI-facing identity. Both cable families use the same
/// anchor interaction; only committing the route differs by domain command.
#[derive(Clone, Copy)]
enum RoutedCableId {
    Ethernet(LinkId),
    Power(OutletId),
}

impl RoutedCableId {
    fn reroute(self, route: Vec<CableRoutePoint>) -> UiAction {
        match self {
            Self::Ethernet(link) => UiAction::RerouteCable { link, route },
            Self::Power(outlet) => UiAction::ReroutePowerCable { outlet, route },
        }
    }
}

struct RoutedCable {
    id: RoutedCableId,
    route: Vec<CableRoutePoint>,
    endpoints: (
        (CableRoutePoint, Option<egui::Pos2>),
        (CableRoutePoint, Option<egui::Pos2>),
    ),
    color: egui::Color32,
}

#[derive(Clone, Copy)]
enum PendingCableId {
    Ethernet(PortId),
    Power(PowerSocket),
}

impl PendingCableId {
    fn anchor_action(self, point: CableRoutePoint) -> UiAction {
        match self {
            Self::Ethernet(_) => UiAction::AddPendingCableRoutePoint(point),
            Self::Power(_) => UiAction::AddPendingPowerRoutePoint(point),
        }
    }
}

struct PendingCable {
    id: PendingCableId,
    start: (CableRoutePoint, Option<egui::Pos2>),
    route: Vec<CableRoutePoint>,
    color: egui::Color32,
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
        .chain(sim.devices().filter_map(|d| {
            (d.rack.is_some()).then_some(match d.kind {
                DeviceKind::Ups(ref x) => x.source,
                _ => None,
            })?
        }))
        .chain(sim.devices().filter_map(|d| {
            (d.rack.is_some()).then_some(match d.kind {
                DeviceKind::Pdu(ref x) => x.source,
                _ => None,
            })?
        }))
        .collect()
}

fn device_power_endpoint(device: &Device) -> Option<PowerEndpoint> {
    match device.kind {
        DeviceKind::Ups(ref x) => x.source.map(PowerEndpoint::Source),
        DeviceKind::Pdu(ref x) => x.source.map(PowerEndpoint::Source),
        DeviceKind::PatchPanel(_) | DeviceKind::CableManager(_) => None,
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
    ups_faces: Handle<Image>,
    pdu_faces: Handle<Image>,
    jacket: Handle<Image>,
    plug: Handle<Image>,
    power_connectors: Handle<Image>,
    power_plugs_rear: Handle<Image>,
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
    ups_faces: egui::TextureId,
    pdu_faces: egui::TextureId,
    jacket: egui::TextureId,
    plug: egui::TextureId,
    power_connectors: egui::TextureId,
    power_plugs_rear: egui::TextureId,
}

pub fn load_equipment_images(mut images: ResMut<EquipmentImages>, assets: Res<AssetServer>) {
    images.server_front = assets.load("equipment/server_front.png");
    images.server_rear = assets.load("equipment/server_rear_clean.png");
    images.switch_front = assets.load("equipment/switch_front_clean.png");
    images.switch_rear = assets.load("equipment/switch_rear.png");
    images.router_front = assets.load("equipment/router_rear_clean.png");
    images.router_rear = assets.load("equipment/router_front_clean.png");
    images.ups_faces = assets.load("equipment/ups_faces.png");
    images.pdu_faces = assets.load("equipment/pdu_faces.png");
    images.jacket = assets.load("cables/pvc_jacket.png");
    images.plug = assets.load("cables/rj45_plug.png");
    images.power_connectors = assets.load("equipment/power_connectors.png");
    images.power_plugs_rear = assets.load("equipment/power_plugs_rear.png");
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
            ups_faces: contexts.add_image(EguiTextureHandle::Strong(images.ups_faces.clone())),
            pdu_faces: contexts.add_image(EguiTextureHandle::Strong(images.pdu_faces.clone())),
            jacket: contexts.add_image(EguiTextureHandle::Strong(images.jacket.clone())),
            plug: contexts.add_image(EguiTextureHandle::Strong(images.plug.clone())),
            power_connectors: contexts
                .add_image(EguiTextureHandle::Strong(images.power_connectors.clone())),
            power_plugs_rear: contexts
                .add_image(EguiTextureHandle::Strong(images.power_plugs_rear.clone())),
        });
    }
    let ctx = contexts.ctx_mut()?;
    if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
        state.pending_cable = None;
        state.pending_cable_route.clear();
        state.pending_power_outlet = None;
        state.pending_power_inlet = None;
    }
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
    inventory::show(&mut viewport_ui, &snapshot.0, &mut state, &mut actions);
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
    shop::show(&mut viewport_ui, &snapshot.0, &mut state.shop, &mut actions);
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
                    [(Workspace::Room, "ROOM"), (Workspace::Rack, "RACK"), (Workspace::Topology, "TOPOLOGY")]
                {
                    if ui
                        .selectable_label(state.workspace == workspace, label)
                        .clicked()
                    {
                        state.workspace = workspace;
                    }
                }
                ui.separator();
                if ui.selectable_label(state.shop.open, "SHOP").clicked() {
                    state.shop.open = !state.shop.open;
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
                if state.pending_power_outlet.is_some() || state.pending_power_inlet.is_some() {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 196, 64),
                        format!(
                            "POWER CABLE: {} ANCHOR{} → SELECT COMPLEMENTARY SOCKET",
                            state.pending_power_route.len(),
                            if state.pending_power_route.len() == 1 {
                                ""
                            } else {
                                "S"
                            }
                        ),
                    );
                    if ui.small_button("Cancel power").clicked() {
                        state.pending_power_outlet = None;
                        state.pending_power_inlet = None;
                        state.pending_power_route.clear();
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
                Selection::PowerCable(outlet) => power_cable_inspector(ui, sim, outlet, actions),
            }
        });
}

fn power_cable_inspector(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    outlet: OutletId,
    actions: &mut MessageWriter<UiAction>,
) {
    ui.heading("Power cable");
    let Some(endpoint) = sim.power.connections.get(&outlet).copied() else {
        ui.label("Power cable no longer connected.");
        return;
    };
    let destination = match endpoint {
        PowerEndpoint::Device(id) => sim
            .device(id)
            .map_or_else(|| format!("Device {id:?}"), |d| d.name.clone()),
        PowerEndpoint::Source(source) => source_label(sim, source),
    };
    ui.label(format!(
        "Source: {} C13-{}",
        source_label(sim, outlet.source),
        outlet.index + 1
    ));
    ui.label(format!("Destination: {destination}"));
    ui.label(format!("Cord: {:?}", sim.power.cord_kind(outlet)));
    let anchors = sim.power.cord_routes.get(&outlet).map_or(0, Vec::len);
    ui.label(format!("Rack anchors: {anchors}"));
    ui.weak(
        "In Rack view, click a rail anchor to add or remove it from this selected power cable.",
    );
    if anchors > 0 && ui.small_button("Clear rack anchors").clicked() {
        actions.write(UiAction::ReroutePowerCable {
            outlet,
            route: Vec::new(),
        });
    }
    if ui.button("Unplug cable").clicked() {
        actions.write(UiAction::DisconnectPower(outlet));
    }
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
    power_controls(ui, sim, device, actions);
    if let DeviceKind::Server(server) = &device.kind {
        if let Some(hardware) = &server.hardware {
            server_hardware_inspector(ui, sim, id, hardware, actions);
        }
    }
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

fn server_hardware_inspector(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    device: DeviceId,
    hardware: &cloud_provider_sim::ServerHardware,
    actions: &mut MessageWriter<UiAction>,
) {
    use cloud_provider_sim::{PciCard, ServerPartKind, server_catalog};
    let catalog = server_catalog();
    let chassis = &catalog.chassis;
    ui.separator();
    ui.strong("Server hardware");
    ui.label(format!(
        "CPU {}/{} · RAM {}/{} · PSU {}/{}",
        hardware.cpus.len(),
        chassis.cpu_sockets,
        hardware.ram.len(),
        chassis.dimm_slots,
        hardware.power_supplies.len(),
        chassis.psu_bays
    ));
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
    ui.label(format!("PCIe lanes: {used_lanes}/{cpu_lanes}"));
    if hardware.ready() {
        ui.colored_label(egui::Color32::GREEN, "Required hardware installed");
    } else {
        ui.weak("Install a CPU, RAM and power supply to complete the server.");
    }
    for (index, slot) in chassis.pcie_slots.iter().enumerate() {
        let installed = hardware.pcie.get(index).and_then(Option::as_deref);
        ui.horizontal(|ui| {
            ui.label(format!(
                "{}: x{} / Gen {}",
                slot.name, slot.lanes, slot.generation
            ));
            if let Some(id) = installed {
                let name = catalog
                    .parts
                    .iter()
                    .find(|part| part.id == id)
                    .map_or(id, |part| part.name.as_str());
                ui.label(name);
                if ui.button("Remove").clicked() {
                    actions.write(UiAction::RemoveServerPart {
                        device,
                        part_id: id.into(),
                        slot: Some(index),
                    });
                }
            } else {
                ui.weak("empty");
                for part in &catalog.parts {
                    let ServerPartKind::PciCard {
                        card:
                            PciCard::Ethernet {
                                lanes,
                                generation,
                                width,
                                ..
                            },
                    } = &part.kind
                    else {
                        continue;
                    };
                    if sim.server_parts.get(&part.id).copied().unwrap_or(0) > 0
                        && slot.lanes >= *lanes
                        && slot.width >= *width
                        && slot.generation >= *generation
                        && used_lanes + u16::from(*lanes) <= cpu_lanes
                        && ui.button(format!("Install {}", part.name)).clicked()
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
            ui.label(format!(
                "{} · inventory {owned} · installed {installed}",
                part.name
            ));
            if ui
                .add_enabled(owned > 0, egui::Button::new("Install"))
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
                && ui.button("Remove").clicked()
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
    ui.strong("Drive bays");
    ui.weak("Buy drives in Compute → Storage drives, then install them here.");
    for (bay, slot) in chassis.drive_bays.iter().enumerate() {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{} ({})", slot.name, slot.interface));
            if let Some(id) = hardware.drives.get(bay).and_then(Option::as_deref) {
                if let Some(drive) = cloud_provider_sim::drive_catalog()
                    .drives
                    .iter()
                    .find(|d| d.id == id)
                {
                    ui.label(format!(
                        "{} · read {} MB/s / {} IOPS · write {} MB/s / {} IOPS",
                        drive.name,
                        drive.read_mb_s,
                        drive.read_iops,
                        drive.write_mb_s,
                        drive.write_iops
                    ));
                } else {
                    ui.label(id);
                }
                if ui.button("Remove drive").clicked() {
                    actions.write(UiAction::RemoveDrive { device, bay });
                }
            } else {
                ui.weak("empty");
                for drive in &cloud_provider_sim::drive_catalog().drives {
                    if drive.interface == slot.interface
                        && sim.drive_inventory.get(&drive.id).copied().unwrap_or(0) > 0
                        && ui.button(format!("Install {}", drive.name)).clicked()
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

fn power_controls(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    device: &Device,
    actions: &mut MessageWriter<UiAction>,
) {
    let source = match device.kind {
        DeviceKind::Ups(ref x) => x.source,
        DeviceKind::Pdu(ref x) => x.source,
        _ => None,
    };
    if let Some(power) = sim.power.device_status(device.id) {
        if matches!(device.kind, DeviceKind::Router(_)) {
            let ac = PowerCordKind::Cisco66WAdapter.input_load(power.load);
            ui.label(format!(
                "Cisco adapter: 66 W max · 12 V · 5.5 A max · Router: {} W DC / {:.2} A · AC: {} W / {} VA / {:.2} A",
                power.load.watts,
                power.load.watts as f32 / 12.0,
                ac.watts,
                ac.va,
                ac.current_ma as f32 / 1000.0,
            ));
        } else {
            ui.label(format!(
                "Load: {} W / {} VA / {:.2} A",
                power.load.watts,
                power.load.va,
                power.load.current_ma as f32 / 1000.0
            ));
        }
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
        ui.weak("Power inlet · click the device socket, then a compatible C13 outlet in the rack.");
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
                    "LINUX CONSOLE • LIVE"
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
        Workspace::Room => room::show(viewport, sim, state, actions),
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
        let Some(rack) = state.active_rack.and_then(|id| sim.rack(id))
            .or_else(|| sim.racks().min_by_key(|r| r.id)) else {
            return;
        };
        ui.vertical_centered(|ui| {
            ui.heading(format!("{} · {:?} SIDE", rack.name, state.rack_side));
            ui.horizontal(|ui| {
                ui.menu_button(format!("Select rack: {}", rack.name), |ui| {
                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                        let mut racks: Vec<_> = sim.racks().collect();
                        racks.sort_by_key(|rack| rack.id);
                        for candidate in racks {
                            if ui.selectable_label(candidate.id == rack.id, &candidate.name).clicked() {
                                state.active_rack = Some(candidate.id);
                                ui.close();
                            }
                        }
                    });
                });
                if ui.button("Room view").clicked() { state.workspace = Workspace::Room; }
            });
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
                let mut power_socket_rects: Vec<(PowerSocket, egui::Rect)> = Vec::new();
                let mut route_anchors: Vec<(RackId, u8, RackSide, u16, egui::Pos2)> = Vec::new();
                let mut placement_preview = None;
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
                                let response = ui.allocate_response(egui::vec2(44.0, 28.0), egui::Sense::click()).on_hover_text(if occupied { "C13 occupied · right-click to unplug" } else { "C13 outlet · click to connect" });
                                paint_power_socket(ui.painter(), textures.power_connectors, response.rect.shrink(2.0));
                                power_socket_rects.push((PowerSocket::Outlet(outlet), response.rect));
                                if state.pending_power_outlet == Some(outlet)
                                    || state.pending_power_inlet.is_some()
                                    || state.selected == Selection::PowerCable(outlet)
                                    || response.hovered()
                                {
                                    ui.painter().rect_stroke(response.rect.shrink(1.0), 2.0, egui::Stroke::new(2.0, if state.selected == Selection::PowerCable(outlet) { egui::Color32::from_rgb(120, 205, 255) } else if response.hovered() { egui::Color32::LIGHT_BLUE } else { egui::Color32::from_rgb(255, 196, 64) }), egui::StrokeKind::Inside);
                                }
                                power_outlets.insert(outlet, response.rect.center());
                                if response.clicked() { actions.write(if occupied { UiAction::SelectPowerCable(outlet) } else { UiAction::PowerSocket(crate::app::PowerSocket::Outlet(outlet)) }); }
                                response.context_menu(|menu| { if occupied && menu.button("Unplug cable").clicked() { actions.write(UiAction::DisconnectPower(outlet)); menu.close(); } });
                            }
                        });
                        for unit in (1..=rack.units).rev() {
                            let (row_rect, row_response) = ui.allocate_exact_size(
                                egui::vec2(rack_width, row_height),
                                egui::Sense::click(),
                            );
                            let row = layout.row(row_rect);
                            let panel_rect = rack_device_panel_rect(
                                rack,
                                layout.row_height,
                                unit,
                                row.face,
                            );
                            row.paint(ui.painter(), unit, rack.occupies(unit).is_some());
                            for (offset_cm, position) in [0, 48].into_iter().zip(row.anchors) {
                                route_anchors.push((rack.id, unit, state.rack_side, offset_cm, position));
                            }
                            if let Some(device_id) = rack.occupies(unit) {
                                // A 2U chassis occupies two rack placements but has one
                                // continuous face. Paint and interact with it only at the
                                // placement's first unit; the following row remains part of
                                // the same panel span.
                                // A multi-U device is painted from its first (lowest)
                                // placement unit. Painting it from the second occupied
                                // row shifts the whole face and all socket hitboxes up by
                                // one U, which was especially visible on the UPS.
                                if rack
                                    .placements
                                    .iter()
                                    .find(|(id, _)| *id == device_id)
                                    .is_some_and(|(_, placement)| placement.unit != unit)
                                {
                                    continue;
                                }
                                let device = sim.device(device_id).expect("rack device exists");
                                let row_response = ui.interact(
                                    panel_rect, egui::Id::new(("device-panel", device_id.0)), egui::Sense::click(),
                                );
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
                                            let socket = rack_port_rect(
                                                &device.kind,
                                                panel_rect,
                                                index,
                                                port.connector,
                                                state.rack_side,
                                            );
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
                                            | DeviceKind::Switch(_) | DeviceKind::Router(_)
                                            | DeviceKind::Ups(_) | DeviceKind::Pdu(_));
                                        if textured {
                                            let texture = match device.kind {
                                                DeviceKind::Server(_) if state.rack_side == RackSide::Front => textures.server_front,
                                                DeviceKind::Server(_) => textures.server_rear,
                                                DeviceKind::Switch(_) if state.rack_side == RackSide::Front => textures.switch_front,
                                                DeviceKind::Switch(_) => textures.switch_rear,
                                                DeviceKind::Router(_) if state.rack_side == RackSide::Front => textures.router_front,
                                                DeviceKind::Router(_) => textures.router_rear,
                                                DeviceKind::Ups(_) => textures.ups_faces,
                                                DeviceKind::Pdu(_) => textures.pdu_faces,
                                                _ => unreachable!(),
                                            };
                                            ui.painter().image(texture, panel_rect, equipment_uv(&device.kind, state.rack_side), if device.powered { egui::Color32::WHITE } else { egui::Color32::from_gray(90) });
                                            if state.rack_side == RackSide::Rear {
                                                paint_server_backplane(ui.painter(), panel_rect, &device.kind, device.powered);
                                            } else {
                                                paint_server_drives(ui.painter(), panel_rect, &device.kind);
                                            }
                                            if matches!(device.kind, DeviceKind::Ups(_)) && state.rack_side == RackSide::Front {
                                                let lcd = ups_lcd_rect(panel_rect);
                                                ui.painter().rect_filled(lcd, 1.0, egui::Color32::from_rgb(12, 25, 28));
                                                let telemetry = match device.kind {
                                                    DeviceKind::Ups(ref ups) => ups.source.and_then(|s| sim.power.source_telemetry(s)),
                                                    _ => None,
                                                };
                                                let (battery, load) = telemetry.map_or((0, 0), |t| {
                                                    let cap = match device.kind {
                                                        DeviceKind::Ups(ref ups) => ups.source.and_then(|s| match s { SourceId::Ups(id) => sim.power.ups.get(&id).map(|u| u.spec.battery_wh), _ => None }),
                                                        _ => None,
                                                    }.unwrap_or(0);
                                                    (if cap == 0 { 0 } else { (t.battery_mwh / 1000 * 100 / u64::from(cap)) as u32 }, t.output.watts)
                                                });
                                                let font = egui::FontId::monospace((lcd.height() * 0.22).clamp(5.0, 9.0));
                                                ui.painter().text(egui::pos2(lcd.center().x, lcd.center().y - font.size), egui::Align2::CENTER_CENTER, format!("BAT {battery}%"), font.clone(), egui::Color32::from_rgb(112, 235, 184));
                                                ui.painter().text(egui::pos2(lcd.center().x, lcd.center().y + font.size), egui::Align2::CENTER_CENTER, format!("LOAD {load}W"), font, egui::Color32::from_rgb(112, 235, 184));
                                                let button = egui::Rect::from_center_size(normalized_panel_position(panel_rect, (0.814, 0.29)), egui::vec2(18.0, 18.0));
                                                let response = ui.interact(button, egui::Id::new(("ups-power-button", device_id.0)), egui::Sense::click());
                                                if response.clicked() {
                                                    let enabled = match device.kind {
                                                        DeviceKind::Ups(ref ups) => ups.source.is_some_and(|s| sim.power.source_enabled(s)),
                                                        _ => false,
                                                    };
                                                    actions.write(UiAction::TogglePower(device_id, !enabled));
                                                }
                                            }
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
                                let inlet_meta = device.kind.power_inlet();
                                if inlet_meta.is_some_and(|inlet| inlet.side == state.rack_side)
                                    && device.rack.is_some_and(|p| p.unit == unit)
                                    && !matches!(device.kind, DeviceKind::PatchPanel(_) | DeviceKind::CableManager(_))
                                {
                                    let inlet = device_power_inlet_rect(&device.kind, panel_rect).expect("device has power inlet");
                                    let endpoint = device_power_endpoint(device);
                                    if let Some(endpoint) = endpoint {
                                        power_socket_rects.push((PowerSocket::Inlet(endpoint), inlet));
                                    }
                                    let fed = endpoint.and_then(|e| sim.power.connections.iter().find(|(_, x)| **x == e).map(|(o, _)| *o));
                                    let cable_end = inlet.center();
                                    if let Some(endpoint) = endpoint { power_endpoints.push((endpoint, cable_end)); }
                                    let response = ui.interact(inlet, egui::Id::new(("power-inlet", device_id.0)), egui::Sense::click()).on_hover_text(if fed.is_some() { "Occupied · right-click to unplug" } else { "Power inlet · click to connect" });
                                    if endpoint.is_some_and(|e| state.pending_power_inlet == Some(e))
                                        || state.pending_power_outlet.is_some()
                                        || fed.is_some_and(|outlet| state.selected == Selection::PowerCable(outlet))
                                        || response.hovered()
                                    {
                                        ui.painter().rect_stroke(inlet, 2.0, egui::Stroke::new(2.0, if fed.is_some_and(|outlet| state.selected == Selection::PowerCable(outlet)) { egui::Color32::from_rgb(120, 205, 255) } else if response.hovered() { egui::Color32::LIGHT_BLUE } else { egui::Color32::from_rgb(255, 196, 64) }), egui::StrokeKind::Inside);
                                    }
                                    if response.clicked() && let Some(endpoint) = endpoint { actions.write(UiAction::PowerSocket(crate::app::PowerSocket::Inlet(endpoint))); }
                                    response.context_menu(|menu| { if let Some(outlet) = fed && menu.button("Unplug cable").clicked() { actions.write(UiAction::DisconnectPower(outlet)); menu.close(); } });
                                }
                                if state.rack_side == RackSide::Rear && device.rack.is_some_and(|p| p.unit == unit) {
                                    let source = match device.kind { DeviceKind::Ups(ref x) => x.source, DeviceKind::Pdu(ref x) => x.source, _ => None };
                                    if let Some(source) = source {
                                        let count = sim.power.outlets(source);
                                        for n in 0..count {
                                            let socket = source_outlet_rect(&device.kind, panel_rect, n);
                                            let occupied = sim.power.connections.contains_key(&OutletId { source, index: n as u8 });
                                            let outlet_id = OutletId { source, index: n as u8 };
                                            power_socket_rects.push((PowerSocket::Outlet(outlet_id), socket));
                                            let outlet_end = socket.center();
                                            let response = ui.interact(socket, egui::Id::new(("power-outlet", device_id.0, n)), egui::Sense::click()).on_hover_text(if occupied { "C13 occupied" } else { "C13 outlet · select as source" });
                                            if state.pending_power_outlet == Some(outlet_id)
                                                || state.pending_power_inlet.is_some()
                                                || state.selected == Selection::PowerCable(outlet_id)
                                                || response.hovered()
                                            {
                                                ui.painter().rect_stroke(socket, 2.0, egui::Stroke::new(2.0, if state.selected == Selection::PowerCable(outlet_id) { egui::Color32::from_rgb(120, 205, 255) } else if response.hovered() { egui::Color32::LIGHT_BLUE } else { egui::Color32::from_rgb(255, 196, 64) }), egui::StrokeKind::Inside);
                                            }
                                            power_outlets.insert(OutletId { source, index: n as u8 }, outlet_end);
                                            if response.clicked() { actions.write(if occupied { UiAction::SelectPowerCable(outlet_id) } else { UiAction::PowerSocket(crate::app::PowerSocket::Outlet(outlet_id)) }); }
                                            response.context_menu(|menu| { if occupied && menu.button("Unplug cable").clicked() { actions.write(UiAction::DisconnectPower(OutletId { source, index: n as u8 })); menu.close(); } });
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
                                        state.rack_side,
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
                                    if hovered && let Some(id) = selected_inventory {
                                        let device = sim.device(id).expect("inventory device exists");
                                        let height = device.template().rack_units();
                                        let valid = unit as u16 + height as u16 - 1 <= rack.units as u16
                                            && (unit..unit.saturating_add(height)).all(|u| rack.occupies(u).is_none());
                                        let rect = egui::Rect::from_min_max(
                                            panel_rect.left_top() - egui::vec2(0.0, row_height * (height - 1) as f32),
                                            panel_rect.right_bottom(),
                                        );
                                        placement_preview = Some((id, rect, valid));
                                    }
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

                if let Some((id, rect, valid)) = placement_preview {
                    let device = sim.device(id).expect("inventory device exists");
                    let color = if valid { egui::Color32::LIGHT_GREEN } else { egui::Color32::LIGHT_RED };
                    if let Some(texture) = equipment_texture(textures, &device.kind, state.rack_side) {
                        ui.painter().image(texture, rect, equipment_uv(&device.kind, state.rack_side),
                            egui::Color32::WHITE.gamma_multiply(0.65));
                    } else {
                        ui.painter().rect_filled(rect, 1.0, egui::Color32::from_black_alpha(200));
                        if matches!(device.kind, DeviceKind::PatchPanel(_)) {
                            for (index, port_id) in device.ports().iter().enumerate() {
                                let port = sim.port(*port_id).expect("patch port exists");
                                if port.side == state.rack_side {
                                    let socket = rack_port_rect(&device.kind, rect, index, port.connector, state.rack_side);
                                    ui.painter().rect_filled(socket, 1.0, egui::Color32::from_gray(60));
                                }
                            }
                        } else {
                            for slot in 1..5 {
                                let center = egui::pos2(rect.left() + rect.width() * (slot * 48 / 5) as f32 / 48.0, rect.center().y);
                                ui.painter().circle_stroke(center, 5.0, egui::Stroke::new(2.0, egui::Color32::from_gray(90)));
                            }
                        }
                    }
                    ui.painter().rect_stroke(rect, 1.0, egui::Stroke::new(2.0, color), egui::StrokeKind::Inside);
                }

                // Power leads follow the same physical rack view and rope solver as network cables.
                let mut power_rope_cables = Vec::new();
                let anchors: Vec<_> = route_anchors.iter().map(|&(rack, unit, side, offset_cm, pos)| {
                    (CableRoutePoint { rack, unit, side, offset_cm }, pos)
                }).collect();
                let mut power_paths = HashMap::new();
                let mut power_connectors = HashMap::new();
                for (outlet, endpoint) in &sim.power.connections {
                    let target_pos = power_endpoints.iter().find(|(e, _)| e == endpoint).map(|(_, p)| *p);
                    let source_pos = power_outlets.get(outlet).copied();
                    let Some(source_location) = power_socket_location(sim, PowerSocket::Outlet(*outlet), state.rack_side) else { continue };
                    let Some(target_location) = power_socket_location(sim, PowerSocket::Inlet(*endpoint), state.rack_side) else { continue };
                    let route = sim.power.cord_routes.get(outlet).map_or(&[][..], Vec::as_slice);
                    let spans = cables::visible_spans((source_location, source_pos), (target_location, target_pos), route, &anchors);
                    let Some(first) = spans.first() else { continue };
                    let source = first[0];
                    let target = *spans.last().unwrap().last().unwrap();
                    let connectors = [PowerSocket::Outlet(*outlet), PowerSocket::Inlet(*endpoint)]
                        .iter().filter_map(|socket| power_socket_rects.iter()
                            .find(|(candidate, _)| candidate == socket).map(|(_, rect)| cables::CableConnector {
                                socket: *rect,
                                kind: power_connector_kind(sim, *socket),
                            })).collect();
                    power_connectors.insert(*outlet, connectors);
                    power_paths.insert(*outlet, spans);
                    power_rope_cables.push((*outlet, source, target, 0));
                }
                if let Some(outlet) = cables.show_power(
                    ui,
                    &power_rope_cables,
                    PowerCableView {
                        paths: power_paths.clone(),
                        connectors: power_connectors,
                        routed: sim.power.cord_routes.iter().filter(|(_, route)| !route.is_empty()).map(|(outlet, _)| *outlet).collect(),
                        plug: textures.power_plugs_rear,
                        jacket: textures.jacket,
                        origin: rack_frame.response.rect.left_top(),
                        pixels_per_cm: panel_width / 48.26,
                        floor_y: rack_frame.response.rect.bottom() + 110.0,
                        selected: match state.selected {
                            Selection::PowerCable(outlet) => Some(outlet),
                            _ => None,
                        },
                        visibility: state.cable_visibility,
                        socket_rects: power_socket_rects
                            .iter()
                            .map(|(_, rect)| *rect)
                            .chain(port_visuals.iter().map(|(_, rect)| *rect))
                            .chain(anchors.iter().map(|(_, pos)| egui::Rect::from_center_size(*pos, egui::vec2(14.0, 14.0))))
                            .collect(),
                        interaction_enabled: state.pending_power_outlet.is_none()
                            && state.pending_power_inlet.is_none()
                            && state.pending_cable.is_none(),
                    },
                ) {
                    actions.write(UiAction::SelectPowerCable(outlet));
                }

                let positions: HashMap<_, _> = port_visuals.iter().copied().collect();
                let anchors: Vec<_> = route_anchors.iter().map(|&(rack, unit, side, offset_cm, pos)| {
                    (CableRoutePoint { rack, unit, side, offset_cm }, pos)
                }).collect();
                let selected_route = match state.selected {
                    Selection::PowerCable(outlet) => sim.power.connections.get(&outlet).and_then(|endpoint| {
                        Some(RoutedCable {
                            id: RoutedCableId::Power(outlet),
                            route: sim.power.cord_routes.get(&outlet).cloned().unwrap_or_default(),
                            endpoints: (
                                (power_socket_location(sim, PowerSocket::Outlet(outlet), state.rack_side)?, power_outlets.get(&outlet).copied()),
                                (power_socket_location(sim, PowerSocket::Inlet(*endpoint), state.rack_side)?, power_endpoints.iter().find(|(e, _)| e == endpoint).map(|(_, p)| *p)),
                            ),
                            color: egui::Color32::from_rgb(70, 110, 120),
                        })
                    }),
                    _ => selected_link_id(state, sim).and_then(|link_id| {
                        let link = sim.link(link_id)?;
                        Some(RoutedCable {
                            id: RoutedCableId::Ethernet(link_id),
                            route: link.route.clone(),
                            endpoints: (
                                (cables::port_location(sim, link.a)?, positions.get(&link.a).map(egui::Rect::center)),
                                (cables::port_location(sim, link.b)?, positions.get(&link.b).map(egui::Rect::center)),
                            ),
                            color: cable_color_value(link.color),
                        })
                    }),
                };
                let pending = state.pending_power_outlet
                    .map(|outlet| PendingCableId::Power(PowerSocket::Outlet(outlet)))
                    .or_else(|| state.pending_power_inlet.map(|endpoint| PendingCableId::Power(PowerSocket::Inlet(endpoint))))
                    .or_else(|| state.pending_cable.map(PendingCableId::Ethernet))
                    .and_then(|id| {
                        let (start, route, color) = match id {
                            PendingCableId::Ethernet(port) => (
                                (cables::port_location(sim, port)?, positions.get(&port).map(egui::Rect::center)),
                                state.pending_cable_route.clone(), cable_color_value(state.cable_color),
                            ),
                            PendingCableId::Power(socket) => (
                                (power_socket_location(sim, socket, state.rack_side)?,
                                    power_socket_rects.iter().find(|(candidate, _)| *candidate == socket).map(|(_, rect)| rect.center())),
                                state.pending_power_route.clone(), egui::Color32::from_rgb(31, 35, 38),
                            ),
                        };
                        Some(PendingCable { id, start, route, color })
                    });
                let creation_targets: Vec<_> = pending.as_ref().map(|pending| match pending.id {
                    PendingCableId::Ethernet(first) => port_visuals.iter().filter_map(|(port, rect)| {
                        Some(cables::CreationTarget {
                            location: cables::port_location(sim, *port)?, rect: *rect,
                            same_socket: first == *port,
                            valid: sim.quote_routed_colored_cable(first, *port, state.cable_length_cm, state.cable_color, &state.pending_cable_route).is_ok(),
                        })
                    }).collect(),
                    PendingCableId::Power(source) => power_socket_rects.iter().filter_map(|(target, rect)| {
                        let valid = match (source, *target) {
                            (PowerSocket::Outlet(outlet), PowerSocket::Inlet(endpoint))
                            | (PowerSocket::Inlet(endpoint), PowerSocket::Outlet(outlet)) => power_preview_is_valid(sim, outlet, endpoint),
                            _ => false,
                        };
                        Some(cables::CreationTarget {
                            location: power_socket_location(sim, *target, state.rack_side)?, rect: *rect,
                            same_socket: source == *target, valid,
                        })
                    }).collect(),
                }).unwrap_or_default();
                for (rack_id, unit, side, offset_cm, position) in route_anchors {
                    let point = CableRoutePoint { rack: rack_id, unit, side, offset_cm };
                    if let Some(pending) = pending.as_ref() {
                        if cables::creation_anchor(ui, point, position, &pending.route) {
                            actions.write(pending.id.anchor_action(point));
                        }
                        continue;
                    }
                    let Some(cable) = selected_route.as_ref() else {
                        let anchor_rect = egui::Rect::from_center_size(position, egui::vec2(14.0, 14.0));
                        ui.interact(anchor_rect, egui::Id::new(("route-anchor", rack_id.0, unit, side == RackSide::Front, offset_cm)), egui::Sense::hover())
                            .on_hover_text("Cable route anchor · select a cable to add or remove this anchor");
                        ui.painter().circle_filled(position, 5.0, egui::Color32::from_rgb(76, 104, 115));
                        continue;
                    };
                        let used = cable.route.iter().position(|candidate| *candidate == point);
                        let anchor_rect =
                            egui::Rect::from_center_size(position, egui::vec2(24.0, 24.0));
                        let response = ui.interact(
                            anchor_rect,
                            egui::Id::new((
                                "cable-route-anchor",
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
                        let mut proposed = cable.route.clone();
                        if let Some(index) = used { proposed.remove(index); } else { proposed.push(point); }
                        if response.hovered() {
                            let preview_route = if used.is_some() { &cable.route } else { &proposed };
                            for path in cables::visible_spans(cable.endpoints.0, cable.endpoints.1, preview_route, &anchors) {
                                cables::paint_preview(ui, path, cable.color);
                            }
                        }
                        if response.clicked() { actions.write(cable.id.reroute(proposed)); }
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
                        preview: None,
                        socket_rects: port_visuals
                            .iter()
                            .map(|(_, rect)| *rect)
                            .chain(power_socket_rects.iter().map(|(_, rect)| *rect))
                            .collect(),
                        interaction_enabled: state.pending_power_outlet.is_none()
                            && state.pending_power_inlet.is_none()
                            && state.pending_cable.is_none(),
                    },
                ) {
                    actions.write(UiAction::SelectLink(link));
                }
                if let Some(pending) = pending.as_ref() {
                    cables::show_creation_preview(ui, pending.start, &pending.route,
                        pending.color, &creation_targets, &anchors);
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
                            match sim.quote_routed_colored_cable(
                                first,
                                port_id,
                                state.cable_length_cm,
                                state.cable_color,
                                &state.pending_cable_route,
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
        DeviceKind::Ups(_) if side == RackSide::Front => (90.0 / 2048.0, 0.0, 1958.0 / 2048.0, 0.5),
        DeviceKind::Ups(_) => (0.0, 0.5, 1.0, 1.0),
        DeviceKind::Pdu(_) if side == RackSide::Front => {
            (78.0 / 2048.0, 128.0 / 768.0, 1970.0 / 2048.0, 370.0 / 768.0)
        }
        DeviceKind::Pdu(_) => (78.0 / 2048.0, 375.0 / 768.0, 1970.0 / 2048.0, 619.0 / 768.0),
        _ => (0.0, 0.0, 1.0, 1.0),
    };
    egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, bottom))
}

fn normalized_panel_position(panel: egui::Rect, normalized: (f32, f32)) -> egui::Pos2 {
    egui::pos2(
        panel.left() + panel.width() * normalized.0,
        panel.top() + panel.height() * normalized.1,
    )
}

fn device_power_inlet_rect(kind: &DeviceKind, panel: egui::Rect) -> Option<egui::Rect> {
    let inlet = kind.power_inlet()?;
    if inlet.connector == PowerInletConnector::CiscoFourPin {
        return Some(egui::Rect::from_center_size(
            normalized_panel_position(panel, inlet.position),
            egui::vec2(
                (panel.width() * 0.04).clamp(12.0, 24.0),
                (panel.height() * 0.4).clamp(12.0, 24.0),
            ),
        ));
    }
    Some(
        equipment::equipment_power_port_rect(kind, "c14", 0, panel).unwrap_or_else(|| {
            egui::Rect::from_center_size(
                normalized_panel_position(panel, inlet.position),
                egui::vec2(16.0, 22.0),
            )
        }),
    )
}

fn power_connector_kind(sim: &NetworkSim, socket: PowerSocket) -> cables::ConnectorKind {
    if let PowerSocket::Inlet(PowerEndpoint::Device(id)) = socket
        && sim
            .device(id)
            .and_then(|device| device.kind.power_inlet())
            .is_some_and(|inlet| inlet.connector == PowerInletConnector::CiscoFourPin)
    {
        return cables::ConnectorKind::CiscoFourPin;
    }
    cables::ConnectorKind::Iec
}

fn source_outlet_rect(kind: &DeviceKind, panel: egui::Rect, index: usize) -> egui::Rect {
    if let Some(rect) = equipment::equipment_power_port_rect(kind, "c13", index, panel) {
        return rect;
    }
    if let Some((x, y)) = equipment_power_port_position(kind, "c13", index) {
        return egui::Rect::from_center_size(
            normalized_panel_position(panel, (x, y)),
            egui::vec2(panel.width() * 0.064, panel.height() * 0.40),
        );
    }
    let (x, y, w, h) = match kind {
        DeviceKind::Ups(_) => (
            [0.611, 0.679, 0.748, 0.816][index.min(3)],
            0.479,
            0.064,
            0.40,
        ),
        DeviceKind::Pdu(_) => (0.105 + 0.0928 * index.min(7) as f32, 0.50, 0.073, 0.64),
        _ => (0.5, 0.5, 0.06, 0.4),
    };
    egui::Rect::from_center_size(
        normalized_panel_position(panel, (x, y)),
        egui::vec2(panel.width() * w, panel.height() * h),
    )
}

/// Power sockets are part of the equipment face interaction layer. They stay
/// deliberately simple so the photographed UPS/PDU/device faces remain the
/// only raster artwork used for connectors.
fn paint_power_socket(painter: &egui::Painter, texture: egui::TextureId, rect: egui::Rect) {
    painter.image(
        texture,
        rect,
        egui::Rect::from_min_max(egui::pos2(0.025, 0.15), egui::pos2(0.338, 0.34)),
        egui::Color32::WHITE,
    );
}

fn equipment_texture(
    textures: &EquipmentTextures,
    kind: &DeviceKind,
    side: RackSide,
) -> Option<egui::TextureId> {
    Some(match kind {
        DeviceKind::Server(_) if side == RackSide::Front => textures.server_front,
        DeviceKind::Server(_) => textures.server_rear,
        DeviceKind::Switch(_) if side == RackSide::Front => textures.switch_front,
        DeviceKind::Switch(_) => textures.switch_rear,
        DeviceKind::Router(_) if side == RackSide::Front => textures.router_front,
        DeviceKind::Router(_) => textures.router_rear,
        DeviceKind::Ups(_) => textures.ups_faces,
        DeviceKind::Pdu(_) => textures.pdu_faces,
        _ => return None,
    })
}

fn ups_lcd_rect(panel: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_max(
        normalized_panel_position(panel, ((0.681 * 2048.0 - 90.0) / 1868.0, 0.25)),
        normalized_panel_position(panel, ((0.781 * 2048.0 - 90.0) / 1868.0, 0.71)),
    )
}

fn power_socket_location(
    sim: &NetworkSim,
    socket: PowerSocket,
    view_side: RackSide,
) -> Option<CableRoutePoint> {
    let source_device = |source| {
        sim.devices().find(|device| match &device.kind {
            DeviceKind::Ups(ups) => ups.source == Some(source),
            DeviceKind::Pdu(pdu) => pdu.source == Some(source),
            _ => false,
        })
    };
    let (device, inlet) = match socket {
        PowerSocket::Outlet(outlet) => {
            if let SourceId::Rack(rack) = outlet.source {
                return Some(CableRoutePoint {
                    rack,
                    unit: sim.rack(rack)?.units,
                    side: view_side,
                    offset_cm: 0,
                });
            }
            (source_device(outlet.source)?, false)
        }
        PowerSocket::Inlet(PowerEndpoint::Device(id)) => (sim.device(id)?, true),
        PowerSocket::Inlet(PowerEndpoint::Source(source)) => (source_device(source)?, true),
    };
    let placement = device.rack?;
    let side = if inlet {
        device.kind.power_inlet()?.side
    } else {
        RackSide::Rear
    };
    Some(CableRoutePoint {
        rack: placement.rack,
        unit: placement.unit,
        side,
        offset_cm: if inlet { 48 } else { 0 },
    })
}

fn power_preview_is_valid(sim: &NetworkSim, outlet: OutletId, endpoint: PowerEndpoint) -> bool {
    let mut probe = sim.clone();
    probe
        .execute(cloud_provider_sim::Command::ConnectPower { outlet, endpoint })
        .is_ok()
}

/// Return the one continuous face occupied by a device, including the rack gap
/// between rows when its placement is taller than one unit.
fn rack_device_panel_rect(
    rack: &Rack,
    row_height: f32,
    unit: u8,
    row_face: egui::Rect,
) -> egui::Rect {
    let placement = rack
        .placements
        .iter()
        .find(|(_, placement)| {
            unit >= placement.unit && unit < placement.unit.saturating_add(placement.height.max(1))
        })
        .map(|(_, placement)| placement);
    let height = placement.map_or(1, |p| p.height.max(1));
    let offset = placement.map_or(0, |p| unit - p.unit) as f32 * row_height;
    egui::Rect::from_min_max(
        egui::pos2(
            row_face.left(),
            row_face.top() + offset - row_height * (height - 1) as f32,
        ),
        row_face.right_bottom() + egui::vec2(0.0, offset),
    )
}

fn rack_port_position(
    kind: &DeviceKind,
    rect: egui::Rect,
    index: usize,
    connector: PortConnector,
    side: RackSide,
) -> egui::Pos2 {
    let (x, y) = equipment::equipment_port_position(kind, connector, side, index)
        .unwrap_or_else(|| kind.port_position_normalized(index));
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
    side: RackSide,
) -> egui::Rect {
    let center = rack_port_position(kind, panel, index, connector, side);
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

fn paint_server_backplane(
    painter: &egui::Painter,
    panel: egui::Rect,
    kind: &DeviceKind,
    powered: bool,
) {
    use cloud_provider_sim::{PciCard, ServerPartKind, server_catalog};
    let DeviceKind::Server(server) = kind else {
        return;
    };
    let Some(hardware) = &server.hardware else {
        return;
    };
    let catalog = server_catalog();
    for (slot_index, slot) in catalog.chassis.pcie_slots.iter().enumerate() {
        let Some(part_id) = hardware.pcie.get(slot_index).and_then(Option::as_deref) else {
            continue;
        };
        let Some(part) = catalog.parts.iter().find(|part| part.id == part_id) else {
            continue;
        };
        let [left, top, right, bottom] = slot.face_rect;
        let plate = egui::Rect::from_min_max(
            egui::pos2(
                panel.left() + panel.width() * left,
                panel.top() + panel.height() * top,
            ),
            egui::pos2(
                panel.left() + panel.width() * right,
                panel.top() + panel.height() * bottom,
            ),
        );
        let metal = if powered {
            egui::Color32::from_rgb(79, 86, 89)
        } else {
            egui::Color32::from_gray(59)
        };
        painter.rect_filled(plate, 1.0, metal);
        painter.rect_stroke(
            plate,
            1.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(125)),
            egui::StrokeKind::Inside,
        );
        match &part.kind {
            ServerPartKind::PciCard {
                card: PciCard::Ethernet { .. },
            } => {
                for port_id in hardware.card_ports.get(slot_index).into_iter().flatten() {
                    let Some(index) = server.ports.iter().position(|id| id == port_id) else {
                        continue;
                    };
                    let socket =
                        rack_port_rect(kind, panel, index, PortConnector::Rj45, RackSide::Rear);
                    painter.rect_filled(socket, 1.0, egui::Color32::from_rgb(8, 12, 14));
                    painter.rect_stroke(
                        socket,
                        1.0,
                        egui::Stroke::new(1.0, egui::Color32::from_gray(162)),
                        egui::StrokeKind::Inside,
                    );
                    painter.rect_filled(
                        socket.shrink2(egui::vec2(socket.width() * 0.20, socket.height() * 0.24)),
                        0.0,
                        egui::Color32::from_rgb(24, 31, 33),
                    );
                }
            }
            _ => {}
        }
    }
}

fn paint_server_drives(painter: &egui::Painter, panel: egui::Rect, kind: &DeviceKind) {
    let DeviceKind::Server(server) = kind else {
        return;
    };
    let Some(hardware) = &server.hardware else {
        return;
    };
    for (bay, slot) in cloud_provider_sim::server_catalog()
        .chassis
        .drive_bays
        .iter()
        .enumerate()
    {
        let Some(id) = hardware.drives.get(bay).and_then(Option::as_deref) else {
            continue;
        };
        let [left, top, right, bottom] = slot.face_rect;
        let tray = egui::Rect::from_min_max(
            egui::pos2(
                panel.left() + panel.width() * left,
                panel.top() + panel.height() * top,
            ),
            egui::pos2(
                panel.left() + panel.width() * right,
                panel.top() + panel.height() * bottom,
            ),
        );
        painter.rect_stroke(
            tray,
            2.0,
            egui::Stroke::new(1.5, egui::Color32::from_gray(190)),
            egui::StrokeKind::Inside,
        );
        let model = cloud_provider_sim::drive_catalog()
            .drives
            .iter()
            .find(|drive| drive.id == id);
        let label = model.map_or("DRIVE", |drive| {
            if drive.kind == cloud_provider_sim::DriveKind::Ssd {
                "SSD"
            } else {
                "HDD"
            }
        });
        let badge = egui::Rect::from_min_size(
            tray.left_bottom() - egui::vec2(0.0, 11.0),
            egui::vec2(tray.width(), 11.0),
        );
        painter.rect_filled(badge, 1.0, egui::Color32::from_black_alpha(210));
        painter.text(
            badge.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::monospace(8.0),
            egui::Color32::from_rgb(175, 214, 216),
        );
    }
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

#[cfg(test)]
mod power_geometry_tests {
    use super::*;

    #[test]
    fn two_u_panel_is_one_coherent_span() {
        let rack = Rack {
            id: RackId(1),
            name: "test".into(),
            units: 4,
            placements: vec![(
                DeviceId(7),
                RackPlacement {
                    rack: RackId(1),
                    unit: 2,
                    height: 2,
                },
            )],
        };
        let face = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(480.0, 40.0));
        let panel = rack_device_panel_rect(&rack, 41.0, 2, face);
        assert_eq!(panel.top(), face.top() - 41.0);
        assert_eq!(panel.bottom(), face.bottom());
        assert_eq!(panel.height(), 81.0);
        let upper_face = face.translate(egui::vec2(0.0, -41.0));
        assert_eq!(rack_device_panel_rect(&rack, 41.0, 3, upper_face), panel);
    }

    #[test]
    fn two_u_power_hitbox_uses_full_asset_rectangle() {
        let kind = DeviceKind::Ups(cloud_provider_sim::Ups::default());
        let panel = egui::Rect::from_min_size(egui::pos2(20.0, 50.0), egui::vec2(480.0, 81.0));
        let inlet = equipment::equipment_power_port_rect(&kind, "c14", 0, panel).unwrap();
        assert!((inlet.width() - 480.0 * (0.9542687 - 0.88508135)).abs() < 0.001);
        assert!((inlet.height() - 81.0 * (0.62403804 - 0.2737745)).abs() < 0.001);
        assert!(panel.contains_rect(inlet));
        for index in 0..4 {
            assert!(panel.contains_rect(source_outlet_rect(&kind, panel, index)));
        }
    }

    #[test]
    fn router_power_inlet_uses_requested_front_normalized_position() {
        let panel = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(480.0, 40.0));
        let inlet = normalized_panel_position(panel, (0.146, 0.63));
        assert!((inlet.x - (10.0 + 480.0 * 0.146)).abs() < f32::EPSILON);
        assert!((inlet.y - (20.0 + 40.0 * 0.63)).abs() < f32::EPSILON);
        let mut sim = NetworkSim::new();
        let SimEvent::DeviceAdded(id) = sim
            .execute(cloud_provider_sim::Command::BuyDevice {
                kind: DeviceTemplate::Router,
            })
            .unwrap()[0]
        else {
            unreachable!()
        };
        let kind = &sim.device(id).unwrap().kind;
        let hitbox = device_power_inlet_rect(kind, panel).unwrap();
        assert!(hitbox.center().distance(inlet) < 0.001);
        let rear_c14 = equipment::equipment_power_port_rect(kind, "c14", 0, panel).unwrap();
        assert!(!hitbox.intersects(rear_c14));
        assert!(matches!(
            power_connector_kind(&sim, PowerSocket::Inlet(PowerEndpoint::Device(id))),
            cables::ConnectorKind::CiscoFourPin
        ));
        assert!(matches!(
            power_connector_kind(
                &sim,
                PowerSocket::Outlet(OutletId {
                    source: SourceId::Rack(RackId(1)),
                    index: 0
                })
            ),
            cables::ConnectorKind::Iec
        ));
    }

    #[test]
    fn power_preview_uses_canonical_cord_validation() {
        let mut sim = NetworkSim::new();
        let device = match sim
            .execute(cloud_provider_sim::Command::BuyDevice {
                kind: DeviceTemplate::Router,
            })
            .unwrap()[0]
        {
            SimEvent::DeviceAdded(id) => id,
            _ => unreachable!(),
        };
        sim.execute(cloud_provider_sim::Command::PlaceDevice {
            device,
            rack: RackId(1),
            unit: 1,
        })
        .unwrap();
        let outlet = OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 0,
        };
        assert!(power_preview_is_valid(
            &sim,
            outlet,
            PowerEndpoint::Device(device)
        ));

        let pdu = match sim
            .execute(cloud_provider_sim::Command::BuyDevice {
                kind: DeviceTemplate::Pdu,
            })
            .unwrap()[0]
        {
            SimEvent::DeviceAdded(id) => id,
            _ => unreachable!(),
        };
        sim.execute(cloud_provider_sim::Command::PlaceDevice {
            device: pdu,
            rack: RackId(1),
            unit: 2,
        })
        .unwrap();
        let source = match sim.device(pdu).unwrap().kind {
            DeviceKind::Pdu(ref pdu) => pdu.source.unwrap(),
            _ => unreachable!(),
        };
        assert!(!power_preview_is_valid(
            &sim,
            OutletId { source, index: 0 },
            PowerEndpoint::Source(source),
        ));
    }

    #[test]
    fn ups_lcd_uses_real_screen_bounds() {
        let panel = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(480.0, 40.0));
        let lcd = ups_lcd_rect(panel);
        let uv = equipment_uv(
            &DeviceKind::Ups(cloud_provider_sim::Ups::default()),
            RackSide::Front,
        );
        assert!(
            (uv.left() + (lcd.left() - panel.left()) / panel.width() * uv.width() - 0.681).abs()
                < 0.0001
        );
        assert!(
            (uv.left() + (lcd.right() - panel.left()) / panel.width() * uv.width() - 0.781).abs()
                < 0.0001
        );
        assert!((lcd.top() - (20.0 + 40.0 * 0.25)).abs() < f32::EPSILON);
        assert!((lcd.bottom() - (20.0 + 40.0 * 0.71)).abs() < f32::EPSILON);
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
    fn server_rear_backplane_gains_card_face_and_jacks_when_installed() {
        use cloud_provider_sim::{Command, SimEvent};
        let mut sim = NetworkSim::new();
        let SimEvent::DeviceAdded(id) = sim.execute(Command::BuyServerChassis).unwrap()[0] else {
            panic!()
        };
        let empty = sim.device(id).unwrap().kind.clone();
        for part_id in ["xeon_e_2434", "intel_i350_t4"] {
            sim.execute(Command::BuyServerPart {
                part_id: part_id.into(),
            })
            .unwrap();
            sim.execute(Command::InstallServerPart {
                device: id,
                part_id: part_id.into(),
                slot: None,
            })
            .unwrap();
        }
        let populated = sim.device(id).unwrap().kind.clone();
        let panel = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 60.0));
        let shape_count = |kind: &DeviceKind| {
            let ctx = egui::Context::default();
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                paint_server_backplane(ui.painter(), panel, kind, true);
            });
            let count = output.shapes.len();
            output.textures_delta.clear();
            count
        };
        assert!(shape_count(&populated) > shape_count(&empty));
    }

    #[test]
    fn server_front_marks_only_installed_drive_bays() {
        use cloud_provider_sim::{Command, SimEvent};
        let mut sim = NetworkSim::new();
        let SimEvent::DeviceAdded(id) = sim.execute(Command::BuyServerChassis).unwrap()[0] else {
            panic!()
        };
        let empty = sim.device(id).unwrap().kind.clone();
        sim.execute(Command::BuyDrive {
            drive_id: "enterprise_ssd_960gb".into(),
        })
        .unwrap();
        sim.execute(Command::InstallDrive {
            device: id,
            drive_id: "enterprise_ssd_960gb".into(),
            bay: Some(2),
        })
        .unwrap();
        let populated = sim.device(id).unwrap().kind.clone();
        let panel = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 60.0));
        let shape_count = |kind: &DeviceKind| {
            let ctx = egui::Context::default();
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                paint_server_drives(ui.painter(), panel, kind)
            });
            let count = output.shapes.len();
            output.textures_delta.clear();
            count
        };
        assert!(shape_count(&populated) > shape_count(&empty));
    }

    #[test]
    fn rack_face_uses_nineteen_inch_one_u_proportions() {
        let layout = RackLayout::new(820.0);
        let row = layout.row(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(layout.width, layout.row_height),
        ));
        let mounting_width = row.mounts[1].right() - row.mounts[0].left();
        assert!((mounting_width / layout.row_height - 19.0 / 1.75).abs() < 0.001);
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
        let kind = DeviceKind::PatchPanel(cloud_provider_sim::PatchPanel { ports: vec![] });
        let panel = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 50.0));
        for indices in [&front, &rear] {
            let sockets: Vec<_> = indices
                .iter()
                .map(|index| {
                    rack_port_rect(
                        &kind,
                        panel,
                        *index,
                        PortConnector::Rj45,
                        if index % 2 == 0 {
                            RackSide::Rear
                        } else {
                            RackSide::Front
                        },
                    )
                })
                .collect();
            for (index, socket) in sockets.iter().enumerate() {
                assert!(panel.contains_rect(*socket));
                assert!(
                    sockets[index + 1..]
                        .iter()
                        .all(|other| !socket.intersects(*other))
                );
            }
        }
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
