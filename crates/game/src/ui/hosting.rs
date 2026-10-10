use crate::app::{UiAction, UiState};
use crate::localization::{tr, tr_args};
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::*;
mod addressing;
mod assignments;
mod node_networks;
mod status;

fn command(actions: &mut MessageWriter<UiAction>, command: HostingCommand) {
    actions.write(UiAction::NetworkCommand(Command::Hosting(command)));
}
fn number(
    ui: &mut egui::Ui,
    label: impl Into<egui::WidgetText>,
    value: &mut impl egui::emath::Numeric,
    range: std::ops::RangeInclusive<f64>,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).range(range));
    });
}
fn date(ms: u64) -> String {
    tr_args(
        "hosting.date",
        &[
            (ms / 86_400_000 + 1).to_string(),
            format!("{:02}", ms / 3_600_000 % 24),
            format!("{:02}", ms / 60_000 % 60),
        ],
    )
}

pub(super) fn show(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    if !state.hosting.open {
        return;
    }
    let mut open = true;
    egui::Window::new(tr("hosting.title"))
        .id(egui::Id::new("hosting-management"))
        .open(&mut open)
        .default_size(egui::vec2(1050.0, 740.0))
        .show(viewport, |ui| {
            ui.horizontal(|ui| {
                ui.strong(date(sim.hosting.now_ms));
                ui.label(tr_args(
                    "hosting.summary",
                    &[
                        format!("{:.0}", sim.hosting.reputation * 100.0),
                        (sim.hosting.vms.len()).to_string(),
                        (sim.hosting.payments.len()).to_string(),
                    ],
                ));
            });
            if let Err(reason) = sim.hosting_provider_site_status() {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(egui::Color32::LIGHT_RED, reason);
                    if ui.small_button(tr("hosting.configure-site")).clicked() {
                        state.hosting.tab = 5;
                    }
                });
            }
            ui.horizontal(|ui| {
                for (i, label) in [
                    tr("hosting.products"),
                    tr("hosting.nodes"),
                    tr("hosting.services"),
                    tr("hosting.customers"),
                    tr("hosting.market"),
                    tr("hosting.provider-site"),
                    tr("hosting.assigned-ipv4"),
                ]
                .iter()
                .enumerate()
                {
                    ui.selectable_value(&mut state.hosting.tab, i, label.clone());
                }
            });
            ui.separator();
            match state.hosting.tab {
                0 => products(ui, sim, state, actions),
                1 => nodes(ui, sim, state, actions),
                2 => services(ui, sim, state, actions),
                3 => customers(ui, sim),
                4 => market(ui, sim, state, actions),
                5 => provider_site(ui, sim, state, actions),
                _ => assignments::show(ui, sim, state),
            }
        });
    state.hosting.open = open;
}
fn load_provider_draft(sim: &NetworkSim, state: &mut UiState) {
    state.hosting.provider_public_ipv4 = sim
        .hosting
        .provider_site_ipv4
        .map(|ip| ip.to_string())
        .unwrap_or_default();
    state.hosting.provider_loaded = true;
}
fn provider_site(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    if !state.hosting.provider_loaded {
        load_provider_draft(sim, state);
    }
    ui.label(tr("hosting.provider-guide"));
    ui.monospace("./setup-main-hoster-server.sh");
    ui.horizontal(|ui| {
        ui.label(tr("hosting.site-public-ip"));
        ui.text_edit_singleline(&mut state.hosting.provider_public_ipv4);
    });
    let address = state.hosting.provider_public_ipv4.trim().parse().ok();
    let validation = address
        .ok_or_else(|| tr("hosting.site-invalid-ip"))
        .and_then(|ip| sim.validate_hosting_provider_site_address(ip));
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                validation.is_ok(),
                egui::Button::new(tr("hosting.apply-provider")),
            )
            .clicked()
        {
            command(actions, HostingCommand::SetProviderSiteAddress(address));
        }
        if ui.button(tr("hosting.reload-provider")).clicked() {
            load_provider_draft(sim, state);
        }
        if ui
            .add_enabled(
                sim.hosting.provider_site_ipv4.is_some(),
                egui::Button::new(tr("hosting.detach-provider")),
            )
            .clicked()
        {
            command(actions, HostingCommand::SetProviderSiteAddress(None));
        }
    });
    if let Err(error) = validation {
        ui.colored_label(egui::Color32::LIGHT_RED, error);
    }
    ui.separator();
    match sim.hosting_provider_site_status() {
        Ok(()) => {
            ui.colored_label(egui::Color32::LIGHT_GREEN, tr("hosting.site-online"));
        }
        Err(error) => {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
    }
    if let Some(id) = sim.hosting_provider_server() {
        if let Some(server) = sim.device(id) {
            ui.label(tr_args(
                "hosting.site-destination",
                std::slice::from_ref(&server.name),
            ));
        }
        if ui
            .button(tr_args("hosting.open-console", &[id.to_string()]))
            .clicked()
        {
            state.terminal_windows.insert(id);
            state.terminal_window_focus.insert(id);
        }
    }
    let traffic = sim.hosting.site_traffic;
    ui.label(tr_args(
        "hosting.site-traffic",
        &[
            format!("{:.3}", traffic.delivered_mbps),
            format!("{:.1}", traffic.latency_ms),
            format!("{:.1}", traffic.loss * 100.0),
            traffic.bytes.to_string(),
        ],
    ));
    ui.label(tr("hosting.prepare-guide"));
    ui.label(tr("hosting.site-outage-guide"));
    ui.label(tr("hosting.site-service-guide"));
    ui.monospace("systemctl status hosting-provider");
}

