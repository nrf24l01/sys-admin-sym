use bevy_egui::egui::{self, Color32, Pos2, Rect, Vec2};
use cloud_provider_sim::{LinkId, NetworkSim, PortId};
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

    fn step(&mut self, a: Pos2, b: Pos2, floor: f32, grab: Option<(usize, Pos2)>) {
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
                let pinned_a = i == 0 || grab.is_some_and(|(index, _)| index == i);
                let pinned_b = i + 1 == SEGMENTS || grab.is_some_and(|(index, _)| index == i + 1);
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
}

#[derive(Default)]
pub(super) struct CableScene {
    ropes: HashMap<LinkId, Rope>,
    accumulator: f32,
    grab: Option<(LinkId, usize)>,
}

pub(super) struct CableView {
    pub origin: Pos2,
    pub pixels_per_cm: f32,
    pub floor_y: f32,
    pub jacket: egui::TextureId,
    pub plug: egui::TextureId,
    pub selected: Option<LinkId>,
}

impl CableScene {
    /// Returns the clicked cable; dragging changes only its transient shape.
    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        sim: &NetworkSim,
        ports: &HashMap<PortId, Rect>,
        view: CableView,
    ) -> Option<LinkId> {
        self.ropes.retain(|id, _| {
            sim.link(*id)
                .is_some_and(|link| ports.contains_key(&link.a) && ports.contains_key(&link.b))
        });
        if self
            .grab
            .is_some_and(|(id, _)| !self.ropes.contains_key(&id))
        {
            self.grab = None;
        }
        let local = |p: Pos2| Pos2::ZERO + (p - view.origin) / view.pixels_per_cm;
        let screen = |p: Pos2| view.origin + p.to_vec2() * view.pixels_per_cm;
        // End-on projection: the seated plug and its cable outlet share the
        // socket center. Only the flexible jacket is affected by gravity.
        let exit = |rect: Rect| rect.center();
        let mut links: Vec<_> = sim
            .links()
            .filter(|link| ports.contains_key(&link.a) && ports.contains_key(&link.b))
            .collect();
        links.sort_by_key(|l| l.id);
        for link in &links {
            let (a, b) = (local(exit(ports[&link.a])), local(exit(ports[&link.b])));
            let rope = self
                .ropes
                .entry(link.id)
                .or_insert_with(|| Rope::new(a, b, link.length_cm as f32));
            if rope.length != link.length_cm as f32 {
                *rope = Rope::new(a, b, link.length_cm as f32);
            }
            rope.pin_endpoints(a, b);
        }
        let (pointer, pressed, down, dt) = ui.input(|i| {
            (
                i.pointer.interact_pos(),
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.stable_dt,
            )
        });
        let mut selected = None;
        if !down {
            self.grab = None;
        }
        if let Some(pointer) = pointer.filter(|p| ui.clip_rect().contains(*p)) {
            // Socket interactions keep priority, including plugs over their sockets.
            if !ports.values().any(|r| r.expand(3.0).contains(pointer)) {
                let nearest = links
                    .iter()
                    .filter_map(|link| {
                        let rope = &self.ropes[&link.id];
                        rope.points
                            .windows(2)
                            .enumerate()
                            .map(|(i, pair)| {
                                (
                                    link.id,
                                    (i + 1).clamp(1, SEGMENTS - 1),
                                    distance_to_segment(pointer, screen(pair[0]), screen(pair[1])),
                                )
                            })
                            .min_by(|a, b| a.2.total_cmp(&b.2))
                    })
                    .filter(|(_, _, distance)| *distance < 9.0)
                    .min_by(|a, b| a.2.total_cmp(&b.2));
                if let Some((id, index, _)) = nearest {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    if pressed {
                        self.grab = Some((id, index));
                        selected = Some(id);
                    }
                }
            }
        }
        let floor = (view.floor_y - view.origin.y) / view.pixels_per_cm;
        self.accumulator = (self.accumulator + dt.clamp(0.0, 0.066)).min(STEP * 8.0);
        while self.accumulator >= STEP {
            for link in &links {
                let (a, b) = (local(exit(ports[&link.a])), local(exit(ports[&link.b])));
                let grab = self
                    .grab
                    .filter(|(id, _)| *id == link.id)
                    .and_then(|(_, i)| {
                        pointer.map(|p| {
                            // Keep the grabbed point reachable from both anchored ends.
                            let rope_length = self.ropes[&link.id].length;
                            let mut target = local(p);
                            for _ in 0..4 {
                                for (anchor, reach) in [
                                    (a, rope_length * i as f32 / SEGMENTS as f32),
                                    (b, rope_length * (SEGMENTS - i) as f32 / SEGMENTS as f32),
                                ] {
                                    let delta = target - anchor;
                                    if delta.length() > reach {
                                        target = anchor + delta.normalized() * reach;
                                    }
                                }
                            }
                            target.y = target.y.min(floor);
                            (i, target)
                        })
                    });
                self.ropes
                    .get_mut(&link.id)
                    .unwrap()
                    .step(a, b, floor, grab);
            }
            self.accumulator -= STEP;
        }
        if self.grab.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        // Seat every connector first so all cable jackets pass in front of them,
        // including cables crossing another lead's connector.
        for link in &links {
            for id in [link.a, link.b] {
                let socket = ports[&id];
                ui.painter().image(
                    view.plug,
                    plug_rect(socket),
                    Rect::from_min_max(egui::pos2(0.10, 0.17), egui::pos2(0.90, 1.0)),
                    Color32::WHITE,
                );
                let width = (view.pixels_per_cm * 0.55).clamp(4.0, 7.5);
                ui.painter()
                    .circle_filled(socket.center(), width * 0.65, Color32::from_gray(18));
            }
        }
        for link in &links {
            let path: Vec<_> = self.ropes[&link.id]
                .points
                .iter()
                .map(|p| screen(*p))
                .collect();
            let width = (view.pixels_per_cm * 0.55).clamp(4.0, 7.5);
            let color = super::cable_color(link.id);
            let shadow: Vec<_> = path.iter().map(|p| *p + Vec2::new(2.0, 3.0)).collect();
            ui.painter().add(egui::Shape::line(
                shadow,
                egui::Stroke::new(width + 3.0, Color32::from_black_alpha(150)),
            ));
            if view.selected == Some(link.id) {
                ui.painter().add(egui::Shape::line(
                    path.clone(),
                    egui::Stroke::new(width + 3.0, Color32::from_rgb(255, 196, 64)),
                ));
            }
            ui.painter().add(egui::Shape::line(
                path.clone(),
                egui::Stroke::new(width, color),
            ));
            paint_jacket(ui.painter(), &path, width, view.jacket);
        }
        if !links.is_empty() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(16));
        }
        selected
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

fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let delta = b - a;
    let t = ((p - a).dot(delta) / delta.length_sq().max(0.0001)).clamp(0.0, 1.0);
    p.distance(a + delta * t)
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
    use cloud_provider_sim::{CableSupply, Command, DeviceTemplate, RackId, SimEvent};

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
                    },
                );
            });
            // This headless geometry check does not upload GPU textures.
            output.textures_delta.clear();
            let rope = &scene.ropes[&link];
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
