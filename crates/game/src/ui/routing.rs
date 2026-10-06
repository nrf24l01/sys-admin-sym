use crate::app::{EditorDrafts, RouteDraft, UiAction, UiState};
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::*;

pub(super) fn show(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    drafts: &mut EditorDrafts,
    actions: &mut MessageWriter<UiAction>,
) {
    let Some(id) = state.routing_device else {
        return;
    };
    let Some(device) = sim.device(id) else {
        state.routing_device = None;
        return;
    };
    let DeviceKind::Router(router) = &device.kind else {
        state.routing_device = None;
        return;
    };
    let draft = drafts.routes.entry(id).or_default();
    let mut open = true;
    egui::Window::new(format!("{} · Routing table", device.name))
        .id(egui::Id::new(("router-routes", id)))
        .open(&mut open)
        .default_width(760.0)
        .show(viewport, |ui| {
            ui.label("This router forwards packets through its configured interfaces and connected cables.");
            ui.weak("Longest prefix wins; lower preference wins for equal prefixes.");
            egui::ScrollArea::both().max_height(260.0).show(ui, |ui| {
                egui::Grid::new(("route-table", id)).striped(true).show(ui, |ui| {
                    for heading in ["Destination", "Next hop", "Outgoing interface", "Domain", "Preference", "Type", ""] {
                        ui.strong(heading);
                    }
                    ui.end_row();
                    for interface in &router.interfaces {
                        let Some(address) = interface.address else { continue };
                        let Ok(prefix) = Ipv4Prefix::new(address, interface.prefix) else { continue };
                        let vlan = interface.vlan.unwrap_or(VlanId(1));
                        ui.label(prefix.to_string());
                        ui.label("On-link");
                        interface_link(ui, sim, interface.port, vlan, actions);
                        ui.label(sim.provider().domain(interface.port, vlan).0.to_string());
                        ui.label("0");
                        ui.label("Connected");
                        ui.weak("Automatic");
                        ui.end_row();
                    }
                    for route in &router.domain_routes {
                        ui.label(route.prefix.to_string());
                        ui.label(route.next_hop.map_or_else(|| "On-link".into(), |ip| ip.to_string()));
                        interface_link(ui, sim, route.port, route.vlan, actions);
                        ui.label(route.domain.0.to_string());
                        ui.label(route.preference.to_string());
                        ui.label(if route.track_neighbor { "Static · tracked" } else { "Static" });
                        ui.horizontal(|ui| {
                            if ui.small_button("Edit").clicked() { *draft = RouteDraft::edit(*route); }
                            if ui.small_button("Remove").clicked() {
                                actions.write(UiAction::NetworkCommand(Command::Provider(ProviderCommand::RemoveDomainRoute(*route))));
                            }
                        });
                        ui.end_row();
                    }
                    for route in &router.routes {
                        ui.label(format!("{}/{}", route.network, route.prefix));
                        ui.label(route.via.map_or_else(|| "On-link".into(), |ip| ip.to_string()));
                        let vlan = router.interfaces.iter().find(|i| i.port == route.egress).and_then(|i| i.vlan).unwrap_or(VlanId(1));
                        interface_link(ui, sim, route.egress, vlan, actions);
                        ui.label("0");
                        ui.label("—");
                        ui.label("Static · IOS");
                        if ui.small_button("Remove").clicked() {
                            actions.write(UiAction::NetworkCommand(Command::RemoveStaticRoute { router: id, route: route.clone() }));
                        }
                        ui.end_row();
                    }
                });
            });
            ui.separator();
            ui.strong(if draft.editing.is_some() { "Edit static route" } else { "Add static route" });
            let interfaces: Vec<_> = router.interfaces.iter().filter(|i| i.address.is_some()).collect();
            if interfaces.is_empty() {
                ui.label("Configure an IPv4 address on a router port first.");
                return;
            }
            if draft.interface.is_none() {
                draft.interface = Some((interfaces[0].port, interfaces[0].vlan.unwrap_or(VlanId(1))));
            }
            egui::Grid::new(("route-editor", id)).num_columns(2).show(ui, |ui| {
                ui.label("Destination prefix"); ui.text_edit_singleline(&mut draft.destination); ui.end_row();
                ui.label("Outgoing interface");
                egui::ComboBox::from_id_salt(("route-egress", id))
                    .selected_text(draft.interface.map_or_else(|| "Select interface".into(), |(p, v)| interface_name(sim, p, v)))
                    .show_ui(ui, |ui| {
                        for interface in &interfaces {
                            let vlan = interface.vlan.unwrap_or(VlanId(1));
                            ui.selectable_value(&mut draft.interface, Some((interface.port, vlan)), interface_name(sim, interface.port, vlan));
                        }
                    });
                ui.end_row();
                ui.label("Delivery");
                ui.horizontal(|ui| { ui.radio_value(&mut draft.direct, false, "Via gateway"); ui.radio_value(&mut draft.direct, true, "On-link"); });
                ui.end_row();
                if !draft.direct {
                    ui.label("Next-hop IPv4"); ui.text_edit_singleline(&mut draft.next_hop); ui.end_row();
                }
                ui.label("Preference"); ui.add(egui::DragValue::new(&mut draft.preference).range(1..=u32::MAX)); ui.end_row();
            });
            if let Some((port, vlan)) = draft.interface {
                ui.weak(format!("Routing domain {} (from the selected interface)", sim.provider().domain(port, vlan).0));
            }
            ui.add_enabled(!draft.direct, egui::Checkbox::new(&mut draft.track_neighbor, "Withdraw this route if the next hop cannot be resolved"));
            if let Some(error) = &draft.error { ui.colored_label(egui::Color32::LIGHT_RED, error); }
            ui.horizontal(|ui| {
                if ui.button("Apply route").clicked() {
                    match draft.command(sim, id) {
                        Ok(command) => { draft.error = None; actions.write(UiAction::NetworkCommand(command)); }
                        Err(error) => draft.error = Some(error),
                    }
                }
                if ui.button("New route / cancel edit").clicked() { *draft = RouteDraft::default(); }
            });
        });
    if !open {
        state.routing_device = None;
    }
}

