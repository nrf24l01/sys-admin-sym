use crate::app::{Selection, UiAction, UiState, Workspace};
use bevy::prelude::MessageWriter;
use bevy_egui::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use cloud_provider_sim::{CableRoutePoint, NetworkSim, RackId, RoomPosition};

const ROOM_CANVAS_WIDTH: f32 = 1040.0;
const ROOM_CANVAS_HEIGHT: f32 = 1820.0;

fn paint_floor(ui: &egui::Ui, rect: Rect, room: &cloud_provider_sim::DataCenterRoom) {
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
        painter.text(
            egui::pos2(aisle.left() + 24.0, aisle.center().y),
            egui::Align2::LEFT_CENTER,
            if row % 2 == 0 {
                "COLD AISLE"
            } else {
                "SERVICE AISLE"
            },
            egui::FontId::monospace(10.0),
            Color32::from_gray(155),
        );
    }
    let tray_x = screen(RoomPosition { x_cm: 630, y_cm: 0 }, rect, room).x;
    painter.line_segment(
        [
            egui::pos2(tray_x, rect.top() + 25.0),
            egui::pos2(tray_x, rect.bottom() - 25.0),
        ],
        Stroke::new(14.0, Color32::from_rgb(35, 57, 62)),
    );
    painter.line_segment(
        [
            egui::pos2(tray_x, rect.top() + 25.0),
            egui::pos2(tray_x, rect.bottom() - 25.0),
        ],
        Stroke::new(2.0, Color32::from_rgb(95, 142, 150)),
    );
    for row in 0..10 {
        let y = screen(
            RoomPosition {
                x_cm: 0,
                y_cm: 180 + row * 270,
            },
            rect,
            room,
        )
        .y;
        painter.text(
            egui::pos2(rect.left() + 18.0, y),
            egui::Align2::LEFT_CENTER,
            format!("ROW {:02}", row + 1),
            egui::FontId::monospace(12.0),
            Color32::from_gray(185),
        );
    }
    for column in 0..5 {
        let x = screen(
            RoomPosition {
                x_cm: 180 + column * 300,
                y_cm: 0,
            },
            rect,
            room,
        )
        .x;
        painter.text(
            egui::pos2(x, rect.top() + 18.0),
            egui::Align2::CENTER_CENTER,
            format!("BAY {}", (b'A' + column as u8) as char),
            egui::FontId::monospace(12.0),
            Color32::from_gray(185),
        );
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
        "ENTRANCE",
        egui::FontId::monospace(11.0),
        Color32::from_gray(170),
    );
}

fn screen(position: RoomPosition, rect: Rect, room: &cloud_provider_sim::DataCenterRoom) -> Pos2 {
    egui::pos2(
        rect.left() + rect.width() * f32::from(position.x_cm) / f32::from(room.width_cm),
        rect.top() + rect.height() * f32::from(position.y_cm) / f32::from(room.depth_cm),
    )
}

fn room_position(
    point: Pos2,
    rect: Rect,
    room: &cloud_provider_sim::DataCenterRoom,
) -> RoomPosition {
    RoomPosition {
        x_cm: ((point.x - rect.left()) / rect.width() * f32::from(room.width_cm))
            .clamp(0.0, f32::from(room.width_cm)) as u16,
        y_cm: ((point.y - rect.top()) / rect.height() * f32::from(room.depth_cm))
            .clamp(0.0, f32::from(room.depth_cm)) as u16,
    }
}

