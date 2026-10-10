use super::*;

pub(super) fn show(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    node: DeviceId,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    ui.separator();
    ui.strong(tr("hosting.interface-networks"));
    ui.label(tr("hosting.interface-networks-guide"));
    let ports: Vec<_> = sim
        .device(node)
        .into_iter()
        .flat_map(|d| d.ports())
        .copied()
        .filter(|id| {
            sim.port(*id)
                .is_some_and(|p| p.name != "mgmt0" && matches!(p.config, PortConfig::Server(_)))
        })
        .collect();
    let mut automatic = state.hosting.node_networks.is_none();
    if ui
        .checkbox(&mut automatic, tr("hosting.interface-networks-auto"))
        .changed()
    {
        state.hosting.node_networks = (!automatic).then(Vec::new);
    }
    if let Some(networks) = &mut state.hosting.node_networks {
        let mut remove = None;
        for (i, network) in networks.iter_mut().enumerate() {
            ui.push_id(("node-network", i), |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.monospace(network.subnet.to_string());
                    interface(ui, sim, &ports, &mut network.port, i);
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
        if state
            .hosting
            .node_network_port
            .is_none_or(|id| !ports.contains(&id))
        {
            state.hosting.node_network_port = ports.first().copied();
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(tr("hosting.network-subnet"));
            ui.add(
                egui::TextEdit::singleline(&mut state.hosting.node_network_subnet)
                    .desired_width(170.0),
            );
            egui::ComboBox::from_id_salt("node-network-owned-subnet")
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
                            state.hosting.node_network_subnet = pool.prefix.to_string();
                        }
                    }
                });
        });
        ui.horizontal_wrapped(|ui| {
            if let Some(port) = &mut state.hosting.node_network_port {
                interface(ui, sim, &ports, port, "new");
            }
            ui.checkbox(
                &mut state.hosting.node_network_tagged,
                tr("hosting.network-tagged"),
            );
            if state.hosting.node_network_tagged {
                state.hosting.node_network_vlan = state.hosting.node_network_vlan.max(1);
                ui.add(egui::DragValue::new(&mut state.hosting.node_network_vlan).range(1..=4094));
            } else {
                ui.label(tr("hosting.network-native"));
            }
            let subnet = state
                .hosting
                .node_network_subnet
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
                    subnet.is_some() && state.hosting.node_network_port.is_some(),
                    egui::Button::new(tr("hosting.network-add")),
                )
                .clicked()
            {
                networks.push(HostingNodeNetwork {
                    port: state.hosting.node_network_port.unwrap(),
                    subnet: subnet.unwrap(),
                    vlan: state
                        .hosting
                        .node_network_tagged
                        .then_some(VlanId(state.hosting.node_network_vlan)),
                });
                state.hosting.node_network_subnet.clear();
            }
        });
    }
    if ui.button(tr("hosting.network-apply")).clicked() {
        command(
            actions,
            HostingCommand::SetNodeNetworks {
                node,
                networks: state.hosting.node_networks.clone(),
            },
        );
    }
}

fn interface(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    ports: &[PortId],
    selected: &mut PortId,
    salt: impl std::hash::Hash + std::fmt::Debug,
) {
    egui::ComboBox::from_id_salt(("node-network-interface", salt))
        .selected_text(
            sim.port(*selected)
                .map_or_else(|| selected.to_string(), |p| p.name.clone()),
        )
        .show_ui(ui, |ui| {
            for id in ports {
                ui.selectable_value(selected, *id, &sim.port(*id).unwrap().name);
            }
        });
}