fn interface_name(sim: &NetworkSim, port: PortId, vlan: VlanId) -> String {
    let name = sim.port(port).map_or("Missing port", |p| p.name.as_str());
    format!("{name} · VLAN {}", vlan.0)
}

fn interface_link(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    port: PortId,
    vlan: VlanId,
    actions: &mut MessageWriter<UiAction>,
) {
    if ui
        .link(interface_name(sim, port, vlan))
        .on_hover_text(if sim.port_link_up(port) {
            "Link up · open port settings"
        } else {
            "Link down · open port settings"
        })
        .clicked()
    {
        actions.write(UiAction::SelectPort(port));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        ecs::system::SystemState,
        prelude::{Messages, World},
    };

    #[test]
    fn applying_a_gui_route_changes_only_the_selected_router() {
        let mut sim = NetworkSim::new();
        sim.money = 100_000;
        let mut routers = Vec::new();
        for _ in 0..2 {
            let events = sim
                .execute(Command::BuyDevice {
                    kind: DeviceTemplate::Router,
                })
                .unwrap();
            let SimEvent::DeviceAdded(id) = events[0] else {
                panic!()
            };
            routers.push(id);
        }
        let port = sim.device(routers[0]).unwrap().ports()[0];
        sim.execute(Command::ConfigureRouterInterface {
            port,
            name: "WAN".into(),
            vlan: None,
            address: Some("192.0.2.2".parse().unwrap()),
            prefix: 30,
            internet_connected: false,
        })
        .unwrap();
        let mut state = UiState {
            routing_device: Some(routers[0]),
            ..Default::default()
        };
        let mut drafts = EditorDrafts::default();
        drafts.routes.insert(
            routers[0],
            RouteDraft {
                next_hop: "192.0.2.1".into(),
                ..Default::default()
            },
        );
        let ctx = egui::Context::default();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut system = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut position = None;
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
                        &mut drafts,
                        &mut system.get_mut(&mut world).unwrap(),
                    )
                },
            );
            position = output.shapes.iter().find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.text() == "Apply route"
                {
                    Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center())
                } else {
                    None
                }
            });
            output.textures_delta.clear();
        }
        let position = position.expect("route apply button is visible");
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
                        &mut drafts,
                        &mut system.get_mut(&mut world).unwrap(),
                    )
                },
            );
            output.textures_delta.clear();
        }
        let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        assert_eq!(actions.len(), 1);
        let UiAction::NetworkCommand(command) = &actions[0] else {
            panic!("expected typed route command")
        };
        sim.execute(command.clone()).unwrap();
        let DeviceKind::Router(first) = &sim.device(routers[0]).unwrap().kind else {
            panic!()
        };
        assert_eq!(first.domain_routes.len(), 1);
        assert_eq!(first.domain_routes[0].port, port);
        assert_eq!(
            first.domain_routes[0].next_hop,
            Some("192.0.2.1".parse().unwrap())
        );
        let DeviceKind::Router(second) = &sim.device(routers[1]).unwrap().kind else {
            panic!()
        };
        assert!(second.domain_routes.is_empty());
    }
}
