use crate::app::*;
use crate::localization::tr;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiTextureHandle, egui};
use cloud_provider_sim::*;
use std::collections::HashMap;
mod cables;
mod equipment;
mod inventory;
mod rack;
mod ranges;
mod room;
mod routing;
mod settings;
mod shop;
mod terminal_renderer;
mod upstream;
use cables::{CableScene, CableView, PowerCableView};
use equipment::equipment_power_port_position;
use rack::RackLayout;
use terminal_renderer::TerminalRenderer;

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

fn inventory_model_name(device: &Device) -> String {
    let name = crate::localization::device_name(device);
    name.split_once(" #")
        .map_or_else(|| name.clone(), |(model, _)| model.to_owned())
}

fn source_label(sim: &NetworkSim, source: SourceId) -> String {
    match source {
        SourceId::Rack(id) => sim.rack(id).map_or_else(
            || crate::localization::tr_args("ui.rack.3", &[(id).to_string()]),
            |r| crate::localization::tr_args("ui.mains", &[crate::localization::rack_name(r)]),
        ),
        SourceId::Ups(id) => sim
            .devices()
            .find_map(|d| match d.kind {
                DeviceKind::Ups(ref u) if u.source == Some(source) => Some(d.name.clone()),
                _ => None,
            })
            .unwrap_or_else(|| crate::localization::tr_args("ui.ups.2", &[(id).to_string()])),
        SourceId::Pdu(id) => sim
            .devices()
            .find_map(|d| match d.kind {
                DeviceKind::Pdu(ref p) if p.source == Some(source) => Some(d.name.clone()),
                _ => None,
            })
            .unwrap_or_else(|| crate::localization::tr_args("ui.pdu.2", &[(id).to_string()])),
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
    game_settings: Res<crate::settings::GameSettings>,
) -> Result {
    let _language = game_settings.localization.enter();
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
    top_bar(&mut viewport_ui, &snapshot.0, &mut state);
    if let Some(error) = state.error_dialog.clone() {
        let mut open = true;
        egui::Window::new(tr("error.title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(egui::vec2(420.0, 150.0))
            .show(&viewport_ui, |ui| {
                ui.colored_label(egui::Color32::LIGHT_RED, tr("ui.operation-failed"));
                ui.separator();
                ui.label(error.render());
                if ui.button(tr("ui.ok")).clicked() {
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
    TerminalRenderer::panel(&mut viewport_ui, &snapshot.0, &mut state, &mut actions);
    let textures = images.textures.expect("equipment textures registered");
    workspace(
        &mut viewport_ui,
        &snapshot.0,
        &mut state,
        &textures,
        &mut images.cables,
        &mut actions,
    );
    routing::show(
        &mut viewport_ui,
        &snapshot.0,
        &mut state,
        &mut drafts,
        &mut actions,
    );
    ranges::show(&mut viewport_ui, &snapshot.0, &mut state, &mut actions);
    shop::show(&mut viewport_ui, &snapshot.0, &mut state.shop, &mut actions);
    settings::show(
        &mut viewport_ui,
        &mut state.settings,
        &game_settings,
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

fn top_bar(viewport: &mut egui::Ui, sim: &NetworkSim, state: &mut UiState) -> egui::Rect {
    egui::Panel::top("top_bar")
        .default_size(64.0)
        .max_size(80.0)
        .resizable(false)
        .show(viewport, |ui| {
            ui.horizontal(|ui| {
                ui.heading(tr("ui.cloud-provider-room-01"));
                ui.separator();
                ui.strong(crate::localization::tr_args(
                    "format.price",
                    &[(sim.money).to_string()],
                ));
                for (workspace, label) in [
                    (Workspace::Room, "ui.room"),
                    (Workspace::Rack, "ui.rack"),
                    (Workspace::Topology, "ui.topology"),
                ] {
                    if ui
                        .selectable_label(state.workspace == workspace, tr(label))
                        .clicked()
                    {
                        state.workspace = workspace;
                    }
                }
                ui.separator();
                if ui
                    .selectable_label(state.ranges_open, tr("ui.ip-ranges"))
                    .clicked()
                {
                    state.ranges_open = !state.ranges_open;
                }
                if ui
                    .selectable_label(state.shop.open, tr("ui.shop"))
                    .clicked()
                {
                    state.shop.open = !state.shop.open;
                }
                if ui
                    .selectable_label(state.settings.open, tr("ui.settings"))
                    .clicked()
                {
                    state.settings.open = !state.settings.open;
                }
                ui.separator();
                if state.pending_cable.is_some() {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 196, 64),
                        crate::localization::tr_args(
                            "ui.rj45-cable-anchor-select-port",
                            &[
                                (state.pending_cable_route.len()).to_string(),
                                (if state.pending_cable_route.len() == 1 {
                                    ""
                                } else {
                                    "S"
                                })
                                .to_string(),
                            ],
                        ),
                    );
                    if ui.small_button(tr("ui.cancel-cable")).clicked() {
                        state.pending_cable = None;
                        state.pending_cable_route.clear();
                    }
                    ui.separator();
                }
                if state.pending_power_outlet.is_some() || state.pending_power_inlet.is_some() {
                    ui.colored_label(
                        egui::Color32::from_rgb(255, 196, 64),
                        crate::localization::tr_args(
                            "ui.power-cable-anchor-select-complementary-socket",
                            &[
                                (state.pending_power_route.len()).to_string(),
                                (if state.pending_power_route.len() == 1 {
                                    ""
                                } else {
                                    "S"
                                })
                                .to_string(),
                            ],
                        ),
                    );
                    if ui.small_button(tr("ui.cancel-power")).clicked() {
                        state.pending_power_outlet = None;
                        state.pending_power_inlet = None;
                        state.pending_power_route.clear();
                    }
                    ui.separator();
                }
                if let Some((notice, ok)) = &state.notice {
                    ui.separator();
                    ui.colored_label(
                        if *ok {
                            egui::Color32::LIGHT_GREEN
                        } else {
                            egui::Color32::LIGHT_RED
                        },
                        notice.render(),
                    );
                }
            });
            let revision = (sim.topology_revision, sim.routing_revision);
            if state.network_summary.revision != Some(revision) {
                state.network_summary.resources = sim.datacenter_resources();
                state.network_summary.revision = Some(revision);
            }
            let resources = state.network_summary.resources;
            ui.horizontal(|ui| {
                ui.weak(crate::localization::tr_args(
                    "ui.lan-reachable-core-mhz-weighted-gb-nic",
                    &[
                        (resources.lan.compute_mhz).to_string(),
                        (resources.lan.memory_score_gb).to_string(),
                        (resources.lan.network_mbps).to_string(),
                    ],
                ));
                ui.separator();
                ui.weak(crate::localization::tr_args(
                    "ui.public-reachable-core-mhz-weighted-gb-nic",
                    &[
                        (resources.global.compute_mhz).to_string(),
                        (resources.global.memory_score_gb).to_string(),
                        (resources.global.network_mbps).to_string(),
                    ],
                ));
            });
        })
        .response
        .rect
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
            ui.heading(tr("inspector.title"));
            ui.separator();
            match state.selected {
                Selection::None => {
                    ui.label(tr("ui.select-a-device-port-or-cable"));
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
    ui.heading(tr("ui.power-cable"));
    let Some(endpoint) = sim.power.connections.get(&outlet).copied() else {
        ui.label(tr("ui.power-cable-no-longer-connected"));
        return;
    };
    let destination = match endpoint {
        PowerEndpoint::Device(id) => sim.device(id).map_or_else(
            || crate::localization::tr_args("ui.device", &[format!("{:?}", id)]),
            |d| d.name.clone(),
        ),
        PowerEndpoint::Source(source) => source_label(sim, source),
    };
    ui.label(crate::localization::tr_args(
        "ui.source-c13",
        &[
            (source_label(sim, outlet.source)).to_string(),
            (outlet.index + 1).to_string(),
        ],
    ));
    ui.label(crate::localization::tr_args(
        "ui.destination.2",
        std::slice::from_ref(&destination),
    ));
    ui.label(crate::localization::tr_args(
        "ui.cord",
        &[format!("{:?}", sim.power.cord_kind(outlet))],
    ));
    let anchors = sim.power.cord_routes.get(&outlet).map_or(0, Vec::len);
    ui.label(crate::localization::tr_args(
        "ui.rack-anchors",
        &[(anchors).to_string()],
    ));
    ui.weak(tr("ui.in-rack-view-click-a-rail-anchor"));
    if anchors > 0 && ui.small_button(tr("ui.clear-rack-anchors")).clicked() {
        actions.write(UiAction::ReroutePowerCable {
            outlet,
            route: Vec::new(),
        });
    }
    if ui.button(tr("ui.unplug-cable")).clicked() {
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
        ui.label(tr("ui.device-no-longer-exists"));
        return;
    };
    ui.heading(crate::localization::device_name(device));
    power_controls(ui, sim, device, actions);
    if let DeviceKind::Server(server) = &device.kind
        && let Some(hardware) = &server.hardware
    {
        server_hardware_inspector(ui, sim, id, hardware, actions);
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
            && ui
                .button(tr("ui.eject-from-rack"))
                .on_hover_text(tr(
                    "ui.uninstall-this-device-connected-cables-are-unplugged",
                ))
                .clicked()
        {
            actions.write(UiAction::Remove(id));
        }
        ui.separator();
        ui.weak(tr("ui.passive-rack-hardware-has-no-power-status"));
        return;
    }
    ui.colored_label(
        if actual_powered {
            egui::Color32::GREEN
        } else {
            egui::Color32::DARK_GRAY
        },
        tr(if actual_powered {
            "ui.power-on"
        } else {
            "ui.power-off"
        }),
    );
    if device.rack.is_some()
        && ui
            .button(tr("ui.eject-from-rack"))
            .on_hover_text(tr(
                "ui.uninstall-this-device-connected-cables-are-unplugged",
            ))
            .clicked()
    {
        actions.write(UiAction::Remove(id));
    }
    if device.rack.is_none() {
        ui.label(tr("ui.select-an-empty-rack-unit-in-the"));
        state.workspace = Workspace::Rack;
    }
    ui.separator();
    if matches!(device.kind, DeviceKind::Router(_)) && ui.button(tr("ui.routing-table")).clicked() {
        actions.write(UiAction::OpenRouting(id));
    }
    ui.strong(tr("ui.ports"));
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
                    tr(if link.is_some() { "●" } else { "○" }),
                );
                if ui
                    .add_enabled(
                        port.connector.supports_cabling(),
                        egui::Button::selectable(false, &port.name),
                    )
                    .on_hover_text(tr(if port.connector.supports_cabling() {
                        "ui.select-port"
                    } else {
                        "ui.sfp-cabling-is-not-implemented-yet"
                    }))
                    .clicked()
                {
                    actions.write(UiAction::SelectPort(*port_id));
                }
                ui.weak(tr(connector_label(port.connector)));
                ui.end_row();
            }
        });
    if let DeviceKind::Switch(sw) = &device.kind {
        ui.separator();
        ui.strong(tr("ui.vlan-database"));
        for vlan in &sw.vlans {
            ui.label(crate::localization::tr_args(
                "vlan.entry",
                &[(vlan.id.0).to_string(), (vlan.name).to_string()],
            ));
        }
        ui.horizontal(|ui| {
            ui.label(tr("ui.id"));
            ui.text_edit_singleline(&mut state.new_vlan_id);
        });
        ui.horizontal(|ui| {
            ui.label(tr("ui.name"));
            ui.text_edit_singleline(&mut state.new_vlan_name);
        });
        if ui.button(tr("ui.create-vlan")).clicked() {
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
    ui.strong(tr("ui.server-hardware"));
    ui.label(crate::localization::tr_args(
        "ui.cpu-ram-integrated-psu-w",
        &[
            (hardware.cpus.len()).to_string(),
            (chassis.cpu_sockets).to_string(),
            (hardware.ram.len()).to_string(),
            (chassis.dimm_slots).to_string(),
            (chassis.integrated_psu_watts).to_string(),
        ],
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
    if hardware.ready() {
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
                if ui.button(tr("ui.remove")).clicked() {
                    actions.write(UiAction::RemoveServerPart {
                        device,
                        part_id: id.into(),
                        slot: Some(index),
                    });
                }
            } else {
                ui.weak(tr("ui.empty"));
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
                        && ui
                            .button(crate::localization::tr_args(
                                "hardware.install-item",
                                &[crate::localization::item_name(&part.id, &part.name)],
                            ))
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
            if ui
                .add_enabled(owned > 0, egui::Button::new(tr("ui.install")))
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
                && ui.button(tr("ui.remove")).clicked()
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
            ui.label(crate::localization::tr_args(
                "ui.cisco-adapter-66-w-max-12-v",
                &[
                    (power.load.watts).to_string(),
                    format!("{:.2}", power.load.watts as f32 / 12.0),
                    (ac.watts).to_string(),
                    (ac.va).to_string(),
                    format!("{:.2}", ac.current_ma as f32 / 1000.0),
                ],
            ));
        } else {
            ui.label(crate::localization::tr_args(
                "ui.load-w-va-a",
                &[
                    (power.load.watts).to_string(),
                    (power.load.va).to_string(),
                    format!("{:.2}", power.load.current_ma as f32 / 1000.0),
                ],
            ));
        }
        ui.colored_label(
            if power.effective {
                egui::Color32::LIGHT_GREEN
            } else {
                egui::Color32::YELLOW
            },
            tr(if power.effective {
                "ui.effective-power-on"
            } else if power.requested {
                "ui.requested-on-source-unavailable"
            } else {
                "ui.requested-off"
            }),
        );
        if ui
            .button(tr(if power.requested {
                "ui.turn-device-off"
            } else {
                "ui.turn-device-on"
            }))
            .clicked()
        {
            actions.write(UiAction::TogglePower(device.id, !power.requested));
        }
    }
    if let Some(source) = source {
        let telemetry = sim.power.source_telemetry(source);
        ui.separator();
        ui.strong(tr(if matches!(device.kind, DeviceKind::Ups(_)) {
            "ui.ups-status"
        } else {
            "ui.pdu-status"
        }));
        let enabled = sim.power.source_enabled(source);
        if ui
            .button(tr(if enabled {
                "ui.turn-source-off"
            } else {
                "ui.turn-source-on"
            }))
            .clicked()
        {
            actions.write(UiAction::TogglePower(device.id, !enabled));
        }
        if let Some(t) = telemetry {
            let state = if !t.available {
                "ui.off-tripped-no-input"
            } else if matches!(source, SourceId::Ups(_)) && !t.input_available {
                "ui.on-battery"
            } else {
                "ui.on-mains"
            };
            ui.label(crate::localization::tr_args(
                "ui.output-w-va-a",
                &[
                    tr(state),
                    (t.output.watts).to_string(),
                    (t.output.va).to_string(),
                    format!("{:.2}", t.output.current_ma as f32 / 1000.0),
                ],
            ));
            ui.label(crate::localization::tr_args(
                "ui.input-w-va-a",
                &[
                    (t.input.watts).to_string(),
                    (t.input.va).to_string(),
                    format!("{:.2}", t.input.current_ma as f32 / 1000.0),
                ],
            ));
            if let SourceId::Ups(id) = source {
                let cap = sim.power.ups.get(&id).map_or(0, |u| u.spec.battery_wh);
                ui.label(crate::localization::tr_args(
                    "ui.battery-wh",
                    &[
                        (t.battery_mwh / 1000).to_string(),
                        (cap).to_string(),
                        format!(
                            "{:.0}",
                            if cap == 0 {
                                0.0
                            } else {
                                t.battery_mwh as f32 / (cap as f32 * 1000.0) * 100.0
                            }
                        ),
                        (t.runtime_seconds.map_or(String::new(), |s| {
                            crate::localization::tr_args(
                                "power.runtime",
                                &[(s / 60).to_string(), (s % 60).to_string()],
                            )
                        }))
                        .to_string(),
                    ],
                ));
                if sim.power.ups.get(&id).is_some_and(|u| u.tripped)
                    && ui.button(tr("ui.reset-ups-breaker")).clicked()
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
                && ui.button(tr("ui.reset-pdu-breaker")).clicked()
            {
                actions.write(UiAction::ResetPower(source));
            }
        }
        ui.label(tr("ui.input-connect-this-device-s-c14-inlet"));
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
            ui.label(crate::localization::tr_args(
                "ui.fed-by-c13",
                &[
                    (source_label(sim, outlet.source)).to_string(),
                    (outlet.index + 1).to_string(),
                ],
            ));
            if ui.small_button(tr("ui.unplug")).clicked() {
                actions.write(UiAction::DisconnectPower(outlet));
            }
        });
    } else if endpoint.is_some() {
        ui.weak(tr("ui.power-inlet-click-the-device-socket-then"));
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
        ui.label(tr("ui.port-no-longer-exists"));
        return;
    };
    if let Some(outlet) = sim.network_outlet(id) {
        ui.heading(if matches!(outlet.kind, NetworkOutletKind::Uplink { .. }) {
            port.name.clone()
        } else {
            outlet.name()
        });
        ui.label(crate::localization::tr_args(
            "ui.rj45-port",
            &[(id.0).to_string()],
        ));
        match outlet.kind {
            NetworkOutletKind::Lan { .. } => {
                ui.label(tr("ui.passive-room-socket-wire-it-to-your"));
            }
            NetworkOutletKind::Uplink { .. } => {
                upstream::show(ui, sim, id, actions);
            }
        }
        if let Some(link) = sim.link_for_port(id) {
            ui.label(crate::localization::tr_args(
                "ui.connected-cable",
                &[(link.id.0).to_string()],
            ));
            if ui.button(tr("ui.disconnect-cable")).clicked() {
                actions.write(UiAction::Disconnect(link.id));
            }
        } else if ui.button(tr("ui.connect-rj45-cable")).clicked() {
            actions.write(UiAction::CablePort(id));
        }
        return;
    }
    let owner = sim.device(port.device).expect("port owner exists");
    ui.heading(crate::localization::tr_args(
        "port.qualified-name",
        &[
            crate::localization::device_name(owner),
            (port.name).to_string(),
        ],
    ));
    ui.label(crate::localization::tr_args(
        "ui.connector",
        &[tr(connector_label(port.connector))],
    ));
    if !port.connector.supports_cabling() {
        ui.colored_label(
            egui::Color32::from_rgb(255, 196, 64),
            tr("ui.sfp-is-visible-on-the-physical-device"),
        );
        return;
    }
    let link = sim.link_for_port(id);
    ui.label(match link {
        Some(link) => crate::localization::tr_args("ui.link-up-cable", &[(link.id.0).to_string()]),
        None => tr("ui.link-down"),
    });
    if link.is_none() && ui.button(tr("ui.connect-rj45-cable")).clicked() {
        actions.write(UiAction::CablePort(id));
    }
    ui.weak(tr("ui.select-the-other-socket-in-the-rack"));
    if let Some(link) = link
        && ui.button(tr("ui.disconnect-cable")).clicked()
    {
        actions.write(UiAction::Disconnect(link.id));
    }
    if matches!(
        owner.kind,
        DeviceKind::PatchPanel(_) | DeviceKind::CableManager(_)
    ) {
        ui.weak(tr("ui.passive-port-cable-connections-are-managed-from"));
        return;
    }
    ui.separator();
    match &port.config {
        PortConfig::Server(config) => {
            ui.label(crate::localization::tr_args(
                "ui.current-ipv4",
                &[(config.ipv4.as_ref().map_or_else(
                    || tr("ui.unassigned"),
                    |ip| format!("{}/{}", ip.address, ip.prefix),
                ))
                .to_string()],
            ));
            if ui.button(tr("ui.assign-next-room-lan-ipv4")).clicked() {
                actions.write(UiAction::AssignLanIpv4 { port: id });
            }
            for block in sim
                .public_ipv4_blocks()
                .iter()
                .filter(|_| port.name != "mgmt0")
            {
                let prefix = Ipv4Prefix::new(block.network, PublicIpv4Block::PREFIX).expect("/29");
                let uplink = sim
                    .range_uplink(prefix)
                    .and_then(|id| sim.network_outlet(id))
                    .map_or_else(|| tr("ui.unassigned"), |outlet| outlet.name());
                ui.horizontal(|ui| {
                    ui.label(crate::localization::tr_args(
                        "ui.29-via",
                        &[(block.network).to_string(), (uplink).to_string()],
                    ));
                    if ui
                        .button(crate::localization::tr_args(
                            "ui.assign-public-ip",
                            &[(block.network).to_string()],
                        ))
                        .clicked()
                    {
                        actions.write(UiAction::AssignPublicIpv4 {
                            port: id,
                            network: block.network,
                        });
                    }
                });
            }
            let draft = drafts.servers.entry(id).or_insert_with(|| {
                let ip = config.ipv4.as_ref();
                ServerDraft {
                    synced_ipv4: config.ipv4.clone(),
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
            if draft.synced_ipv4 != config.ipv4 {
                draft.address = config
                    .ipv4
                    .as_ref()
                    .map(|ip| ip.address.to_string())
                    .unwrap_or_default();
                draft.prefix = config
                    .ipv4
                    .as_ref()
                    .map_or_else(|| "24".into(), |ip| ip.prefix.to_string());
                draft.gateway = config
                    .ipv4
                    .as_ref()
                    .and_then(|ip| ip.gateway)
                    .map_or_else(String::new, |ip| ip.to_string());
                draft.vlan = config
                    .ipv4
                    .as_ref()
                    .and_then(|ip| ip.vlan)
                    .map_or_else(String::new, |vlan| vlan.0.to_string());
                draft.synced_ipv4 = config.ipv4.clone();
            }
            ui.label(tr("ui.hostname"));
            ui.text_edit_singleline(&mut draft.hostname);
            ui.label(tr("ui.ipv4-address"));
            ui.text_edit_singleline(&mut draft.address);
            ui.horizontal(|ui| {
                ui.label(tr("ui.prefix"));
                ui.text_edit_singleline(&mut draft.prefix);
            });
            ui.label(tr("ui.access-vlan-optional-blank-untagged"));
            ui.text_edit_singleline(&mut draft.vlan);
            ui.label(tr("ui.default-gateway"));
            ui.text_edit_singleline(&mut draft.gateway);
            if ui.button(tr("ui.apply-server-config")).clicked() {
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
            ui.checkbox(&mut draft.trunk, tr("ui.trunk-mode"));
            if draft.trunk {
                ui.label(tr("ui.allowed-vlans-comma-separated"));
                ui.text_edit_singleline(&mut draft.allowed);
            } else {
                ui.label(tr("ui.access-vlan-optional-blank-untagged"));
                ui.text_edit_singleline(&mut draft.vlan);
            }
            if ui.button(tr("ui.apply-switch-port")).clicked() {
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
            ui.label(tr("ui.interface-name"));
            ui.text_edit_singleline(&mut draft.name);
            ui.label(tr("ui.ipv4-blank-unconfigured"));
            ui.text_edit_singleline(&mut draft.address);
            ui.horizontal(|ui| {
                ui.label(tr("ui.prefix"));
                ui.add(egui::TextEdit::singleline(&mut draft.prefix).desired_width(55.0));
                ui.label(tr("ui.vlan"));
                ui.add(egui::TextEdit::singleline(&mut draft.vlan).desired_width(55.0));
            });
            if ui.button(tr("ui.routing-table")).clicked() {
                actions.write(UiAction::OpenRouting(port.device));
            }
            draft.internet = false;
            if ui.button(tr("ui.apply-router-interface")).clicked() {
                actions.write(UiAction::ApplyRouter(id));
            }
        }
        PortConfig::PatchPanel | PortConfig::CableManager | PortConfig::Infrastructure => {
            ui.label(tr("ui.passive-physical-interface-paired-ports-follow-the"));
        }
    }
    ui.separator();
    if ui
        .button(tr("ui.flush-configuration"))
        .on_hover_text(tr("ui.reset-this-interface-to-its-device-template"))
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
        ui.label(tr("ui.cable-no-longer-exists"));
        return;
    };
    let endpoint = |id| {
        sim.port(id)
            .map(|p| {
                crate::localization::tr_args(
                    "port.qualified-name",
                    &[
                        (sim.device(p.device).map(|d| d.name.as_str()).unwrap_or("?")).to_string(),
                        (p.name).to_string(),
                    ],
                )
            })
            .unwrap_or_default()
    };
    ui.heading(crate::localization::tr_args(
        "ui.cable",
        &[(id.0).to_string()],
    ));
    ui.label(crate::localization::tr_args(
        "ui.m-ethernet-lead-2-rj45-plugs",
        &[
            format!("{:.2}", link.length_cm as f32 / 100.0),
            crate::localization::cable_color(link.color),
        ],
    ));
    ui.weak(tr("cable.description"));
    ui.label(endpoint(link.a));
    ui.label("↕");
    ui.label(endpoint(link.b));
    ui.separator();
    ui.strong(tr("ui.manual-route"));
    ui.weak(tr("ui.ordered-rack-anchors-shape-the-physical-cable"));
    let route = link.route.clone();
    for (index, point) in route.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.monospace(crate::localization::tr_args(
                "ui.u-cm",
                &[
                    (index + 1).to_string(),
                    (point.unit).to_string(),
                    crate::localization::rack_side(point.side),
                    (point.offset_cm).to_string(),
                ],
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
            if ui.small_button(tr("ui.left")).clicked() {
                let mut moved = *point;
                moved.offset_cm = 0;
                actions.write(UiAction::MoveCableRoutePoint {
                    link: id,
                    index,
                    point: moved,
                });
            }
            if ui.small_button(tr("ui.right")).clicked() {
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
        ui.label(tr("ui.no-anchors-click-a-rack-anchor-to"));
    }
    if ui.button(tr("ui.disconnect")).clicked() {
        actions.write(UiAction::Disconnect(id));
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
        let Some(rack) = state
            .active_rack
            .and_then(|id| sim.rack(id))
            .or_else(|| sim.racks().min_by_key(|r| r.id))
        else {
            return;
        };
        ui.vertical_centered(|ui| {
            ui.heading(crate::localization::tr_args(
                "ui.side",
                &[
                    crate::localization::rack_name(rack),
                    crate::localization::rack_side(state.rack_side),
                ],
            ));
            ui.horizontal(|ui| {
                ui.menu_button(
                    crate::localization::tr_args(
                        "ui.select-rack",
                        &[crate::localization::rack_name(rack)],
                    ),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(300.0)
                            .show(ui, |ui| {
                                let mut racks: Vec<_> = sim.racks().collect();
                                racks.sort_by_key(|rack| rack.id);
                                for candidate in racks {
                                    if ui
                                        .selectable_label(candidate.id == rack.id, &candidate.name)
                                        .clicked()
                                    {
                                        state.active_rack = Some(candidate.id);
                                        ui.close();
                                    }
                                }
                            });
                    },
                );
                if ui.button(tr("ui.room-view")).clicked() {
                    state.workspace = Workspace::Room;
                }
            });
            ui.horizontal(|ui| {
                for side in [RackSide::Front, RackSide::Rear] {
                    if ui
                        .selectable_label(
                            state.rack_side == side,
                            crate::localization::rack_side(side),
                        )
                        .clicked()
                    {
                        state.rack_side = side;
                    }
                }
                ui.separator();
                ui.label(tr("ui.cables.2"));
                for visibility in [
                    CableVisibility::All,
                    CableVisibility::Selected,
                    CableVisibility::Hidden,
                ] {
                    if ui
                        .selectable_label(
                            state.cable_visibility == visibility,
                            crate::localization::cable_visibility(visibility),
                        )
                        .clicked()
                    {
                        state.cable_visibility = visibility;
                    }
                }
            });
            ui.weak(tr("cable.install-guide"));
            if let Some(rp) = sim.power.racks.get(&rack.id) {
                ui.horizontal(|ui| {
                    let reading = sim.power.source_telemetry(SourceId::Rack(rack.id));
                    let watts = reading.map_or(0, |x| x.output.watts);
                    let amps = reading.map_or(0.0, |x| x.output.current_ma as f32 / 1000.0);
                    ui.label(crate::localization::tr_args(
                        "ui.power-mains-breaker-w-a-4-c13",
                        &[
                            tr(if rp.mains_on { "ui.on" } else { "ui.off" }),
                            tr(if rp.breaker_on { "ui.ok" } else { "ui.tripped" }),
                            (watts).to_string(),
                            format!("{:.2}", amps),
                        ],
                    ));
                    if ui
                        .button(tr(if rp.mains_on {
                            "ui.mains-off"
                        } else {
                            "ui.mains-on"
                        }))
                        .clicked()
                    {
                        actions.write(UiAction::RackMains(rack.id, !rp.mains_on));
                    }
                    if !rp.breaker_on && ui.button(tr("ui.reset-breaker")).clicked() {
                        actions.write(UiAction::ResetPower(SourceId::Rack(rack.id)));
                    }
                });
                for source in power_sources(sim)
                    .into_iter()
                    .filter(|s| !matches!(s, SourceId::Rack(_)))
                {
                    if let Some(t) = sim.power.source_telemetry(source) {
                        ui.label(crate::localization::tr_args(
                            "ui.w-va",
                            &[
                                (source_label(sim, source)).to_string(),
                                (t.output.watts).to_string(),
                                (t.output.va).to_string(),
                                tr(if t.available {
                                    "ui.online"
                                } else {
                                    "ui.offline"
                                }),
                            ],
                        ));
                    }
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
                        layout.paint_crossbar(
                            ui,
                            &crate::localization::tr_args(
                                "rack.crossbar",
                                &[
                                    (rack.units).to_string(),
                                    crate::localization::rack_side(state.rack_side),
                                ],
                            )
                            .to_uppercase(),
                        );
                        ui.horizontal(|ui| {
                            ui.label(tr("ui.rack-c13"));
                            for n in 0..4u8 {
                                let outlet = OutletId {
                                    source: SourceId::Rack(rack.id),
                                    index: n,
                                };
                                let occupied = sim.power.connections.contains_key(&outlet);
                                let response = ui
                                    .allocate_response(egui::vec2(44.0, 28.0), egui::Sense::click())
                                    .on_hover_text(tr(if occupied {
                                        "ui.c13-occupied-right-click-to-unplug"
                                    } else {
                                        "ui.c13-outlet-click-to-connect"
                                    }));
                                paint_power_socket(
                                    ui.painter(),
                                    textures.power_connectors,
                                    response.rect.shrink(2.0),
                                );
                                power_socket_rects
                                    .push((PowerSocket::Outlet(outlet), response.rect));
                                if state.pending_power_outlet == Some(outlet)
                                    || state.pending_power_inlet.is_some()
                                    || state.selected == Selection::PowerCable(outlet)
                                    || response.hovered()
                                {
                                    ui.painter().rect_stroke(
                                        response.rect.shrink(1.0),
                                        2.0,
                                        egui::Stroke::new(
                                            2.0,
                                            if state.selected == Selection::PowerCable(outlet) {
                                                egui::Color32::from_rgb(120, 205, 255)
                                            } else if response.hovered() {
                                                egui::Color32::LIGHT_BLUE
                                            } else {
                                                egui::Color32::from_rgb(255, 196, 64)
                                            },
                                        ),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                                power_outlets.insert(outlet, response.rect.center());
                                if response.clicked() {
                                    actions.write(if occupied {
                                        UiAction::SelectPowerCable(outlet)
                                    } else {
                                        UiAction::PowerSocket(crate::app::PowerSocket::Outlet(
                                            outlet,
                                        ))
                                    });
                                }
                                response.context_menu(|menu| {
                                    if occupied && menu.button(tr("ui.unplug-cable")).clicked() {
                                        actions.write(UiAction::DisconnectPower(outlet));
                                        menu.close();
                                    }
                                });
                            }
                        });
                        if let Some(outlet) = sim
                            .network_outlets()
                            .find(|outlet| outlet.kind == NetworkOutletKind::Lan { rack: rack.id })
                        {
                            ui.horizontal(|ui| {
                                ui.label(tr("ui.room-lan"));
                                let connected = sim.link_for_port(outlet.port).is_some();
                                let response = ui
                                    .allocate_response(egui::vec2(86.0, 27.0), egui::Sense::click())
                                    .on_hover_text(crate::localization::tr_args(
                                        "ui.rack-lan-port-click-to-connect",
                                        &[(outlet.port.0).to_string()],
                                    ));
                                ui.painter().rect_filled(
                                    response.rect.shrink(1.0),
                                    2.0,
                                    egui::Color32::from_rgb(37, 49, 55),
                                );
                                let socket = egui::Rect::from_center_size(
                                    egui::pos2(
                                        response.rect.left() + 17.0,
                                        response.rect.center().y,
                                    ),
                                    egui::vec2(25.0, 18.0),
                                );
                                room::NetworkSocketRenderer::paint(
                                    ui.painter(),
                                    socket,
                                    connected,
                                    state.selected == Selection::Port(outlet.port),
                                );
                                ui.painter().text(
                                    egui::pos2(
                                        response.rect.left() + 33.0,
                                        response.rect.center().y,
                                    ),
                                    egui::Align2::LEFT_CENTER,
                                    tr("ui.lan"),
                                    egui::FontId::monospace(10.0),
                                    egui::Color32::WHITE,
                                );
                                port_visuals.push((outlet.port, response.rect));
                            });
                        }
                        for unit in (1..=rack.units).rev() {
                            let (row_rect, row_response) = ui.allocate_exact_size(
                                egui::vec2(rack_width, row_height),
                                egui::Sense::click(),
                            );
                            let row = layout.row(row_rect);
                            let panel_rect =
                                rack_device_panel_rect(rack, layout.row_height, unit, row.face);
                            row.paint(ui.painter(), unit, rack.occupies(unit).is_some());
                            for (offset_cm, position) in [0, 48].into_iter().zip(row.anchors) {
                                route_anchors.push((
                                    rack.id,
                                    unit,
                                    state.rack_side,
                                    offset_cm,
                                    position,
                                ));
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
                                    panel_rect,
                                    egui::Id::new(("device-panel", device_id.0)),
                                    egui::Sense::click(),
                                );
                                row_response.context_menu(|menu| {
                                    menu.label(crate::localization::device_name(device));
                                    if let Some(endpoint) = device_power_endpoint(device)
                                        && let Some((outlet, _)) = sim
                                            .power
                                            .connections
                                            .iter()
                                            .find(|(_, target)| **target == endpoint)
                                        && menu
                                            .button(crate::localization::tr_args(
                                                "ui.unplug-power-c13",
                                                &[
                                                    (source_label(sim, outlet.source)).to_string(),
                                                    (outlet.index + 1).to_string(),
                                                ],
                                            ))
                                            .clicked()
                                    {
                                        actions.write(UiAction::DisconnectPower(*outlet));
                                        menu.close();
                                    }
                                    if menu.button(tr("ui.eject-from-rack")).clicked() {
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
                                                panel_rect.left()
                                                    + panel_rect.width() * offset_cm as f32 / 48.0,
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
                                        let sides: Vec<_> = device
                                            .ports()
                                            .iter()
                                            .map(|id| {
                                                sim.port(*id).expect("patch port exists").side
                                            })
                                            .collect();
                                        for (slot, index) in
                                            visible_patch_port_indices(&sides, state.rack_side)
                                                .into_iter()
                                                .enumerate()
                                        {
                                            let port_id = &device.ports()[index];
                                            let port =
                                                sim.port(*port_id).expect("patch port exists");
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
                                                crate::localization::tr_args(
                                                    "format.two-digit-number",
                                                    &[format!("{:02}", slot + 1)],
                                                ),
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
                                                panel_rect.left()
                                                    + panel_rect.width() * offset_cm as f32 / 48.0,
                                                panel_rect.center().y,
                                            );
                                            ui.painter().circle_stroke(
                                                center,
                                                5.0,
                                                egui::Stroke::new(
                                                    2.0,
                                                    egui::Color32::from_rgb(82, 92, 99),
                                                ),
                                            );
                                        }
                                    }
                                    _ => {
                                        let textured = matches!(
                                            device.kind,
                                            DeviceKind::Server(_)
                                                | DeviceKind::Switch(_)
                                                | DeviceKind::Router(_)
                                                | DeviceKind::Ups(_)
                                                | DeviceKind::Pdu(_)
                                        );
                                        if textured {
                                            let texture = match device.kind {
                                                DeviceKind::Server(_)
                                                    if state.rack_side == RackSide::Front =>
                                                {
                                                    textures.server_front
                                                }
                                                DeviceKind::Server(_) => textures.server_rear,
                                                DeviceKind::Switch(_)
                                                    if state.rack_side == RackSide::Front =>
                                                {
                                                    textures.switch_front
                                                }
                                                DeviceKind::Switch(_) => textures.switch_rear,
                                                DeviceKind::Router(_)
                                                    if state.rack_side == RackSide::Front =>
                                                {
                                                    textures.router_front
                                                }
                                                DeviceKind::Router(_) => textures.router_rear,
                                                DeviceKind::Ups(_) => textures.ups_faces,
                                                DeviceKind::Pdu(_) => textures.pdu_faces,
                                                _ => unreachable!(),
                                            };
                                            ui.painter().image(
                                                texture,
                                                panel_rect,
                                                equipment_uv(&device.kind, state.rack_side),
                                                if device.powered {
                                                    egui::Color32::WHITE
                                                } else {
                                                    egui::Color32::from_gray(90)
                                                },
                                            );
                                            if state.rack_side == RackSide::Rear {
                                                paint_server_backplane(
                                                    ui.painter(),
                                                    panel_rect,
                                                    &device.kind,
                                                    device.powered,
                                                );
                                            } else {
                                                paint_server_drives(
                                                    ui.painter(),
                                                    panel_rect,
                                                    &device.kind,
                                                );
                                            }
                                            if matches!(device.kind, DeviceKind::Ups(_))
                                                && state.rack_side == RackSide::Front
                                            {
                                                let lcd = ups_lcd_rect(panel_rect);
                                                ui.painter().rect_filled(
                                                    lcd,
                                                    1.0,
                                                    egui::Color32::from_rgb(12, 25, 28),
                                                );
                                                let telemetry = match device.kind {
                                                    DeviceKind::Ups(ref ups) => {
                                                        ups.source.and_then(|s| {
                                                            sim.power.source_telemetry(s)
                                                        })
                                                    }
                                                    _ => None,
                                                };
                                                let (battery, load) =
                                                    telemetry.map_or((0, 0), |t| {
                                                        let cap = match device.kind {
                                                            DeviceKind::Ups(ref ups) => {
                                                                ups.source.and_then(|s| match s {
                                                                    SourceId::Ups(id) => {
                                                                        sim.power.ups.get(&id).map(
                                                                            |u| u.spec.battery_wh,
                                                                        )
                                                                    }
                                                                    _ => None,
                                                                })
                                                            }
                                                            _ => None,
                                                        }
                                                        .unwrap_or(0);
                                                        (
                                                            if cap == 0 {
                                                                0
                                                            } else {
                                                                (t.battery_mwh / 1000 * 100
                                                                    / u64::from(cap))
                                                                    as u32
                                                            },
                                                            t.output.watts,
                                                        )
                                                    });
                                                let font = egui::FontId::monospace(
                                                    (lcd.height() * 0.22).clamp(5.0, 9.0),
                                                );
                                                ui.painter().text(
                                                    egui::pos2(
                                                        lcd.center().x,
                                                        lcd.center().y - font.size,
                                                    ),
                                                    egui::Align2::CENTER_CENTER,
                                                    crate::localization::tr_args(
                                                        "ui.bat",
                                                        &[(battery).to_string()],
                                                    ),
                                                    font.clone(),
                                                    egui::Color32::from_rgb(112, 235, 184),
                                                );
                                                ui.painter().text(
                                                    egui::pos2(
                                                        lcd.center().x,
                                                        lcd.center().y + font.size,
                                                    ),
                                                    egui::Align2::CENTER_CENTER,
                                                    crate::localization::tr_args(
                                                        "ui.load-w",
                                                        &[(load).to_string()],
                                                    ),
                                                    font,
                                                    egui::Color32::from_rgb(112, 235, 184),
                                                );
                                                let button = egui::Rect::from_center_size(
                                                    normalized_panel_position(
                                                        panel_rect,
                                                        (0.814, 0.29),
                                                    ),
                                                    egui::vec2(18.0, 18.0),
                                                );
                                                let response = ui.interact(
                                                    button,
                                                    egui::Id::new((
                                                        "ups-power-button",
                                                        device_id.0,
                                                    )),
                                                    egui::Sense::click(),
                                                );
                                                if response.clicked() {
                                                    let enabled = match device.kind {
                                                        DeviceKind::Ups(ref ups) => {
                                                            ups.source.is_some_and(|s| {
                                                                sim.power.source_enabled(s)
                                                            })
                                                        }
                                                        _ => false,
                                                    };
                                                    actions.write(UiAction::TogglePower(
                                                        device_id, !enabled,
                                                    ));
                                                }
                                            }
                                        } else {
                                            ui.painter().rect_filled(
                                                panel_rect.shrink(3.0),
                                                1.0,
                                                egui::Color32::from_rgb(38, 44, 50),
                                            );
                                            ui.painter().rect_stroke(
                                                panel_rect.shrink(8.0),
                                                1.0,
                                                egui::Stroke::new(
                                                    1.0,
                                                    egui::Color32::from_gray(75),
                                                ),
                                                egui::StrokeKind::Inside,
                                            );
                                            ui.painter().circle_filled(
                                                egui::pos2(
                                                    panel_rect.left() + 14.0,
                                                    panel_rect.center().y,
                                                ),
                                                4.0,
                                                if device.powered {
                                                    egui::Color32::GREEN
                                                } else {
                                                    egui::Color32::from_gray(35)
                                                },
                                            );
                                            if matches!(
                                                device.kind,
                                                DeviceKind::Switch(_) | DeviceKind::Router(_)
                                            ) {
                                                for fan in 0..3 {
                                                    ui.painter().circle_stroke(
                                                        egui::pos2(
                                                            panel_rect.center().x
                                                                + (fan as f32 - 1.0) * 18.0,
                                                            panel_rect.center().y,
                                                        ),
                                                        7.0,
                                                        egui::Stroke::new(
                                                            1.0,
                                                            egui::Color32::from_gray(85),
                                                        ),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                                let inlet_meta = device.kind.power_inlet();
                                if inlet_meta.is_some_and(|inlet| inlet.side == state.rack_side)
                                    && device.rack.is_some_and(|p| p.unit == unit)
                                    && !matches!(
                                        device.kind,
                                        DeviceKind::PatchPanel(_) | DeviceKind::CableManager(_)
                                    )
                                {
                                    let inlet = device_power_inlet_rect(&device.kind, panel_rect)
                                        .expect("device has power inlet");
                                    let endpoint = device_power_endpoint(device);
                                    if let Some(endpoint) = endpoint {
                                        power_socket_rects
                                            .push((PowerSocket::Inlet(endpoint), inlet));
                                    }
                                    let fed = endpoint.and_then(|e| {
                                        sim.power
                                            .connections
                                            .iter()
                                            .find(|(_, x)| **x == e)
                                            .map(|(o, _)| *o)
                                    });
                                    let cable_end = inlet.center();
                                    if let Some(endpoint) = endpoint {
                                        power_endpoints.push((endpoint, cable_end));
                                    }
                                    let response = ui
                                        .interact(
                                            inlet,
                                            egui::Id::new(("power-inlet", device_id.0)),
                                            egui::Sense::click(),
                                        )
                                        .on_hover_text(tr(if fed.is_some() {
                                            "ui.occupied-right-click-to-unplug"
                                        } else {
                                            "ui.power-inlet-click-to-connect"
                                        }));
                                    if endpoint
                                        .is_some_and(|e| state.pending_power_inlet == Some(e))
                                        || state.pending_power_outlet.is_some()
                                        || fed.is_some_and(|outlet| {
                                            state.selected == Selection::PowerCable(outlet)
                                        })
                                        || response.hovered()
                                    {
                                        ui.painter().rect_stroke(
                                            inlet,
                                            2.0,
                                            egui::Stroke::new(
                                                2.0,
                                                if fed.is_some_and(|outlet| {
                                                    state.selected == Selection::PowerCable(outlet)
                                                }) {
                                                    egui::Color32::from_rgb(120, 205, 255)
                                                } else if response.hovered() {
                                                    egui::Color32::LIGHT_BLUE
                                                } else {
                                                    egui::Color32::from_rgb(255, 196, 64)
                                                },
                                            ),
                                            egui::StrokeKind::Inside,
                                        );
                                    }
                                    if response.clicked()
                                        && let Some(endpoint) = endpoint
                                    {
                                        actions.write(UiAction::PowerSocket(
                                            crate::app::PowerSocket::Inlet(endpoint),
                                        ));
                                    }
                                    response.context_menu(|menu| {
                                        if let Some(outlet) = fed
                                            && menu.button(tr("ui.unplug-cable")).clicked()
                                        {
                                            actions.write(UiAction::DisconnectPower(outlet));
                                            menu.close();
                                        }
                                    });
                                }
                                if state.rack_side == RackSide::Rear
                                    && device.rack.is_some_and(|p| p.unit == unit)
                                {
                                    let source = match device.kind {
                                        DeviceKind::Ups(ref x) => x.source,
                                        DeviceKind::Pdu(ref x) => x.source,
                                        _ => None,
                                    };
                                    if let Some(source) = source {
                                        let count = sim.power.outlets(source);
                                        for n in 0..count {
                                            let socket =
                                                source_outlet_rect(&device.kind, panel_rect, n);
                                            let occupied =
                                                sim.power.connections.contains_key(&OutletId {
                                                    source,
                                                    index: n as u8,
                                                });
                                            let outlet_id = OutletId {
                                                source,
                                                index: n as u8,
                                            };
                                            power_socket_rects
                                                .push((PowerSocket::Outlet(outlet_id), socket));
                                            let outlet_end = socket.center();
                                            let response = ui
                                                .interact(
                                                    socket,
                                                    egui::Id::new(("power-outlet", device_id.0, n)),
                                                    egui::Sense::click(),
                                                )
                                                .on_hover_text(tr(if occupied {
                                                    "ui.c13-occupied"
                                                } else {
                                                    "ui.c13-outlet-select-as-source"
                                                }));
                                            if state.pending_power_outlet == Some(outlet_id)
                                                || state.pending_power_inlet.is_some()
                                                || state.selected
                                                    == Selection::PowerCable(outlet_id)
                                                || response.hovered()
                                            {
                                                ui.painter().rect_stroke(
                                                    socket,
                                                    2.0,
                                                    egui::Stroke::new(
                                                        2.0,
                                                        if state.selected
                                                            == Selection::PowerCable(outlet_id)
                                                        {
                                                            egui::Color32::from_rgb(120, 205, 255)
                                                        } else if response.hovered() {
                                                            egui::Color32::LIGHT_BLUE
                                                        } else {
                                                            egui::Color32::from_rgb(255, 196, 64)
                                                        },
                                                    ),
                                                    egui::StrokeKind::Inside,
                                                );
                                            }
                                            power_outlets.insert(
                                                OutletId {
                                                    source,
                                                    index: n as u8,
                                                },
                                                outlet_end,
                                            );
                                            if response.clicked() {
                                                actions.write(if occupied {
                                                    UiAction::SelectPowerCable(outlet_id)
                                                } else {
                                                    UiAction::PowerSocket(
                                                        crate::app::PowerSocket::Outlet(outlet_id),
                                                    )
                                                });
                                            }
                                            response.context_menu(|menu| {
                                                if occupied
                                                    && menu.button(tr("ui.unplug-cable")).clicked()
                                                {
                                                    actions.write(UiAction::DisconnectPower(
                                                        OutletId {
                                                            source,
                                                            index: n as u8,
                                                        },
                                                    ));
                                                    menu.close();
                                                }
                                            });
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
                                if !matches!(
                                    device.kind,
                                    DeviceKind::PatchPanel(_) | DeviceKind::CableManager(_)
                                ) {
                                    ui.painter().circle_filled(
                                        egui::pos2(
                                            panel_rect.right() - 10.0,
                                            panel_rect.top() + 10.0,
                                        ),
                                        4.0,
                                        if device.powered {
                                            egui::Color32::GREEN
                                        } else {
                                            egui::Color32::from_gray(45)
                                        },
                                    );
                                }
                                if matches!(device.kind, DeviceKind::Server(_))
                                    && state.rack_side == RackSide::Front
                                {
                                    let power_rect = egui::Rect::from_center_size(
                                        egui::pos2(
                                            panel_rect.right() - panel_rect.width() * 0.04,
                                            panel_rect.top() + panel_rect.height() * 0.14,
                                        ),
                                        egui::vec2(18.0, 18.0),
                                    );
                                    if ui
                                        .interact(
                                            power_rect,
                                            egui::Id::new(("rack-power", device_id.0)),
                                            egui::Sense::click(),
                                        )
                                        .on_hover_text(tr("ui.power-on-off"))
                                        .clicked()
                                    {
                                        actions.write(UiAction::TogglePower(
                                            device_id,
                                            !sim.power
                                                .device_status(device_id)
                                                .is_some_and(|p| p.requested),
                                        ));
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
                                        && !matches!(
                                            device.kind,
                                            DeviceKind::PatchPanel(_) | DeviceKind::CableManager(_)
                                        )
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
                                        let device =
                                            sim.device(id).expect("inventory device exists");
                                        let height = device.template().rack_units();
                                        let valid = unit as u16 + height as u16 - 1
                                            <= rack.units as u16
                                            && (unit..unit.saturating_add(height))
                                                .all(|u| rack.occupies(u).is_none());
                                        let rect = egui::Rect::from_min_max(
                                            panel_rect.left_top()
                                                - egui::vec2(0.0, row_height * (height - 1) as f32),
                                            panel_rect.right_bottom(),
                                        );
                                        placement_preview = Some((id, rect, valid));
                                    }
                                    ui.painter().rect_filled(
                                        panel_rect,
                                        1.0,
                                        if hovered {
                                            egui::Color32::from_rgb(28, 52, 48)
                                        } else {
                                            egui::Color32::from_rgb(15, 25, 26)
                                        },
                                    );
                                    ui.painter().rect_stroke(
                                        panel_rect,
                                        1.0,
                                        egui::Stroke::new(
                                            1.0,
                                            egui::Color32::from_rgb(67, 115, 105),
                                        ),
                                        egui::StrokeKind::Inside,
                                    );
                                    ui.painter().text(
                                        panel_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        crate::localization::tr_args(
                                            "ui.install-at-u",
                                            &[format!("{:02}", unit)],
                                        ),
                                        egui::FontId::monospace(11.0),
                                        egui::Color32::from_rgb(146, 186, 175),
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
                        layout.paint_crossbar(ui, "ui.cable-service-space");
                    });

                if let Some((id, rect, valid)) = placement_preview {
                    let device = sim.device(id).expect("inventory device exists");
                    let color = if valid {
                        egui::Color32::LIGHT_GREEN
                    } else {
                        egui::Color32::LIGHT_RED
                    };
                    if let Some(texture) =
                        equipment_texture(textures, &device.kind, state.rack_side)
                    {
                        ui.painter().image(
                            texture,
                            rect,
                            equipment_uv(&device.kind, state.rack_side),
                            egui::Color32::WHITE.gamma_multiply(0.65),
                        );
                    } else {
                        ui.painter()
                            .rect_filled(rect, 1.0, egui::Color32::from_black_alpha(200));
                        if matches!(device.kind, DeviceKind::PatchPanel(_)) {
                            for (index, port_id) in device.ports().iter().enumerate() {
                                let port = sim.port(*port_id).expect("patch port exists");
                                if port.side == state.rack_side {
                                    let socket = rack_port_rect(
                                        &device.kind,
                                        rect,
                                        index,
                                        port.connector,
                                        state.rack_side,
                                    );
                                    ui.painter().rect_filled(
                                        socket,
                                        1.0,
                                        egui::Color32::from_gray(60),
                                    );
                                }
                            }
                        } else {
                            for slot in 1..5 {
                                let center = egui::pos2(
                                    rect.left() + rect.width() * (slot * 48 / 5) as f32 / 48.0,
                                    rect.center().y,
                                );
                                ui.painter().circle_stroke(
                                    center,
                                    5.0,
                                    egui::Stroke::new(2.0, egui::Color32::from_gray(90)),
                                );
                            }
                        }
                    }
                    ui.painter().rect_stroke(
                        rect,
                        1.0,
                        egui::Stroke::new(2.0, color),
                        egui::StrokeKind::Inside,
                    );
                }

                // Power leads follow the same physical rack view and rope solver as network cables.
                let mut power_rope_cables = Vec::new();
                let anchors: Vec<_> = route_anchors
                    .iter()
                    .map(|&(rack, unit, side, offset_cm, pos)| {
                        (
                            CableRoutePoint {
                                rack,
                                unit,
                                side,
                                offset_cm,
                            },
                            pos,
                        )
                    })
                    .collect();
                let mut power_paths = HashMap::new();
                let mut power_connectors = HashMap::new();
                for (outlet, endpoint) in &sim.power.connections {
                    let target_pos = power_endpoints
                        .iter()
                        .find(|(e, _)| e == endpoint)
                        .map(|(_, p)| *p);
                    let source_pos = power_outlets.get(outlet).copied();
                    let Some(source_location) =
                        power_socket_location(sim, PowerSocket::Outlet(*outlet), state.rack_side)
                    else {
                        continue;
                    };
                    let Some(target_location) =
                        power_socket_location(sim, PowerSocket::Inlet(*endpoint), state.rack_side)
                    else {
                        continue;
                    };
                    let route = sim
                        .power
                        .cord_routes
                        .get(outlet)
                        .map_or(&[][..], Vec::as_slice);
                    let spans = cables::visible_spans(
                        (source_location, source_pos),
                        (target_location, target_pos),
                        route,
                        &anchors,
                    );
                    let Some(first) = spans.first() else { continue };
                    let source = first[0];
                    let target = *spans.last().unwrap().last().unwrap();
                    let connectors = [PowerSocket::Outlet(*outlet), PowerSocket::Inlet(*endpoint)]
                        .iter()
                        .filter_map(|socket| {
                            power_socket_rects
                                .iter()
                                .find(|(candidate, _)| candidate == socket)
                                .map(|(_, rect)| cables::CableConnector {
                                    socket: *rect,
                                    kind: power_connector_kind(sim, *socket),
                                })
                        })
                        .collect();
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
                        routed: sim
                            .power
                            .cord_routes
                            .iter()
                            .filter(|(_, route)| !route.is_empty())
                            .map(|(outlet, _)| *outlet)
                            .collect(),
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
                            .chain(anchors.iter().map(|(_, pos)| {
                                egui::Rect::from_center_size(*pos, egui::vec2(14.0, 14.0))
                            }))
                            .collect(),
                        interaction_enabled: state.pending_power_outlet.is_none()
                            && state.pending_power_inlet.is_none()
                            && state.pending_cable.is_none(),
                    },
                ) {
                    actions.write(UiAction::SelectPowerCable(outlet));
                }

                let positions: HashMap<_, _> = port_visuals.iter().copied().collect();
                let anchors: Vec<_> = route_anchors
                    .iter()
                    .map(|&(rack, unit, side, offset_cm, pos)| {
                        (
                            CableRoutePoint {
                                rack,
                                unit,
                                side,
                                offset_cm,
                            },
                            pos,
                        )
                    })
                    .collect();
                let selected_route = match state.selected {
                    Selection::PowerCable(outlet) => {
                        sim.power.connections.get(&outlet).and_then(|endpoint| {
                            Some(RoutedCable {
                                id: RoutedCableId::Power(outlet),
                                route: sim
                                    .power
                                    .cord_routes
                                    .get(&outlet)
                                    .cloned()
                                    .unwrap_or_default(),
                                endpoints: (
                                    (
                                        power_socket_location(
                                            sim,
                                            PowerSocket::Outlet(outlet),
                                            state.rack_side,
                                        )?,
                                        power_outlets.get(&outlet).copied(),
                                    ),
                                    (
                                        power_socket_location(
                                            sim,
                                            PowerSocket::Inlet(*endpoint),
                                            state.rack_side,
                                        )?,
                                        power_endpoints
                                            .iter()
                                            .find(|(e, _)| e == endpoint)
                                            .map(|(_, p)| *p),
                                    ),
                                ),
                                color: egui::Color32::from_rgb(70, 110, 120),
                            })
                        })
                    }
                    _ => selected_link_id(state, sim).and_then(|link_id| {
                        let link = sim.link(link_id)?;
                        Some(RoutedCable {
                            id: RoutedCableId::Ethernet(link_id),
                            route: link.route.clone(),
                            endpoints: (
                                (
                                    cables::port_location(sim, link.a)?,
                                    positions.get(&link.a).map(egui::Rect::center),
                                ),
                                (
                                    cables::port_location(sim, link.b)?,
                                    positions.get(&link.b).map(egui::Rect::center),
                                ),
                            ),
                            color: cable_color_value(link.color),
                        })
                    }),
                };
                let pending = state
                    .pending_power_outlet
                    .map(|outlet| PendingCableId::Power(PowerSocket::Outlet(outlet)))
                    .or_else(|| {
                        state
                            .pending_power_inlet
                            .map(|endpoint| PendingCableId::Power(PowerSocket::Inlet(endpoint)))
                    })
                    .or_else(|| state.pending_cable.map(PendingCableId::Ethernet))
                    .and_then(|id| {
                        let (start, route, color) = match id {
                            PendingCableId::Ethernet(port) => (
                                (
                                    cables::port_location(sim, port)?,
                                    positions.get(&port).map(egui::Rect::center),
                                ),
                                state.pending_cable_route.clone(),
                                cable_color_value(state.cable_color),
                            ),
                            PendingCableId::Power(socket) => (
                                (
                                    power_socket_location(sim, socket, state.rack_side)?,
                                    power_socket_rects
                                        .iter()
                                        .find(|(candidate, _)| *candidate == socket)
                                        .map(|(_, rect)| rect.center()),
                                ),
                                state.pending_power_route.clone(),
                                egui::Color32::from_rgb(31, 35, 38),
                            ),
                        };
                        Some(PendingCable {
                            id,
                            start,
                            route,
                            color,
                        })
                    });
                let creation_targets: Vec<_> =
                    pending
                        .as_ref()
                        .map(|pending| match pending.id {
                            PendingCableId::Ethernet(first) => port_visuals
                                .iter()
                                .filter_map(|(port, rect)| {
                                    Some(cables::CreationTarget {
                                        location: cables::port_location(sim, *port)?,
                                        rect: *rect,
                                        same_socket: first == *port,
                                        valid: sim
                                            .quote_routed_colored_cable(
                                                first,
                                                *port,
                                                state.cable_length_cm,
                                                state.cable_color,
                                                &state.pending_cable_route,
                                            )
                                            .is_ok(),
                                    })
                                })
                                .collect(),
                            PendingCableId::Power(source) => power_socket_rects
                                .iter()
                                .filter_map(|(target, rect)| {
                                    let valid = match (source, *target) {
                                        (
                                            PowerSocket::Outlet(outlet),
                                            PowerSocket::Inlet(endpoint),
                                        )
                                        | (
                                            PowerSocket::Inlet(endpoint),
                                            PowerSocket::Outlet(outlet),
                                        ) => power_preview_is_valid(sim, outlet, endpoint),
                                        _ => false,
                                    };
                                    Some(cables::CreationTarget {
                                        location: power_socket_location(
                                            sim,
                                            *target,
                                            state.rack_side,
                                        )?,
                                        rect: *rect,
                                        same_socket: source == *target,
                                        valid,
                                    })
                                })
                                .collect(),
                        })
                        .unwrap_or_default();
                for (rack_id, unit, side, offset_cm, position) in route_anchors {
                    let point = CableRoutePoint {
                        rack: rack_id,
                        unit,
                        side,
                        offset_cm,
                    };
                    if let Some(pending) = pending.as_ref() {
                        if cables::creation_anchor(ui, point, position, &pending.route) {
                            actions.write(pending.id.anchor_action(point));
                        }
                        continue;
                    }
                    let Some(cable) = selected_route.as_ref() else {
                        let anchor_rect =
                            egui::Rect::from_center_size(position, egui::vec2(14.0, 14.0));
                        ui.interact(
                            anchor_rect,
                            egui::Id::new((
                                "route-anchor",
                                rack_id.0,
                                unit,
                                side == RackSide::Front,
                                offset_cm,
                            )),
                            egui::Sense::hover(),
                        )
                        .on_hover_text(tr("ui.cable-route-anchor-select-a-cable-to"));
                        ui.painter().circle_filled(
                            position,
                            5.0,
                            egui::Color32::from_rgb(76, 104, 115),
                        );
                        continue;
                    };
                    let used = cable.route.iter().position(|candidate| *candidate == point);
                    let anchor_rect =
                        egui::Rect::from_center_size(position, egui::vec2(24.0, 24.0));
                    let response = ui
                        .interact(
                            anchor_rect,
                            egui::Id::new((
                                "cable-route-anchor",
                                rack_id.0,
                                unit,
                                side == RackSide::Front,
                                offset_cm,
                            )),
                            egui::Sense::click(),
                        )
                        .on_hover_text(tr("ui.click-to-add-or-remove-this-anchor"));
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
                    if let Some(index) = used {
                        proposed.remove(index);
                    } else {
                        proposed.push(point);
                    }
                    if response.hovered() {
                        let preview_route = if used.is_some() {
                            &cable.route
                        } else {
                            &proposed
                        };
                        for path in cables::visible_spans(
                            cable.endpoints.0,
                            cable.endpoints.1,
                            preview_route,
                            &anchors,
                        ) {
                            cables::paint_preview(ui, path, cable.color);
                        }
                    }
                    if response.clicked() {
                        actions.write(cable.id.reroute(proposed));
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
                    cables::show_creation_preview(
                        ui,
                        pending.start,
                        &pending.route,
                        pending.color,
                        &creation_targets,
                        &anchors,
                    );
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
                                Ok(q) if q.reused => crate::localization::tr_args(
                                    "ui.reuse-m-finished-lead-no-materials-used",
                                    &[format!("{:.2}", q.length_cm as f32 / 100.0)],
                                ),
                                Ok(q) => crate::localization::tr_args(
                                    "ui.cut-m-cable-2-rj45-connectors",
                                    &[format!("{:.2}", q.length_cm as f32 / 100.0)],
                                ),
                                Err(error) => crate::localization::tr_args(
                                    "format.diagnostic-line",
                                    &[crate::localization::UiMessage::from(error).render()],
                                ),
                            }
                        })
                        .unwrap_or_default();
                    let response = response.on_hover_text(crate::localization::tr_args(
                        "port.tooltip",
                        &[
                            (sim.device(port.device)
                                .map(|d| d.name.as_str())
                                .unwrap_or("?"))
                            .to_string(),
                            (port.name).to_string(),
                            tr(connector_label(port.connector)),
                            if supported {
                                if link.is_some() {
                                    tr("port.connected-guide")
                                } else {
                                    tr("port.connect-guide")
                                }
                            } else {
                                tr("port.sfp-unavailable")
                            },
                            (quote).to_string(),
                        ],
                    ));
                    response.context_menu(|menu| {
                        menu.label(tr("ui.cable-actions"));
                        if let Some(link) = link {
                            if menu.button(tr("ui.unplug-cable")).clicked() {
                                actions.write(UiAction::Disconnect(link.id));
                                menu.close();
                            }
                        } else if supported && menu.button(tr("ui.connect-rj45-cable")).clicked() {
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
        if let ServerPartKind::PciCard {
            card: PciCard::Ethernet { .. },
        } = &part.kind
        {
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
        let label = model.map_or("ui.drive", |drive| {
            if drive.kind == cloud_provider_sim::DriveKind::Ssd {
                "ui.ssd"
            } else {
                "ui.hdd"
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
            tr(label),
            egui::FontId::monospace(8.0),
            egui::Color32::from_rgb(175, 214, 216),
        );
    }
}

fn connector_label(connector: PortConnector) -> &'static str {
    match connector {
        PortConnector::Rj45 => "ui.rj45",
        PortConnector::Sfp => "ui.sfp-not-implemented",
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
        ui.heading(tr("topology.title"));
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
                crate::localization::device_name(device),
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
    fn navigation_bar_does_not_cover_the_room() {
        let sim = NetworkSim::new();
        let ctx = egui::Context::default();
        let mut state = UiState::default();
        let mut height = 0.0;
        for _ in 0..2 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 800.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    height = top_bar(ui, &sim, &mut state).height();
                },
            );
            output.textures_delta.clear();
        }
        assert!(
            height > 0.0 && height <= 80.0,
            "navigation bar height: {height}"
        );
    }

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
                        output = TerminalRenderer::output_scroll(ui, 58.0)
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
