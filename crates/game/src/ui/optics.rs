//! Optical equipment UI sends commands; physical behavior belongs to sim.
use crate::app::{ShopCategory, ShopSection, ShopState, UiAction};
use crate::localization::{item_description, item_name, tr, tr_args};
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::*;

pub(super) fn offers(state: &ShopState, money: i64) -> Vec<(&'static str, i64, OpticsCommand)> {
    if state.category != ShopCategory::Network
        || state.section.is_some_and(|s| s != ShopSection::Optics)
        || state.rack_units.is_some()
        || state.ports.is_some()
        || state.outlets.is_some()
    {
        return Vec::new();
    }
    let catalog = optics_catalog();
    let all = catalog
        .modules
        .iter()
        .filter(|m| !matches!(m.medium, ModuleMedium::DirectAttach))
        .map(|m| {
            (
                m.id.as_str(),
                m.price,
                OpticsCommand::BuyTransceiver {
                    model: m.id.clone(),
                },
            )
        })
        .chain(catalog.cables.iter().map(|m| {
            (
                m.id.as_str(),
                m.price,
                OpticsCommand::BuyAssembly {
                    model: m.id.clone(),
                },
            )
        }))
        .chain(catalog.hardware.iter().map(|m| {
            (
                m.id.as_str(),
                m.price,
                OpticsCommand::BuyHardware {
                    model: m.id.clone(),
                },
            )
        }));
    let search = state.search.trim().to_lowercase();
    all.filter(|(id, price, _)| {
        (!state.affordable_only || *price <= money)
            && state.max_price.is_none_or(|max| *price <= max)
            && (search.is_empty()
                || format!("{} {} {}", id, item_name(id, id), item_description(id))
                    .to_lowercase()
                    .contains(&search))
    })
    .collect()
}
pub(super) fn shop_offers(
    ui: &mut egui::Ui,
    offers: &[(&str, i64, OpticsCommand)],
    money: i64,
    actions: &mut MessageWriter<UiAction>,
) {
    for (id, price, command) in offers {
        ui.push_id(id, |ui| {
            ui.group(|ui| {
                ui.strong(item_name(id, id));
                ui.weak(item_description(id));
                if ui
                    .add_enabled(
                        *price <= money,
                        egui::Button::new(tr_args("shop.buy", &[price.to_string()])),
                    )
                    .clicked()
                {
                    actions.write(UiAction::NetworkCommand(Command::Optics(command.clone())));
                }
            });
        });
    }
}
pub(super) fn port_controls(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    port: PortId,
    actions: &mut MessageWriter<UiAction>,
) {
    let Some(p) = sim.port(port) else { return };
    let status = sim.link_status(port);
    if let Some(fault) = status.fault {
        ui.weak(crate::localization::link_fault(fault));
    }
    if p.connector == PortConnector::Rj45 {
        return;
    }
    ui.separator();
    if let Some(cage) = sim.cage_profile(port) {
        ui.strong(tr_args(
            "optics.cage-modes",
            &[cage
                .modes
                .iter()
                .map(|m| m.speed.mbps().to_string())
                .collect::<Vec<_>>()
                .join(" / ")],
        ));
        if let Some(module) = sim.installed_transceiver(port) {
            ui.label(item_name(&module.model_id, &module.model_id));
            ui.weak(item_description(&module.model_id));
            if ui.button(tr("optics.remove-module")).clicked() {
                actions.write(UiAction::NetworkCommand(Command::Optics(
                    OpticsCommand::RemoveTransceiver { port },
                )));
            }
        } else if let Some(module) = sim.endpoint_module(port) {
            ui.label(item_name(&module.id, &module.id));
            ui.weak(tr("optics.attached-ends"));
        } else {
            ui.label(tr("optics.empty-cage"));
            for module in sim
                .optics
                .transceivers
                .values()
                .filter(|m| m.port.is_none())
            {
                if optics_catalog()
                    .module(&module.model_id)
                    .is_some_and(|m| cage.supports(m))
                {
                    ui.push_id(module.id.0, |ui| {
                        if ui
                            .button(tr_args(
                                "hardware.install-item",
                                &[item_name(&module.model_id, &module.model_id)],
                            ))
                            .clicked()
                        {
                            actions.write(UiAction::NetworkCommand(Command::Optics(
                                OpticsCommand::InstallTransceiver {
                                    port,
                                    module: module.id,
                                },
                            )));
                        }
                    });
                }
            }
        }
    }
    if let Some(module) = sim.endpoint_module(port) {
        if module.dom {
            if let Some(reading) = status.optical.iter().find(|r| r.port == port) {
                ui.monospace(tr_args(
                    "optics.light",
                    &[
                        format!("{:.2}", reading.tx_mdbm as f32 / 1000.0),
                        format!("{:.2}", reading.rx_mdbm as f32 / 1000.0),
                    ],
                ));
            } else {
                ui.weak(tr("optics.no-light"));
            }
        } else {
            ui.weak(tr("optics.no-dom"));
        }
    }
    if let Some(link) = sim.link_for_port(port) {
        if let Some(cable) = sim.connected_assembly(link.id) {
            ui.label(item_name(&cable.model_id, &cable.model_id));
            if optics_catalog()
                .cable(&cable.model_id)
                .is_some_and(|m| matches!(m.medium, AssemblyMedium::Fiber { strands: 2, .. }))
                && ui.button(tr("optics.flip-polarity")).clicked()
            {
                actions.write(UiAction::NetworkCommand(Command::Optics(
                    OpticsCommand::FlipPolarity { assembly: cable.id },
                )));
            }
        }
    } else {
        ui.weak(tr("optics.select-cable"));
        for assembly in sim.optics.assemblies.values().filter(|a| a.link.is_none()) {
            let Some(model) = optics_catalog().cable(&assembly.model_id) else {
                continue;
            };
            let fits = match &model.medium {
                AssemblyMedium::Fiber { .. } => {
                    p.connector == PortConnector::Lc
                        || p.connector == PortConnector::Sfp
                            && sim
                                .endpoint_module(port)
                                .is_some_and(|m| matches!(m.medium, ModuleMedium::Optical { .. }))
                }
                AssemblyMedium::Dac { transceiver } | AssemblyMedium::Aoc { transceiver } => {
                    sim.endpoint_module(port).is_none()
                        && sim.cage_profile(port).is_some_and(|c| {
                            optics_catalog()
                                .module(transceiver)
                                .is_some_and(|m| c.supports(m))
                        })
                }
            };
            if fits {
                ui.push_id(("assembly", assembly.id.0), |ui| {
                    if ui
                        .button(tr_args(
                            "optics.connect-assembly",
                            &[item_name(&model.id, &model.id)],
                        ))
                        .clicked()
                    {
                        actions.write(UiAction::StartAssembly {
                            port,
                            assembly: assembly.id,
                        });
                    }
                });
            }
        }
    }
}
pub(super) fn paint_socket(
    painter: &egui::Painter,
    rect: egui::Rect,
    sim: &NetworkSim,
    port: PortId,
) {
    let Some(p) = sim.port(port) else { return };
    if p.connector == PortConnector::Lc {
        painter.rect_filled(rect, 1.0, egui::Color32::from_rgb(45, 90, 165));
        for offset in [-0.2, 0.2] {
            painter.rect_filled(
                egui::Rect::from_center_size(
                    rect.center() + egui::vec2(rect.width() * offset, 0.0),
                    egui::vec2(rect.width() * 0.3, rect.height() * 0.65),
                ),
                1.0,
                egui::Color32::from_gray(15),
            );
        }
    } else if p.connector == PortConnector::Sfp && sim.endpoint_module(port).is_some() {
        painter.rect_filled(rect.shrink(1.0), 1.0, egui::Color32::from_gray(145));
        painter.rect_filled(rect.shrink(3.0), 1.0, egui::Color32::from_rgb(25, 75, 105));
    }
}

