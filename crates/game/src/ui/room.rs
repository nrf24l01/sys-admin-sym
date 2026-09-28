use crate::app::{Selection, UiAction, UiState, Workspace};
use bevy::prelude::MessageWriter;
use bevy_egui::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use cloud_provider_sim::{CableRoutePoint, NetworkSim, RackId, RoomPosition};

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
            if ui.button("Buy 12U rack · $500").clicked() { actions.write(UiAction::AddRack); }
            if ui.selectable_label(state.placing_room_anchor, "Place ceiling cable manager").clicked() {
                state.placing_room_anchor = !state.placing_room_anchor;
            }
        });
        ui.label("Click a rack to open it. Drag racks and cable managers to arrange the room. To connect racks, select a port, choose ceiling managers here, then select the destination port in its rack.");
        let available = ui.available_size();
        let size = Vec2::new(available.x.max(400.0), available.y.max(300.0));
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, Color32::from_rgb(18, 25, 31));
        painter.rect_stroke(rect, 4.0, Stroke::new(2.0, Color32::from_gray(85)), egui::StrokeKind::Inside);

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
            let body = Rect::from_center_size(center, Vec2::new(86.0, 60.0));
            let hit = ui.interact(body, egui::Id::new(("room-rack", rack.id.0)), Sense::click_and_drag());
            painter.rect_filled(body, 4.0, Color32::from_rgb(44, 56, 68));
            painter.rect_stroke(body, 4.0, Stroke::new(2.0, if state.active_rack == Some(rack.id) { Color32::LIGHT_BLUE } else { Color32::from_gray(125) }), egui::StrokeKind::Inside);
            painter.text(body.center_top() + Vec2::new(0.0, 17.0), egui::Align2::CENTER_CENTER, &rack.name, egui::FontId::proportional(13.0), Color32::WHITE);
            painter.text(body.center_bottom() - Vec2::new(0.0, 15.0), egui::Align2::CENTER_CENTER, format!("{}U · {} devices", rack.units, rack.placements.len()), egui::FontId::proportional(11.0), Color32::LIGHT_GRAY);
            if hit.clicked() { state.active_rack = Some(rack.id); state.workspace = Workspace::Rack; }
            if hit.drag_stopped() && let Some(pointer) = hit.interact_pointer_pos() {
                actions.write(UiAction::MoveRackInRoom { rack: rack.id, position: room_position(pointer, rect, &sim.room) });
            }
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
}
