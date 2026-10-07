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
#[derive(Clone, Copy)]
pub(super) struct ShopTextures {
    pub modules: egui::TextureId,
    pub cables: egui::TextureId,
}

pub(super) fn shop_offers(
    ui: &mut egui::Ui,
    offers: &[(&str, i64, OpticsCommand)],
    money: i64,
    textures: ShopTextures,
    actions: &mut MessageWriter<UiAction>,
) {
    for (id, price, command) in offers {
        ui.push_id(id, |ui| {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    if let Some((texture, uv)) = shop_artwork(id, textures) {
                        let aspect = uv.width() / uv.height();
                        let size = egui::vec2(64.0 * aspect.min(1.0), 64.0 / aspect.max(1.0));
                        ui.add(egui::Image::new((texture, size)).uv(uv));
                    }
                    ui.strong(item_name(id, id));
                });
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
            let mut modules = std::collections::BTreeMap::new();
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
                    let (_, count) = modules
                        .entry(module.model_id.as_str())
                        .or_insert((module.id, 0));
                    *count += 1;
                }
            }
            for (model, (module, count)) in modules {
                ui.push_id(("module-stock", model), |ui| {
                    let label = tr_args(
                        "optics.inventory-count",
                        &[item_name(model, model), count.to_string()],
                    );
                    if ui
                        .button(tr_args("hardware.install-item", &[label]))
                        .clicked()
                    {
                        actions.write(UiAction::NetworkCommand(Command::Optics(
                            OpticsCommand::InstallTransceiver { port, module },
                        )));
                    }
                });
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
        let mut cables = std::collections::BTreeMap::new();
        for assembly in sim
            .optics
            .assemblies
            .values()
            .filter(|a| sim.assembly_supported_at_port(a.id, port))
        {
            let (_, count) = cables
                .entry(assembly.model_id.as_str())
                .or_insert((assembly.id, 0));
            *count += 1;
        }
        if cables.is_empty() {
            ui.weak(tr("optics.no-compatible-cables"));
        }
        for (model, (assembly, count)) in cables {
            ui.push_id(("cable-stock", model), |ui| {
                if ui
                    .button(tr_args(
                        "optics.inventory-count",
                        &[item_name(model, model), count.to_string()],
                    ))
                    .on_hover_text(item_description(model))
                    .clicked()
                {
                    actions.write(UiAction::StartAssembly { port, assembly });
                }
            });
        }
    }
}
/// Keep cable selection next to the socket; the inspector uses the same controls.
pub(super) fn socket_picker(
    response: &egui::Response,
    sim: &NetworkSim,
    port: PortId,
    actions: &mut MessageWriter<UiAction>,
) {
    egui::Popup::from_toggle_button_response(response)
        .width(320.0)
        .show(|ui| {
            ui.strong(tr("optics.socket-menu"));
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    port_controls(ui, sim, port, actions);
                });
        });
}

/// Crops of the unmodified 1254×1254 connector atlas. Keep these in one place
/// so rack sockets and seated cable ends use the same hardware presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OpticalSprite {
    EmptyCage,
    DuplexModule,
    SimplexModule,
    CopperModule,
    DuplexPlug,
    SimplexPlug,
    DacPlug,
    AocPlug,
}
impl OpticalSprite {
    pub(super) fn uv(self) -> egui::Rect {
        let (left, top, right, bottom) = match self {
            Self::EmptyCage => (132.0, 42.0, 522.0, 266.0),
            Self::DuplexModule => (735.0, 40.0, 1128.0, 281.0),
            Self::SimplexModule => (139.0, 314.0, 518.0, 554.0),
            Self::CopperModule => (737.0, 312.0, 1131.0, 561.0),
            Self::DuplexPlug => (182.0, 600.0, 506.0, 886.0),
            Self::SimplexPlug => (782.0, 600.0, 1094.0, 888.0),
            Self::DacPlug => (154.0, 896.0, 527.0, 1238.0),
            Self::AocPlug => (733.0, 896.0, 1135.0, 1240.0),
        };
        egui::Rect::from_min_max(
            egui::pos2(left / 1254.0, top / 1254.0),
            egui::pos2(right / 1254.0, bottom / 1254.0),
        )
    }
}

