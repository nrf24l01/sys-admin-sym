use bevy_egui::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use cloud_provider_sim::{RACK_FACE_WIDTH_CM, RACK_GAP_CM};

const FACE_ASPECT: f32 = 19.0 / 1.75;
const LABEL_WIDTH: f32 = 22.0;
const CABLE_RAIL_WIDTH: f32 = 26.0;
const MOUNT_WIDTH: f32 = 20.0;
const SIDE_WIDTH: f32 = LABEL_WIDTH + CABLE_RAIL_WIDTH + MOUNT_WIDTH;

/// One geometry source for the cabinet, equipment, screw holes, and cable anchors.
pub(super) struct RackLayout {
    pub width: f32,
    pub face_width: f32,
    pub row_height: f32,
    gap: f32,
}

pub(super) struct RackRow {
    pub face: Rect,
    pub mounts: [Rect; 2],
    pub cable_rails: [Rect; 2],
    pub anchors: [Pos2; 2],
    labels: [Pos2; 2],
}

impl RackLayout {
    pub fn new(available_width: f32) -> Self {
        let face_height = ((available_width - SIDE_WIDTH * 2.0) / FACE_ASPECT).clamp(30.0, 64.0);
        let face_width = face_height * FACE_ASPECT;
        let gap = RACK_GAP_CM * face_width / RACK_FACE_WIDTH_CM;
        Self {
            width: face_width + SIDE_WIDTH * 2.0,
            face_width,
            row_height: face_height + gap,
            gap,
        }
    }

    pub fn row(&self, rect: Rect) -> RackRow {
        let face = Rect::from_min_max(
            egui::pos2(rect.left() + SIDE_WIDTH, rect.top() + self.gap * 0.5),
            egui::pos2(rect.right() - SIDE_WIDTH, rect.bottom() - self.gap * 0.5),
        );
        let strip = |left, width| {
            Rect::from_min_size(
                egui::pos2(left, rect.top()),
                Vec2::new(width, rect.height()),
            )
        };
        let mounts = [
            strip(face.left() - MOUNT_WIDTH, MOUNT_WIDTH),
            strip(face.right(), MOUNT_WIDTH),
        ];
        let cable_rails = [
            strip(mounts[0].left() - CABLE_RAIL_WIDTH, CABLE_RAIL_WIDTH),
            strip(mounts[1].right(), CABLE_RAIL_WIDTH),
        ];
        RackRow {
            face,
            mounts,
            anchors: cable_rails.map(|rail| rail.center()),
            cable_rails,
            labels: [
                egui::pos2(rect.left() + LABEL_WIDTH * 0.5, rect.center().y),
                egui::pos2(rect.right() - LABEL_WIDTH * 0.5, rect.center().y),
            ],
        }
    }

    pub fn paint_crossbar(&self, ui: &mut egui::Ui, label: &str) {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(self.width, 28.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 2.0, Color32::from_rgb(38, 43, 48));
        painter.line_segment(
            [rect.left_top(), rect.right_top()],
            Stroke::new(1.0, Color32::from_gray(83)),
        );
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(2.0, Color32::from_gray(7)),
        );
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::monospace(11.0),
            Color32::from_gray(159),
        );
        for x in [
            rect.left() + LABEL_WIDTH + CABLE_RAIL_WIDTH * 0.5,
            rect.right() - LABEL_WIDTH - CABLE_RAIL_WIDTH * 0.5,
        ] {
            paint_screw(painter, egui::pos2(x, rect.center().y));
        }
    }
}

