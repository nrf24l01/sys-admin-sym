use crate::app::CableVisibility;
use bevy_egui::egui::{self, Color32, Pos2, Rect, Vec2};
use cloud_provider_sim::{CableRoutePoint, LinkId, NetworkSim, OutletId, PortId};
use std::collections::HashMap;

const SEGMENTS: usize = 64;
// More segments need more constraint passes to retain the same stretch resistance.
const CONSTRAINT_PASSES: usize = 96;
const STEP: f32 = 1.0 / 120.0;

/// Presentation-only Verlet rope, measured in centimeters so scrolling and
/// resizing never impart an impulse. Endpoints are constrained to the plugs.
struct Rope {
    points: Vec<Pos2>,
    previous: Vec<Pos2>,
    length: f32,
    endpoints: (Pos2, Pos2),
}
impl Rope {
    fn new(a: Pos2, b: Pos2, length: f32) -> Self {
        let slack = (length * length - a.distance(b).powi(2)).max(0.0).sqrt() * 0.35;
        let points: Vec<_> = (0..=SEGMENTS)
            .map(|i| {
                let t = i as f32 / SEGMENTS as f32;
                a.lerp(b, t) + Vec2::new(0.0, (std::f32::consts::PI * t).sin() * slack)
            })
            .collect();
        Self {
            previous: points.clone(),
            points,
            length,
            endpoints: (a, b),
        }
    }

    #[cfg(test)]
    fn step(&mut self, a: Pos2, b: Pos2, floor: f32, grab: Option<(usize, Pos2)>) {
        self.step_with_pins(a, b, floor, grab, &[]);
    }

    fn step_with_pins(
        &mut self,
        a: Pos2,
        b: Pos2,
        floor: f32,
        grab: Option<(usize, Pos2)>,
        pins: &[(usize, Pos2)],
    ) {
        self.endpoints = (a, b);
        for i in 1..SEGMENTS {
            let point = self.points[i];
            let velocity = (point - self.previous[i]) * 0.975;
            self.points[i] += velocity + Vec2::new(0.0, 981.0 * STEP * STEP);
            self.previous[i] = point;
        }
        // A moved endpoint cannot cause an unsatisfiable solver. Domain cable
        // length remains unchanged; overstretch is only clamped for rendering.
        let segment_length = self.length.max(a.distance(b) * 1.01) / SEGMENTS as f32;
        for _ in 0..CONSTRAINT_PASSES {
            self.points[0] = a;
            self.points[SEGMENTS] = b;
            for &(index, point) in pins {
                self.points[index] = point;
                self.previous[index] = point;
            }
            if let Some((i, p)) = grab {
                self.points[i] = p;
            }
            for i in 0..SEGMENTS {
                let delta = self.points[i + 1] - self.points[i];
                let distance = delta.length();
                if distance < 0.00001 {
                    continue;
                }
                let correction = delta * (1.0 - segment_length / distance);
                let pinned_a = i == 0
                    || pins.iter().any(|(index, _)| *index == i)
                    || grab.is_some_and(|(index, _)| index == i);
                let pinned_b = i + 1 == SEGMENTS
                    || pins.iter().any(|(index, _)| *index == i + 1)
                    || grab.is_some_and(|(index, _)| index == i + 1);
                match (pinned_a, pinned_b) {
                    (false, false) => {
                        self.points[i] += correction * 0.5;
                        self.points[i + 1] -= correction * 0.5;
                    }
                    (true, false) => self.points[i + 1] -= correction,
                    (false, true) => self.points[i] += correction,
                    _ => {}
                }
            }
            for i in 1..SEGMENTS {
                if self.points[i].y > floor {
                    self.points[i].y = floor;
                    self.previous[i].x += (self.points[i].x - self.previous[i].x) * 0.25;
                }
            }
        }
        self.points[0] = a;
        self.points[SEGMENTS] = b;
        for &(index, point) in pins {
            self.points[index] = point;
            self.previous[index] = point;
        }
        if let Some((i, p)) = grab {
            self.points[i] = p;
            self.previous[i] = p;
        }
    }

    fn pin_endpoints(&mut self, a: Pos2, b: Pos2) {
        self.endpoints = (a, b);
        self.points[0] = a;
        self.previous[0] = a;
        self.points[SEGMENTS] = b;
        self.previous[SEGMENTS] = b;
    }

    fn pin_points(&mut self, pins: &[(usize, Pos2)]) {
        for &(index, point) in pins {
            self.points[index] = point;
            self.previous[index] = point;
        }
    }

    fn seed_pinned_path(&mut self, a: Pos2, b: Pos2, pins: &[(usize, Pos2)]) {
        let mut stops = Vec::with_capacity(pins.len() + 2);
        stops.push((0, a));
        stops.extend_from_slice(pins);
        stops.push((SEGMENTS, b));
        for pair in stops.windows(2) {
            let ((start, from), (end, to)) = (pair[0], pair[1]);
            for index in start..=end {
                let t = (index - start) as f32 / (end - start).max(1) as f32;
                self.points[index] = from.lerp(to, t);
                self.previous[index] = self.points[index];
            }
        }
    }
}

/// Shared cable parent; each connector family owns a typed instance.
struct CableLayer<Id> {
    ropes: HashMap<Id, Rope>,
    accumulator: f32,
    grab: Option<(Id, usize)>,
}

impl<Id> Default for CableLayer<Id> {
    fn default() -> Self {
        Self {
            ropes: HashMap::new(),
            accumulator: 0.0,
            grab: None,
        }
    }
}

#[derive(Default)]
pub(super) struct CableScene {
    ethernet: CableLayer<(LinkId, usize)>,
    power: CableLayer<(OutletId, usize)>,
}

pub(super) struct CableView {
    pub origin: Pos2,
    pub pixels_per_cm: f32,
    pub floor_y: f32,
    pub jacket: egui::TextureId,
    pub plug: egui::TextureId,
    pub selected: Option<LinkId>,
    pub visibility: CableVisibility,
    pub anchors: Vec<(CableRoutePoint, Pos2)>,
    pub preview: Option<(Vec<Pos2>, Color32)>,
    pub socket_rects: Vec<Rect>,
    pub interaction_enabled: bool,
}

pub(super) struct PowerCableView {
    pub paths: HashMap<OutletId, Vec<Vec<Pos2>>>,
    pub connectors: HashMap<OutletId, Vec<CableConnector>>,
    pub routed: std::collections::HashSet<OutletId>,
    pub plug: egui::TextureId,
    pub origin: Pos2,
    pub pixels_per_cm: f32,
    pub floor_y: f32,
    pub selected: Option<OutletId>,
    pub visibility: CableVisibility,
    pub socket_rects: Vec<Rect>,
    pub interaction_enabled: bool,
    pub jacket: egui::TextureId,
}

#[cfg(test)]
impl CableView {
    fn routed_path(&self, a: Pos2, b: Pos2, route: &[CableRoutePoint]) -> Vec<Pos2> {
        std::iter::once(a)
            .chain(
                route
                    .iter()
                    .filter_map(|point| anchor_position(point, &self.anchors)),
            )
            .chain(std::iter::once(b))
            .collect()
    }
}

