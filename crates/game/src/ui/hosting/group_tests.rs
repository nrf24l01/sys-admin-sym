use super::*;
use bevy::ecs::system::SystemState;
use bevy::prelude::{Messages, World};

#[test]
fn groups_tab_is_clickable_in_english_and_russian() {
    let mut localization = crate::localization::Localization::default();
    for (language, label) in [("en", "Groups"), ("ru", "Группы")] {
        localization.select(language).unwrap();
        let _scope = localization.enter();
        let sim = NetworkSim::new();
        let ctx = egui::Context::default();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut writer = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut state = UiState::default();
        state.hosting.open = true;
        let mut position = None;
        for _ in 0..3 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1600.0, 1000.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    show(
                        ui,
                        &sim,
                        &mut state,
                        &mut writer.get_mut(&mut world).unwrap(),
                    )
                },
            );
            position = output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            });
            output.textures_delta.clear();
        }
        let pos = position.unwrap_or_else(|| panic!("Missing Groups tab in {language}"));
        for pressed in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1600.0, 1000.0),
                    )),
                    events: vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
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
                        &mut writer.get_mut(&mut world).unwrap(),
                    )
                },
            );
            output.textures_delta.clear();
        }
        assert_eq!(state.hosting.tab, 7);
    }
}

fn render(
    ctx: &egui::Context,
    world: &mut World,
    writer: &mut SystemState<MessageWriter<UiAction>>,
    sim: &NetworkSim,
    state: &mut UiState,
    events: Vec<egui::Event>,
) -> Vec<(String, egui::Pos2)> {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1800.0, 2400.0),
            )),
            events,
            ..Default::default()
        },
        |ui| match state.hosting.tab {
            7 => groups::show(ui, sim, state, &mut writer.get_mut(world).unwrap()),
            1 => nodes(ui, sim, state, &mut writer.get_mut(world).unwrap()),
            _ => products(ui, sim, state, &mut writer.get_mut(world).unwrap()),
        },
    );
    let labels = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => Some((
                text.galley.text().to_string(),
                text.pos + text.galley.size() * 0.5,
            )),
            _ => None,
        })
        .collect();
    output.textures_delta.clear();
    labels
}

fn click(
    ctx: &egui::Context,
    world: &mut World,
    writer: &mut SystemState<MessageWriter<UiAction>>,
    sim: &NetworkSim,
    state: &mut UiState,
    label: &str,
) {
    render(ctx, world, writer, sim, state, vec![]);
    let labels = render(ctx, world, writer, sim, state, vec![]);
    let pos = labels
        .iter()
        .find(|(text, _)| text.starts_with(label))
        .unwrap_or_else(|| panic!("Missing {label}: {labels:?}"))
        .1;
    for pressed in [true, false] {
        render(
            ctx,
            world,
            writer,
            sim,
            state,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

fn fixture() -> (
    NetworkSim,
    DeviceId,
    World,
    SystemState<MessageWriter<'static, UiAction>>,
    UiState,
) {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let SimEvent::DeviceAdded(node) = sim.execute(Command::BuyServerFullPack).unwrap()[0] else {
        panic!()
    };
    sim.buy_public_ipv4_subnet(24).unwrap();
    let mut world = World::new();
    world.init_resource::<Messages<UiAction>>();
    let writer = SystemState::<MessageWriter<UiAction>>::new(&mut world);
    (sim, node, world, writer, UiState::default())
}

#[test]
fn group_editor_saves_member_policy_range_and_named_vlan_rule_in_both_languages() {
    let mut localization = crate::localization::Localization::default();
    for language in ["en", "ru"] {
        localization.select(language).unwrap();
        let _scope = localization.enter();
        let (mut sim, node, mut world, mut writer, mut state) = fixture();
        let range = sim.provider().pools()[0].prefix;
        let subnet = Ipv4Prefix::new(range.network(), 29).unwrap().to_string();
        let ctx = egui::Context::default();
        state.hosting.tab = 7;
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &tr("hosting.group-new"),
        );
        state.hosting.group.name = "Compute east".into();
        let name = sim.device(node).unwrap().name.clone();
        click(&ctx, &mut world, &mut writer, &sim, &mut state, &name);
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &tr("hosting.node-ip-inherit"),
        );
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &range.to_string(),
        );
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &tr("hosting.interface-networks-auto"),
        );
        state.hosting.group_network_interface = "eth0".into();
        state.hosting.group_network_subnet = subnet.clone();
        state.hosting.group_network_tagged = true;
        state.hosting.group_network_vlan = 10;
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &tr("hosting.network-add"),
        );
        state.hosting.group.policy.cpu_overcommit_percent = 350;
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &tr("hosting.group-save"),
        );
        let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        let [UiAction::NetworkCommand(Command::Hosting(HostingCommand::SaveNodeGroup(group)))] =
            actions.as_slice()
        else {
            panic!("Expected one saved group")
        };
        assert_eq!(group.name, "Compute east");
        assert_eq!(group.nodes, [node].into());
        assert_eq!(group.policy.cpu_overcommit_percent, 350);
        assert_eq!(group.ipv4_ranges, Some([range].into()));
        assert_eq!(
            group.networks,
            Some(vec![HostingGroupNetwork {
                interface: "eth0".into(),
                subnet: subnet.parse().unwrap(),
                vlan: Some(VlanId(10))
            }])
        );
        sim.execute(Command::Hosting(HostingCommand::SaveNodeGroup(
            group.clone(),
        )))
        .unwrap();
        assert_eq!(sim.hosting_node_group(node).unwrap().name, "Compute east");
        assert_eq!(sim.hosting.nodes[&node].policy.cpu_overcommit_percent, 350);
    }
}

