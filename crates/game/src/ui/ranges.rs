use crate::app::{UiAction, UiState};
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::*;

pub(super) fn show(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    if !state.ranges_open {
        return;
    }
    let pools = sim.provider().pools();
    if !pools.iter().any(|p| Some(p.prefix) == state.selected_range) {
        state.selected_range = pools.first().map(|p| p.prefix);
        state.range_loaded_for = None;
    }
    let mut uplinks: Vec<_> = sim
        .network_outlets()
        .filter(|o| matches!(o.kind, NetworkOutletKind::Uplink { .. }))
        .collect();
    uplinks.sort_by_key(|o| o.port);
    let mut open = true;
    egui::Window::new("IP ranges")
        .id(egui::Id::new("ip-ranges-window"))
        .open(&mut open)
        .default_size(egui::vec2(820.0, 460.0))
        .show(viewport, |ui| {
            ui.label("Choose where the carrier delivers each range, then configure your router using the addresses below.");
            ui.horizontal(|ui| {
                if ui.add_enabled(sim.money >= PublicIpv4Block::PRICE && sim.public_ipv4_blocks().len() < 32, egui::Button::new(format!("Order /29 range · ${}", PublicIpv4Block::PRICE))).clicked() {
                    actions.write(UiAction::BuyPublicIpv4Pool);
                }
                ui.weak(format!("{} range(s) owned", pools.len()));
            });
            ui.separator();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(180.0);
                    ui.strong("IPv4 ranges");
                    egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                        for pool in pools {
                            if ui.selectable_label(state.selected_range == Some(pool.prefix), pool.prefix.to_string()).clicked() {
                                state.selected_range = Some(pool.prefix);
                            }
                        }
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_min_width(480.0);
                    let Some(prefix) = state.selected_range else {
                        ui.label("Order a range to see its addressing and uplink settings.");
                        return;
                    };
                    if state.range_loaded_for != Some(prefix) {
                        state.range_uplink = sim.range_uplink(prefix).or_else(|| uplinks.first().map(|o| o.port));
                        state.range_loaded_for = Some(prefix);
                    }
                    ui.heading(prefix.to_string());
                    let assignment = sim.range_uplink(prefix);
                    ui.label(assignment.and_then(|id| sim.network_outlet(id)).map_or_else(|| "Delivery: unassigned".into(), |o| format!("Delivery: {}", uplink_label(sim, o.port))));
                    ui.label("Deliver this range through");
                    egui::ComboBox::from_id_salt("range-uplink")
                        .selected_text(state.range_uplink.and_then(|id| sim.network_outlet(id)).map_or_else(|| "Select uplink".into(), |o| uplink_label(sim, o.port)))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut state.range_uplink, None, "Unassigned");
                            for outlet in &uplinks {
                                ui.selectable_value(&mut state.range_uplink, Some(outlet.port), uplink_label(sim, outlet.port));
                            }
                        });
                    if ui.button("Apply range delivery").clicked() {
                        actions.write(UiAction::NetworkCommand(Command::Provider(ProviderCommand::RouteOwnedRange { prefix, uplink: state.range_uplink })));
                    }
                    let Some(uplink) = state.range_uplink else {
                        ui.weak("Select an uplink to view the addressing instructions.");
                        return;
                    };
                    let handoff = match sim.range_handoff(prefix, uplink) {
                        Ok(handoff) => handoff,
                        Err(error) => { ui.colored_label(egui::Color32::LIGHT_RED, error.to_string()); return; }
                    };
                    ui.separator();
                    ui.strong("Higher network · router WAN interface");
                    egui::Grid::new("range-wan-details").num_columns(2).show(ui, |ui| {
                        detail(ui, "Higher network subnet", handoff.upstream_subnet);
                        detail(ui, "Higher network gateway", handoff.upstream_gateway);
                        detail(ui, "Your router WAN IPv4", format!("{}/{}", handoff.router_address, handoff.upstream_subnet.length()));
                        detail(ui, "Router default route", format!("0.0.0.0/0 via {}", handoff.upstream_gateway));
                    });
                    ui.weak("Cable this uplink to your router. On that interface, configure the WAN IPv4 above and add a default route through the higher network gateway.");
                    ui.separator();
                    ui.strong("Your range · router LAN interface");
                    egui::Grid::new("range-lan-details").num_columns(2).show(ui, |ui| {
                        detail(ui, "IPv4 range", prefix);
                        detail(ui, "Router LAN / server gateway", format!("{}/{}", handoff.local_gateway, prefix.length()));
                    });
                    ui.weak("Configure the range gateway on another router interface. Connect your servers through that interface or a switch; give them addresses in this range and use this LAN gateway.");
                    ui.weak("Applying delivery updates the carrier route. Configure the router ports and routing table yourself.");
                });
            });
        });
    state.ranges_open = open;
}

fn uplink_label(sim: &NetworkSim, port: PortId) -> String {
    sim.port(port)
        .map_or_else(|| format!("Uplink socket {}", port.0), |p| p.name.clone())
}

fn detail(ui: &mut egui::Ui, label: &str, value: impl std::fmt::Display) {
    ui.label(label);
    ui.monospace(value.to_string());
    ui.end_row();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        ecs::system::SystemState,
        prelude::{Messages, World},
    };

    #[test]
    fn range_menu_displays_the_selected_handoff_and_applies_delivery() {
        let mut sim = NetworkSim::new();
        let block = sim.buy_public_ipv4_pool().unwrap();
        let range = Ipv4Prefix::new(block.network, PublicIpv4Block::PREFIX).unwrap();
        let mut uplinks: Vec<_> = sim
            .network_outlets()
            .filter(|o| matches!(o.kind, NetworkOutletKind::Uplink { .. }))
            .map(|o| o.port)
            .collect();
        uplinks.sort();
        let mut state = UiState {
            ranges_open: true,
            selected_range: Some(range),
            range_loaded_for: Some(range),
            range_uplink: Some(uplinks[1]),
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut system = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut position = None;
        let mut text = Vec::new();
        for _ in 0..2 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 900.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    show(
                        ui,
                        &sim,
                        &mut state,
                        &mut system.get_mut(&mut world).unwrap(),
                    )
                },
            );
            text = output
                .shapes
                .iter()
                .filter_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape {
                        Some(text.galley.text().to_string())
                    } else {
                        None
                    }
                })
                .collect();
            position = output.shapes.iter().find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.text() == "Apply range delivery"
                {
                    Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center())
                } else {
                    None
                }
            });
            output.textures_delta.clear();
        }
        assert!(
            text.iter().any(|s| s == "192.0.2.5"),
            "rendered text: {text:?}"
        );
        assert!(text.iter().any(|s| s == "192.0.2.6/30"));
        assert!(text.iter().any(|s| s == "203.0.113.1/29"));
        let position = position.expect("range delivery apply button is visible");
        for pressed in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| {
                    show(
                        ui,
                        &sim,
                        &mut state,
                        &mut system.get_mut(&mut world).unwrap(),
                    )
                },
            );
            output.textures_delta.clear();
        }
        let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        assert_eq!(actions.len(), 1);
        let UiAction::NetworkCommand(command) = &actions[0] else {
            panic!("expected typed range delivery command")
        };
        sim.execute(command.clone()).unwrap();
        assert_eq!(sim.range_uplink(range), Some(uplinks[1]));
        assert_eq!(
            sim.provider().circuit(uplinks[1]).unwrap().routes[0]
                .next_hop
                .to_string(),
            "192.0.2.6"
        );
    }
}