pub(super) fn port_location(sim: &NetworkSim, id: PortId) -> Option<CableRoutePoint> {
    let port = sim.port(id)?;
    let device = sim.device(port.device)?;
    let placement = device.rack?;
    let index = device
        .ports()
        .iter()
        .position(|candidate| *candidate == id)?;
    Some(CableRoutePoint {
        rack: placement.rack,
        unit: placement.unit,
        side: port.side,
        offset_cm: (device.kind.port_position_normalized(index).0 * 48.0) as u16,
    })
}

/// A cable is an ordered physical path. A hidden face breaks its projection:
/// never join two visible sections through equipment on the current face.
pub(super) fn visible_spans(
    a: (CableRoutePoint, Option<Pos2>),
    b: (CableRoutePoint, Option<Pos2>),
    route: &[CableRoutePoint],
    anchors: &[(CableRoutePoint, Pos2)],
) -> Vec<Vec<Pos2>> {
    let nodes: Vec<_> = std::iter::once(a)
        .chain(
            route
                .iter()
                .map(|point| (*point, anchor_position(point, anchors))),
        )
        .chain(std::iter::once(b))
        .collect();
    let mut spans = Vec::new();
    let mut span = Vec::new();
    for (index, (location, position)) in nodes.iter().enumerate() {
        let Some(position) = position else { continue };
        let rail = |other_index: usize| {
            let other = &nodes[other_index].0;
            // A selected anchor is the crossing itself. Never replace it by
            // a nearer rail or by a point at the socket's unit.
            if index > 0 && index + 1 < nodes.len() {
                return *position;
            }
            if other_index > 0 && other_index + 1 < nodes.len() && other.rack == location.rack {
                return anchor_position(
                    &CableRoutePoint {
                        side: location.side,
                        ..*other
                    },
                    anchors,
                )
                .unwrap_or(*position);
            }
            // Only a cable without a defined crossing needs an automatic rail.
            let offset = if location.offset_cm + other.offset_cm < 48 {
                0
            } else {
                48
            };
            // Both faces must project the same physical crossing. Without an
            // explicit route, use the first endpoint's unit on both faces.
            let crossing_unit = if nodes.len() == 2 && nodes[0].0.rack == nodes[1].0.rack {
                nodes[0].0.unit
            } else {
                location.unit
            };
            [offset]
                .into_iter()
                .filter_map(|offset_cm| {
                    anchor_position(
                        &CableRoutePoint {
                            unit: crossing_unit,
                            offset_cm,
                            ..*location
                        },
                        anchors,
                    )
                })
                .min_by(|a, b| a.distance(*position).total_cmp(&b.distance(*position)))
                .unwrap_or(*position)
        };
        if index > 0 && nodes[index - 1].1.is_none() {
            span.push(rail(index - 1));
        }
        span.push(*position);
        if index + 1 < nodes.len() && nodes[index + 1].1.is_none() {
            span.push(rail(index + 1));
            spans.push(std::mem::take(&mut span));
        }
    }
    if !span.is_empty() {
        spans.push(span);
    }
    for span in &mut spans {
        span.dedup();
    }
    spans.retain(|span| span.len() >= 2);
    spans
}

pub(super) fn anchor_position(
    point: &CableRoutePoint,
    anchors: &[(CableRoutePoint, Pos2)],
) -> Option<Pos2> {
    let find = |target: &CableRoutePoint| {
        anchors
            .iter()
            .find(|(anchor, _)| anchor == target)
            .map(|(_, pos)| *pos)
    };
    find(point).or_else(|| {
        // Retain saved interior routes even after a cable manager is removed.
        let left = find(&CableRoutePoint {
            offset_cm: 0,
            ..*point
        })?;
        let right = find(&CableRoutePoint {
            offset_cm: 48,
            ..*point
        })?;
        Some(left.lerp(right, point.offset_cm.min(48) as f32 / 48.0))
    })
}

pub(super) fn paint_preview(ui: &egui::Ui, path: Vec<Pos2>, color: Color32) {
    ui.painter().add(egui::Shape::line(
        path.clone(),
        egui::Stroke::new(7.0, Color32::from_black_alpha(110)),
    ));
    ui.painter().add(egui::Shape::line(
        path,
        egui::Stroke::new(4.0, color.gamma_multiply(0.65)),
    ));
}

pub(super) struct CreationTarget {
    pub location: CableRoutePoint,
    pub rect: Rect,
    pub same_socket: bool,
    pub valid: bool,
}

/// Creation uses only presentation data; cable families cannot change its
/// snapping, colors, rail transitions, or anchor behavior.
pub(super) fn show_creation_preview(
    ui: &egui::Ui,
    start: (CableRoutePoint, Option<Pos2>),
    route: &[CableRoutePoint],
    color: Color32,
    targets: &[CreationTarget],
    anchors: &[(CableRoutePoint, Pos2)],
) {
    let Some(pointer) = ui
        .input(|input| input.pointer.hover_pos())
        .filter(|pointer| ui.clip_rect().contains(*pointer))
    else {
        return;
    };
    let (paths, color) = creation_preview_paths(pointer, start, route, color, targets, anchors);
    for path in paths {
        paint_preview(ui, path, color);
    }
}

fn creation_preview_paths(
    pointer: Pos2,
    start: (CableRoutePoint, Option<Pos2>),
    route: &[CableRoutePoint],
    color: Color32,
    targets: &[CreationTarget],
    anchors: &[(CableRoutePoint, Pos2)],
) -> (Vec<Vec<Pos2>>, Color32) {
    let mut route = route.to_vec();
    let mut color = color;
    let target = if let Some(target) = targets.iter().find(|target| target.rect.contains(pointer)) {
        if target.same_socket {
            color = Color32::from_rgb(255, 196, 64);
        } else if !target.valid {
            color = Color32::from_rgb(235, 70, 70);
        }
        (target.location, Some(target.rect.center()))
    } else if let Some((point, position)) = anchors.iter().find(|(_, position)| {
        Rect::from_center_size(*position, Vec2::splat(24.0)).contains(pointer)
    }) {
        if !route.contains(point) {
            route.push(*point);
        }
        (*point, Some(*position))
    } else {
        let location = anchors
            .iter()
            .min_by(|(_, a), (_, b)| a.distance(pointer).total_cmp(&b.distance(pointer)))
            .map(|(point, _)| *point)
            .unwrap_or(start.0);
        (location, Some(pointer))
    };
    (visible_spans(start, target, &route, anchors), color)
}

pub(super) fn creation_anchor(
    ui: &egui::Ui,
    point: CableRoutePoint,
    position: Pos2,
    route: &[CableRoutePoint],
) -> bool {
    let response = ui
        .interact(
            Rect::from_center_size(position, Vec2::splat(24.0)),
            egui::Id::new((
                "pending-cable-route-anchor",
                point.rack.0,
                point.unit,
                point.side == cloud_provider_sim::RackSide::Front,
                point.offset_cm,
            )),
            egui::Sense::click(),
        )
        .on_hover_text("Click to add or remove this cable route anchor");
    ui.painter().circle_filled(
        position,
        5.0,
        if route.contains(&point) || response.hovered() {
            Color32::from_rgb(255, 196, 64)
        } else {
            Color32::from_rgb(92, 112, 125)
        },
    );
    response.clicked()
}

