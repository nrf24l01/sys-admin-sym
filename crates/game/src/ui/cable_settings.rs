use crate::app::{UiAction, UiState};
use crate::localization::tr;
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::{Command, NetworkSim};

pub(super) fn show(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    let mut settings = sim.cable_settings();
    ui.add_enabled_ui(state.pending_assembly.is_none(), |ui| {
        let mut automatic = state.cable_length_cm.is_none();
        if ui
            .checkbox(&mut automatic, tr("cable.auto-length"))
            .changed()
        {
            state.cable_length_cm = if automatic { None } else { Some(100) };
        }
        if let Some(cm) = &mut state.cable_length_cm {
            ui.horizontal(|ui| {
                ui.label(tr("ui.cut-length"));
                meters(ui, cm, 1);
            });
        } else {
            ui.horizontal(|ui| {
                ui.label(tr("cable.extra-percent"));
                ui.add(
                    egui::DragValue::new(&mut settings.extra_percent)
                        .range(0..=100)
                        .suffix("%"),
                );
            })
            .response
            .on_hover_text(tr("cable.extra-help"));
            ui.horizontal(|ui| {
                ui.label(tr("cable.extra-length"));
                meters(ui, &mut settings.extra_cm, 0);
            })
            .response
            .on_hover_text(tr("cable.extra-help"));
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(
                        settings.extra_percent == 0 && settings.extra_cm == 0,
                        tr("cable.minimum-preset"),
                    )
                    .clicked()
                {
                    settings.extra_percent = 0;
                    settings.extra_cm = 0;
                }
                if ui
                    .selectable_label(
                        settings.extra_percent == 10 && settings.extra_cm == 50,
                        tr("cable.service-preset"),
                    )
                    .on_hover_text(tr("cable.service-preset-help"))
                    .clicked()
                {
                    settings.extra_percent = 10;
                    settings.extra_cm = 50;
                }
            });
            ui.checkbox(&mut settings.reuse_longer_leads, tr("cable.prefer-reuse"))
                .on_hover_text(tr("cable.reuse-help"));
            if settings.extra_percent > 0 || settings.extra_cm > 0 {
                ui.weak(crate::localization::tr_args(
                    "cable.allowance-summary",
                    &[
                        settings.extra_percent.to_string(),
                        format!("{:.2}", settings.extra_cm as f64 / 100.0),
                    ],
                ));
            }
        }
    });
    if settings != sim.cable_settings() {
        actions.write(UiAction::NetworkCommand(Command::SetCableSettings {
            settings,
        }));
    }
}

fn meters(ui: &mut egui::Ui, cm: &mut u32, minimum: u32) {
    ui.add(
        egui::DragValue::new(cm)
            .range(minimum..=10_000)
            .speed(5)
            .custom_formatter(|value, _| format!("{:.2}", value / 100.0))
            .custom_parser(parse_meters)
            .suffix(tr("cable.meters")),
    );
}

fn parse_meters(text: &str) -> Option<f64> {
    let meters: f64 = text.trim().replace(',', ".").parse().ok()?;
    (meters.is_finite() && (0.0..=100.0).contains(&meters)).then(|| (meters * 100.0).round())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_input_accepts_local_decimal_separators_and_rounds_to_centimeters() {
        assert_eq!(parse_meters(" 1,25 "), Some(125.0));
        assert_eq!(parse_meters("0.506"), Some(51.0));
        assert_eq!(parse_meters("100"), Some(10_000.0));
        for invalid in ["NaN", "inf", "-1", "100.01", "", "1.2.3"] {
            assert_eq!(parse_meters(invalid), None, "{invalid}");
        }
    }
}