fn shop_artwork(id: &str, textures: ShopTextures) -> Option<(egui::TextureId, egui::Rect)> {
    if let Some(module) = optics_catalog().module(id) {
        let sprite = match module.medium {
            ModuleMedium::Optical { strands: 1, .. } => OpticalSprite::SimplexModule,
            ModuleMedium::Optical { .. } => OpticalSprite::DuplexModule,
            ModuleMedium::Copper => OpticalSprite::CopperModule,
            ModuleMedium::DirectAttach => return None,
        };
        return Some((textures.modules, sprite.uv()));
    }
    optics_catalog().cable(id).map(|model| {
        // Dedicated connector sprites include the entire housing and tip,
        // with transparent padding. Never crop connectors out of cable coils.
        let (column, row) = match model.medium {
            AssemblyMedium::Fiber { strands: 1, .. } => (1.0, 0.0),
            AssemblyMedium::Fiber { .. } => (0.0, 0.0),
            AssemblyMedium::Dac { .. } => (0.0, 1.0),
            AssemblyMedium::Aoc { .. } => (1.0, 1.0),
        };
        (
            textures.cables,
            egui::Rect::from_min_size(egui::pos2(column * 0.5, row * 0.5), egui::vec2(0.5, 0.5)),
        )
    })
}

fn socket_sprite(sim: &NetworkSim, port: PortId) -> Option<OpticalSprite> {
    let p = sim.port(port)?;
    if p.connector == PortConnector::Lc {
        return Some(OpticalSprite::DuplexModule);
    }
    if p.connector != PortConnector::Sfp {
        return None;
    }
    Some(match sim.endpoint_module(port).map(|m| &m.medium) {
        Some(ModuleMedium::Optical { strands: 1, .. }) => OpticalSprite::SimplexModule,
        Some(ModuleMedium::Optical { .. }) => OpticalSprite::DuplexModule,
        Some(ModuleMedium::Copper) => OpticalSprite::CopperModule,
        _ => OpticalSprite::EmptyCage,
    })
}