pub(super) fn show(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    egui::CentralPanel::default().show(viewport, |ui| {
        ui.horizontal(|ui| {
            ui.heading(format!("DATACENTER / {}", sim.room.name));
            ui.separator();
            ui.label("50 × 42U racks · 10 rows × 5 bays");
            ui.separator();
            if ui.selectable_label(state.placing_room_anchor, "Place ceiling cable manager").clicked() {
                state.placing_room_anchor = !state.placing_room_anchor;
            }
        });
        ui.label("Click a rack to open it. Route between racks through the cable tray managers. Drag managers to adjust routes; scroll to move around the room.");
        egui::ScrollArea::both().id_salt("datacenter-floor").show(ui, |ui| {
        let size = Vec2::new(ROOM_CANVAS_WIDTH, ROOM_CANVAS_HEIGHT);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let painter = ui.painter_at(rect);
        paint_floor(ui, rect, &sim.room);

        let rack_center = |rack: RackId| screen(sim.rack_room_position(rack), rect, &sim.room);
        let route_position = |point: &CableRoutePoint| -> Option<Pos2> {
            if let Some(id) = point.room_anchor_id() {
                sim.room.cable_anchors.iter().find(|anchor| anchor.id == id)
                    .map(|anchor| screen(anchor.position, rect, &sim.room))
            } else {
                sim.rack(point.rack).map(|_| rack_center(point.rack))
            }
        };
        let source_rack_for = |source: cloud_provider_sim::SourceId| -> Option<RackId> {
            match source {
                cloud_provider_sim::SourceId::Rack(id) => Some(id),
                _ => sim.devices().find(|device| match &device.kind {
                    cloud_provider_sim::DeviceKind::Ups(ups) => ups.source == Some(source),
                    cloud_provider_sim::DeviceKind::Pdu(pdu) => pdu.source == Some(source),
                    _ => false,
                }).and_then(|device| device.rack).map(|placement| placement.rack),
            }
        };
        for link in sim.links() {
            let Some(a) = sim.port(link.a).and_then(|p| sim.device(p.device)).and_then(|d| d.rack) else { continue };
            let Some(b) = sim.port(link.b).and_then(|p| sim.device(p.device)).and_then(|d| d.rack) else { continue };
            if a.rack == b.rack { continue; }
            let mut points = vec![rack_center(a.rack)];
            points.extend(link.route.iter().filter_map(&route_position));
            points.push(rack_center(b.rack));
            let selected = state.selected == Selection::Link(link.id);
            let stroke = Stroke::new(if selected { 4.0 } else { 2.0 }, if selected { Color32::LIGHT_BLUE } else { Color32::from_rgb(75, 170, 150) });
            for (index, pair) in points.windows(2).enumerate() {
                painter.line_segment([pair[0], pair[1]], stroke);
                let hit = Rect::from_two_pos(pair[0], pair[1]).expand(5.0);
                if ui.interact(hit, egui::Id::new(("room-link", link.id.0, index)), Sense::click()).clicked() {
                    actions.write(UiAction::SelectLink(link.id));
                }
            }
        }
        for (outlet, endpoint) in &sim.power.connections {
            let source_rack = source_rack_for(outlet.source);
            let target_rack = match endpoint {
                cloud_provider_sim::PowerEndpoint::Device(id) => sim.device(*id).and_then(|d| d.rack).map(|p| p.rack),
                cloud_provider_sim::PowerEndpoint::Source(source) => source_rack_for(*source),
            };
            if let (Some(a), Some(b)) = (source_rack, target_rack) && a != b {
                let mut points = vec![rack_center(a)];
                if let Some(route) = sim.power.cord_routes.get(outlet) { points.extend(route.iter().filter_map(&route_position)); }
                points.push(rack_center(b));
                for pair in points.windows(2) {
                    painter.line_segment([pair[0], pair[1]], Stroke::new(2.0, Color32::from_rgb(210, 170, 80)));
                }
            }
        }
        let pending_start = state.pending_cable
            .and_then(|port| sim.port(port))
            .and_then(|port| sim.device(port.device))
            .and_then(|device| device.rack)
            .map(|placement| placement.rack)
            .or_else(|| state.pending_power_outlet.and_then(|outlet| source_rack_for(outlet.source)));
        if let Some(start) = pending_start {
            let route = if state.pending_cable.is_some() { &state.pending_cable_route } else { &state.pending_power_route };
            let mut points = vec![rack_center(start)];
            points.extend(route.iter().filter_map(&route_position));
            for pair in points.windows(2) {
                painter.line_segment([pair[0], pair[1]], Stroke::new(3.0, Color32::from_rgb(245, 190, 75)));
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
            let hit = ui.interact(body, egui::Id::new(("room-rack", rack.id.0)), Sense::click());
            painter.rect_filled(body, 3.0, Color32::from_rgb(29, 36, 42));
            painter.rect_stroke(body, 3.0, Stroke::new(3.0, if state.active_rack == Some(rack.id) { Color32::LIGHT_BLUE } else { Color32::from_gray(140) }), egui::StrokeKind::Inside);
            let lid = body.shrink2(Vec2::new(8.0, 9.0));
            painter.rect_filled(lid, 2.0, Color32::from_rgb(75, 84, 91));
            for stripe in 0..4 {
                let y = lid.top() + 10.0 + stripe as f32 * 10.0;
                painter.line_segment([egui::pos2(lid.left() + 8.0, y), egui::pos2(lid.right() - 8.0, y)], Stroke::new(2.0, Color32::from_gray(115)));
            }
            painter.text(body.center_top() + Vec2::new(0.0, 17.0), egui::Align2::CENTER_CENTER, format!("R{:02}", rack.id.0), egui::FontId::monospace(12.0), Color32::WHITE);
            painter.text(body.center_bottom() - Vec2::new(0.0, 12.0), egui::Align2::CENTER_CENTER, format!("{} / {}U", rack.placements.len(), rack.units), egui::FontId::monospace(10.0), Color32::WHITE);
            if hit.clicked() { state.active_rack = Some(rack.id); state.workspace = Workspace::Rack; }
        }
        for anchor in &sim.room.cable_anchors {
            let center = screen(anchor.position, rect, &sim.room);
            let hit = ui.interact(Rect::from_center_size(center, Vec2::splat(28.0)), egui::Id::new(("room-anchor", anchor.id)), Sense::click_and_drag());
            painter.circle_filled(center, 10.0, Color32::from_rgb(120, 170, 190));
            painter.text(center + Vec2::new(0.0, 17.0), egui::Align2::CENTER_TOP, format!("CM {}", anchor.id), egui::FontId::proportional(11.0), Color32::LIGHT_GRAY);
            if hit.clicked() {
                let point = CableRoutePoint::room_anchor(anchor.id);
                if state.pending_cable.is_some() { actions.write(UiAction::AddPendingCableRoutePoint(point)); }
                else if state.pending_power_outlet.is_some() || state.pending_power_inlet.is_some() { actions.write(UiAction::AddPendingPowerRoutePoint(point)); }
                else if let Selection::Link(id) = state.selected {
                    if let Some(link) = sim.link(id) {
                        let mut route = link.route.clone();
                        if let Some(index) = route.iter().position(|candidate| *candidate == point) { route.remove(index); }
                        else { route.push(point); }
                        actions.write(UiAction::RerouteCable { link: id, route });
                    }
                } else if let Selection::PowerCable(outlet) = state.selected {
                    let mut route = sim.power.cord_routes.get(&outlet).cloned().unwrap_or_default();
                    if let Some(index) = route.iter().position(|candidate| *candidate == point) { route.remove(index); }
                    else { route.push(point); }
                    actions.write(UiAction::ReroutePowerCable { outlet, route });
                }
            }
            if hit.drag_stopped() && let Some(pointer) = hit.interact_pointer_pos() {
                actions.write(UiAction::MoveRoomCableAnchor { id: anchor.id, position: room_position(pointer, rect, &sim.room) });
            }
        }
        if state.placing_room_anchor && response.clicked() && let Some(pointer) = response.interact_pointer_pos() {
            actions.write(UiAction::AddRoomCableAnchor(room_position(pointer, rect, &sim.room)));
            state.placing_room_anchor = false;
        }
        });
    });
}