/// Family adapters describe cables; this parent owns every visual and input rule.
#[derive(Clone, Copy)]
pub(super) enum ConnectorKind {
    Rj45,
    Iec,
    CiscoFourPin,
}

#[derive(Clone, Copy)]
pub(super) struct CableConnector {
    pub socket: Rect,
    pub kind: ConnectorKind,
}

impl CableConnector {
    fn uv(self) -> Rect {
        let (left, top, right, bottom) = match self.kind {
            ConnectorKind::Rj45 => (0.10, 0.17, 0.90, 1.0),
            // End-on cable-exit faces, measured in the 1774 × 887 rear atlas.
            ConnectorKind::Iec => (98.0 / 1774.0, 164.0 / 887.0, 950.0 / 1774.0, 696.0 / 887.0),
            ConnectorKind::CiscoFourPin => (
                1146.0 / 1774.0,
                137.0 / 887.0,
                1604.0 / 1774.0,
                695.0 / 887.0,
            ),
        };
        Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, bottom))
    }

    fn cable_exit(self) -> Vec2 {
        match self.kind {
            ConnectorKind::Rj45 => Vec2::splat(0.5),
            ConnectorKind::Iec => Vec2::new(0.495, 0.70),
            ConnectorKind::CiscoFourPin => Vec2::new(0.50, 0.745),
        }
    }

    fn rect(self) -> Rect {
        if matches!(self.kind, ConnectorKind::Rj45) {
            return plug_rect(self.socket);
        }
        let uv = self.uv();
        let width = self.socket.width().clamp(12.0, 28.0);
        let height = width * uv.height() / (uv.width() * 2.0);
        let exit = self.cable_exit();
        Rect::from_min_size(
            self.socket.center() - Vec2::new(width * exit.x, height * exit.y),
            Vec2::new(width, height),
        )
    }
}

struct RenderCable<Id> {
    id: Id,
    paths: Vec<Vec<Pos2>>,
    length_cm: Option<f32>,
    connectors: Vec<CableConnector>,
    color: Color32,
    selected: bool,
    routed: bool,
}

struct LayerView {
    origin: Pos2,
    pixels_per_cm: f32,
    floor_y: f32,
    jacket: egui::TextureId,
    plug: egui::TextureId,
    visibility: CableVisibility,
    selection_active: bool,
    socket_rects: Vec<Rect>,
    interaction_enabled: bool,
}

impl<Id: Copy + Eq + std::hash::Hash> CableLayer<(Id, usize)> {
    fn show(
        &mut self,
        ui: &mut egui::Ui,
        cables: &[RenderCable<Id>],
        view: LayerView,
    ) -> Option<Id> {
        let local = |p: Pos2| Pos2::ZERO + (p - view.origin) / view.pixels_per_cm;
        let screen = |p: Pos2| view.origin + p.to_vec2() * view.pixels_per_cm;
        let mut spans = Vec::new();
        for cable in cables.iter().filter(|cable| match view.visibility {
            CableVisibility::All => true,
            CableVisibility::Selected => cable.selected,
            CableVisibility::Hidden => false,
        }) {
            let distances: Vec<f32> = cable
                .paths
                .iter()
                .map(|path| {
                    path.windows(2)
                        .map(|pair| pair[0].distance(pair[1]))
                        .sum::<f32>()
                        / view.pixels_per_cm
                })
                .collect();
            let total = distances.iter().sum::<f32>();
            let length = cable.length_cm.unwrap_or(total * 1.05).max(total);
            for (index, path) in cable
                .paths
                .iter()
                .enumerate()
                .filter(|(_, path)| path.len() >= 2)
            {
                let share = if total > 0.001 {
                    distances[index] / total
                } else {
                    1.0 / cable.paths.len() as f32
                };
                spans.push(((cable.id, index), cable, path, length * share));
            }
        }
        self.ropes
            .retain(|key, _| spans.iter().any(|(candidate, _, _, _)| candidate == key));
        if !view.interaction_enabled
            || self
                .grab
                .is_some_and(|(key, _)| !self.ropes.contains_key(&key))
        {
            self.grab = None;
        }
        for (key, _, path, length) in &spans {
            let (a, b) = (local(path[0]), local(*path.last().unwrap()));
            let rope = self
                .ropes
                .entry(*key)
                .or_insert_with(|| Rope::new(a, b, *length));
            if (rope.length - length).abs() > 0.01 {
                *rope = Rope::new(a, b, *length);
            }
            rope.pin_endpoints(a, b);
            let pins: Vec<_> = route_pin_indices(path)
                .into_iter()
                .map(|(i, p)| (i, local(p)))
                .collect();
            if pins
                .iter()
                .any(|(i, p)| rope.points[*i].distance(*p) > 0.01)
            {
                rope.seed_pinned_path(a, b, &pins);
            } else {
                rope.pin_points(&pins);
            }
        }
        let (pointer, pressed, down, dt) = ui.input(|input| {
            (
                input.pointer.interact_pos(),
                input.pointer.primary_pressed(),
                input.pointer.primary_down(),
                input.stable_dt,
            )
        });
        if !down {
            self.grab = None;
        }
        let mut clicked = None;
        if view.interaction_enabled {
            if let Some(pointer) = pointer.filter(|p| {
                ui.clip_rect().contains(*p)
                    && !view
                        .socket_rects
                        .iter()
                        .any(|rect| rect.expand(3.0).contains(*p))
            }) {
                let nearest = spans
                    .iter()
                    .filter_map(|(key, cable, _, _)| {
                        let rope = &self.ropes[key];
                        rope.points
                            .windows(2)
                            .enumerate()
                            .map(|(index, pair)| {
                                (
                                    *key,
                                    *cable,
                                    (index + 1).clamp(1, SEGMENTS - 1),
                                    distance_to_segment(pointer, screen(pair[0]), screen(pair[1])),
                                )
                            })
                            .min_by(|a, b| a.3.total_cmp(&b.3))
                    })
                    .filter(|(_, _, _, distance)| *distance < 9.0)
                    .min_by(|a, b| a.3.total_cmp(&b.3));
                if let Some((key, cable, index, _)) = nearest {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    if pressed {
                        if !cable.routed {
                            self.grab = Some((key, index));
                        }
                        clicked = Some(key.0);
                    }
                }
            }
        }
        let floor = (view.floor_y - view.origin.y) / view.pixels_per_cm;
        self.accumulator = (self.accumulator + dt.clamp(0.0, 0.066)).min(STEP * 8.0);
        while self.accumulator >= STEP {
            for (key, _, path, _) in &spans {
                let (a, b) = (local(path[0]), local(*path.last().unwrap()));
                let rope = self.ropes.get_mut(key).unwrap();
                let grab = self
                    .grab
                    .filter(|(candidate, _)| candidate == key)
                    .and_then(|(_, index)| {
                        pointer.map(|pointer| {
                            let mut target = local(pointer);
                            for _ in 0..4 {
                                for (anchor, reach) in [
                                    (a, rope.length * index as f32 / SEGMENTS as f32),
                                    (b, rope.length * (SEGMENTS - index) as f32 / SEGMENTS as f32),
                                ] {
                                    let delta = target - anchor;
                                    if delta.length() > reach {
                                        target = anchor + delta.normalized() * reach;
                                    }
                                }
                            }
                            target.y = target.y.min(floor);
                            (index, target)
                        })
                    });
                let pins: Vec<_> = route_pin_indices(path)
                    .into_iter()
                    .map(|(i, p)| (i, local(p)))
                    .collect();
                rope.step_with_pins(a, b, floor, grab, &pins);
            }
            self.accumulator -= STEP;
        }
        let width = (view.pixels_per_cm * 0.55).clamp(4.0, 7.5);
        let mut painted = std::collections::HashSet::new();
        for (_, cable, _, _) in &spans {
            if !painted.insert(cable.id) {
                continue;
            }
            for connector in &cable.connectors {
                ui.painter()
                    .image(view.plug, connector.rect(), connector.uv(), Color32::WHITE);
            }
        }
        for (key, cable, _, _) in &spans {
            let path: Vec<_> = self.ropes[key].points.iter().map(|p| screen(*p)).collect();
            let color = if view.selection_active && !cable.selected {
                cable.color.gamma_multiply(0.35)
            } else {
                cable.color
            };
            paint_cable(ui, &path, width, color, cable.selected, view.jacket);
        }
        if self.grab.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        if !spans.is_empty() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(16));
        }
        clicked
    }
}

