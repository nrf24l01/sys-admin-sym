use crate::app::{UiAction, UiState};
use crate::localization::tr;
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
    egui::Window::new(tr("ranges.title"))
        .id(egui::Id::new("ip-ranges-window"))
        .open(&mut open)
        .default_size(egui::vec2(820.0, 460.0))
        .show(viewport, |ui| {
            ui.label(tr("ui.choose-where-the-carrier-delivers-each-range"));
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        sim.money >= PublicIpv4Block::PRICE && sim.public_ipv4_blocks().len() < 32,
                        egui::Button::new(crate::localization::tr_args(
                            "ui.order-29-range",
                            &[(PublicIpv4Block::PRICE).to_string()],
                        )),
                    )
                    .clicked()
                {
                    actions.write(UiAction::BuyPublicIpv4Pool);
                }
                ui.weak(crate::localization::tr_args(
                    "ui.range-s-owned",
                    &[(pools.len()).to_string()],
                ));
            });
            ui.separator();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(180.0);
                    ui.strong(tr("ranges.heading"));
                    egui::ScrollArea::vertical()
                        .max_height(380.0)
                        .show(ui, |ui| {
                            for pool in pools {
                                if ui
                                    .selectable_label(
                                        state.selected_range == Some(pool.prefix),
                                        pool.prefix.to_string(),
                                    )
                                    .clicked()
                                {
                                    state.selected_range = Some(pool.prefix);
                                }
                            }
                        });
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_min_width(480.0);
                    let Some(prefix) = state.selected_range else {
                        ui.label(tr("ui.order-a-range-to-see-its-addressing"));
                        return;
                    };
                    if state.range_loaded_for != Some(prefix) {
                        state.range_uplink = sim
                            .range_uplink(prefix)
                            .or_else(|| uplinks.first().map(|o| o.port));
                        state.range_loaded_for = Some(prefix);
                    }
                    ui.heading(prefix.to_string());
                    let assignment = sim.range_uplink(prefix);
                    ui.label(
                        assignment
                            .and_then(|id| sim.network_outlet(id))
                            .map_or_else(
                                || tr("ui.delivery-unassigned"),
                                |o| {
                                    crate::localization::tr_args(
                                        "ui.delivery.2",
                                        &[(uplink_label(sim, o.port)).to_string()],
                                    )
                                },
                            ),
                    );
                    ui.label(tr("ui.deliver-this-range-through"));
                    egui::ComboBox::from_id_salt("range-uplink")
                        .selected_text(
                            state
                                .range_uplink
                                .and_then(|id| sim.network_outlet(id))
                                .map_or_else(
                                    || tr("ui.select-uplink"),
                                    |o| uplink_label(sim, o.port),
                                ),
                        )
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut state.range_uplink, None, tr("ui.unassigned"));
                            for outlet in &uplinks {
                                ui.selectable_value(
                                    &mut state.range_uplink,
                                    Some(outlet.port),
                                    uplink_label(sim, outlet.port),
                                );
                            }
                        });
                    if ui.button(tr("ui.apply-range-delivery")).clicked() {
                        actions.write(UiAction::NetworkCommand(Command::Provider(
                            ProviderCommand::RouteOwnedRange {
                                prefix,
                                uplink: state.range_uplink,
                            },
                        )));
                    }
                    let Some(uplink) = state.range_uplink else {
                        ui.weak(tr("ui.select-an-uplink-to-view-the-addressing"));
                        return;
                    };
                    let handoff = match sim.range_handoff(prefix, uplink) {
                        Ok(handoff) => handoff,
                        Err(error) => {
                            ui.colored_label(
                                egui::Color32::LIGHT_RED,
                                crate::localization::UiMessage::from(error).render(),
                            );
                            return;
                        }
                    };
                    ui.separator();
                    ui.strong(tr("ui.higher-network-router-wan-interface"));
                    egui::Grid::new("range-wan-details")
                        .num_columns(2)
                        .show(ui, |ui| {
                            detail(ui, "ui.higher-network-subnet", handoff.upstream_subnet);
                            detail(ui, "ui.higher-network-gateway", handoff.upstream_gateway);
                            detail(
                                ui,
                                "ui.your-router-wan-ipv4",
                                crate::localization::tr_args(
                                    "network.prefix",
                                    &[
                                        (handoff.router_address).to_string(),
                                        (handoff.upstream_subnet.length()).to_string(),
                                    ],
                                ),
                            );
                            detail(
                                ui,
                                "ui.router-default-route",
                                crate::localization::tr_args(
                                    "ui.0-0-0-0-0-via",
                                    &[(handoff.upstream_gateway).to_string()],
                                ),
                            );
                        });
                    ui.weak(tr("ui.cable-this-uplink-to-your-router-on"));
                    ui.separator();
                    ui.strong(tr("ui.your-range-router-lan-interface"));
                    egui::Grid::new("range-lan-details")
                        .num_columns(2)
                        .show(ui, |ui| {
                            detail(ui, "ui.ipv4-range", prefix);
                            detail(
                                ui,
                                "ui.router-lan-server-gateway",
                                crate::localization::tr_args(
                                    "network.prefix",
                                    &[
                                        (handoff.local_gateway).to_string(),
                                        (prefix.length()).to_string(),
                                    ],
                                ),
                            );
                        });
                    ui.weak(tr("ui.configure-the-range-gateway-on-another-router"));
                    ui.weak(tr(
                        "ui.applying-delivery-updates-the-carrier-route-configure",
                    ));
                });
            });
        });
    state.ranges_open = open;
}

fn uplink_label(sim: &NetworkSim, port: PortId) -> String {
    sim.port(port).map_or_else(
        || crate::localization::tr_args("ui.uplink-socket", &[(port.0).to_string()]),
        |p| p.name.clone(),
    )
}

fn detail(ui: &mut egui::Ui, label: &str, value: impl std::fmt::Display) {
    ui.label(tr(label));
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