pub(super) fn inventory(ui: &mut egui::Ui, sim: &NetworkSim) {
    ui.heading(tr("optics.inventory"));
    let mut counts = std::collections::BTreeMap::<&str, u32>::new();
    for module in sim
        .optics
        .transceivers
        .values()
        .filter(|m| m.port.is_none())
    {
        *counts.entry(&module.model_id).or_default() += 1;
    }
    for cable in sim.optics.assemblies.values().filter(|m| m.link.is_none()) {
        *counts.entry(&cable.model_id).or_default() += 1;
    }
    if counts.is_empty() {
        ui.weak(tr("optics.inventory-empty"));
    }
    for (id, count) in counts {
        ui.label(tr_args(
            "optics.inventory-count",
            &[item_name(id, id), count.to_string()],
        ));
    }
    ui.weak(tr("optics.inventory-guide"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::localization::Localization;
    use bevy::ecs::system::SystemState;
    use bevy::prelude::{Messages, World};

    #[test]
    fn localized_port_install_button_emits_a_typed_inventory_command() {
        for language in ["en", "ru"] {
            let mut localization = Localization::default();
            localization.select(language).unwrap();
            let _scope = localization.enter();
            let mut sim = NetworkSim::new();
            let SimEvent::DeviceAdded(device) = sim
                .execute(Command::BuyDevice {
                    kind: DeviceTemplate::Switch,
                })
                .unwrap()[0]
            else {
                panic!()
            };
            let port = sim.device(device).unwrap().ports()[24];
            sim.execute(Command::Optics(OpticsCommand::BuyTransceiver {
                model: "sfp_1g_sx".into(),
            }))
            .unwrap();
            let module = *sim.optics.transceivers.keys().next().unwrap();
            let mut world = World::new();
            world.init_resource::<Messages<UiAction>>();
            let mut system = SystemState::<MessageWriter<UiAction>>::new(&mut world);
            let ctx = egui::Context::default();
            let expected = tr_args("hardware.install-item", &[item_name("sfp_1g_sx", "")]);
            let mut position = None;
            for _ in 0..2 {
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    port_controls(ui, &sim, port, &mut system.get_mut(&mut world).unwrap());
                });
                position = output.shapes.iter().find_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape
                        && text.galley.text() == expected
                    {
                        Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center())
                    } else {
                        None
                    }
                });
                output.textures_delta.clear();
            }
            let position = position.expect("localized install action is visible");
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
                        port_controls(ui, &sim, port, &mut system.get_mut(&mut world).unwrap());
                    },
                );
                output.textures_delta.clear();
            }
            let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
            assert!(
                matches!(actions.as_slice(), [UiAction::NetworkCommand(Command::Optics(OpticsCommand::InstallTransceiver { port: p, module: m }))] if *p == port && *m == module)
            );
        }
    }

    #[test]
    fn empty_cage_offers_attached_cables_but_requires_a_module_for_fiber() {
        let mut sim = NetworkSim::new();
        let SimEvent::DeviceAdded(device) = sim
            .execute(Command::Optics(OpticsCommand::BuyHardware {
                model: "switch_10g".into(),
            }))
            .unwrap()[0]
        else {
            panic!()
        };
        let port = sim.device(device).unwrap().ports()[24];
        for model in ["fiber_om3_2_3m", "dac_10g_3m"] {
            sim.execute(Command::Optics(OpticsCommand::BuyAssembly {
                model: model.into(),
            }))
            .unwrap();
        }
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut system = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        for installed in [false, true] {
            if installed {
                sim.execute(Command::Optics(OpticsCommand::BuyTransceiver {
                    model: "sfpplus_10g_sr".into(),
                }))
                .unwrap();
                let module = *sim.optics.transceivers.keys().next().unwrap();
                sim.execute(Command::Optics(OpticsCommand::InstallTransceiver {
                    port,
                    module,
                }))
                .unwrap();
            }
            let ctx = egui::Context::default();
            let mut labels = Vec::new();
            for _ in 0..2 {
                let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                    port_controls(ui, &sim, port, &mut system.get_mut(&mut world).unwrap());
                });
                labels = output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                        _ => None,
                    })
                    .collect();
                output.textures_delta.clear();
            }
            let fiber = tr_args(
                "optics.connect-assembly",
                &[item_name("fiber_om3_2_3m", "")],
            );
            let dac = tr_args("optics.connect-assembly", &[item_name("dac_10g_3m", "")]);
            assert_eq!(labels.contains(&fiber), installed);
            assert_eq!(labels.contains(&dac), !installed);
        }
    }

    #[test]
    fn catalog_search_uses_item_translations_and_filters_affordability() {
        let mut localization = Localization::default();
        localization.select("ru").unwrap();
        let _scope = localization.enter();
        let mut state = ShopState {
            category: ShopCategory::Network,
            section: Some(ShopSection::Optics),
            search: "патч-панель".into(),
            ..Default::default()
        };
        assert!(matches!(
            offers(&state, 6000).as_slice(),
            [("fiber_panel", _, OpticsCommand::BuyHardware { .. })]
        ));
        state.affordable_only = true;
        assert!(offers(&state, 0).is_empty());
        state.search = "sfpplus_10g_sr".into();
        assert!(
            offers(&state, 6000)
                .iter()
                .any(|(id, _, _)| *id == "sfpplus_10g_sr")
        );
        state.section = Some(ShopSection::Routers);
        assert!(offers(&state, 6000).is_empty());
    }
}
