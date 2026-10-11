use super::*;
use crate::ui::test_windows;
use bevy::{
    ecs::system::SystemState,
    prelude::{Messages, World},
};

#[test]
fn ordered_vm_scroll_viewport_tracks_window_growth_and_shrinkage() {
    let ctx = egui::Context::default();
    let mut sim = NetworkSim::new();
    for id in 1..=30 {
        sim.hosting.services.insert(
            id,
            HostingService {
                id,
                customer: id,
                product: 1,
                vm: None,
                provisioning: None,
                retained_traffic_bytes: 0,
                purchased_at_ms: 0,
                expires_at_ms: MONTH_MS,
                monthly_price_cents: 500,
                renewals: 0,
                terminated_at_ms: Some(0),
                satisfaction: 1.0,
                observed_quality: 1.0,
                availability: 1.0,
            },
        );
    }
    let mut world = World::new();
    world.init_resource::<Messages<UiAction>>();
    let mut writer = SystemState::<MessageWriter<UiAction>>::new(&mut world);
    let mut state = UiState::default();
    state.hosting.open = true;
    state.hosting.tab = 2;
    let mut frame = |events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(2000.0, 1500.0),
                )),
                events,
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
        output
    };
    for _ in 0..3 {
        frame(vec![]);
    }
    for size in [
        egui::vec2(1300.0, 1000.0),
        egui::vec2(1000.0, 450.0),
        egui::vec2(1200.0, 900.0),
    ] {
        let output = test_windows::resize(&ctx, "hosting-management", size, &mut frame);
        let window = test_windows::rect(&ctx, "hosting-management");
        let clip = test_windows::text_clip(
            &output,
            &tr_args(
                "hosting.service-row",
                &[
                    "1".into(),
                    "1".into(),
                    "1".into(),
                    date(MONTH_MS),
                    "0".into(),
                ],
            ),
        );
        test_windows::fills_window_bottom(window, clip);
    }
}

#[test]
fn populated_vm_rows_fill_a_tall_window_and_scroll_to_the_last_service() {
    for language in ["en", "ru"] {
        let mut locale = crate::localization::Localization::default();
        locale.select(language).unwrap();
        let _scope = locale.enter();
        let ctx = egui::Context::default();
        let mut sim = NetworkSim::new();
        for id in 1..=30 {
            sim.hosting.vms.insert(
                id,
                VirtualMachine {
                    id,
                    customer: id,
                    host: DeviceId(1),
                    product: 1,
                    service: id,
                    plan: HostingProduct::default(),
                    interfaces: vec![],
                    ips: vec!["203.0.113.12".parse().unwrap()],
                    ip_ranges: vec![],
                    physical_disk_gb: 1.0,
                    disk_used_gb: 1.0,
                    power: if id % 3 == 0 {
                        VmPower::Stopped
                    } else {
                        VmPower::Running
                    },
                    host_memory_failure: false,
                    customer_restart_at_ms: 0,
                    expires_at_ms: MONTH_MS,
                    demand: WorkloadDemand::default(),
                    workload_override: None,
                    metrics: VmMetrics {
                        guest_memory_limited_mb: if id % 2 == 0 { 100.0 } else { 0.0 },
                        ..Default::default()
                    },
                    burst_credit_seconds: 0.0,
                    reboot_until_ms: 0,
                },
            );
            sim.hosting.services.insert(
                id,
                HostingService {
                    id,
                    customer: id,
                    product: 1,
                    vm: Some(id),
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
        }
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut writer = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut state = UiState::default();
        state.hosting.open = true;
        state.hosting.tab = 2;
        let mut frame = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(2000.0, 1800.0),
                    )),
                    events,
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
            output
        };
        for _ in 0..3 {
            frame(vec![]);
        }
        let output = test_windows::resize(
            &ctx,
            "hosting-management",
            egui::vec2(1100.0, 1500.0),
            &mut frame,
        );
        let prefix = if language == "en" {
            "Service "
        } else {
            "Услуга "
        };
        let headers: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if text.galley.text().starts_with(prefix)
                        && shape.clip_rect.contains(text.pos) =>
                {
                    Some((text.pos.y, shape.clip_rect))
                }
                _ => None,
            })
            .collect();
        assert!(headers.len() > 1);
        let tallest_row = headers
            .windows(2)
            .map(|pair| pair[1].0 - pair[0].0)
            .fold(0.0, f32::max);
        let (last_y, clip) = headers.last().unwrap();
        assert!(
            clip.bottom() - last_y <= tallest_row + 25.0,
            "{language}: visible rows stop at {last_y}, viewport ends at {}, tallest row {tallest_row}",
            clip.bottom()
        );

        let output = test_windows::resize(
            &ctx,
            "hosting-management",
            egui::vec2(1100.0, 600.0),
            &mut frame,
        );
        let pointer = test_windows::text_clip(
            &output,
            &tr_args(
                "hosting.service-row",
                &[
                    "1".into(),
                    "1".into(),
                    "1".into(),
                    date(MONTH_MS),
                    "0".into(),
                ],
            ),
        )
        .center();
        for _ in 0..6 {
            frame(vec![
                egui::Event::PointerMoved(pointer),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -100000.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        let output = frame(vec![]);
        let last = tr_args(
            "hosting.service-row",
            &[
                "30".into(),
                "30".into(),
                "1".into(),
                date(MONTH_MS),
                "0".into(),
            ],
        );
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.text() == last && shape.clip_rect.contains(text.pos))),
            "Last service must remain reachable after resizing and scrolling");
    }
}
