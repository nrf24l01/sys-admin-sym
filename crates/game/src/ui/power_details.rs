use crate::app::UiAction;
use crate::localization::{tr, tr_args};
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::*;

pub(super) fn consumption(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    device: &Device,
    actions: &mut MessageWriter<UiAction>,
) {
    let Some(power) = sim.device_consumption(device.id) else {
        return;
    };
    ui.collapsing(tr("power.estimate"), |ui| {
        ui.label(tr_args(
            "power.current-peak",
            &[
                power.current.watts.to_string(),
                format!("{:.1}", f64::from(power.peak_mw) / 1000.0),
            ],
        ));
        ui.small(tr("power.estimate-note"));
        for part in &power.components {
            ui.horizontal(|ui| {
                ui.label(tr(&format!("power.component.{}", part.kind.key())));
                let value = if device.powered { part.milliwatts } else { 0 };
                ui.label(format!("{:.2} W", f64::from(value) / 1000.0));
            });
        }
        let utilization = if device.powered {
            power.utilization
        } else {
            DeviceWorkload::default()
        };
        let network = if device.powered {
            power.network_utilization
        } else {
            0
        };
        ui.label(tr_args(
            "power.utilization",
            &[
                format!("{:.1}", f64::from(utilization.cpu) / 10.0),
                format!("{:.1}", f64::from(utilization.memory) / 10.0),
                format!("{:.1}", f64::from(utilization.storage) / 10.0),
                format!("{:.1}", f64::from(network) / 10.0),
            ],
        ));
        if matches!(device.kind, DeviceKind::Server(_) | DeviceKind::Router(_)) {
            ui.separator();
            ui.label(tr("power.workload"));
            ui.small(tr("power.workload-note"));
            let draft = ui.make_persistent_id(("power-workload", device.id.0));
            let mut workload = ui
                .data(|data| data.get_temp::<DeviceWorkload>(draft))
                .unwrap_or_else(|| sim.device_workload(device.id));
            let mut commit = false;
            let mut dragging = false;
            for (key, value) in [
                ("cpu", &mut workload.cpu),
                ("memory", &mut workload.memory),
                ("storage", &mut workload.storage),
            ] {
                if matches!(device.kind, DeviceKind::Router(_)) && key != "cpu" {
                    continue;
                }
                let mut percent = *value / 10;
                let response = ui.add(
                    egui::Slider::new(&mut percent, 0..=100)
                        .text(tr(&format!("power.component.{key}")))
                        .suffix("%"),
                );
                if response.changed() {
                    *value = percent * 10;
                }
                dragging |= response.dragged();
                commit |= response.drag_stopped() || (response.changed() && !response.dragged());
            }
            if ui.button(tr("power.clear-workload")).clicked() {
                workload = DeviceWorkload::default();
                commit = true;
            }
            ui.data_mut(|data| {
                if dragging {
                    data.insert_temp(draft, workload);
                } else {
                    data.remove::<DeviceWorkload>(draft);
                }
            });
            if commit {
                actions.write(UiAction::NetworkCommand(Command::SetDeviceWorkload {
                    device: device.id,
                    workload,
                }));
            }
        }
    });
}