impl CableScene {
    pub(super) fn show_power(
        &mut self,
        ui: &mut egui::Ui,
        cables: &[(OutletId, Pos2, Pos2, u32)],
        view: PowerCableView,
    ) -> Option<OutletId> {
        let descriptors: Vec<_> = cables
            .iter()
            .map(|(id, a, b, length)| {
                let paths = view
                    .paths
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| vec![vec![*a, *b]]);
                RenderCable {
                    id: *id,
                    routed: view.routed.contains(id) || paths.iter().any(|path| path.len() > 2),
                    paths,
                    length_cm: (*length > 0).then_some(*length as f32),
                    connectors: view.connectors.get(id).cloned().unwrap_or_default(),
                    color: Color32::from_rgb(31, 35, 38),
                    selected: view.selected == Some(*id),
                }
            })
            .collect();
        self.power.show(
            ui,
            &descriptors,
            LayerView {
                origin: view.origin,
                pixels_per_cm: view.pixels_per_cm,
                floor_y: view.floor_y,
                jacket: view.jacket,
                plug: view.plug,
                visibility: view.visibility,
                selection_active: view.selected.is_some(),
                socket_rects: view.socket_rects,
                interaction_enabled: view.interaction_enabled && self.ethernet.grab.is_none(),
            },
        )
    }

    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        sim: &NetworkSim,
        ports: &HashMap<PortId, Rect>,
        view: CableView,
    ) -> Option<LinkId> {
        let selected_ports: std::collections::HashSet<_> = view
            .selected
            .and_then(|id| sim.link(id))
            .map(|link| sim.physical_path(link.a).into_iter().collect())
            .unwrap_or_default();
        let mut descriptors: Vec<_> = sim
            .links()
            .filter_map(|link| {
                let (a, b) = port_location(sim, link.a).zip(port_location(sim, link.b))?;
                let paths = visible_spans(
                    (a, ports.get(&link.a).map(Rect::center)),
                    (b, ports.get(&link.b).map(Rect::center)),
                    &link.route,
                    &view.anchors,
                );
                let complete = ports.contains_key(&link.a)
                    && ports.contains_key(&link.b)
                    && link
                        .route
                        .iter()
                        .all(|point| anchor_position(point, &view.anchors).is_some());
                Some(RenderCable {
                    id: link.id,
                    paths,
                    length_cm: complete.then_some(link.length_cm as f32),
                    connectors: [link.a, link.b]
                        .iter()
                        .filter_map(|id| {
                            ports.get(id).map(|socket| CableConnector {
                                socket: *socket,
                                kind: ConnectorKind::Rj45,
                            })
                        })
                        .collect(),
                    color: super::cable_color_value(link.color),
                    selected: view.selected == Some(link.id)
                        || selected_ports.contains(&link.a)
                        || selected_ports.contains(&link.b),
                    routed: !link.route.is_empty(),
                })
            })
            .collect();
        descriptors.sort_by_key(|cable| cable.id);
        let sockets = view
            .socket_rects
            .iter()
            .copied()
            .chain(ports.values().copied())
            .chain(
                view.anchors
                    .iter()
                    .map(|(_, position)| Rect::from_center_size(*position, Vec2::splat(14.0))),
            )
            .collect();
        let clicked = self.ethernet.show(
            ui,
            &descriptors,
            LayerView {
                origin: view.origin,
                pixels_per_cm: view.pixels_per_cm,
                floor_y: view.floor_y,
                jacket: view.jacket,
                plug: view.plug,
                visibility: view.visibility,
                selection_active: view.selected.is_some(),
                socket_rects: sockets,
                interaction_enabled: view.interaction_enabled && self.power.grab.is_none(),
            },
        );
        if let Some((path, color)) = view.preview {
            paint_preview(ui, path, color);
        }
        clicked
    }
}
fn plug_rect(socket: Rect) -> Rect {
    // A short foreshortened housing stays centered over the occupied jack.
    let size = Vec2::new(
        socket.width().clamp(10.0, 20.0),
        socket.height().clamp(10.0, 18.0),
    );
    Rect::from_center_size(socket.center(), size)
}

fn paint_cable(
    ui: &egui::Ui,
    path: &[Pos2],
    width: f32,
    color: Color32,
    selected: bool,
    jacket: egui::TextureId,
) {
    let shadow = path.iter().map(|p| *p + Vec2::new(2.0, 3.0)).collect();
    ui.painter().add(egui::Shape::line(
        shadow,
        egui::Stroke::new(width + 3.0, Color32::from_black_alpha(150)),
    ));
    if selected {
        ui.painter().add(egui::Shape::line(
            path.to_vec(),
            egui::Stroke::new(width + 3.0, Color32::from_rgb(255, 196, 64)),
        ));
    }
    ui.painter().add(egui::Shape::line(
        path.to_vec(),
        egui::Stroke::new(width, color),
    ));
    paint_jacket(ui.painter(), path, width, jacket);
}

fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let delta = b - a;
    let t = ((p - a).dot(delta) / delta.length_sq().max(0.0001)).clamp(0.0, 1.0);
    p.distance(a + delta * t)
}

