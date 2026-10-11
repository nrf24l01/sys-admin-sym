use super::*;

fn load(state: &mut UiState, mut group: HostingNodeGroup) {
    if group.id == 0 {
        group.name = tr("hosting.group-new");
    }
    state.hosting.group = group;
    state.hosting.group_loaded = true;
    state.hosting.group_range_input.clear();
    state.hosting.group_network_interface = "eth0".into();
    state.hosting.group_network_subnet.clear();
    state.hosting.group_network_vlan = 1;
    state.hosting.group_network_tagged = false;
}

pub(super) fn show(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    if !state.hosting.group_loaded {
        load(state, HostingNodeGroup::default());
    }
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(260.0);
            if ui.button(tr("hosting.group-new")).clicked() {
                load(state, HostingNodeGroup::default());
            }
            egui::ScrollArea::vertical()
                .id_salt("hosting-groups")
                .max_height(ui.available_height())
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for group in sim.hosting.node_groups.values() {
                        if ui
                            .selectable_label(state.hosting.group.id == group.id, &group.name)
                            .clicked()
                        {
                            load(state, group.clone());
                        }
                        ui.small(summary(sim, group));
                    }
                });
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("hosting-group-editor")
            .max_height(ui.available_height())
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(tr("hosting.group-guide"));
                    ui.horizontal(|ui| {
                        ui.label(tr("hosting.group-name"));
                        ui.text_edit_singleline(&mut state.hosting.group.name);
                    });
                    ui.strong(tr("hosting.group-members"));
                    let mut servers: Vec<_> = sim
                        .devices()
                        .filter(|d| matches!(d.kind, DeviceKind::Server(_)))
                        .filter(|d| !sim.hosting_provider_installed(d.id))
                        .collect();
                    servers.sort_by_key(|d| d.id);
                    for d in servers {
                        let other = sim
                            .hosting_node_group(d.id)
                            .filter(|g| g.id != state.hosting.group.id);
                        let mut selected = state.hosting.group.nodes.contains(&d.id);
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    other.is_none(),
                                    egui::Checkbox::new(&mut selected, &d.name),
                                )
                                .changed()
                            {
                                if selected {
                                    state.hosting.group.nodes.insert(d.id);
                                } else {
                                    state.hosting.group.nodes.remove(&d.id);
                                }
                            }
                            if let Some(group) = other {
                                ui.small(tr_args(
                                    "hosting.group-member-in",
                                    std::slice::from_ref(&group.name),
                                ));
                            }
                        });
                    }
                    ui.separator();
                    ranges(ui, sim, state);
                    networks(ui, sim, state);
                    ui.separator();
                    ui.strong(tr("hosting.group-policy"));
                    policy::show(ui, &mut state.hosting.group.policy);
                    let validation = sim.validate_hosting_node_group(&state.hosting.group);
                    if let Err(error) = &validation {
                        ui.colored_label(egui::Color32::LIGHT_RED, error);
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                validation.is_ok(),
                                egui::Button::new(tr("hosting.group-save")),
                            )
                            .clicked()
                        {
                            command(
                                actions,
                                HostingCommand::SaveNodeGroup(state.hosting.group.clone()),
                            );
                        }
                        if state.hosting.group.id != 0 {
                            if ui.button(tr("hosting.group-reload")).clicked()
                                && let Some(group) =
                                    sim.hosting.node_groups.get(&state.hosting.group.id)
                            {
                                load(state, group.clone());
                            }
                            if ui.button(tr("hosting.group-delete")).clicked() {
                                command(
                                    actions,
                                    HostingCommand::DeleteNodeGroup(state.hosting.group.id),
                                );
                                load(state, HostingNodeGroup::default());
                            }
                        }
                    });
                });
            });
    });
}

fn ranges(ui: &mut egui::Ui, sim: &NetworkSim, state: &mut UiState) {
    ui.strong(tr("hosting.node-ip-ranges"));
    let mut inherit = state.hosting.group.ipv4_ranges.is_none();
    if ui
        .checkbox(&mut inherit, tr("hosting.node-ip-inherit"))
        .changed()
    {
        state.hosting.group.ipv4_ranges = (!inherit).then(Default::default);
    }
    if let Some(ranges) = &mut state.hosting.group.ipv4_ranges {
        for pool in sim.provider().pools() {
            let mut selected = ranges.contains(&pool.prefix);
            if ui
                .add_enabled(
                    pool.prefix.length() <= 30,
                    egui::Checkbox::new(&mut selected, pool.prefix.to_string()),
                )
                .changed()
            {
                if selected {
                    ranges.insert(pool.prefix);
                } else {
                    ranges.remove(&pool.prefix);
                }
            }
        }
        ui.horizontal(|ui| {
            ui.label(tr("hosting.node-ip-subnet"));
            ui.text_edit_singleline(&mut state.hosting.group_range_input);
            let prefix = state
                .hosting
                .group_range_input
                .trim()
                .parse::<Ipv4Prefix>()
                .ok()
                .filter(|r| r.length() <= 30 && sim.provider().owns(*r));
            if ui
                .add_enabled(
                    prefix.is_some(),
                    egui::Button::new(tr("hosting.node-ip-add")),
                )
                .clicked()
            {
                ranges.insert(prefix.unwrap());
                state.hosting.group_range_input.clear();
            }
        });
        let mut remove = None;
        for prefix in ranges.iter() {
            ui.horizontal(|ui| {
                ui.monospace(prefix.to_string());
                if ui.small_button(tr("hosting.remove")).clicked() {
                    remove = Some(*prefix);
                }
            });
        }
        if let Some(prefix) = remove {
            ranges.remove(&prefix);
        }
    }
}

