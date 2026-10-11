use super::*;

pub(super) fn show(ui: &mut egui::Ui, sim: &NetworkSim, state: &mut UiState) {
    ui.label(tr("hosting.assigned-ipv4-guide"));
    ui.horizontal(|ui| {
        ui.label(tr("hosting.ip-node-filter"));
        egui::ComboBox::from_id_salt("hosting-assignment-node")
            .selected_text(
                state
                    .hosting
                    .ip_node_filter
                    .and_then(|id| sim.device(id))
                    .map_or_else(|| tr("hosting.all-nodes"), |d| d.name.clone()),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut state.hosting.ip_node_filter,
                    None,
                    tr("hosting.all-nodes"),
                );
                for device in sim
                    .devices()
                    .filter(|d| matches!(d.kind, DeviceKind::Server(_)))
                {
                    ui.selectable_value(
                        &mut state.hosting.ip_node_filter,
                        Some(device.id),
                        &device.name,
                    );
                }
            });
    });
    for pool in sim.provider().pools() {
        let u = sim.hosting_range_usage(pool.prefix);
        ui.label(tr_args(
            "hosting.range-row",
            &[
                pool.prefix.to_string(),
                u.total.to_string(),
                u.allocated.to_string(),
                u.available.to_string(),
            ],
        ));
    }
    let rows: Vec<_> = sim
        .hosting_ipv4_assignments()
        .into_iter()
        .filter(|r| {
            state
                .hosting
                .ip_node_filter
                .is_none_or(|id| r.node == Some(id))
        })
        .collect();
    egui::ScrollArea::vertical()
        .id_salt("hosting-assigned-ips")
        .max_height(ui.available_height())
        .auto_shrink([false, false])
        .show_rows(ui, 65.0, rows.len(), |ui, indices| {
            for i in indices {
                let row = &rows[i];
                ui.push_id(row.address, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.monospace(format!("{} · {}", row.address, row.range));
                        if let Some(service) = row.service {
                            ui.label(tr_args(
                                "hosting.ip-service",
                                &[
                                    service.to_string(),
                                    row.customer.unwrap().to_string(),
                                    row.vm.map_or_else(
                                        || tr("hosting.ip-retained"),
                                        |id| format!("VM {id}"),
                                    ),
                                ],
                            ));
                            if ui.small_button(tr("hosting.view-service")).clicked() {
                                state.hosting.selected_service = Some(service);
                                state.hosting.tab = 2;
                            }
                        } else {
                            ui.label(tr(if row.node.is_some() {
                                "hosting.ip-infrastructure"
                            } else {
                                "hosting.ip-reserved"
                            }));
                        }
                    });
                    if let Some(node) = row.node {
                        ui.horizontal(|ui| {
                            ui.label(
                                sim.device(node)
                                    .map_or_else(|| node.to_string(), |d| d.name.clone()),
                            );
                            if let Some(port) = row.port.and_then(|p| sim.port(p)) {
                                ui.monospace(&port.name);
                            }
                            if ui
                                .small_button(tr_args("hosting.open-console", &[node.to_string()]))
                                .clicked()
                            {
                                state.terminal_windows.insert(node);
                                state.terminal_window_focus.insert(node);
                            }
                        });
                    }
                    ui.separator();
                });
            }
        });
}