/// Turn a routed polyline into a flexible lead. Endpoints and rack anchors are
/// kept fixed while any length beyond the geometric path becomes a visible sag.
/// Maps every interior route anchor onto a stable rope particle. The solver
/// then treats these particles as fixed while the cable between them moves.
fn route_pin_indices(path: &[Pos2]) -> Vec<(usize, Pos2)> {
    let total: f32 = path.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
    if path.len() < 3 || total < 0.001 {
        return vec![];
    }
    let mut travelled = 0.0;
    let mut previous_index = 0;
    let last_anchor = path.len() - 2;
    path.windows(2)
        .enumerate()
        .filter_map(|(segment, pair)| {
            travelled += pair[0].distance(pair[1]);
            if segment == last_anchor {
                return None;
            }
            let remaining = last_anchor - segment;
            let index = ((travelled / total * SEGMENTS as f32).round() as usize)
                .clamp(previous_index + 1, SEGMENTS - remaining);
            previous_index = index;
            Some((index, path[segment + 1]))
        })
        .collect()
}
fn paint_jacket(painter: &egui::Painter, path: &[Pos2], width: f32, texture: egui::TextureId) {
    let mut mesh = egui::Mesh::with_texture(texture);
    for (i, p) in path.iter().enumerate() {
        let previous = path[i.saturating_sub(1)];
        let next = path[(i + 1).min(path.len() - 1)];
        let tangent = (next - previous).normalized();
        let normal = Vec2::new(-tangent.y, tangent.x) * width * 0.5;
        // The repeating material is mapped along each segment; adjacent endpoints
        // share vertices so bends do not have detached seams.
        let u = (i % 2) as f32;
        for (offset, v) in [(normal, 0.0), (-normal, 1.0)] {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: *p + offset,
                uv: egui::pos2(u, v),
                color: Color32::from_white_alpha(145),
            });
        }
        if i > 0 {
            let n = (i * 2) as u32;
            mesh.indices
                .extend_from_slice(&[n - 2, n - 1, n, n, n - 1, n + 1]);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloud_provider_sim::{
        CableSupply, Command, DeviceTemplate, RackId, RackSide, SimEvent, SourceId,
    };

    #[test]
    fn power_plugs_use_distinct_artwork_without_stretching() {
        let socket = Rect::from_center_size(egui::pos2(100.0, 40.0), Vec2::new(22.0, 16.0));
        let iec = CableConnector {
            socket,
            kind: ConnectorKind::Iec,
        };
        let cisco = CableConnector {
            socket,
            kind: ConnectorKind::CiscoFourPin,
        };
        assert_ne!(iec.uv(), cisco.uv());
        for connector in [iec, cisco] {
            let rect = connector.rect();
            let uv = connector.uv();
            let exit = connector.cable_exit();
            assert!(
                (rect.min + Vec2::new(rect.width() * exit.x, rect.height() * exit.y))
                    .distance(socket.center())
                    < 0.001
            );
            assert!((rect.width() / rect.height() - uv.width() * 2.0 / uv.height()).abs() < 0.001);
            assert!(Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)).contains_rect(uv));
        }
    }

    #[test]
    fn explicit_anchor_order_and_crossing_height_are_preserved() {
        let point = CableRoutePoint {
            rack: RackId(1),
            unit: 1,
            side: RackSide::Front,
            offset_cm: 0,
        };
        let first = CableRoutePoint {
            unit: 9,
            offset_cm: 48,
            ..point
        };
        let second = CableRoutePoint { unit: 3, ..point };
        let third = CableRoutePoint { unit: 7, ..first };
        let a = egui::pos2(100.0, 450.0);
        let b = egui::pos2(120.0, 400.0);
        let positions = [
            egui::pos2(300.0, 50.0),
            egui::pos2(20.0, 350.0),
            egui::pos2(300.0, 150.0),
        ];
        let anchors = vec![
            (first, positions[0]),
            (second, positions[1]),
            (third, positions[2]),
        ];
        assert_eq!(
            visible_spans(
                (point, Some(a)),
                (point, Some(b)),
                &[first, second, third],
                &anchors
            ),
            vec![vec![a, positions[0], positions[1], positions[2], b]]
        );
        let rear_first = CableRoutePoint {
            side: RackSide::Rear,
            ..first
        };
        let rear_second = CableRoutePoint {
            side: RackSide::Rear,
            ..second
        };
        assert_eq!(
            visible_spans(
                (point, Some(a)),
                (point, Some(b)),
                &[rear_first, rear_second],
                &anchors
            ),
            vec![vec![a, positions[0]], vec![positions[1], b]]
        );
        let (paths, _) = creation_preview_paths(
            positions[0],
            (point, Some(a)),
            &[first, second, third],
            Color32::WHITE,
            &[],
            &anchors,
        );
        assert_eq!(
            paths,
            vec![vec![
                a,
                positions[0],
                positions[1],
                positions[2],
                positions[0]
            ]],
            "hovering an existing anchor must not shorten the chosen route"
        );
    }

    #[test]
    fn creation_preview_follows_pointer_through_anchors_and_across_faces() {
        let front = CableRoutePoint {
            rack: RackId(1),
            unit: 1,
            side: RackSide::Front,
            offset_cm: 0,
        };
        let rear = CableRoutePoint {
            side: RackSide::Rear,
            ..front
        };
        let anchor = egui::pos2(20.0, 40.0);
        let anchors = vec![(front, anchor)];
        let start = egui::pos2(80.0, 40.0);
        let pointer = egui::pos2(160.0, 100.0);
        let color = Color32::WHITE;
        assert_eq!(
            creation_preview_paths(
                pointer,
                (front, Some(start)),
                &[front],
                color,
                &[],
                &anchors
            ),
            (vec![vec![start, anchor, pointer]], color)
        );
        assert_eq!(
            creation_preview_paths(pointer, (rear, None), &[], color, &[], &anchors),
            (vec![vec![anchor, pointer]], color)
        );
        assert_eq!(
            creation_preview_paths(anchor, (front, Some(start)), &[front], color, &[], &anchors),
            (vec![vec![start, anchor]], color)
        );
    }

    #[test]
    fn creation_preview_snaps_to_sockets_and_uses_shared_validity_colors() {
        let point = CableRoutePoint {
            rack: RackId(1),
            unit: 1,
            side: RackSide::Front,
            offset_cm: 0,
        };
        let start = egui::pos2(20.0, 30.0);
        let target = Rect::from_center_size(egui::pos2(100.0, 80.0), Vec2::splat(12.0));
        for (same_socket, valid, expected) in [
            (false, true, Color32::WHITE),
            (false, false, Color32::from_rgb(235, 70, 70)),
            (true, false, Color32::from_rgb(255, 196, 64)),
        ] {
            let (paths, color) = creation_preview_paths(
                target.center() + Vec2::splat(2.0),
                (point, Some(start)),
                &[],
                Color32::WHITE,
                &[CreationTarget {
                    location: point,
                    rect: target,
                    same_socket,
                    valid,
                }],
                &[],
            );
            assert_eq!(paths, vec![vec![start, target.center()]]);
            assert_eq!(color, expected);
        }
    }

    #[test]
    fn parent_renders_and_simulates_both_cable_ids_identically() {
        fn render<Id: Copy + Eq + std::hash::Hash>(
            id: Id,
        ) -> (Vec<Pos2>, Vec<egui::epaint::ClippedShape>) {
            let path = vec![egui::pos2(30.0, 40.0), egui::pos2(110.0, 40.0)];
            let sockets: Vec<_> = path
                .iter()
                .map(|p| Rect::from_center_size(*p, Vec2::splat(12.0)))
                .collect();
            let cable = RenderCable {
                id,
                paths: vec![path],
                length_cm: None,
                connectors: sockets
                    .iter()
                    .map(|socket| CableConnector {
                        socket: *socket,
                        kind: ConnectorKind::Rj45,
                    })
                    .collect(),
                color: Color32::from_gray(80),
                selected: true,
                routed: false,
            };
            let mut layer = CableLayer::default();
            let ctx = egui::Context::default();
            let view = || LayerView {
                origin: Pos2::ZERO,
                pixels_per_cm: 1.0,
                floor_y: 300.0,
                jacket: egui::TextureId::User(1),
                plug: egui::TextureId::User(2),
                visibility: CableVisibility::All,
                selection_active: true,
                socket_rects: sockets.clone(),
                interaction_enabled: true,
            };
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                layer.show(ui, std::slice::from_ref(&cable), view());
            });
            output.textures_delta.clear();
            let pointer = layer.ropes[&(id, 0)].points[SEGMENTS / 2];
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(pointer),
                        egui::Event::PointerButton {
                            pos: pointer,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| {
                    assert!(layer.show(ui, std::slice::from_ref(&cable), view()) == Some(id));
                },
            );
            output.textures_delta.clear();
            assert!(layer.grab.is_some());
            (layer.ropes[&(id, 0)].points.clone(), output.shapes)
        }
        let ethernet = render(LinkId(1));
        let power = render(OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 0,
        });
        assert_eq!(ethernet.0, power.0);
        assert_eq!(ethernet.1, power.1);
    }

    #[test]
    fn hidden_route_sections_split_the_projection_for_all_cable_families() {
        let front = CableRoutePoint {
            rack: RackId(1),
            unit: 1,
            side: RackSide::Front,
            offset_cm: 0,
        };
        let rear = CableRoutePoint {
            side: RackSide::Rear,
            ..front
        };
        let anchors = vec![
            (front, egui::pos2(0.0, 50.0)),
            (
                CableRoutePoint {
                    offset_cm: 48,
                    ..front
                },
                egui::pos2(300.0, 50.0),
            ),
        ];
        let a = egui::pos2(50.0, 50.0);
        let b = egui::pos2(100.0, 50.0);
        assert_eq!(
            visible_spans((front, Some(a)), (rear, None), &[], &anchors),
            vec![vec![a, egui::pos2(0.0, 50.0)]]
        );
        assert_eq!(
            visible_spans((rear, None), (front, Some(b)), &[], &anchors),
            vec![vec![egui::pos2(0.0, 50.0), b]]
        );
        assert!(visible_spans((rear, None), (rear, None), &[], &anchors).is_empty());
        let spans = visible_spans((front, Some(a)), (front, Some(b)), &[rear], &anchors);
        assert_eq!(
            spans,
            vec![
                vec![a, egui::pos2(0.0, 50.0)],
                vec![egui::pos2(0.0, 50.0), b]
            ]
        );
    }

    #[test]
    fn automatic_cross_face_rail_uses_same_unit_on_both_faces() {
        let front = CableRoutePoint {
            rack: RackId(1),
            unit: 19,
            side: RackSide::Front,
            offset_cm: 0,
        };
        let rear = CableRoutePoint {
            unit: 10,
            side: RackSide::Rear,
            ..front
        };
        let front_socket = egui::pos2(50.0, 19.0);
        let rear_socket = egui::pos2(50.0, 10.0);
        let front_rail = egui::pos2(0.0, 19.0);
        let rear_rail = egui::pos2(0.0, 19.0);
        let anchors = vec![
            (front, front_rail),
            (
                CableRoutePoint {
                    side: RackSide::Rear,
                    ..front
                },
                rear_rail,
            ),
            (rear, egui::pos2(0.0, 10.0)),
        ];
        assert_eq!(
            visible_spans((front, Some(front_socket)), (rear, None), &[], &anchors),
            vec![vec![front_socket, front_rail]]
        );
        assert_eq!(
            visible_spans((front, None), (rear, Some(rear_socket)), &[], &anchors),
            vec![vec![rear_rail, rear_socket]]
        );
    }

    #[test]
    fn cross_face_ethernet_keeps_visible_plug_and_selectable_jacket() {
        let mut sim = NetworkSim::new();
        let mut ports = Vec::new();
        for (kind, unit) in [(DeviceTemplate::Switch, 1), (DeviceTemplate::Server, 2)] {
            let SimEvent::DeviceAdded(id) = sim.execute(Command::BuyDevice { kind }).unwrap()[0]
            else {
                unreachable!()
            };
            sim.execute(Command::PlaceDevice {
                device: id,
                rack: RackId(1),
                unit,
            })
            .unwrap();
            ports.push(sim.device(id).unwrap().ports()[0]);
        }
        for supply in [CableSupply::CableBox305m, CableSupply::Rj45Pack20] {
            sim.execute(Command::BuyCableSupply { supply }).unwrap();
        }
        sim.execute(Command::Connect {
            a: ports[0],
            b: ports[1],
        })
        .unwrap();
        let link = sim.link_for_port(ports[0]).unwrap().id;
        assert_ne!(
            sim.port(ports[0]).unwrap().side,
            sim.port(ports[1]).unwrap().side
        );
        let mut scene = CableScene::default();
        for id in [ports[0], ports[1], ports[0]] {
            let location = port_location(&sim, id).unwrap();
            let socket = Rect::from_center_size(egui::pos2(100.0, 60.0), Vec2::splat(12.0));
            let visible_ports = HashMap::from([(id, socket)]);
            let mut anchors = vec![
                (
                    CableRoutePoint {
                        offset_cm: 0,
                        ..location
                    },
                    egui::pos2(20.0, 60.0),
                ),
                (
                    CableRoutePoint {
                        offset_cm: 48,
                        ..location
                    },
                    egui::pos2(200.0, 60.0),
                ),
            ];
            // The rack view supplies rail positions for every unit, including
            // the opposite endpoint's unit where this cable crosses faces.
            let source = port_location(&sim, ports[0]).unwrap();
            if source.unit != location.unit {
                anchors.push((
                    CableRoutePoint {
                        unit: source.unit,
                        offset_cm: 0,
                        ..location
                    },
                    egui::pos2(20.0, 60.0),
                ));
                anchors.push((
                    CableRoutePoint {
                        unit: source.unit,
                        offset_cm: 48,
                        ..location
                    },
                    egui::pos2(200.0, 60.0),
                ));
            }
            let ctx = egui::Context::default();
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(500.0))),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                scene.show(
                    ui,
                    &sim,
                    &visible_ports,
                    CableView {
                        origin: Pos2::ZERO,
                        pixels_per_cm: 5.0,
                        floor_y: 400.0,
                        jacket: egui::TextureId::User(1),
                        plug: egui::TextureId::User(2),
                        selected: Some(link),
                        visibility: CableVisibility::Selected,
                        anchors: anchors.clone(),
                        preview: None,
                        socket_rects: vec![socket],
                        interaction_enabled: true,
                    },
                );
            });
            assert_eq!(scene.ethernet.ropes.len(), 1);
            let rope = &scene.ethernet.ropes[&(link, 0)];
            let endpoint = if id == ports[0] {
                rope.points[0]
            } else {
                rope.points[SEGMENTS]
            };
            assert!(endpoint.distance(Pos2::ZERO + socket.center().to_vec2() / 5.0) < 0.001);
            let plugs = output
                .shapes
                .iter()
                .filter(|shape| {
                    matches!(&shape.shape,
                egui::Shape::Mesh(mesh) if mesh.texture_id == egui::TextureId::User(2))
                })
                .count();
            assert_eq!(plugs, 1, "only the visible socket gets a plug");
            output.textures_delta.clear();
            let pointer = Pos2::ZERO + rope.points[SEGMENTS / 2].to_vec2() * 5.0;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(pointer),
                        egui::Event::PointerButton {
                            pos: pointer,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| {
                    assert_eq!(
                        scene.show(
                            ui,
                            &sim,
                            &visible_ports,
                            CableView {
                                origin: Pos2::ZERO,
                                pixels_per_cm: 5.0,
                                floor_y: 400.0,
                                jacket: egui::TextureId::User(1),
                                plug: egui::TextureId::User(2),
                                selected: Some(link),
                                visibility: CableVisibility::Selected,
                                anchors: anchors.clone(),
                                preview: None,
                                socket_rects: vec![socket],
                                interaction_enabled: true,
                            }
                        ),
                        Some(link)
                    );
                },
            );
            output.textures_delta.clear();
        }
    }

    fn power_view() -> PowerCableView {
        PowerCableView {
            paths: HashMap::new(),
            connectors: HashMap::new(),
            routed: Default::default(),
            plug: egui::TextureId::User(2),
            jacket: egui::TextureId::Managed(0),
            origin: Pos2::ZERO,
            pixels_per_cm: 1.0,
            floor_y: 300.0,
            selected: None,
            visibility: CableVisibility::All,
            socket_rects: vec![],
            interaction_enabled: true,
        }
    }

    #[test]
    fn power_sections_have_independent_ropes_and_no_hidden_bridge() {
        let outlet = OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 0,
        };
        let paths = vec![
            vec![egui::pos2(20.0, 40.0), egui::pos2(70.0, 40.0)],
            vec![egui::pos2(220.0, 40.0), egui::pos2(270.0, 40.0)],
        ];
        let mut scene = CableScene::default();
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(500.0))),
                events: vec![
                    egui::Event::PointerMoved(egui::pos2(150.0, 40.0)),
                    egui::Event::PointerButton {
                        pos: egui::pos2(150.0, 40.0),
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ui| {
                let mut view = power_view();
                view.paths.insert(outlet, paths.clone());
                assert_eq!(
                    scene.show_power(ui, &[(outlet, paths[0][0], paths[1][1], 300)], view),
                    None
                );
            },
        );
        output.textures_delta.clear();
        assert_eq!(scene.power.ropes.len(), 2);
        for (index, path) in paths.iter().enumerate() {
            let rope = &scene.power.ropes[&(outlet, index)];
            assert_eq!(rope.points[0], path[0]);
            assert_eq!(rope.points[SEGMENTS], path[1]);
        }
    }

    #[test]
    fn power_ropes_simulate_independently_and_keep_transformed_anchors() {
        let first = OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 0,
        };
        let second = OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 1,
        };
        let cables = vec![
            (first, egui::pos2(10.0, 20.0), egui::pos2(90.0, 20.0), 120),
            (second, egui::pos2(20.0, 40.0), egui::pos2(100.0, 40.0), 120),
        ];
        let mut scene = CableScene {
            power: CableLayer {
                accumulator: STEP * 2.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            assert_eq!(scene.show_power(ui, &cables, power_view()), None);
        });
        output.textures_delta.clear();
        assert_eq!(scene.power.ropes.len(), 2);
        assert_eq!(
            scene.power.ropes[&(first, 0)].points[0],
            egui::pos2(10.0, 20.0)
        );
        assert_eq!(
            scene.power.ropes[&(second, 0)].points[SEGMENTS],
            egui::pos2(100.0, 40.0)
        );
    }

    #[test]
    fn hidden_power_cables_and_socket_regions_cannot_be_grabbed() {
        let outlet = OutletId {
            source: SourceId::Rack(RackId(2)),
            index: 0,
        };
        let cables = vec![(outlet, egui::pos2(10.0, 20.0), egui::pos2(90.0, 20.0), 120)];
        let ctx = egui::Context::default();
        let mut scene = CableScene::default();
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(200.0))),
            events: vec![egui::Event::PointerButton {
                pos: egui::pos2(50.0, 20.0),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        };
        let mut output = ctx.run_ui(raw, |ui| {
            let mut view = power_view();
            view.socket_rects = vec![Rect::from_center_size(
                egui::pos2(50.0, 20.0),
                Vec2::splat(8.0),
            )];
            assert_eq!(scene.show_power(ui, &cables, view), None);
        });
        output.textures_delta.clear();
        assert!(scene.power.grab.is_none());
    }

    #[test]
    fn routed_power_rope_keeps_rail_pin_fixed_after_view_transform() {
        let outlet = OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 0,
        };
        let a = egui::pos2(10.0, 20.0);
        let anchor = egui::pos2(60.0, 20.0);
        let b = egui::pos2(90.0, 80.0);
        let ctx = egui::Context::default();
        let mut scene = CableScene::default();
        for (origin, scale) in [(Pos2::ZERO, 1.0), (egui::pos2(30.0, -100.0), 2.0)] {
            let screen = |p: Pos2| origin + p.to_vec2() * scale;
            let path = vec![screen(a), screen(anchor), screen(b)];
            let index = route_pin_indices(&path)[0].0;
            let cables = vec![(outlet, screen(a), screen(b), 130)];
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                let mut view = power_view();
                view.origin = origin;
                view.pixels_per_cm = scale;
                view.paths.insert(outlet, vec![path.clone()]);
                scene.show_power(ui, &cables, view);
            });
            output.textures_delta.clear();
            assert!(scene.power.ropes[&(outlet, 0)].points[index].distance(anchor) < 0.001);
            assert!(scene.power.ropes[&(outlet, 0)].points[0].distance(a) < 0.001);
            assert!(scene.power.ropes[&(outlet, 0)].points[SEGMENTS].distance(b) < 0.001);
        }
    }

    #[test]
    fn selected_power_click_returns_outlet_and_never_grabs_endpoint() {
        let outlet = OutletId {
            source: SourceId::Rack(RackId(3)),
            index: 0,
        };
        let cables = vec![(outlet, egui::pos2(10.0, 20.0), egui::pos2(90.0, 20.0), 120)];
        let ctx = egui::Context::default();
        let mut scene = CableScene::default();
        let pointer = egui::pos2(50.0, 53.0);
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(200.0))),
            events: vec![
                egui::Event::PointerMoved(pointer),
                egui::Event::PointerButton {
                    pos: pointer,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        };
        let mut output = ctx.run_ui(raw, |ui| {
            assert_eq!(scene.show_power(ui, &cables, power_view()), Some(outlet));
        });
        output.textures_delta.clear();
        let (_, index) = scene.power.grab.expect("clicked lead should be grabbed");
        assert!((1..SEGMENTS).contains(&index));
    }

    #[test]
    fn routed_rope_keeps_the_anchor_fixed_while_its_port_legs_sag() {
        let (a, anchor, b) = (
            egui::pos2(0.0, 0.0),
            egui::pos2(80.0, 0.0),
            egui::pos2(160.0, 0.0),
        );
        let mut rope = Rope::new(a, b, 220.0);
        let pins = vec![(SEGMENTS / 2, anchor)];
        for _ in 0..600 {
            rope.step_with_pins(a, b, 140.0, None, &pins);
        }
        assert_eq!(rope.points[SEGMENTS / 2], anchor);
        assert!(rope.points[SEGMENTS / 4].y > 10.0);
        assert!(rope.points[SEGMENTS * 3 / 4].y > 10.0);
    }

    #[test]
    fn routed_cable_uses_visible_anchors_for_painting_and_clicks() {
        let mut sim = NetworkSim::new();
        let mut ids = vec![];
        for unit in [1, 2] {
            let SimEvent::DeviceAdded(id) = sim
                .execute(Command::BuyDevice {
                    kind: DeviceTemplate::Switch,
                })
                .unwrap()[0]
            else {
                unreachable!()
            };
            sim.execute(Command::PlaceDevice {
                device: id,
                rack: RackId(1),
                unit,
            })
            .unwrap();
            ids.push(sim.device(id).unwrap().ports()[0]);
        }
        for supply in [CableSupply::CableBox305m, CableSupply::Rj45Pack20] {
            sim.execute(Command::BuyCableSupply { supply }).unwrap();
        }
        sim.execute(Command::Connect {
            a: ids[0],
            b: ids[1],
        })
        .unwrap();
        let link = sim.link_for_port(ids[0]).unwrap().id;
        let anchor = CableRoutePoint {
            rack: RackId(1),
            unit: 1,
            side: RackSide::Front,
            offset_cm: 48,
        };
        sim.execute(Command::AddCableRoutePoint {
            link,
            point: anchor,
        })
        .unwrap();
        let a = egui::pos2(50.0, 50.0);
        let b = egui::pos2(50.0, 150.0);
        let corner = egui::pos2(250.0, 50.0);
        let ports = HashMap::from([
            (ids[0], Rect::from_center_size(a, Vec2::splat(12.0))),
            (ids[1], Rect::from_center_size(b, Vec2::splat(12.0))),
        ]);
        let view = || CableView {
            origin: Pos2::ZERO,
            pixels_per_cm: 5.0,
            floor_y: 400.0,
            jacket: egui::TextureId::User(1),
            plug: egui::TextureId::User(2),
            selected: Some(link),
            visibility: CableVisibility::All,
            anchors: vec![(anchor, corner)],
            preview: None,
            socket_rects: vec![],
            interaction_enabled: true,
        };
        assert_eq!(view().routed_path(a, b, &[anchor]), vec![a, corner, b]);
        let other_rack = CableRoutePoint {
            rack: RackId(2),
            ..anchor
        };
        assert_eq!(view().routed_path(a, b, &[other_rack]), vec![a, b]);
        let state = crate::app::UiState {
            selected: crate::app::Selection::Port(ids[0]),
            ..Default::default()
        };
        assert_eq!(super::super::selected_link_id(&state, &sim), Some(link));

        for (pointer, expected) in [(a.lerp(corner, 0.5), Some(link)), (corner, None)] {
            let ctx = egui::Context::default();
            let mut scene = CableScene::default();
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(500.0))),
                events: vec![
                    egui::Event::PointerMoved(pointer),
                    egui::Event::PointerButton {
                        pos: pointer,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                assert_eq!(scene.show(ui, &sim, &ports, view()), expected);
            });
            output.textures_delta.clear();
            assert!(
                scene.ethernet.grab.is_none(),
                "routed cables must keep their anchors fixed"
            );
        }
    }

    #[test]
    fn rendered_cable_anchors_remain_inside_sockets_after_scroll_and_resize() {
        let mut sim = NetworkSim::new();
        let mut ports = vec![];
        for unit in [1, 2] {
            let SimEvent::DeviceAdded(device) = sim
                .execute(Command::BuyDevice {
                    kind: DeviceTemplate::Switch,
                })
                .unwrap()[0]
            else {
                unreachable!()
            };
            sim.execute(Command::PlaceDevice {
                device,
                rack: RackId(1),
                unit,
            })
            .unwrap();
            ports.push(sim.device(device).unwrap().ports()[0]);
        }
        for supply in [CableSupply::CableBox305m, CableSupply::Rj45Pack20] {
            sim.execute(Command::BuyCableSupply { supply }).unwrap();
        }
        sim.execute(Command::Connect {
            a: ports[0],
            b: ports[1],
        })
        .unwrap();
        let link = sim.link_for_port(ports[0]).unwrap().id;
        let mut scene = CableScene::default();
        let ctx = egui::Context::default();
        let anchors = [egui::pos2(5.0, 8.0), egui::pos2(25.0, 18.0)];
        for (origin, scale) in [
            (egui::pos2(20.0, 30.0), 10.0),
            (egui::pos2(80.0, -50.0), 6.0),
        ] {
            let sockets: HashMap<_, _> = ports
                .iter()
                .zip(anchors)
                .map(|(id, anchor)| {
                    (
                        *id,
                        Rect::from_center_size(
                            origin + anchor.to_vec2() * scale,
                            Vec2::new(14.0, 12.0),
                        ),
                    )
                })
                .collect();
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                scene.show(
                    ui,
                    &sim,
                    &sockets,
                    CableView {
                        origin,
                        pixels_per_cm: scale,
                        floor_y: 500.0,
                        jacket: egui::TextureId::User(1),
                        plug: egui::TextureId::User(2),
                        selected: None,
                        visibility: CableVisibility::All,
                        anchors: vec![],
                        preview: None,
                        socket_rects: vec![],
                        interaction_enabled: true,
                    },
                );
            });
            // This headless geometry check does not upload GPU textures.
            output.textures_delta.clear();
            let rope = &scene.ethernet.ropes[&(link, 0)];
            assert!(rope.points[0].distance(anchors[0]) < 0.001);
            assert!(rope.points[SEGMENTS].distance(anchors[1]) < 0.001);
            for socket in sockets.values() {
                assert_eq!(plug_rect(*socket).center(), socket.center());
                assert!(plug_rect(*socket).height() <= 18.0);
            }
        }
    }

    #[test]
    fn gravity_sags_a_rope_with_fixed_endpoints_and_preserves_length() {
        let (a, b) = (egui::pos2(0.0, 0.0), egui::pos2(40.0, 0.0));
        let mut rope = Rope::new(a, b, 75.0);
        for _ in 0..1200 {
            rope.step(a, b, 100.0, None);
        }
        assert_eq!(rope.points[0], a);
        assert_eq!(rope.points[SEGMENTS], b);
        assert!(rope.points[SEGMENTS / 2].y > 20.0);
        let length: f32 = rope.points.windows(2).map(|p| p[0].distance(p[1])).sum();
        assert!((length - 75.0).abs() < 2.0, "length={length}");
    }
    #[test]
    fn rope_survives_drag_release_moved_anchors_and_floor_contact() {
        let (a, b) = (egui::pos2(0.0, 0.0), egui::pos2(25.0, 10.0));
        let mut rope = Rope::new(a, b, 100.0);
        for _ in 0..120 {
            rope.step(a, b, 40.0, Some((16, egui::pos2(10.0, 5.0))));
        }
        let moved = egui::pos2(55.0, 0.0);
        for _ in 0..900 {
            rope.step(a, moved, 40.0, None);
        }
        assert_eq!(rope.points[0], a);
        assert_eq!(rope.points[SEGMENTS], moved);
        assert!(
            rope.points
                .iter()
                .all(|p| p.x.is_finite() && p.y.is_finite() && p.y <= 40.001)
        );
        let length: f32 = rope.points.windows(2).map(|p| p[0].distance(p[1])).sum();
        assert!((length - 100.0).abs() < 3.0, "length={length}");
    }
}
