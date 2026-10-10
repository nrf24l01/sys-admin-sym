use crate::app::{Selection, UiAction, UiState, Workspace};
use crate::localization::tr;
use bevy::prelude::MessageWriter;
use bevy_egui::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use cloud_provider_sim::{
    CableRoutePoint, DATACENTER_CABLE_COLUMNS_CM, NetworkOutletKind, NetworkSim, RackId,
    RoomCableLayout, RoomPosition,
};

const ROOM_CANVAS_WIDTH: f32 = 1040.0;
const ROOM_CANVAS_HEIGHT: f32 = 1820.0;

pub(super) struct NetworkSocketRenderer;

impl NetworkSocketRenderer {
    pub(super) fn paint(painter: &egui::Painter, rect: Rect, connected: bool, selected: bool) {
        let border = if selected {
            Color32::LIGHT_BLUE
        } else if connected {
            Color32::from_rgb(70, 185, 130)
        } else {
            Color32::from_gray(145)
        };
        painter.rect_filled(rect, 2.0, border);
        let opening = rect.shrink2(Vec2::new(2.0, 2.0));
        painter.rect_filled(opening, 1.0, Color32::from_rgb(14, 22, 27));
        let pitch = opening.width() / 9.0;
        for contact in 0..8 {
            let x = opening.left() + pitch * (contact as f32 + 1.0);
            painter.line_segment(
                [
                    Pos2::new(x, opening.top() + 2.0),
                    Pos2::new(x, opening.top() + 6.0),
                ],
                Stroke::new(1.3, Color32::from_rgb(220, 175, 80)),
            );
        }
        painter.rect_filled(
            Rect::from_center_size(
                opening.center_bottom() - Vec2::new(0.0, 2.0),
                Vec2::new(opening.width() * 0.34, 3.0),
            ),
            0.0,
            Color32::from_gray(72),
        );
    }
}

struct RoomFloorRenderer;

impl RoomFloorRenderer {
    fn paint(ui: &egui::Ui, rect: Rect, room: &cloud_provider_sim::DataCenterRoom) {
        let painter = ui.painter_at(rect);
        let tile = 65.0;
        painter.rect_filled(rect, 0.0, Color32::from_rgb(50, 56, 60));
        for column in 0..16 {
            for row in 0..28 {
                let square = Rect::from_min_size(
                    rect.min + Vec2::new(column as f32 * tile, row as f32 * tile),
                    Vec2::splat(tile - 1.0),
                );
                let shade = if (column + row) % 2 == 0 { 58 } else { 63 };
                painter.rect_filled(square, 0.0, Color32::from_rgb(shade, shade + 5, shade + 7));
                painter.circle_filled(
                    square.left_top() + Vec2::splat(5.0),
                    1.0,
                    Color32::from_gray(91),
                );
                painter.circle_filled(
                    square.right_bottom() - Vec2::splat(5.0),
                    1.0,
                    Color32::from_gray(91),
                );
            }
        }
        for row in 0..9 {
            let y = screen(
                RoomPosition {
                    x_cm: 0,
                    y_cm: 315 + row * 270,
                },
                rect,
                room,
            )
            .y;
            let aisle = Rect::from_min_max(
                egui::pos2(rect.left() + 16.0, y - 29.0),
                egui::pos2(rect.right() - 16.0, y + 29.0),
            );
            painter.rect_filled(aisle, 0.0, Color32::from_rgb(68, 74, 76));
            painter.line_segment(
                [aisle.left_top(), aisle.right_top()],
                Stroke::new(1.0, Color32::from_rgb(130, 133, 112)),
            );
            painter.line_segment(
                [aisle.left_bottom(), aisle.right_bottom()],
                Stroke::new(1.0, Color32::from_rgb(130, 133, 112)),
            );
        }
        let mut columns = DATACENTER_CABLE_COLUMNS_CM;
        columns.sort();
        for x_cm in columns {
            let tray_x = screen(RoomPosition { x_cm, y_cm: 0 }, rect, room).x;
            let path = [
                egui::pos2(tray_x, rect.top() + 38.0),
                egui::pos2(tray_x, rect.bottom() - 25.0),
            ];
            painter.line_segment(path, Stroke::new(14.0, Color32::from_rgb(35, 57, 62)));
            painter.line_segment(path, Stroke::new(2.0, Color32::from_rgb(95, 142, 150)));
        }
        for y_cm in RoomCableLayout::horizontal_rows_cm() {
            let y = screen(RoomPosition { x_cm: 0, y_cm }, rect, room).y;
            let path = [
                egui::pos2(rect.left() + 16.0, y),
                egui::pos2(rect.right() - 16.0, y),
            ];
            painter.line_segment(path, Stroke::new(14.0, Color32::from_rgb(35, 57, 62)));
            painter.line_segment(path, Stroke::new(2.0, Color32::from_rgb(95, 142, 150)));
        }
        painter.rect_stroke(
            rect.shrink(4.0),
            2.0,
            Stroke::new(8.0, Color32::from_rgb(108, 113, 116)),
            egui::StrokeKind::Inside,
        );
        let door = Rect::from_center_size(
            egui::pos2(rect.center().x, rect.bottom() - 5.0),
            Vec2::new(110.0, 12.0),
        );
        painter.rect_filled(door, 0.0, Color32::from_rgb(56, 62, 66));
        painter.text(
            door.center() - Vec2::new(0.0, 24.0),
            egui::Align2::CENTER_CENTER,
            tr("ui.entrance"),
            egui::FontId::monospace(11.0),
            Color32::from_gray(170),
        );
    }
}