impl RackRow {
    pub fn paint(&self, painter: &egui::Painter, unit: u8, occupied: bool) {
        for label in self.labels {
            painter.text(
                label,
                egui::Align2::CENTER_CENTER,
                format!("{unit:02}"),
                egui::FontId::monospace(10.0),
                Color32::from_gray(135),
            );
        }
        for rail in self.cable_rails {
            painter.rect_filled(rail, 0.0, Color32::from_rgb(29, 34, 39));
            painter.line_segment(
                [rail.left_top(), rail.left_bottom()],
                Stroke::new(1.0, Color32::from_gray(62)),
            );
            painter.line_segment(
                [rail.right_top(), rail.right_bottom()],
                Stroke::new(2.0, Color32::from_gray(12)),
            );
            // Recessed tie-down slot. Its center is also the interactive route point.
            let slot = Rect::from_center_size(rail.center(), Vec2::new(12.0, 17.0));
            painter.rect_filled(slot, 3.0, Color32::from_gray(9));
            painter.rect_stroke(
                slot,
                3.0,
                Stroke::new(1.0, Color32::from_gray(78)),
                egui::StrokeKind::Inside,
            );
        }
        for mount in self.mounts {
            painter.rect_filled(mount, 0.0, Color32::from_rgb(63, 68, 73));
            painter.line_segment(
                [mount.left_top(), mount.left_bottom()],
                Stroke::new(1.0, Color32::from_gray(105)),
            );
            painter.line_segment(
                [mount.right_top(), mount.right_bottom()],
                Stroke::new(1.0, Color32::from_gray(22)),
            );
            for fraction in [1.0 / 7.0, 0.5, 6.0 / 7.0] {
                let center = egui::pos2(mount.center().x, mount.top() + mount.height() * fraction);
                let hole = Rect::from_center_size(
                    center,
                    Vec2::splat((mount.height() * 0.15).clamp(4.0, 7.0)),
                );
                painter.rect_filled(hole, 0.5, Color32::from_gray(8));
                painter.rect_stroke(
                    hole,
                    0.5,
                    Stroke::new(1.0, Color32::from_gray(110)),
                    egui::StrokeKind::Outside,
                );
            }
            if occupied {
                let ear = Rect::from_min_max(
                    egui::pos2(mount.left() + 1.0, self.face.top()),
                    egui::pos2(mount.right() - 1.0, self.face.bottom()),
                );
                painter.rect_filled(ear, 1.0, Color32::from_rgb(47, 52, 57));
                painter.rect_stroke(
                    ear,
                    1.0,
                    Stroke::new(1.0, Color32::from_gray(89)),
                    egui::StrokeKind::Inside,
                );
                paint_screw(painter, ear.center());
            }
        }
    }
}

fn paint_screw(painter: &egui::Painter, center: Pos2) {
    painter.circle_filled(center + Vec2::new(0.0, 1.0), 4.8, Color32::from_gray(10));
    painter.circle_filled(center, 3.7, Color32::from_gray(144));
    painter.circle_stroke(center, 3.7, Stroke::new(0.8, Color32::from_gray(205)));
    for delta in [Vec2::new(2.2, 0.0), Vec2::new(0.0, 2.2)] {
        painter.line_segment(
            [center - delta, center + delta],
            Stroke::new(1.1, Color32::from_gray(40)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rails_and_anchors_remain_symmetric_and_outside_equipment() {
        for available in [320.0, 640.0, 960.0] {
            let layout = RackLayout::new(available);
            for origin in [Pos2::ZERO, egui::pos2(110.0, -280.0)] {
                let rect = Rect::from_min_size(origin, Vec2::new(layout.width, layout.row_height));
                let row = layout.row(rect);
                assert!((row.face.width() / row.face.height() - FACE_ASPECT).abs() < 0.001);
                assert!((row.face.center().x - rect.center().x).abs() < 0.001);
                for i in 0..2 {
                    assert!(row.cable_rails[i].contains(row.anchors[i]));
                    assert!(!row.face.expand(7.0).contains(row.anchors[i]));
                    assert!(!row.mounts[i].contains(row.anchors[i]));
                    assert!(rect.contains_rect(row.mounts[i]));
                }
                let next = layout.row(rect.translate(Vec2::new(0.0, layout.row_height)));
                assert!((next.mounts[0].top() - row.mounts[0].bottom()).abs() < 0.001);
            }
        }
    }
}
