use super::*;
use crate::app::{ShopCategory, ShopSection, ShopSort};
use cloud_provider_sim::server_catalog;
use std::collections::BTreeSet;

fn server_state() -> ShopState {
    let mut state = ShopState::default();
    state.select(ShopCategory::Compute, Some(ShopSection::Servers));
    state
}

#[test]
fn server_filters_describe_physical_capacity_for_bare_and_populated_configurations() {
    let sim = NetworkSim::new();
    let chassis = &server_catalog().chassis;
    let mut state = server_state();
    let offers = query::filtered(&state, &sim);
    assert_eq!(offers.len(), 2);
    for offer in offers {
        for (key, expected) in [
            ("ram-slots", chassis.dimm_slots),
            ("cpu-sockets", chassis.cpu_sockets),
            ("drive-bays", chassis.drive_bays.len()),
            ("pcie-slots", chassis.pcie_slots.len()),
        ] {
            assert_eq!(offer.numeric(key), expected as u64, "{}: {key}", offer.id);
            state
                .facets
                .insert(key.into(), BTreeSet::from([expected.to_string()]));
        }
        assert_eq!(
            offer.values("socket").next(),
            Some(chassis.cpu_socket.as_str())
        );
        assert_eq!(
            offer.values("memory-type").next(),
            Some(chassis.memory_type.as_str())
        );
        assert_eq!(
            offer.numeric("psu-watts"),
            u64::from(chassis.integrated_psu_watts)
        );
        assert!(
            chassis
                .drive_bays
                .iter()
                .all(|bay| offer.values("drive-interface").any(|v| v == bay.interface))
        );
        assert!(chassis.pcie_slots.iter().all(|slot| {
            offer.numeric("pcie-generation") >= u64::from(slot.generation)
                && offer
                    .values("pcie-width")
                    .any(|v| v == slot.width.to_string())
        }));
    }
    state.facets.insert(
        "socket".into(),
        BTreeSet::from([chassis.cpu_socket.clone()]),
    );
    state.facets.insert(
        "memory-type".into(),
        BTreeSet::from([chassis.memory_type.clone()]),
    );
    assert_eq!(query::filtered(&state, &sim).len(), 2);
    state
        .facets
        .insert("configuration".into(), BTreeSet::from(["full-pack".into()]));
    assert_eq!(query::filtered(&state, &sim)[0].id, "dell_r360_full_pack");
    state.facets.insert(
        "ram-slots".into(),
        BTreeSet::from([(chassis.dimm_slots + 1).to_string()]),
    );
    assert!(query::filtered(&state, &sim).is_empty());
    state.facets.remove("ram-slots");
    assert_eq!(query::filtered(&state, &sim).len(), 1);
}

fn sidebar_frame(
    ctx: &egui::Context,
    state: &mut ShopState,
    sim: &NetworkSim,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(500.0, 1000.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            ui.set_width(230.0);
            filters::sidebar(ui, state, sim);
        },
    );
    output.textures_delta.clear();
    output
}

#[test]
fn singleton_socket_memory_type_and_ram_slot_filters_are_visible_and_selectable() {
    for language in ["en", "ru"] {
        let mut locale = crate::localization::Localization::default();
        locale.select(language).unwrap();
        let _scope = locale.enter();
        let ctx = egui::Context::default();
        let sim = NetworkSim::new();
        let mut state = server_state();
        let chassis = &server_catalog().chassis;
        for (key, value) in [
            ("memory-type", chassis.memory_type.clone()),
            ("socket", chassis.cpu_socket.clone()),
            ("ram-slots", chassis.dimm_slots.to_string()),
        ] {
            let mut output = None;
            for _ in 0..3 {
                output = Some(sidebar_frame(&ctx, &mut state, &sim, vec![]));
            }
            let output = output.unwrap();
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.text() == tr(&format!("shop.spec.{key}")))));
            let position = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == format!("{value} (1)") => {
                        Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| {
                    panic!("missing single-value choice {key}={value} in {language}")
                });
            for pressed in [true, false] {
                sidebar_frame(
                    &ctx,
                    &mut state,
                    &sim,
                    vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            assert!(state.facets[key].contains(&value));
            assert_eq!(query::filtered(&state, &sim).len(), 2);
        }
    }
}

#[test]
fn server_sorting_uses_text_and_numeric_capacity_and_stays_in_appropriate_sections() {
    let server = catalog::catalog()
        .iter()
        .find(|offer| offer.id == "dell_r360")
        .unwrap();
    let mut larger = server.clone();
    larger.id = "larger_server".into();
    for (key, value) in [
        ("socket", "LGA2066"),
        ("memory-type", "DDR4 ECC"),
        ("ram-slots", "8"),
        ("cpu-sockets", "2"),
        ("drive-bays", "8"),
        ("pcie-slots", "4"),
    ] {
        let attribute = larger.attributes.iter_mut().find(|a| a.key == key).unwrap();
        attribute.value = value.into();
        attribute.number = value.parse().ok();
    }
    for (sort, expected) in [
        (ShopSort::CpuSocket, std::cmp::Ordering::Less),
        (ShopSort::MemoryType, std::cmp::Ordering::Greater),
        (ShopSort::RamSlotsDescending, std::cmp::Ordering::Greater),
        (ShopSort::CpuSocketsDescending, std::cmp::Ordering::Greater),
        (ShopSort::DriveBaysDescending, std::cmp::Ordering::Greater),
        (ShopSort::PcieSlotsDescending, std::cmp::Ordering::Greater),
    ] {
        assert_eq!(query::order(server, &larger, sort), expected, "{sort:?}");
        assert!(sort.applies_to(Some(ShopSection::Servers)));
        assert!(!sort.applies_to(Some(ShopSection::Switches)));
    }
    let mut state = server_state();
    state.sort = ShopSort::MemoryType;
    state.select(ShopCategory::Compute, Some(ShopSection::Ram));
    assert_eq!(state.sort, ShopSort::MemoryType);
    state.select(ShopCategory::Compute, Some(ShopSection::Storage));
    assert_eq!(state.sort, ShopSort::Category);
}