fn screen(position: RoomPosition, rect: Rect, room: &cloud_provider_sim::DataCenterRoom) -> Pos2 {
    egui::pos2(
        rect.left() + rect.width() * f32::from(position.x_cm) / f32::from(room.width_cm),
        rect.top() + rect.height() * f32::from(position.y_cm) / f32::from(room.depth_cm),
    )
}

pub(super) fn show(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    egui::CentralPanel::default().show(viewport, |ui| {
        ui.horizontal(|ui| {
            ui.heading(crate::localization::tr_args(
                "ui.datacenter",
                &[crate::localization::room_name(&sim.room)],
            ));
            ui.separator();
            ui.label(tr("ui.50-42u-racks-10-rows-5-bays"));
        });
        ui.label(tr("ui.click-a-rack-to-open-it-fixed"));
        egui::ScrollArea::both()
            .id_salt("datacenter-floor")
            .show(ui, |ui| {
                let size = Vec2::new(ROOM_CANVAS_WIDTH, ROOM_CANVAS_HEIGHT);
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                let painter = ui.painter_at(rect);
                RoomFloorRenderer::paint(ui, rect, &sim.room);

                let rack_center =
                    |rack: RackId| screen(sim.rack_room_position(rack), rect, &sim.room);
                let rack_lan_center = |rack: RackId| rack_center(rack) + Vec2::new(53.0, -31.0);
                let route_position = |point: &CableRoutePoint| -> Option<Pos2> {
                    if let Some(id) = point.room_anchor_id() {
                        sim.room
                            .cable_anchors
                            .iter()
                            .find(|anchor| anchor.id == id)
                            .map(|anchor| screen(anchor.position, rect, &sim.room))
                    } else {
                        sim.rack(point.rack).map(|_| rack_center(point.rack))
                    }
                };
                let source_rack_for = |source: cloud_provider_sim::SourceId| -> Option<RackId> {
                    match source {
                        cloud_provider_sim::SourceId::Rack(id) => Some(id),
                        _ => sim
                            .devices()
                            .find(|device| match &device.kind {
                                cloud_provider_sim::DeviceKind::Ups(ups) => {
                                    ups.source == Some(source)
                                }
                                cloud_provider_sim::DeviceKind::Pdu(pdu) => {
                                    pdu.source == Some(source)
                                }
                                _ => false,
                            })
                            .and_then(|device| device.rack)
                            .map(|placement| placement.rack),
                    }
                };
                for link in sim.links() {
                    let endpoint = |port| {
                        sim.network_outlet(port)
                            .map(|outlet| match outlet.kind {
                                NetworkOutletKind::Uplink { position } => {
                                    screen(position, rect, &sim.room)
                                }
                                NetworkOutletKind::Lan { rack } => rack_lan_center(rack),
                            })
                            .or_else(|| {
                                sim.port(port)
                                    .and_then(|p| sim.device(p.device))
                                    .and_then(|d| d.rack)
                                    .map(|p| rack_center(p.rack))
                            })
                    };
                    let (Some(a), Some(b)) = (endpoint(link.a), endpoint(link.b)) else {
                        continue;
                    };
                    if a == b {
                        continue;
                    }
                    let mut points = vec![a];
                    points.extend(link.route.iter().filter_map(&route_position));
                    points.push(b);
                    let selected = state.selected == Selection::Link(link.id);
                    let stroke = Stroke::new(
                        if selected { 4.0 } else { 2.0 },
                        if selected {
                            Color32::LIGHT_BLUE
                        } else {
                            Color32::from_rgb(75, 170, 150)
                        },
                    );
                    for (index, pair) in points.windows(2).enumerate() {
                        painter.line_segment([pair[0], pair[1]], stroke);
                        let hit = Rect::from_two_pos(pair[0], pair[1]).expand(5.0);
                        if ui
                            .interact(
                                hit,
                                egui::Id::new(("room-link", link.id.0, index)),
                                Sense::click(),
                            )
                            .clicked()
                        {
                            actions.write(UiAction::SelectLink(link.id));
                        }
                    }
                }
                for (outlet, endpoint) in &sim.power.connections {
                    let source_rack = source_rack_for(outlet.source);
                    let target_rack = match endpoint {
                        cloud_provider_sim::PowerEndpoint::Device(id)
                        | cloud_provider_sim::PowerEndpoint::DevicePsu { device: id, .. } => {
                            sim.device(*id).and_then(|d| d.rack).map(|p| p.rack)
                        }
                        cloud_provider_sim::PowerEndpoint::Source(source) => {
                            source_rack_for(*source)
                        }
                    };
                    if let (Some(a), Some(b)) = (source_rack, target_rack)
                        && a != b
                    {
                        let mut points = vec![rack_center(a)];
                        if let Some(route) = sim.power.cord_routes.get(outlet) {
                            points.extend(route.iter().filter_map(&route_position));
                        }
                        points.push(rack_center(b));
                        for pair in points.windows(2) {
                            painter.line_segment(
                                [pair[0], pair[1]],
                                Stroke::new(2.0, Color32::from_rgb(210, 170, 80)),
                            );
                        }
                    }
                }
                let pending_start = state
                    .pending_cable
                    .and_then(|port| {
                        sim.network_outlet(port).map(|outlet| match outlet.kind {
                            NetworkOutletKind::Lan { rack } => rack_lan_center(rack),
                            NetworkOutletKind::Uplink { position } => {
                                screen(position, rect, &sim.room)
                            }
                        })
                    })
                    .or_else(|| {
                        state
                            .pending_cable
                            .and_then(|port| sim.port(port))
                            .and_then(|port| sim.device(port.device))
                            .and_then(|device| device.rack)
                            .map(|placement| rack_center(placement.rack))
                    })
                    .or_else(|| {
                        state
                            .pending_power_outlet
                            .and_then(|outlet| source_rack_for(outlet.source).map(rack_center))
                    });
                if let Some(start) = pending_start {
                    let route = if state.pending_cable.is_some() {
                        &state.pending_cable_route
                    } else {
                        &state.pending_power_route
                    };
                    let mut points = vec![start];
                    points.extend(route.iter().filter_map(&route_position));
                    for pair in points.windows(2) {
                        painter.line_segment(
                            [pair[0], pair[1]],
                            Stroke::new(3.0, Color32::from_rgb(245, 190, 75)),
                        );
                    }
                    if let Some(last) = points.last() {
                        painter.circle_filled(*last, 5.0, Color32::from_rgb(245, 190, 75));
                    }
                }
                let mut racks: Vec<_> = sim.racks().collect();
                racks.sort_by_key(|rack| rack.id);
                for rack in racks {
                    let center = rack_center(rack.id);
                    let body = Rect::from_center_size(center, Vec2::new(82.0, 84.0));
                    let hit = ui.interact(
                        body,
                        egui::Id::new(("room-rack", rack.id.0)),
                        Sense::click(),
                    );
                    painter.rect_filled(body, 3.0, Color32::from_rgb(29, 36, 42));
                    painter.rect_stroke(
                        body,
                        3.0,
                        Stroke::new(
                            3.0,
                            if state.active_rack == Some(rack.id) {
                                Color32::LIGHT_BLUE
                            } else {
                                Color32::from_gray(140)
                            },
                        ),
                        egui::StrokeKind::Inside,
                    );
                    let lid = body.shrink2(Vec2::new(8.0, 9.0));
                    painter.rect_filled(lid, 2.0, Color32::from_rgb(75, 84, 91));
                    for stripe in 0..4 {
                        let y = lid.top() + 10.0 + stripe as f32 * 10.0;
                        painter.line_segment(
                            [
                                egui::pos2(lid.left() + 8.0, y),
                                egui::pos2(lid.right() - 8.0, y),
                            ],
                            Stroke::new(2.0, Color32::from_gray(115)),
                        );
                    }
                    painter.text(
                        body.center_top() + Vec2::new(0.0, 17.0),
                        egui::Align2::CENTER_CENTER,
                        crate::localization::tr_args("ui.r", &[format!("{:02}", rack.id.0)]),
                        egui::FontId::monospace(12.0),
                        Color32::WHITE,
                    );
                    painter.text(
                        body.center_bottom() - Vec2::new(0.0, 12.0),
                        egui::Align2::CENTER_CENTER,
                        crate::localization::tr_args(
                            "rack.occupancy",
                            &[
                                (rack.placements.len()).to_string(),
                                (rack.units).to_string(),
                            ],
                        ),
                        egui::FontId::monospace(10.0),
                        Color32::WHITE,
                    );
                    if hit.clicked() {
                        state.active_rack = Some(rack.id);
                        state.workspace = Workspace::Rack;
                    }
                    if let Some(outlet) = sim
                        .network_outlets()
                        .find(|outlet| outlet.kind == NetworkOutletKind::Lan { rack: rack.id })
                    {
                        let socket =
                            Rect::from_center_size(rack_lan_center(rack.id), Vec2::new(23.0, 17.0));
                        let response = ui
                            .interact(
                                socket,
                                egui::Id::new(("room-lan", outlet.port.0)),
                                Sense::click(),
                            )
                            .on_hover_text(crate::localization::tr_args(
                                "ui.rack-room-lan-port",
                                &[(rack.id.0).to_string(), (outlet.port.0).to_string()],
                            ));
                        NetworkSocketRenderer::paint(
                            &painter,
                            socket,
                            sim.link_for_port(outlet.port).is_some(),
                            state.selected == Selection::Port(outlet.port),
                        );
                        if response.clicked() {
                            actions.write(UiAction::SelectPort(outlet.port));
                            if sim.link_for_port(outlet.port).is_none() {
                                actions.write(UiAction::CablePort(outlet.port));
                            }
                        }
                    }
                }
                for outlet in sim.network_outlets() {
                    let NetworkOutletKind::Uplink { position } = outlet.kind else {
                        continue;
                    };
                    let center = screen(position, rect, &sim.room);
                    let plate = Rect::from_center_size(center, Vec2::new(36.0, 29.0));
                    let hit = ui
                        .interact(
                            plate,
                            egui::Id::new(("room-uplink", outlet.port.0)),
                            Sense::click(),
                        )
                        .on_hover_text(crate::localization::tr_args(
                            "ui.port-click-to-select-or-connect",
                            &[
                                (sim.port(outlet.port)
                                    .map_or("Global uplink", |port| port.name.as_str()))
                                .to_string(),
                                (outlet.port.0).to_string(),
                            ],
                        ));
                    painter.rect_filled(plate, 3.0, Color32::from_rgb(37, 49, 55));
                    painter.rect_stroke(
                        plate,
                        3.0,
                        Stroke::new(2.0, Color32::from_rgb(70, 185, 130)),
                        egui::StrokeKind::Inside,
                    );
                    let socket = Rect::from_center_size(center, Vec2::new(26.0, 19.0));
                    NetworkSocketRenderer::paint(
                        &painter,
                        socket,
                        sim.link_for_port(outlet.port).is_some(),
                        state.selected == Selection::Port(outlet.port),
                    );
                    if hit.clicked() {
                        actions.write(UiAction::SelectPort(outlet.port));
                        if sim.link_for_port(outlet.port).is_none() {
                            actions.write(UiAction::CablePort(outlet.port));
                        }
                    }
                }
                for anchor in &sim.room.cable_anchors {
                    let center = screen(anchor.position, rect, &sim.room);
                    let hit = ui.interact(
                        Rect::from_center_size(center, Vec2::splat(28.0)),
                        egui::Id::new(("room-anchor", anchor.id)),
                        Sense::click(),
                    );
                    painter.circle_filled(center, 10.0, Color32::from_rgb(120, 170, 190));
                    if hit.clicked() {
                        let point = CableRoutePoint::room_anchor(anchor.id);
                        if state.pending_cable.is_some() {
                            actions.write(UiAction::AddPendingCableRoutePoint(point));
                        } else if state.pending_power_outlet.is_some()
                            || state.pending_power_inlet.is_some()
                        {
                            actions.write(UiAction::AddPendingPowerRoutePoint(point));
                        } else if let Selection::Link(id) = state.selected {
                            if let Some(link) = sim.link(id) {
                                let mut route = link.route.clone();
                                if let Some(index) =
                                    route.iter().position(|candidate| *candidate == point)
                                {
                                    route.remove(index);
                                } else {
                                    route.push(point);
                                }
                                actions.write(UiAction::RerouteCable { link: id, route });
                            }
                        } else if let Selection::PowerCable(outlet) = state.selected {
                            let mut route = sim
                                .power
                                .cord_routes
                                .get(&outlet)
                                .cloned()
                                .unwrap_or_default();
                            if let Some(index) =
                                route.iter().position(|candidate| *candidate == point)
                            {
                                route.remove(index);
                            } else {
                                route.push(point);
                            }
                            actions.write(UiAction::ReroutePowerCable { outlet, route });
                        }
                    }
                }
            });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        ecs::system::SystemState,
        prelude::{Messages, World},
    };

    #[test]
    fn room_uses_unlabeled_managers_and_green_uplink_borders() {
        let ctx = egui::Context::default();
        let sim = NetworkSim::new();
        let mut state = UiState::default();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut system = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 2100.0))),
                ..Default::default()
            },
            |ui| {
                show(
                    ui,
                    &sim,
                    &mut state,
                    &mut system.get_mut(&mut world).unwrap(),
                )
            },
        );
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape {
                let label = text.galley.text();
                assert!(
                    !label.starts_with("ROW ")
                        && !label.starts_with("CM ")
                        && !label.starts_with("UPLINK ")
                        && !label.starts_with("BAY ")
                        && !label.contains("AISLE")
                );
            }
        }
        let green_borders = output.shapes.iter().filter(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.stroke.color == Color32::from_rgb(70, 185, 130) && rect.stroke.width == 2.0)).count();
        assert_eq!(green_borders, 2);
        output.textures_delta.clear();
    }
}