pub(super) fn paint_socket(
    painter: &egui::Painter,
    texture: egui::TextureId,
    rect: egui::Rect,
    sim: &NetworkSim,
    port: PortId,
) {
    if let Some(sprite) = socket_sprite(sim, port) {
        painter.image(texture, rect, sprite.uv(), egui::Color32::WHITE);
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
            let expected = tr_args(
                "hardware.install-item",
                &[tr_args(
                    "optics.inventory-count",
                    &[item_name("sfp_1g_sx", ""), "1".into()],
                )],
            );
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
                "optics.inventory-count",
                &[item_name("fiber_om3_2_3m", ""), "1".into()],
            );
            let dac = tr_args(
                "optics.inventory-count",
                &[item_name("dac_10g_3m", ""), "1".into()],
            );
            assert_eq!(labels.contains(&fiber), installed);
            assert_eq!(labels.contains(&dac), !installed);
        }
    }

    #[test]
    fn socket_click_opens_inventory_and_cable_click_starts_connection() {
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
        for model in [
            "fiber_om3_2_3m",
            "fiber_om3_2_3m",
            "fiber_om3_2_3m",
            "fiber_os2_2_3m",
        ] {
            sim.execute(Command::Optics(OpticsCommand::BuyAssembly {
                model: model.into(),
            }))
            .unwrap();
        }
        let assembly = *sim.optics.assemblies.keys().next().unwrap();
        let ctx = egui::Context::default();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut system = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let socket = egui::Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(30.0, 20.0));
        let expected = tr_args(
            "optics.inventory-count",
            &[item_name("fiber_om3_2_3m", ""), "3".into()],
        );
        let unsupported = tr_args(
            "optics.inventory-count",
            &[item_name("fiber_os2_2_3m", ""), "1".into()],
        );
        let mut cable_position = None;
        // Warm up the UI, click the socket, and allow the popup its sizing pass.
        for event in [None, Some(true), Some(false), None, None] {
            let mut input = egui::RawInput::default();
            if let Some(pressed) = event {
                input.events = vec![
                    egui::Event::PointerMoved(socket.center()),
                    egui::Event::PointerButton {
                        pos: socket.center(),
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ];
            }
            let mut output = ctx.run_ui(input, |ui| {
                let response = ui.interact(
                    socket,
                    egui::Id::new("test-optical-socket"),
                    egui::Sense::click(),
                );
                socket_picker(
                    &response,
                    &sim,
                    port,
                    &mut system.get_mut(&mut world).unwrap(),
                );
            });
            let mut matching_rows = 0;
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape {
                    assert_ne!(text.galley.text(), unsupported);
                    if text.galley.text() == expected {
                        matching_rows += 1;
                        cable_position =
                            Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center());
                    }
                }
            }
            assert!(matching_rows <= 1, "identical stock has a single row");
            output.textures_delta.clear();
        }
        let position = cable_position.expect("socket click opens the compatible inventory list");
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
                    let response = ui.interact(
                        socket,
                        egui::Id::new("test-optical-socket"),
                        egui::Sense::click(),
                    );
                    socket_picker(
                        &response,
                        &sim,
                        port,
                        &mut system.get_mut(&mut world).unwrap(),
                    );
                },
            );
            output.textures_delta.clear();
        }
        let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        assert!(
            matches!(actions.as_slice(), [UiAction::StartAssembly { port: p, assembly: a }] if *p == port && *a == assembly)
        );
    }

    #[test]
    fn connector_shop_atlas_decodes_with_transparent_cell_boundaries() {
        use bevy::{
            asset::RenderAssetUsages,
            image::{CompressedImageFormats, Image, ImageSampler, ImageType},
        };
        let image = Image::from_buffer(
            include_bytes!("../../../../assets/cables/optical_shop_connectors.png"),
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::default(),
            RenderAssetUsages::default(),
        )
        .unwrap();
        let (width, height) = (image.width() as usize, image.height() as usize);
        assert_eq!(width % 2, 0);
        assert_eq!(height % 2, 0);
        let data = image.data.as_ref().unwrap();
        assert_eq!(data.len(), width * height * 4);
        let alpha = |x, y| data[(y * width + x) * 4 + 3];
        // Visible pixels touching cell edges would clip a housing or bleed into
        // its neighbor at the actual shop UV boundaries.
        for x in [0, width / 2, width - 1] {
            assert!((0..height).all(|y| alpha(x, y) <= 1));
        }
        for y in [0, height / 2, height - 1] {
            assert!((0..width).all(|x| alpha(x, y) <= 1));
        }
        for (column, row) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let visible = (row * height / 2..(row + 1) * height / 2)
                .step_by(8)
                .any(|y| {
                    (column * width / 2..(column + 1) * width / 2)
                        .step_by(8)
                        .any(|x| alpha(x, y) > 200)
                });
            assert!(visible, "every connector cell contains artwork");
        }
    }

    #[test]
    fn shop_cables_show_only_free_connectors_instead_of_coils_or_seated_modules() {
        let textures = ShopTextures {
            modules: egui::TextureId::User(1),
            cables: egui::TextureId::User(2),
        };
        for model in &optics_catalog().cables {
            let (texture, uv) = shop_artwork(&model.id, textures).unwrap();
            assert_eq!(texture, textures.cables);
            assert_eq!(uv.size(), egui::vec2(0.5, 0.5));
            assert!(uv.min.x >= 0.0 && uv.min.y >= 0.0 && uv.max.x <= 1.0 && uv.max.y <= 1.0);
        }
        assert_eq!(
            shop_artwork("sfpplus_10g_sr", textures).unwrap().0,
            textures.modules
        );
        assert!(shop_artwork("switch_10g", textures).is_none());
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