fn products(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(260.0);
            if ui.button(tr("hosting.create")).clicked() {
                state.hosting.product = HostingProduct::default();
            }
            egui::ScrollArea::vertical()
                .id_salt("hosting-products")
                .max_height(590.0)
                .show(ui, |ui| {
                    for p in sim.hosting.products.values() {
                        if ui
                            .selectable_label(
                                state.hosting.product.id == p.id,
                                tr_args(
                                    "hosting.product-row",
                                    &[
                                        (p.name).to_string(),
                                        format!("{:.2}", p.monthly_price_cents as f64 / 100.0),
                                    ],
                                ),
                            )
                            .clicked()
                        {
                            state.hosting.product = p.clone();
                        }
                        if let Some(s) = sim.hosting.market_stats.get(&p.id) {
                            ui.small(tr_args(
                                "hosting.market-stats",
                                &[
                                    format!("{:.2}", s.attractiveness),
                                    format!("{:.1}", s.expected_orders_per_day),
                                    (s.sales).to_string(),
                                ],
                            ));
                        }
                        ui.small(if p.published {
                            tr("hosting.published")
                        } else {
                            tr("hosting.unpublished")
                        });
                        if p.published
                            && let Err(reason) = sim.hosting_availability(p.id)
                        {
                            ui.small(reason);
                        }
                        ui.separator();
                    }
                });
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("hosting-product-editor")
            .max_height(610.0)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    let p = &mut state.hosting.product;
                    ui.text_edit_singleline(&mut p.name);
                    egui::ComboBox::from_label(tr("hosting.category"))
                        .selected_text(category_name(p.category))
                        .show_ui(ui, |ui| {
                            for c in [
                                HostingCategory::SharedCpu,
                                HostingCategory::DedicatedCpu,
                                HostingCategory::Storage,
                                HostingCategory::DedicatedServer,
                            ] {
                                if ui
                                    .selectable_value(&mut p.category, c, category_name(c))
                                    .changed()
                                    && c == HostingCategory::DedicatedCpu
                                {
                                    p.cpu_policy = CpuPolicy::Dedicated;
                                }
                            }
                        });
                    number(
                        ui,
                        tr("hosting.price"),
                        &mut p.monthly_price_cents,
                        1.0..=100_000_000.0,
                    );
                    number(ui, tr("hosting.vcpu"), &mut p.vcpu, 1.0..=4096.0);
                    number(
                        ui,
                        tr("hosting.ram"),
                        &mut p.ram_mb,
                        128.0..=1_000_000_000.0,
                    );
                    number(
                        ui,
                        tr("hosting.disk"),
                        &mut p.disk_gb,
                        1.0..=1_000_000_000.0,
                    );
                    ui.horizontal(|ui| {
                        ui.label(tr("hosting.storage"));
                        ui.selectable_value(&mut p.storage, HostingStorage::Ssd, "SSD");
                        ui.selectable_value(&mut p.storage, HostingStorage::Hdd, "HDD");
                    });
                    ui.checkbox(&mut p.thin, tr("hosting.thin"));
                    number(
                        ui,
                        tr("hosting.bandwidth"),
                        &mut p.port_mbps,
                        1.0..=100000.0,
                    );
                    let mut limited = p.traffic_gb.is_some();
                    if ui
                        .checkbox(&mut limited, tr("hosting.limit-traffic"))
                        .changed()
                    {
                        p.traffic_gb = limited.then_some(1000);
                    }
                    if let Some(gb) = &mut p.traffic_gb {
                        number(ui, tr("hosting.traffic"), gb, 1.0..=1000000.0);
                    }
                    number(ui, tr("hosting.ipv4"), &mut p.ipv4_count, 0.0..=16.0);
                    ui.horizontal(|ui| {
                        ui.label(tr("hosting.cpu-allocation"));
                        ui.selectable_value(
                            &mut p.cpu_policy,
                            CpuPolicy::Shared,
                            tr("hosting.shared"),
                        );
                        ui.selectable_value(
                            &mut p.cpu_policy,
                            CpuPolicy::Dedicated,
                            tr("hosting.dedicated"),
                        );
                    });
                    number(ui, tr("hosting.weight"), &mut p.cpu_weight, 1.0..=10000.0);
                    number(
                        ui,
                        tr("hosting.cpu-limit"),
                        &mut p.cpu_limit_percent,
                        1.0..=100.0,
                    );
                    number(
                        ui,
                        tr("hosting.cpu-burst"),
                        &mut p.cpu_burst_percent,
                        f64::from(p.cpu_limit_percent)..=100.0,
                    );
                    number(
                        ui,
                        tr("hosting.burst-credit"),
                        &mut p.burst_seconds,
                        0.0..=86400.0,
                    );
                    number(ui, tr("hosting.iops"), &mut p.disk_iops, 1.0..=10000000.0);
                    number(
                        ui,
                        tr("hosting.throughput"),
                        &mut p.disk_mb_s,
                        1.0..=1000000.0,
                    );
                    ui.separator();
                    ui.strong(tr("hosting.eligible"));
                    let mut servers: Vec<_> = sim
                        .devices()
                        .filter(|d| matches!(d.kind, DeviceKind::Server(_)))
                        .filter(|d| !sim.hosting_provider_installed(d.id))
                        .collect();
                    servers.sort_by_key(|d| d.id);
                    for d in servers {
                        let mut selected = p.eligible_nodes.contains(&d.id);
                        if ui
                            .checkbox(
                                &mut selected,
                                tr_args(
                                    "hosting.server-ready",
                                    &[
                                        (d.name).to_string(),
                                        if sim.hosting_node_eligible(d.id) {
                                            tr("hosting.ready")
                                        } else {
                                            tr("hosting.not-eligible")
                                        },
                                    ],
                                ),
                            )
                            .changed()
                        {
                            if selected {
                                p.eligible_nodes.insert(d.id);
                            } else {
                                p.eligible_nodes.remove(&d.id);
                            }
                        }
                    }
                    ui.separator();
                    ui.strong(tr("hosting.ranges"));
                    for pool in sim.provider().pools() {
                        let usage = sim.hosting_range_usage(pool.prefix);
                        let mut selected = p.ipv4_ranges.contains(&pool.prefix);
                        if ui
                            .add_enabled(
                                pool.prefix.length() <= 30,
                                egui::Checkbox::new(
                                    &mut selected,
                                    tr_args(
                                        "hosting.range-row",
                                        &[
                                            (pool.prefix).to_string(),
                                            (usage.total).to_string(),
                                            (usage.allocated).to_string(),
                                            (usage.available).to_string(),
                                        ],
                                    ),
                                ),
                            )
                            .changed()
                        {
                            if selected {
                                p.ipv4_ranges.insert(pool.prefix);
                            } else {
                                p.ipv4_ranges.remove(&pool.prefix);
                            }
                        }
                    }
                    if sim.provider().pools().is_empty() {
                        ui.label(tr("hosting.buy-ranges"));
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(tr("hosting.save")).clicked() {
                            command(actions, HostingCommand::SaveProduct(p.clone()));
                        }
                        if p.id != 0 {
                            let published =
                                sim.hosting.products.get(&p.id).is_some_and(|p| p.published);
                            p.published = published;
                            if ui
                                .button(if published {
                                    tr("hosting.unpublish")
                                } else {
                                    tr("hosting.publish")
                                })
                                .clicked()
                            {
                                command(
                                    actions,
                                    HostingCommand::Publish {
                                        product: p.id,
                                        published: !published,
                                    },
                                );
                            }
                            if ui.button(tr("hosting.delete")).clicked() {
                                command(actions, HostingCommand::DeleteProduct(p.id));
                                *p = HostingProduct::default();
                            }
                        }
                    });
                    if let Err(reason) = sim.validate_hosting_product(p, true) {
                        ui.small(reason);
                    }
                    if p.id != 0 {
                        ui.weak(tr("hosting.attractiveness-guide"));
                        ui.collapsing(tr("hosting.category-attractiveness"), |ui| {
                            for category in CustomerCategory::ALL {
                                ui.label(format!(
                                    "{}: {:.2}",
                                    customer_category_name(category),
                                    sim.hosting_attractiveness(p, category)
                                ));
                            }
                        });
                    }
                });
            });
    });
}
fn nodes(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(260.0);
            let mut servers: Vec<_> = sim
                .devices()
                .filter(|d| matches!(d.kind, DeviceKind::Server(_)))
                .filter(|d| !sim.hosting_provider_installed(d.id))
                .collect();
            servers.sort_by_key(|d| d.id);
            for d in servers {
                ui.selectable_value(&mut state.hosting.selected_node, Some(d.id), &d.name);
                let prepared = sim
                    .hosting
                    .nodes
                    .get(&d.id)
                    .is_some_and(|n| n.prepared_at_ms.is_some());
                ui.small(tr_args(
                    "hosting.node-status",
                    &[
                        if prepared {
                            tr("hosting.prepared")
                        } else {
                            tr("hosting.unprepared")
                        },
                        if sim.hosting_management_connected(d.id) {
                            tr("hosting.connected")
                        } else {
                            tr("hosting.unavailable")
                        },
                    ],
                ));
                if ui
                    .button(tr_args("hosting.open-console", &[(d.id).to_string()]))
                    .clicked()
                {
                    state.terminal_windows.insert(d.id);
                    state.terminal_window_focus.insert(d.id);
                }
            }
            ui.label(tr("hosting.prepare-guide"));
        });
        ui.separator();
        if let Some(id) = state.hosting.selected_node {
            if state.hosting.policy_loaded_for != Some(id) {
                state.hosting.policy = sim
                    .hosting
                    .nodes
                    .get(&id)
                    .map(|n| n.policy.clone())
                    .unwrap_or_default();
                state.hosting.policy_loaded_for = Some(id);
                state.hosting.node_ipv4_ranges = sim
                    .hosting
                    .nodes
                    .get(&id)
                    .and_then(|n| n.ipv4_ranges.clone());
                state.hosting.node_range_input.clear();
                state.hosting.node_networks =
                    sim.hosting.nodes.get(&id).and_then(|n| n.networks.clone());
                state.hosting.node_network_port = None;
                state.hosting.node_network_subnet.clear();
                state.hosting.node_network_vlan = 1;
            }
            egui::ScrollArea::vertical()
                .id_salt("hosting-node-policy")
                .max_height(600.0)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        addressing::show(ui, sim, id, state, actions);
                        node_networks::show(ui, sim, id, state, actions);
                        ui.separator();
                        let policy = &mut state.hosting.policy;
                        for (label, value) in [
                            (
                                tr("hosting.cpu-overcommit"),
                                &mut policy.cpu_overcommit_percent,
                            ),
                            (
                                tr("hosting.ram-overcommit"),
                                &mut policy.ram_overcommit_percent,
                            ),
                            (
                                tr("hosting.disk-overcommit"),
                                &mut policy.storage_overcommit_percent,
                            ),
                            (
                                tr("hosting.io-overcommit"),
                                &mut policy.io_overcommit_percent,
                            ),
                            (
                                tr("hosting.network-overcommit"),
                                &mut policy.network_overcommit_percent,
                            ),
                        ] {
                            number(ui, label, value, 100.0..=10000.0);
                        }
                        number(
                            ui,
                            tr("hosting.reserve-cpu"),
                            &mut policy.reserved_cpu_mhz,
                            0.0..=100000000.0,
                        );
                        number(
                            ui,
                            tr("hosting.reserve-ram"),
                            &mut policy.reserved_ram_mb,
                            0.0..=100000000.0,
                        );
                        number(
                            ui,
                            tr("hosting.reserve-disk"),
                            &mut policy.reserved_disk_gb,
                            0.0..=1000000.0,
                        );
                        number(ui, tr("hosting.swap"), &mut policy.swap_mb, 0.0..=1000000.0);
                        ui.checkbox(&mut policy.ballooning, tr("hosting.ballooning"));
                        ui.checkbox(&mut policy.thin_provisioning, tr("hosting.allow-thin"));
                        if ui.button(tr("hosting.apply-policy")).clicked() {
                            command(
                                actions,
                                HostingCommand::SetNodePolicy {
                                    node: id,
                                    policy: policy.clone(),
                                },
                            );
                        }
                        if let Some(n) = sim.hosting.nodes.get(&id) {
                            let u = &n.usage;
                            let c = sim.hosting_capacity(id, HostingStorage::Ssd);
                            let h = sim.hosting_capacity(id, HostingStorage::Hdd);
                            ui.separator();
                            ui.label(tr_args(
                                "hosting.node-cpu",
                                &[
                                    format!("{:.0}", u.cpu_demand_mhz),
                                    format!("{:.0}", c.cpu_mhz),
                                    format!("{:.0}", u.cpu_used_mhz),
                                    (u.sold_vcpu).to_string(),
                                    format!(
                                        "{:.2}",
                                        u.sold_vcpu as f64 * VCPU_MHZ / c.cpu_mhz.max(1.0)
                                    ),
                                ],
                            ));
                            ui.label(tr_args(
                                "hosting.node-ram",
                                &[
                                    format!("{:.0}", u.memory_used_mb),
                                    format!("{:.0}", c.ram_mb),
                                    (u.sold_ram_mb).to_string(),
                                    format!("{:.2}", u.sold_ram_mb as f64 / c.ram_mb.max(1.0)),
                                    format!("{:.0}", u.swap_used_mb),
                                    (u.oom_events).to_string(),
                                ],
                            ));
                            ui.label(tr_args(
                                "hosting.node-disk",
                                &[
                                    format!("{:.2}", u.physical_disk_gb),
                                    format!("{:.0}", c.disk_gb + h.disk_gb),
                                    (u.virtual_disk_gb).to_string(),
                                    format!(
                                        "{:.2}",
                                        u.virtual_disk_gb as f64 / (c.disk_gb + h.disk_gb).max(1.0)
                                    ),
                                    format!("{:.0}", u.disk_iops),
                                    format!("{:.1}", u.disk_mb_s),
                                ],
                            ));
                            ui.label(tr_args(
                                "hosting.node-network",
                                &[
                                    format!("{:.1}", u.network_mbps),
                                    format!("{:.0}", c.network_mbps),
                                ],
                            ));
                        }
                    });
                });
        }
    });
}
fn services(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    ui.label(tr("hosting.vm-lifecycle-guide"));
    if state.hosting.selected_service.is_some() && ui.button(tr("hosting.all-services")).clicked() {
        state.hosting.selected_service = None;
    }
    let services: Vec<_> = sim
        .hosting
        .services
        .values()
        .filter(|s| state.hosting.selected_service.is_none_or(|id| s.id == id))
        .collect();
    egui::ScrollArea::vertical()
        .id_salt("hosting-services")
        .max_height(610.0)
        .show_rows(ui, 250.0, services.len(), |ui, rows| {
            for i in rows {
                let s = services[i];
                ui.push_id(s.id, |ui| {
                    ui.strong(tr_args(
                        "hosting.service-row",
                        &[
                            (s.id).to_string(),
                            (s.customer).to_string(),
                            (s.product).to_string(),
                            (date(s.expires_at_ms)).to_string(),
                            (s.renewals).to_string(),
                        ],
                    ));
                    if let Some(vm) = s.vm.and_then(|id| sim.hosting.vms.get(&id)) {
                        let row = tr_args(
                            "hosting.vm-row",
                            &[
                                (vm.id).to_string(),
                                (vm.host).to_string(),
                                (vm_power_name(vm.power)).to_string(),
                                format!("{:?}", vm.ips),
                            ],
                        );
                        ui.label(row);
                        status::metrics(ui, vm, s.satisfaction);
                        ui.horizontal(|ui| {
                            let enabled = sim.hosting_node_eligible(vm.host);
                            if ui
                                .add_enabled(enabled, egui::Button::new(tr("hosting.start")))
                                .clicked()
                            {
                                command(
                                    actions,
                                    HostingCommand::SetVmPower {
                                        vm: vm.id,
                                        power: VmPower::Running,
                                    },
                                );
                            }
                            if ui
                                .add_enabled(enabled, egui::Button::new(tr("hosting.shutdown")))
                                .clicked()
                            {
                                command(
                                    actions,
                                    HostingCommand::SetVmPower {
                                        vm: vm.id,
                                        power: VmPower::Stopped,
                                    },
                                );
                            }
                            if ui
                                .add_enabled(enabled, egui::Button::new(tr("hosting.reboot")))
                                .clicked()
                            {
                                command(actions, HostingCommand::RebootVm(vm.id));
                            }
                            if ui
                                .add_enabled(enabled, egui::Button::new(tr("hosting.destroy-vm")))
                                .on_hover_text(tr("hosting.destroy-vm-help"))
                                .clicked()
                            {
                                command(actions, HostingCommand::DestroyServiceVm(s.id));
                            }
                        });
                    } else if let Some(at) = s.terminated_at_ms {
                        ui.label(tr_args("hosting.cancelled", &[(date(at)).to_string()]));
                    } else {
                        ui.colored_label(egui::Color32::LIGHT_RED, tr("hosting.vm-destroyed"));
                        if let Some(lease) = sim.hosting_service_provisioning(s.id) {
                            ui.monospace(tr_args(
                                "hosting.retained-ips",
                                &[lease.host.to_string(), format!("{:?}", lease.ips)],
                            ));
                        }
                        let available = sim.hosting_recreation_availability(s.id);
                        if ui
                            .add_enabled(
                                available.is_ok(),
                                egui::Button::new(tr("hosting.recreate-vm")),
                            )
                            .on_hover_text(tr("hosting.recreate-vm-help"))
                            .clicked()
                        {
                            command(actions, HostingCommand::RecreateServiceVm(s.id));
                        }
                        if let Err(reason) = available {
                            ui.label(reason);
                        }
                    }
                    if s.terminated_at_ms.is_none() {
                        let refund = sim.hosting_refund_cents(s.id).unwrap_or(0);
                        if ui
                            .button(tr_args(
                                "hosting.refund-release",
                                &[format!("{:.2}", refund as f64 / 100.0)],
                            ))
                            .clicked()
                        {
                            command(actions, HostingCommand::RefundService(s.id));
                        }
                    } else if let Some(refund) =
                        sim.hosting.refunds.iter().find(|r| r.service == s.id)
                    {
                        ui.label(tr_args(
                            "hosting.refund-row",
                            &[
                                date(refund.at_ms),
                                s.id.to_string(),
                                format!("{:.2}", refund.amount_cents as f64 / 100.0),
                            ],
                        ));
                    }
                    ui.separator();
                });
            }
        });
}
fn customers(ui: &mut egui::Ui, sim: &NetworkSim) {
    let customers: Vec<_> = sim.hosting.customers.values().collect();
    egui::ScrollArea::vertical()
        .id_salt("hosting-customers")
        .max_height(610.0)
        .show_rows(ui, 80.0, customers.len(), |ui, rows| {
            for i in rows {
                let c = customers[i];
                ui.label(tr_args(
                    "hosting.customer-row",
                    &[
                        (c.id).to_string(),
                        (customer_category_name(c.category)).to_string(),
                        format!("{:?}", c.services),
                        format!("{:.0}", c.satisfaction * 100.0),
                        format!("{:.0}", c.churn_probability * 100.0),
                    ],
                ));
                ui.small(tr_args(
                    "hosting.requirements",
                    &[
                        format!("{:.1}", c.requirements.cpu_mhz / VCPU_MHZ),
                        format!("{:.0}", c.requirements.ram_mb),
                        format!("{:.0}", c.requirements.disk_gb),
                        format!("{:.0}", c.requirements.iops),
                        format!("{:.0}", c.requirements.network_mbps),
                        format!("{:.2}", c.requirements.budget_cents / 100.0),
                    ],
                ));
                ui.separator();
            }
        });
}
fn market(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    if !state.hosting.market_loaded {
        state.hosting.market = sim.hosting.market.clone();
        state.hosting.market_loaded = true;
    }
    let market = &mut state.hosting.market;
    number(
        ui,
        tr("hosting.population"),
        &mut market.population,
        1.0..=1_000_000_000.0,
    );
    number(
        ui,
        tr("hosting.prospects"),
        &mut market.prospects_per_day,
        0.0..=100000.0,
    );
    number(
        ui,
        tr("hosting.time-scale"),
        &mut market.time_scale,
        0.0..=86400.0,
    );
    ui.strong(tr("hosting.periods"));
    let mut remove = None;
    for (i, period) in market.periods.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                ui.label(tr("hosting.months"));
                ui.add(egui::DragValue::new(&mut period.months).range(1..=120));
                ui.label(tr("hosting.probability-weight"));
                ui.add(egui::DragValue::new(&mut period.weight).range(1..=100000));
                if ui.button(tr("hosting.remove")).clicked() {
                    remove = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove {
        market.periods.remove(i);
    }
    if ui.button(tr("hosting.add-period")).clicked() {
        market.periods.push(SubscriptionOption {
            months: 1,
            weight: 1,
        });
    }
    if ui.button(tr("hosting.apply-market")).clicked() {
        command(actions, HostingCommand::SetMarket(market.clone()));
    }
    ui.separator();
    ui.strong(tr("hosting.refunds"));
    for refund in sim.hosting.refunds.iter().rev().take(10) {
        ui.label(tr_args(
            "hosting.refund-row",
            &[
                date(refund.at_ms),
                refund.service.to_string(),
                format!("{:.2}", refund.amount_cents as f64 / 100.0),
            ],
        ));
    }
    ui.strong(tr("hosting.payments"));
    egui::ScrollArea::vertical()
        .id_salt("hosting-payments")
        .max_height(350.0)
        .show(ui, |ui| {
            for payment in sim.hosting.payments.iter().rev().take(100) {
                ui.label(tr_args(
                    "hosting.payment-row",
                    &[
                        (date(payment.at_ms)).to_string(),
                        (payment.service).to_string(),
                        if payment.renewal {
                            tr("hosting.renewal")
                        } else {
                            tr("hosting.purchase")
                        },
                        (payment.months).to_string(),
                        format!("{:.2}", payment.amount_cents as f64 / 100.0),
                    ],
                ));
            }
        });
}

fn category_name(category: HostingCategory) -> String {
    tr(match category {
        HostingCategory::SharedCpu => "hosting.shared-cpu",
        HostingCategory::DedicatedCpu => "hosting.dedicated-cpu",
        HostingCategory::Storage => "hosting.storage-vps",
        HostingCategory::DedicatedServer => "hosting.dedicated-server",
    })
}

fn vm_power_name(power: VmPower) -> String {
    tr(match power {
        VmPower::Running => "hosting.vm-running",
        VmPower::Stopped => "hosting.vm-stopped",
        VmPower::Rebooting => "hosting.vm-rebooting",
        VmPower::Oom => "hosting.vm-oom",
    })
}
fn customer_category_name(category: CustomerCategory) -> String {
    tr(match category {
        CustomerCategory::Developer => "hosting.customer-developer",
        CustomerCategory::Website => "hosting.customer-website",
        CustomerCategory::Ecommerce => "hosting.customer-ecommerce",
        CustomerCategory::GameServer => "hosting.customer-game",
        CustomerCategory::Saas => "hosting.customer-saas",
        CustomerCategory::Database => "hosting.customer-database",
        CustomerCategory::FileStorage => "hosting.customer-storage",
        CustomerCategory::Backup => "hosting.customer-backup",
        CustomerCategory::HighPerformance => "hosting.customer-performance",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::SystemState;
    use bevy::prelude::*;
    use cloud_provider_sim::Command;

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
                    if state.hosting.tab == 1 {
                        egui::vec2(1050.0, 900.0)
                    } else {
                        egui::vec2(1800.0, 2400.0)
                    },
                )),
                events,
                ..Default::default()
            },
            |ui| match state.hosting.tab {
                1 => nodes(ui, sim, state, &mut writer.get_mut(world).unwrap()),
                2 => services(ui, sim, state, &mut writer.get_mut(world).unwrap()),
                6 => assignments::show(ui, sim, state),
                5 => provider_site(ui, sim, state, &mut writer.get_mut(world).unwrap()),
                _ => products(ui, sim, state, &mut writer.get_mut(world).unwrap()),
            },
        );
        let text = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(t) = &shape.shape {
                    Some((
                        t.galley.text().to_string(),
                        egui::Rect::from_min_size(t.pos, t.galley.size()).center(),
                    ))
                } else {
                    None
                }
            })
            .collect();
        output.textures_delta.clear();
        text
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
        let mut text = render(ctx, world, writer, sim, state, vec![]);
        if !text.iter().any(|(text, _)| text.starts_with(label)) {
            render(
                ctx,
                world,
                writer,
                sim,
                state,
                vec![
                    egui::Event::PointerMoved(egui::pos2(600.0, 400.0)),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        delta: egui::vec2(0.0, -2000.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            for _ in 0..30 {
                text = render(ctx, world, writer, sim, state, vec![]);
            }
        }
        let pos = text
            .iter()
            .find(|(text, _)| text.starts_with(label))
            .unwrap_or_else(|| panic!("Missing {label}: {text:?}"))
            .1;
        if state.hosting.tab == 1 {
            assert!(
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1050.0, 900.0))
                    .contains(pos),
                "Node control {label} is outside the viewport: {pos:?}"
            );
        }
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
    #[test]
    fn services_show_provider_failures_red_and_customer_quota_limits_normally() {
        let mut sim = NetworkSim::new();
        let plan = HostingProduct::default();
        sim.hosting.vms.insert(
            1,
            VirtualMachine {
                id: 1,
                customer: 1,
                host: DeviceId(1),
                product: 1,
                service: 1,
                plan,
                interfaces: vec![],
                ips: vec![],
                ip_ranges: vec![],
                physical_disk_gb: 30.0,
                disk_used_gb: 30.0,
                power: VmPower::Running,
                host_memory_failure: false,
                customer_restart_at_ms: 0,
                expires_at_ms: MONTH_MS,
                demand: WorkloadDemand::default(),
                workload_override: None,
                metrics: VmMetrics {
                    cpu_throttled: 0.9,
                    guest_memory_limited_mb: 3000.0,
                    guest_disk_full_gb: 1.0,
                    disk_queue_ops: 3000.0,
                    reachable: true,
                    quality: 1.0,
                    ..Default::default()
                },
                burst_credit_seconds: 0.0,
                reboot_until_ms: 0,
            },
        );
        sim.hosting.services.insert(
            1,
            HostingService {
                id: 1,
                customer: 1,
                product: 1,
                vm: Some(1),
                provisioning: None,
                retained_traffic_bytes: 0,
                purchased_at_ms: 0,
                expires_at_ms: MONTH_MS,
                monthly_price_cents: 500,
                renewals: 0,
                terminated_at_ms: None,
                satisfaction: 1.0,
                observed_quality: 1.0,
                availability: 1.0,
            },
        );
        let mut localization = crate::localization::Localization::default();
        for language in ["en", "ru"] {
            localization.select(language).unwrap();
            let _scope = localization.enter();
            for power in [
                VmPower::Running,
                VmPower::Stopped,
                VmPower::Oom,
                VmPower::Rebooting,
            ] {
                sim.hosting.vms.get_mut(&1).unwrap().power = power;
                for failure in [false, true] {
                    let m = &mut sim.hosting.vms.get_mut(&1).unwrap().metrics;
                    m.cpu_ready = if failure { 0.4 } else { 0.0 };
                    m.swapped_mb = if failure { 100.0 } else { 0.0 };
                    m.disk_ready = if failure { 0.5 } else { 0.0 };
                    m.write_failed_gb = if failure { 0.1 } else { 0.0 };
                    m.reachable = power == VmPower::Running && !failure;
                    m.provider_fault =
                        failure.then_some(HostingProviderFault::PublicNetworkUnavailable);
                    let ctx = egui::Context::default();
                    let mut world = World::new();
                    world.init_resource::<Messages<UiAction>>();
                    let mut writer = SystemState::<MessageWriter<UiAction>>::new(&mut world);
                    let mut labels = Vec::new();
                    for _ in 0..2 {
                        let mut output = ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    egui::vec2(1800.0, 800.0),
                                )),
                                ..Default::default()
                            },
                            |ui| {
                                services(
                                    ui,
                                    &sim,
                                    &mut UiState::default(),
                                    &mut writer.get_mut(&mut world).unwrap(),
                                )
                            },
                        );
                        labels = output
                            .shapes
                            .iter()
                            .filter_map(|shape| match &shape.shape {
                                egui::Shape::Text(t) => Some((
                                    t.galley.text().to_string(),
                                    t.galley.job.sections[0].format.color,
                                )),
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        output.textures_delta.clear();
                    }
                    for key in ["hosting.vm-cpu", "hosting.vm-disk", "hosting.vm-network"] {
                        let template = tr_args(key, &vec!["__value__".into(); 7]);
                        let prefix = template.split("__value__").next().unwrap();
                        let (_, color) = labels
                            .iter()
                            .find(|(s, _)| s.starts_with(prefix))
                            .unwrap_or_else(|| panic!("Missing {key} in {labels:?}"));
                        assert_eq!(*color == egui::Color32::LIGHT_RED, failure, "{key}");
                    }
                    let quota = tr_args("hosting.vm-guest-limits", &["3000".into(), "1.00".into()]);
                    let (_, color) = labels.iter().find(|(s, _)| s == &quota).unwrap();
                    assert_ne!(*color, egui::Color32::LIGHT_RED);
                    if !failure {
                        assert!(
                            labels
                                .iter()
                                .all(|(_, color)| *color != egui::Color32::LIGHT_RED)
                        );
                        if power != VmPower::Running {
                            assert!(
                                labels
                                    .iter()
                                    .any(|(s, _)| s == &tr("hosting.vm-customer-restart"))
                            );
                        }
                    }
                    if failure {
                        assert!(labels.iter().any(|(s, color)| {
                            s == &tr("hosting.vm-route-failed")
                                && *color == egui::Color32::LIGHT_RED
                        }));
                    }
                }
            }
        }
    }
    #[test]
    fn node_editor_applies_a_custom_slice_of_an_owned_24() {
        let mut sim = NetworkSim::new();
        sim.money = 100_000;
        let SimEvent::DeviceAdded(node) = sim.execute(Command::BuyServerFullPack).unwrap()[0]
        else {
            panic!()
        };
        sim.buy_public_ipv4_subnet(24).unwrap();
        let ctx = egui::Context::default();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut writer = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut state = UiState::default();
        state.hosting.tab = 1;
        state.hosting.selected_node = Some(node);
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            "Use product ranges automatically",
        );
        assert!(state.hosting.node_ipv4_ranges.as_ref().unwrap().is_empty());
        state.hosting.node_range_input = "203.0.113.1/29".into();
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            "Add subnet",
        );
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            "Apply node IPv4 ranges",
        );
        let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        let [UiAction::NetworkCommand(command)] = actions.as_slice() else {
            panic!("expected node range command")
        };
        sim.execute(command.clone()).unwrap();
        assert_eq!(
            sim.hosting.nodes[&node].ipv4_ranges,
            Some(["203.0.113.0/29".parse().unwrap()].into())
        );
    }

    #[test]
    fn product_editor_commits_selected_servers_and_multiple_ranges_then_publishes() {
        let mut sim = NetworkSim::new();
        sim.money = 100000;
        let SimEvent::DeviceAdded(host) = sim
            .execute(Command::BuyDevice {
                kind: DeviceTemplate::Server,
            })
            .unwrap()[0]
        else {
            panic!()
        };
        sim.buy_public_ipv4_pool().unwrap();
        sim.buy_public_ipv4_pool().unwrap();
        let host_name = sim.device(host).unwrap().name.clone();
        let ctx = egui::Context::default();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut writer = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut state = UiState::default();
        click(&ctx, &mut world, &mut writer, &sim, &mut state, &host_name);
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            "203.0.113.0/29",
        );
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            "203.0.113.8/29",
        );
        assert!(state.hosting.product.eligible_nodes.contains(&host));
        assert_eq!(state.hosting.product.ipv4_ranges.len(), 2);
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            "Save product",
        );
        let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        let [UiAction::NetworkCommand(command)] = actions.as_slice() else {
            panic!("expected product command")
        };
        sim.execute(command.clone()).unwrap();
        let id = *sim.hosting.products.keys().next().unwrap();
        assert_eq!(sim.hosting.products[&id].eligible_nodes.len(), 1);
        assert_eq!(sim.hosting.products[&id].ipv4_ranges.len(), 2);
        state.hosting.product = sim.hosting.products[&id].clone();
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            "Publish saved product",
        );
        let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        let [UiAction::NetworkCommand(command)] = actions.as_slice() else {
            panic!("expected publish command")
        };
        sim.execute(command.clone()).unwrap();
        assert!(sim.hosting.products[&id].published);
    }
    #[test]
    fn provider_site_editor_only_connects_the_public_address() {
        let mut sim = NetworkSim::new();
        sim.money = 100000;
        let SimEvent::DeviceAdded(server) = sim.execute(Command::BuyServerFullPack).unwrap()[0]
        else {
            panic!()
        };
        sim.buy_public_ipv4_pool().unwrap();
        let ports = sim.device(server).unwrap().ports().to_vec();
        let management = ports
            .iter()
            .copied()
            .find(|p| sim.port(*p).unwrap().name == "mgmt0")
            .unwrap();
        for (port, address) in [(ports[0], "203.0.113.2"), (management, "10.0.0.10")] {
            sim.execute(Command::SetIpv4 {
                port,
                config: Ipv4InterfaceConfig::new(address.parse().unwrap(), 24, None, VlanId(1)),
            })
            .unwrap();
        }
        let ctx = egui::Context::default();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut writer = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut state = UiState::default();
        state.hosting.tab = 5;
        state.hosting.provider_loaded = true;
        state.hosting.provider_public_ipv4 = "203.0.113.2".into();
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            "Connect hoster",
        );
        let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        let [UiAction::NetworkCommand(command)] = actions.as_slice() else {
            panic!("expected controller command")
        };
        sim.execute(command.clone()).unwrap();
        assert_eq!(
            sim.hosting.provider_site_ipv4,
            Some("203.0.113.2".parse().unwrap())
        );
        assert_eq!(sim.hosting_provider_server(), Some(server));
        assert!(!sim.hosting_provider_installed(server));
        assert!(sim.hosting_provider_site_status().is_err()); // Still needs power and a public route.
    }
    #[test]
    fn assigned_ip_list_links_to_destroyed_service_and_refund_button_releases_lease() {
        let mut sim = NetworkSim::new();
        sim.money = 100_000;
        let SimEvent::DeviceAdded(host) = sim.execute(Command::BuyServerFullPack).unwrap()[0]
        else {
            panic!()
        };
        let pool = sim.buy_public_ipv4_pool().unwrap().range();
        let address = "203.0.113.2".parse().unwrap();
        let plan = HostingProduct {
            id: 3,
            eligible_nodes: [host].into(),
            ipv4_ranges: [pool].into(),
            ..Default::default()
        };
        sim.hosting.customers.insert(
            1,
            HostingCustomer {
                id: 1,
                category: CustomerCategory::Website,
                requirements: customer_requirements(CustomerCategory::Website),
                price_sensitivity: 1.0,
                performance_expectation: 0.8,
                reliability_expectation: 0.99,
                services: [2].into(),
                workload_seed: 1,
                satisfaction: 1.0,
                churn_probability: 0.0,
            },
        );
        sim.hosting.services.insert(
            2,
            HostingService {
                id: 2,
                customer: 1,
                product: 3,
                vm: None,
                provisioning: Some(HostingServiceProvisioning {
                    plan,
                    host,
                    ips: vec![address],
                    ip_ranges: vec![pool],
                }),
                retained_traffic_bytes: 0,
                purchased_at_ms: 0,
                expires_at_ms: MONTH_MS,
                monthly_price_cents: 500,
                renewals: 0,
                terminated_at_ms: None,
                satisfaction: 1.0,
                observed_quality: 1.0,
                availability: 1.0,
            },
        );
        sim.hosting.payments.push(HostingPayment {
            order: 1,
            service: 2,
            at_ms: 0,
            months: 1,
            amount_cents: 500,
            renewal: false,
        });
        let ctx = egui::Context::default();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut writer = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut state = UiState::default();
        state.hosting.tab = 6;
        state.hosting.ip_node_filter = Some(host);
        render(&ctx, &mut world, &mut writer, &sim, &mut state, vec![]);
        let labels = render(&ctx, &mut world, &mut writer, &sim, &mut state, vec![]);
        assert!(
            labels
                .iter()
                .any(|(s, _)| s.contains("203.0.113.2") && s.contains("203.0.113.0/29"))
        );
        assert!(
            labels
                .iter()
                .any(|(s, _)| s.contains(&tr("hosting.ip-retained")))
        );
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &tr("hosting.view-service"),
        );
        assert_eq!(state.hosting.tab, 2);
        assert_eq!(state.hosting.selected_service, Some(2));
        let labels = render(&ctx, &mut world, &mut writer, &sim, &mut state, vec![]);
        assert!(labels.iter().any(|(s, _)| s == &tr("hosting.vm-destroyed")));
        assert!(labels.iter().any(|(s, _)| s == &tr("hosting.recreate-vm")));
        let refund_label = tr_args("hosting.refund-release", &["5.00".into()]);
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &refund_label,
        );
        let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        let [UiAction::NetworkCommand(Command::Hosting(HostingCommand::RefundService(id)))] =
            actions.as_slice()
        else {
            panic!("expected refund command")
        };
        assert_eq!(*id, 2);
        sim.execute(Command::Hosting(HostingCommand::RefundService(*id)))
            .unwrap();
        assert!(
            !sim.hosting_ipv4_assignments()
                .iter()
                .any(|r| r.address == address)
        );
        assert_eq!(sim.hosting.refunds[0].amount_cents, 500);
        assert!(sim.hosting.services[&2].terminated_at_ms.is_some());
        let labels = render(&ctx, &mut world, &mut writer, &sim, &mut state, vec![]);
        assert!(
            labels
                .iter()
                .any(|(s, _)| s == &tr_args("hosting.cancelled", &[date(sim.hosting.now_ms)]))
        );
        assert!(!labels.iter().any(|(s, _)| s == &tr("hosting.recreate-vm")));
    }
    #[test]
    fn node_editor_assigns_owned_subnets_to_separate_interfaces_and_vlans() {
        let mut sim = NetworkSim::new();
        sim.money = 100_000;
        let SimEvent::DeviceAdded(node) = sim.execute(Command::BuyServerFullPack).unwrap()[0]
        else {
            panic!()
        };
        sim.buy_public_ipv4_subnet(24).unwrap();
        let ports = sim.device(node).unwrap().ports().to_vec();
        let ctx = egui::Context::default();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut writer = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut state = UiState::default();
        state.hosting.tab = 1;
        state.hosting.selected_node = Some(node);
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &tr("hosting.interface-networks-auto"),
        );
        assert!(state.hosting.node_networks.as_ref().unwrap().is_empty());
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &tr("hosting.network-tagged"),
        );
        for (port, subnet, vlan) in [
            (ports[0], "203.0.113.0/29", 10),
            (ports[1], "203.0.113.8/29", 20),
        ] {
            state.hosting.node_network_port = Some(port);
            state.hosting.node_network_subnet = subnet.into();
            state.hosting.node_network_vlan = vlan;
            click(
                &ctx,
                &mut world,
                &mut writer,
                &sim,
                &mut state,
                &tr("hosting.network-add"),
            );
        }
        click(
            &ctx,
            &mut world,
            &mut writer,
            &sim,
            &mut state,
            &tr("hosting.network-apply"),
        );
        let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        let [
            UiAction::NetworkCommand(Command::Hosting(HostingCommand::SetNodeNetworks {
                node: selected,
                networks,
            })),
        ] = actions.as_slice()
        else {
            panic!("expected interface network mappings command")
        };
        assert_eq!(*selected, node);
        let expected = vec![
            HostingNodeNetwork {
                port: ports[0],
                subnet: "203.0.113.0/29".parse().unwrap(),
                vlan: Some(VlanId(10)),
            },
            HostingNodeNetwork {
                port: ports[1],
                subnet: "203.0.113.8/29".parse().unwrap(),
                vlan: Some(VlanId(20)),
            },
        ];
        assert_eq!(networks, &Some(expected.clone()));
        sim.execute(Command::Hosting(HostingCommand::SetNodeNetworks {
            node: *selected,
            networks: networks.clone(),
        }))
        .unwrap();
        assert_eq!(sim.hosting.nodes[&node].networks, Some(expected));
    }
}
