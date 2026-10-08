use super::*;
use crate::app::{ShopCategory, ShopSection, ShopSort};
use bevy::{
    ecs::system::SystemState,
    prelude::{Messages, World},
};
use std::collections::BTreeSet;

#[test]
fn every_product_has_one_discoverable_section_in_the_right_category() {
    let mut sections = BTreeSet::new();
    for &(category, _, children) in navigation::CATEGORIES {
        for &(section, _) in children {
            assert!(sections.insert(section), "duplicate section {section:?}");
            assert_eq!(section.category(), category);
            assert!(
                catalog::catalog()
                    .iter()
                    .any(|offer| offer.section == section)
            );
        }
    }
    assert!(
        catalog::catalog()
            .iter()
            .all(|offer| sections.contains(&offer.section))
    );
    for (id, section) in [
        ("sfp_1g_sx", ShopSection::Transceivers),
        ("fiber_om3_2_3m", ShopSection::FiberCables),
        ("dac_10g_3m", ShopSection::DirectAttach),
        ("ethernet_cable_box", ShopSection::CopperSupplies),
        ("rj45_connectors", ShopSection::CopperSupplies),
        ("fiber_panel", ShopSection::PatchPanels),
        ("patch_panel", ShopSection::PatchPanels),
        ("cable_manager", ShopSection::CableManagers),
        ("public_ipv4_pool", ShopSection::PublicIp),
    ] {
        assert_eq!(
            catalog::catalog()
                .iter()
                .find(|offer| offer.id == id)
                .unwrap()
                .section,
            section
        );
    }
}

#[test]
fn overview_filters_are_general_and_leaf_filters_belong_to_the_product_type() {
    let mut state = ShopState::default();
    state.select_all();
    assert!(query::facet_keys(&state).is_empty());
    for &(category, _, sections) in navigation::CATEGORIES {
        state.select(category, None);
        assert!(query::facet_keys(&state).is_empty(), "{category:?}");
        for &(section, _) in sections {
            state.select(category, Some(section));
            for key in query::facet_keys(&state) {
                assert!(
                    catalog::catalog()
                        .iter()
                        .filter(|offer| offer.section == section)
                        .any(|offer| offer.values(key).next().is_some()),
                    "{section:?}: {key}"
                );
            }
        }
    }
    state.select(ShopCategory::Compute, Some(ShopSection::Ram));
    assert_eq!(query::facet_keys(&state), &["capacity", "memory-type"]);
    state.select(ShopCategory::Connectivity, Some(ShopSection::FiberCables));
    assert_eq!(
        query::facet_keys(&state),
        &["fiber", "strands", "length", "medium"]
    );
    state.select(ShopCategory::Connectivity, Some(ShopSection::Transceivers));
    assert!(query::facet_keys(&state).contains(&"dom"));
    assert!(!query::facet_keys(&state).contains(&"length"));
    state.select(ShopCategory::Network, Some(ShopSection::Switches));
    assert!(query::facet_keys(&state).contains(&"ports"));
    assert!(!query::facet_keys(&state).contains(&"fiber"));
}

#[test]
fn narrowing_results_keeps_unavailable_filter_choices_visible() {
    let sim = NetworkSim::new();
    let mut state = ShopState::default();
    state.select(ShopCategory::Connectivity, Some(ShopSection::FiberCables));
    let before = query::facet_values("fiber", &state, &sim);
    state
        .facets
        .insert("strands".into(), BTreeSet::from(["simplex".into()]));
    let after = query::facet_values("fiber", &state, &sim);
    assert_eq!(
        before.iter().map(|(value, _)| value).collect::<Vec<_>>(),
        after.iter().map(|(value, _)| value).collect::<Vec<_>>()
    );
    assert!(after.contains(&("OM3".into(), 0)));
    assert!(
        after
            .iter()
            .any(|(value, count)| value == "OS2" && *count > 0)
    );
    state
        .facets
        .insert("fiber".into(), BTreeSet::from(["OM3".into()]));
    assert!(query::filtered(&state, &sim).is_empty());
    assert!(query::facet_values("fiber", &state, &sim).contains(&("OM3".into(), 0)));
    state.facets.remove("strands");
    assert!(!query::filtered(&state, &sim).is_empty());
}

#[test]
fn section_changes_restore_filters_and_reset_only_irrelevant_sorting() {
    let mut state = ShopState::default();
    state.select(ShopCategory::Connectivity, Some(ShopSection::FiberCables));
    state.sort = ShopSort::LengthAscending;
    state.max_price = Some(100);
    state
        .facets
        .insert("length".into(), BTreeSet::from(["300".into()]));
    state.select(ShopCategory::Connectivity, Some(ShopSection::DirectAttach));
    assert!(state.facets.is_empty());
    assert_eq!(state.sort, ShopSort::LengthAscending);
    state.select(ShopCategory::Connectivity, Some(ShopSection::Transceivers));
    assert_eq!(state.sort, ShopSort::Category);
    assert!(!ShopSort::LengthAscending.applies_to(state.section));
    assert!(ShopSort::SpeedDescending.applies_to(state.section));
    state.select(ShopCategory::Connectivity, Some(ShopSection::FiberCables));
    assert_eq!(state.facets["length"], BTreeSet::from(["300".into()]));
    assert_eq!(state.max_price, Some(100));
    state.sort = ShopSort::PriceAscending;
    state.select_all();
    assert!(state.facets.is_empty());
    assert_eq!(state.sort, ShopSort::PriceAscending);
}

#[test]
fn category_shortcuts_work_with_collapsed_filters_in_both_languages() {
    for language in ["en", "ru"] {
        let mut locale = crate::localization::Localization::default();
        locale.select(language).unwrap();
        let _scope = locale.enter();
        let ctx = egui::Context::default();
        let sim = NetworkSim::new();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut system = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut state = ShopState {
            open: true,
            all_categories: true,
            ..Default::default()
        };
        let size = egui::vec2(500.0, 720.0);
        for (key, section) in [
            ("shop.category.connectivity", None),
            ("shop.section.fiber", Some(ShopSection::FiberCables)),
            ("shop.category.connectivity", None),
            ("shop.section.transceivers", Some(ShopSection::Transceivers)),
        ] {
            let mut output = None;
            for _ in 0..3 {
                output = Some(tests::frame(
                    &ctx,
                    &mut state,
                    &sim,
                    &mut world,
                    &mut system,
                    vec![],
                    size,
                ));
            }
            let label = tr(key);
            let position = output
                .unwrap()
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text().starts_with(&label) => {
                        Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing shortcut {label}"));
            assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains(position));
            for pressed in [true, false] {
                tests::frame(
                    &ctx,
                    &mut state,
                    &sim,
                    &mut world,
                    &mut system,
                    vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    size,
                );
            }
            assert_eq!(state.category, ShopCategory::Connectivity);
            assert_eq!(state.section, section);
            assert!(!state.all_categories);
        }
        assert!(
            world
                .resource_mut::<Messages<UiAction>>()
                .drain()
                .next()
                .is_none()
        );
    }
}