#[test]
fn product_group_checkbox_emits_live_group_reference_in_both_languages() {
    let mut localization = crate::localization::Localization::default();
    for language in ["en", "ru"] {
        localization.select(language).unwrap();
        let _scope = localization.enter();
        let (mut sim, node, mut world, mut writer, mut state) = fixture();
        sim.execute(Command::Hosting(HostingCommand::SaveNodeGroup(
            HostingNodeGroup {
                name: "Compute east".into(),
                nodes: [node].into(),
                ..Default::default()
            },
        )))
        .unwrap();
        let group = *sim.hosting.node_groups.keys().next().unwrap();
        let ctx = egui::Context::default();
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            "Compute east",
        );
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            "203.0.113.0/24",
        );
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &tr("hosting.save"),
        );
        let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        let [UiAction::NetworkCommand(Command::Hosting(HostingCommand::SaveProduct(product)))] =
            actions.as_slice()
        else {
            panic!("Expected saved product")
        };
        assert_eq!(product.eligible_groups, [group].into());
        assert!(product.eligible_nodes.is_empty());
        assert_eq!(sim.hosting_product_nodes(product), [node].into());
    }
}

#[test]
fn node_group_selector_joins_and_leaves_and_disables_local_overrides() {
    let mut localization = crate::localization::Localization::default();
    for language in ["en", "ru"] {
        localization.select(language).unwrap();
        let _scope = localization.enter();
        let (mut sim, node, mut world, mut writer, mut state) = fixture();
        sim.execute(Command::Hosting(HostingCommand::SaveNodeGroup(
            HostingNodeGroup {
                name: "Compute east".into(),
                ..Default::default()
            },
        )))
        .unwrap();
        let group = *sim.hosting.node_groups.keys().next().unwrap();
        let ctx = egui::Context::default();
        state.hosting.tab = 1;
        state.hosting.selected_node = Some(node);
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &tr("hosting.group-none"),
        );
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            "Compute east",
        );
        let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        let [
            UiAction::NetworkCommand(Command::Hosting(HostingCommand::SetNodeGroup {
                node: selected,
                group: target,
            })),
        ] = actions.as_slice()
        else {
            panic!("Expected joining command")
        };
        assert_eq!((*selected, *target), (node, Some(group)));
        sim.execute(Command::Hosting(HostingCommand::SetNodeGroup {
            node,
            group: Some(group),
        }))
        .unwrap();
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &tr("hosting.apply-policy"),
        );
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &tr("hosting.node-ip-apply"),
        );
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &tr("hosting.network-apply"),
        );
        assert!(
            world
                .resource_mut::<Messages<UiAction>>()
                .drain()
                .next()
                .is_none()
        );
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &tr("hosting.group-edit"),
        );
        assert_eq!(state.hosting.tab, 7);
        assert_eq!(state.hosting.group.id, group);
        state.hosting.tab = 1;
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            "Compute east",
        );
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &tr("hosting.group-none"),
        );
        let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        let [
            UiAction::NetworkCommand(Command::Hosting(HostingCommand::SetNodeGroup {
                node: selected,
                group: target,
            })),
        ] = actions.as_slice()
        else {
            panic!("Expected leaving command")
        };
        assert_eq!((*selected, *target), (node, None));
    }
}
