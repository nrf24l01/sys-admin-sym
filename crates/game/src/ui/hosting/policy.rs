use super::*;

pub(super) fn show(ui: &mut egui::Ui, policy: &mut HypervisorPolicy) {
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
}
