use super::*;

pub(super) fn show(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    node: DeviceId,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    ui.strong(tr("hosting.node-ip-ranges"));
    ui.label(tr("hosting.node-ip-guide"));
    let mut inherit = state.hosting.node_ipv4_ranges.is_none();
    if ui
        .checkbox(&mut inherit, tr("hosting.node-ip-inherit"))
        .changed()
    {
        state.hosting.node_ipv4_ranges = (!inherit).then(Default::default);
    }
    if let Some(ranges) = &mut state.hosting.node_ipv4_ranges {
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
            ui.text_edit_singleline(&mut state.hosting.node_range_input);
            let prefix = state
                .hosting
                .node_range_input
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
                state.hosting.node_range_input.clear();
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
    if ui.button(tr("hosting.node-ip-apply")).clicked() {
        command(
            actions,
            HostingCommand::SetNodeIpv4Ranges {
                node,
                ranges: state.hosting.node_ipv4_ranges.clone(),
            },
        );
    }
    for port in sim.device(node).into_iter().flat_map(|d| d.ports()) {
        let p = sim.port(*port).unwrap();
        if p.name == "mgmt0" {
            continue;
        }
        if let PortConfig::Server(config) = &p.config {
            for address in config.addresses() {
                ui.monospace(format!(
                    "{} · {}/{} · {} {} · VLAN {} ({})",
                    p.name,
                    address.address,
                    address.prefix,
                    tr("hosting.node-ip-gateway"),
                    address
                        .gateway
                        .map_or_else(|| "—".into(), |g| g.to_string()),
                    address.vlan.unwrap_or(VlanId(1)).0,
                    tr(
                        if config
                            .tagged_vlans
                            .contains(&address.vlan.unwrap_or(VlanId(1)))
                        {
                            "hosting.network-tagged"
                        } else {
                            "hosting.network-native"
                        }
                    )
                ));
            }
        }
    }
}
