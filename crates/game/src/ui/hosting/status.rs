use super::*;

fn line(ui: &mut egui::Ui, text: String, provider_failure: bool) {
    let text = egui::RichText::new(text).small();
    ui.label(if provider_failure {
        text.color(egui::Color32::LIGHT_RED)
    } else {
        text
    });
}

pub(super) fn metrics(ui: &mut egui::Ui, vm: &VirtualMachine, satisfaction: f64) {
    let m = &vm.metrics;
    line(
        ui,
        tr_args(
            "hosting.vm-cpu",
            &[
                format!("{:.0}", m.cpu_used_mhz),
                format!("{:.0}", vm.demand.cpu_mhz),
                format!("{:.1}", m.cpu_ready * 100.0),
                format!("{:.1}", m.cpu_throttled * 100.0),
                format!("{:.0}", m.resident_mb),
                format!("{:.0}", m.swapped_mb),
            ],
        ),
        m.cpu_ready > 0.001 || m.swapped_mb > 0.0,
    );
    line(
        ui,
        tr_args(
            "hosting.vm-disk",
            &[
                format!("{:.2}", vm.disk_used_gb),
                vm.plan.disk_gb.to_string(),
                format!("{:.2}", vm.physical_disk_gb),
                format!("{:.0}", m.disk_iops),
                format!("{:.1}", m.disk_mb_s),
                format!("{:.0}", m.disk_latency_ms),
                format!("{:.2}", m.write_failed_gb),
            ],
        ),
        m.disk_ready > 0.001 || m.write_failed_gb > 0.0,
    );
    line(
        ui,
        tr_args(
            "hosting.vm-network",
            &[
                format!("{:.1}", m.network_mbps),
                format!("{:.0}", m.network_latency_ms),
                format!("{:.1}", m.packet_loss * 100.0),
                format!("{:.2}", m.traffic_bytes as f64 / 1e9),
                if m.reachable {
                    tr("hosting.yes")
                } else {
                    tr("hosting.no")
                },
                format!("{:.0}", satisfaction * 100.0),
            ],
        ),
        matches!(
            m.provider_fault,
            Some(
                HostingProviderFault::HostUnavailable
                    | HostingProviderFault::PublicNetworkUnavailable
            )
        ) || m.packet_loss > 0.001,
    );
    if m.guest_memory_limited_mb > 0.0 || m.guest_disk_full_gb > 0.0 {
        line(
            ui,
            tr_args(
                "hosting.vm-guest-limits",
                &[
                    format!("{:.0}", m.guest_memory_limited_mb),
                    format!("{:.2}", m.guest_disk_full_gb),
                ],
            ),
            false,
        );
    }
    if let Some(fault) = m.provider_fault {
        line(
            ui,
            tr(match fault {
                HostingProviderFault::HostUnavailable => "hosting.vm-host-failed",
                HostingProviderFault::ManagementUnavailable => "hosting.vm-management-failed",
                HostingProviderFault::PublicNetworkUnavailable => "hosting.vm-route-failed",
                HostingProviderFault::HostMemoryExhausted => "hosting.vm-host-memory-failed",
            }),
            true,
        );
    } else if vm.power != VmPower::Running {
        line(ui, tr("hosting.vm-customer-restart"), false);
    }
}
