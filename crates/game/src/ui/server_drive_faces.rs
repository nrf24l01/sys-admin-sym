//! Inventory controls every visible drive carrier, including shop and placement previews.
use bevy_egui::egui;
use cloud_provider_sim::{DriveKind, ServerModel, drive_catalog};

pub(super) const EMPTY_BAY_COLOR: egui::Color32 = egui::Color32::from_rgb(9, 12, 14);

pub(super) fn paint(
    painter: &egui::Painter,
    panel: egui::Rect,
    model: &ServerModel,
    drives: &[Option<String>],
    powered: bool,
) {
    for (bay, slot) in model.chassis.drive_bays.iter().enumerate() {
        let [left, top, right, bottom] = slot.face_rect;
        let rect = egui::Rect::from_min_max(
            egui::pos2(
                panel.left() + panel.width() * left,
                panel.top() + panel.height() * top,
            ),
            egui::pos2(
                panel.left() + panel.width() * right,
                panel.top() + panel.height() * bottom,
            ),
        );
        // Opaque cavities also erase carriers in unretouched source photographs.
        painter.rect_filled(rect, 1.0, EMPTY_BAY_COLOR);
        let edge = egui::Color32::from_gray(47);
        painter.rect_stroke(
            rect,
            1.0,
            egui::Stroke::new(0.6, edge),
            egui::StrokeKind::Inside,
        );
        let Some(id) = drives.get(bay).and_then(Option::as_deref) else {
            continue;
        };
        let portrait = rect.height() > rect.width();
        let carrier = rect.shrink(1.0);
        painter.rect_filled(carrier, 1.0, egui::Color32::from_rgb(43, 48, 52));
        painter.rect_stroke(
            carrier,
            1.0,
            egui::Stroke::new(0.8, egui::Color32::from_gray(110)),
            egui::StrokeKind::Inside,
        );
        let handle = if portrait {
            egui::Rect::from_min_max(
                carrier.left_top() + egui::vec2(carrier.width() * 0.2, carrier.height() * 0.70),
                carrier.right_bottom() - egui::vec2(carrier.width() * 0.2, carrier.height() * 0.10),
            )
        } else {
            egui::Rect::from_min_max(
                carrier.left_top() + egui::vec2(carrier.width() * 0.08, carrier.height() * 0.25),
                carrier.right_bottom() - egui::vec2(carrier.width() * 0.2, carrier.height() * 0.25),
            )
        };
        painter.rect_filled(handle, 1.0, egui::Color32::from_gray(23));
        let led_color = if powered {
            egui::Color32::from_rgb(77, 171, 95)
        } else {
            egui::Color32::from_gray(65)
        };
        painter.circle_filled(carrier.right_top() + egui::vec2(-2.0, 3.0), 1.0, led_color);
        if rect.height() >= 16.0 && rect.width() >= 12.0 {
            let key = drive_catalog()
                .drives
                .iter()
                .find(|drive| drive.id == id)
                .map_or("ui.drive", |drive| {
                    if drive.kind == DriveKind::Ssd {
                        "ui.ssd"
                    } else {
                        "ui.hdd"
                    }
                });
            let badge = egui::Rect::from_min_size(
                rect.left_bottom() - egui::vec2(0.0, 10.0),
                egui::vec2(rect.width(), 10.0),
            );
            painter.rect_filled(badge, 0.0, egui::Color32::from_black_alpha(210));
            painter.text(
                badge.center(),
                egui::Align2::CENTER_CENTER,
                crate::localization::tr(key),
                egui::FontId::monospace(7.0),
                egui::Color32::from_rgb(175, 214, 216),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloud_provider_sim::{
        Command, DeviceKind, NetworkSim, ServerConfiguration, SimEvent, server_catalog,
    };

    fn carrier_count(model: &ServerModel, drives: &[Option<String>]) -> usize {
        let context = egui::Context::default();
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            paint(
                ui.painter(),
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1500.0, 200.0)),
                model,
                drives,
                false,
            );
        });
        let count = output
            .shapes
            .iter()
            .filter(|s| {
                matches!(&s.shape,
                    egui::Shape::Rect(rect) if rect.fill == egui::Color32::from_rgb(43,48,52)
                )
            })
            .count();
        output.textures_delta.clear();
        count
    }

    #[test]
    fn every_model_shows_carriers_only_until_its_drive_is_removed() {
        for model in &server_catalog().models {
            let mut sim = NetworkSim::new();
            sim.money = 100_000;
            let SimEvent::DeviceAdded(device) = sim
                .execute(Command::BuyServer {
                    model_id: model.id.clone(),
                    configuration: ServerConfiguration::Chassis,
                })
                .unwrap()[0]
            else {
                panic!()
            };
            let count = |sim: &NetworkSim| {
                let DeviceKind::Server(server) = &sim.device(device).unwrap().kind else {
                    panic!()
                };
                carrier_count(model, &server.hardware.as_ref().unwrap().drives)
            };
            assert_eq!(count(&sim), 0, "{} bare chassis", model.id);
            let bay = model.chassis.drive_bays.len() - 1;
            sim.execute(Command::BuyDrive {
                drive_id: "enterprise_ssd_960gb".into(),
            })
            .unwrap();
            sim.execute(Command::InstallDrive {
                device,
                drive_id: "enterprise_ssd_960gb".into(),
                bay: Some(bay),
            })
            .unwrap();
            assert_eq!(count(&sim), 1, "{} installed drive", model.id);
            sim.execute(Command::RemoveDrive { device, bay }).unwrap();
            assert_eq!(count(&sim), 0, "{} removed drive", model.id);
        }
    }
}
