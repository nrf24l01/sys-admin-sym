use crate::app::UiAction;
use crate::localization::tr;
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::*;

pub(super) fn show(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    port: PortId,
    actions: &mut MessageWriter<UiAction>,
) {
    ui.strong(tr("ui.range-delivery-uplink"));
    ui.label(tr("ui.select-the-ipv4-ranges-delivered-through-this"));
    if ui.button(tr("ui.open-ip-ranges")).clicked() {
        actions.write(UiAction::OpenIpRanges);
    }
    if let Some(circuit) = sim.provider().circuit(port) {
        ui.label(crate::localization::tr_args(
            "ui.higher-network-gateway.2",
            &[(circuit.address).to_string(), (circuit.prefix).to_string()],
        ));
        for route in &circuit.routes {
            ui.label(crate::localization::tr_args(
                "ui.router-wan",
                &[(route.prefix).to_string(), (route.next_hop).to_string()],
            ));
        }
    } else {
        ui.weak(tr("ui.no-ranges-assigned-yet"));
    }
}
