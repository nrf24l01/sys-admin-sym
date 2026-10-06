use crate::app::UiAction;
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::*;

pub(super) fn show(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    port: PortId,
    actions: &mut MessageWriter<UiAction>,
) {
    ui.strong("Range delivery uplink");
    ui.label("Select the IPv4 ranges delivered through this socket in IP RANGES.");
    if ui.button("Open IP ranges…").clicked() {
        actions.write(UiAction::OpenIpRanges);
    }
    if let Some(circuit) = sim.provider().circuit(port) {
        ui.label(format!(
            "Higher network gateway: {}/{}",
            circuit.address, circuit.prefix
        ));
        for route in &circuit.routes {
            ui.label(format!("{} → router WAN {}", route.prefix, route.next_hop));
        }
    } else {
        ui.weak("No ranges assigned yet.");
    }
}