fn networks(ui: &mut egui::Ui, sim: &NetworkSim, state: &mut UiState) {
    ui.separator();
    ui.strong(tr("hosting.interface-networks"));
    ui.label(tr("hosting.group-networks-guide"));
    let mut automatic = state.hosting.group.networks.is_none();
    if ui
        .checkbox(&mut automatic, tr("hosting.interface-networks-auto"))
        .changed()
    {
        state.hosting.group.networks = (!automatic).then(Vec::new);
    }
    if let Some(networks) = &mut state.hosting.group.networks {
        let mut remove = None;
        for (i, network) in networks.iter_mut().enumerate() {
            ui.push_id(("group-network", i), |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.monospace(network.subnet.to_string());
                    ui.add(egui::TextEdit::singleline(&mut network.interface).desired_width(90.0));
                    let mut tagged = network.vlan.is_some();
                    if ui
                        .checkbox(&mut tagged, tr("hosting.network-tagged"))
                        .changed()
                    {
                        network.vlan = tagged.then_some(VlanId(1));
                    }
                    if let Some(vlan) = &mut network.vlan {
                        ui.add(egui::DragValue::new(&mut vlan.0).range(1..=4094));
                    } else {
                        ui.label(tr("hosting.network-native"));
                    }
                    if ui.small_button(tr("hosting.remove")).clicked() {
                        remove = Some(i);
                    }
                });
            });
        }
        if let Some(i) = remove {
            networks.remove(i);
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("hosting.network-subnet"));
            ui.add(
                egui::TextEdit::singleline(&mut state.hosting.group_network_subnet)
                    .desired_width(170.0),
            );
            egui::ComboBox::from_id_salt("group-network-owned-subnet")
                .selected_text(tr("hosting.network-owned-ranges"))
                .show_ui(ui, |ui| {
                    for pool in sim
                        .provider()
                        .pools()
                        .iter()
                        .filter(|p| p.prefix.length() <= 30)
                    {
                        if ui
                            .selectable_label(false, pool.prefix.to_string())
                            .clicked()
                        {
                            state.hosting.group_network_subnet = pool.prefix.to_string();
                        }
                    }
                });
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("hosting.group-interface-name"));
            ui.add(
                egui::TextEdit::singleline(&mut state.hosting.group_network_interface)
                    .desired_width(90.0),
            );
            ui.checkbox(
                &mut state.hosting.group_network_tagged,
                tr("hosting.network-tagged"),
            );
            if state.hosting.group_network_tagged {
                state.hosting.group_network_vlan = state.hosting.group_network_vlan.max(1);
                ui.add(egui::DragValue::new(&mut state.hosting.group_network_vlan).range(1..=4094));
            } else {
                ui.label(tr("hosting.network-native"));
            }
            let subnet = state
                .hosting
                .group_network_subnet
                .trim()
                .parse::<Ipv4Prefix>()
                .ok()
                .filter(|r| r.length() <= 30 && sim.provider().owns(*r))
                .filter(|r| {
                    networks
                        .iter()
                        .all(|n| !n.subnet.contains_prefix(*r) && !r.contains_prefix(n.subnet))
                });
            if ui
                .add_enabled(
                    subnet.is_some() && !state.hosting.group_network_interface.trim().is_empty(),
                    egui::Button::new(tr("hosting.network-add")),
                )
                .clicked()
            {
                networks.push(HostingGroupNetwork {
                    interface: state.hosting.group_network_interface.trim().into(),
                    subnet: subnet.unwrap(),
                    vlan: state
                        .hosting
                        .group_network_tagged
                        .then_some(VlanId(state.hosting.group_network_vlan)),
                });
                state.hosting.group_network_subnet.clear();
            }
        });
    }
}

fn summary(sim: &NetworkSim, group: &HostingNodeGroup) -> String {
    tr_args(
        "hosting.group-counts",
        &[
            group.nodes.len().to_string(),
            group
                .nodes
                .iter()
                .filter(|id| sim.hosting_node_eligible(**id))
                .count()
                .to_string(),
        ],
    )
}

pub(super) fn product_selection(ui: &mut egui::Ui, sim: &NetworkSim, product: &mut HostingProduct) {
    ui.strong(tr("hosting.groups"));
    ui.label(tr("hosting.group-product-guide"));
    for group in sim.hosting.node_groups.values() {
        let mut selected = product.eligible_groups.contains(&group.id);
        if ui
            .checkbox(
                &mut selected,
                tr_args(
                    "hosting.group-product-row",
                    &[group.name.clone(), summary(sim, group)],
                ),
            )
            .changed()
        {
            if selected {
                product.eligible_groups.insert(group.id);
            } else {
                product.eligible_groups.remove(&group.id);
            }
        }
    }
    ui.separator();
}

pub(super) fn node_membership(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    node: DeviceId,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    let current = sim.hosting_node_group(node);
    let mut selected = current.map(|g| g.id);
    egui::ComboBox::from_id_salt(("hosting-node-group", node))
        .selected_text(current.map_or_else(|| tr("hosting.group-none"), |g| g.name.clone()))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut selected, None, tr("hosting.group-none"));
            for group in sim.hosting.node_groups.values() {
                ui.selectable_value(&mut selected, Some(group.id), &group.name);
            }
        });
    if selected != current.map(|g| g.id) {
        command(
            actions,
            HostingCommand::SetNodeGroup {
                node,
                group: selected,
            },
        );
        state.hosting.policy_loaded_for = None;
    }
    if let Some(group) = current {
        ui.label(tr_args(
            "hosting.group-inherited",
            std::slice::from_ref(&group.name),
        ));
        if ui.button(tr("hosting.group-edit")).clicked() {
            load(state, group.clone());
            state.hosting.tab = 7;
        }
    }
    ui.separator();
}
