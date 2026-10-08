use super::*;
use crate::app::{PendingPurchase, ShopCategory, ShopSection, ShopSort};
use bevy::{
    ecs::system::SystemState,
    prelude::{Messages, World},
};
use cloud_provider_sim::*;
use std::collections::BTreeSet;

fn offer(id: &str) -> &'static catalog::Offer {
    catalog::catalog().iter().find(|o| o.id == id).unwrap()
}

#[test]
fn every_offer_has_a_unique_identity_valid_price_and_complete_artwork() {
    let offers = catalog::catalog();
    assert_eq!(offers.len(), 48);
    assert_eq!(
        offers.iter().map(|o| &o.id).collect::<BTreeSet<_>>().len(),
        offers.len()
    );
    for offer in offers {
        assert_eq!(offer.price(), offer.item.unit_price().unwrap());
        let art = artwork::artwork(offer, artwork::test_textures())
            .unwrap_or_else(|| panic!("missing {}", offer.id));
        assert!(
            art.uv.min.x >= 0.0
                && art.uv.min.y >= 0.0
                && art.uv.max.x <= 1.0
                && art.uv.max.y <= 1.0
        );
        assert!(art.aspect > 0.0);
    }
}

#[test]
fn product_atlas_decodes_and_contains_twelve_padded_cells() {
    use bevy::{
        asset::RenderAssetUsages,
        image::{CompressedImageFormats, Image, ImageSampler, ImageType},
    };
    let image = Image::from_buffer(
        include_bytes!("../../../../../assets/equipment/shop_products.png"),
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::default(),
        RenderAssetUsages::default(),
    )
    .unwrap();
    let (width, height) = (image.width() as usize, image.height() as usize);
    assert_eq!(width % 4, 0);
    assert_eq!(height % 3, 0);
    let pixels = image.data.as_ref().unwrap();
    let alpha = |x, y| pixels[(y * width + x) * 4 + 3];
    for x in [0, width / 4, width / 2, 3 * width / 4, width - 1] {
        assert!((0..height).all(|y| alpha(x, y) <= 1));
    }
    for y in [0, height / 3, 2 * height / 3, height - 1] {
        assert!((0..width).all(|x| alpha(x, y) <= 1));
    }
    for row in 0..3 {
        for column in 0..4 {
            assert!(
                (row * height / 3..(row + 1) * height / 3)
                    .step_by(8)
                    .any(|y| (column * width / 4..(column + 1) * width / 4)
                        .step_by(8)
                        .any(|x| alpha(x, y) > 200)),
                "cell {column},{row}"
            );
        }
    }
}

#[test]
fn filters_combine_and_variants_respect_their_individual_prices() {
    let sim = NetworkSim::new();
    let mut state = ShopState {
        category: ShopCategory::Compute,
        section: Some(ShopSection::Servers),
        max_price: Some(1000),
        ..Default::default()
    };
    let offers = query::filtered(&state, &sim);
    assert_eq!(
        offers.iter().map(|o| o.id.as_str()).collect::<Vec<_>>(),
        ["dell_r360"]
    );
    state.max_price = None;
    state.min_price = Some(ServerFullPack::price());
    assert_eq!(
        query::filtered(&state, &sim)
            .iter()
            .map(|o| o.id.as_str())
            .collect::<Vec<_>>(),
        ["dell_r360_full_pack"]
    );
    state.select(ShopCategory::Connectivity, Some(ShopSection::FiberCables));
    state.clear_filters();
    state
        .facets
        .insert("fiber".into(), BTreeSet::from(["OM3".into(), "OM4".into()]));
    state
        .facets
        .insert("length".into(), BTreeSet::from(["300".into()]));
    assert_eq!(query::filtered(&state, &sim).len(), 2);
    state.search = "OM4 duplex".into();
    assert_eq!(
        query::filtered(&state, &sim)
            .iter()
            .map(|o| o.id.as_str())
            .collect::<Vec<_>>(),
        ["fiber_om4_2_3m"]
    );
}

#[test]
fn category_changes_deactivate_irrelevant_filters_and_restore_section_facets() {
    let sim = NetworkSim::new();
    let mut state = ShopState::default();
    state.select(ShopCategory::Network, Some(ShopSection::Switches));
    state
        .facets
        .insert("ports".into(), BTreeSet::from(["24".into()]));
    state.rack_units = Some(1);
    state.select(ShopCategory::Compute, Some(ShopSection::Storage));
    assert!(state.facets.is_empty());
    assert_eq!(state.rack_units, None);
    assert_eq!(query::filtered(&state, &sim).len(), 2);
    state.select(ShopCategory::Network, Some(ShopSection::Switches));
    assert!(state.facets["ports"].contains("24"));
    assert_eq!(query::filtered(&state, &sim).len(), 2);
    state.select_all();
    assert_eq!(query::filtered(&state, &sim).len(), 48);
}

#[test]
fn facet_counts_preserve_selected_zero_results_and_offer_other_choices() {
    let sim = NetworkSim::new();
    let mut state = ShopState {
        category: ShopCategory::Connectivity,
        section: Some(ShopSection::FiberCables),
        ..Default::default()
    };
    state
        .facets
        .insert("fiber".into(), BTreeSet::from(["unknown".into()]));
    let choices = query::facet_values("fiber", &state, &sim);
    assert!(choices.contains(&("unknown".into(), 0)));
    assert!(
        choices
            .iter()
            .any(|(value, count)| value == "OM3" && *count > 0)
    );
    assert!(query::filtered(&state, &sim).is_empty());
}

#[test]
fn sorting_and_cable_grouping_keep_selected_variants_concrete() {
    let sim = NetworkSim::new();
    let state = ShopState {
        category: ShopCategory::Connectivity,
        section: Some(ShopSection::FiberCables),
        sort: ShopSort::PriceDescending,
        ..Default::default()
    };
    let offers = query::filtered(&state, &sim);
    assert!(
        offers
            .windows(2)
            .all(|pair| pair[0].price() >= pair[1].price())
    );
    let groups = query::grouped(&offers);
    let os2 = groups
        .iter()
        .find(|group| group[0].family == "fiber_os2_1")
        .unwrap();
    assert_eq!(os2.len(), 5);
    assert!(
        os2.iter()
            .all(|offer| matches!(&offer.item,PurchaseItem::Assembly(id) if id==&offer.id))
    );
}

#[test]
fn localized_search_covers_model_names_descriptions_and_specifications() {
    let mut locale = crate::localization::Localization::default();
    locale.select("ru").unwrap();
    let _scope = locale.enter();
    let sim = NetworkSim::new();
    let mut state = ShopState {
        category: ShopCategory::Compute,
        section: Some(ShopSection::Storage),
        search: "твердотельный SATA".into(),
        ..Default::default()
    };
    assert_eq!(query::filtered(&state, &sim)[0].id, "enterprise_ssd_960gb");
    state.select_all();
    state.search = "10G SFP+".into();
    assert!(
        query::filtered(&state, &sim)
            .iter()
            .any(|o| o.id == "intel_x520_da2")
    );
    state.search = "C1111".into();
    assert_eq!(query::filtered(&state, &sim).len(), 1);
}

#[test]
fn stale_purchase_replies_do_not_unlock_a_pending_order() {
    let mut state = ShopState {
        pending: Some(PendingPurchase {
            request_id: 42,
            offer_id: "enterprise_ssd_960gb".into(),
            quantity: 2,
        }),
        ..Default::default()
    };
    state.finish_purchase(
        41,
        Ok(PurchaseReceipt {
            quantity: 1,
            total: 230,
        }),
    );
    assert!(state.pending.is_some());
    assert!(state.feedback.is_none());
    state.finish_purchase(
        42,
        Err(SimError::InsufficientFunds {
            needed: 460,
            available: 400,
        }),
    );
    assert!(state.pending.is_none());
    assert!(state.feedback.as_ref().unwrap().result.is_err());
}

pub(super) fn frame(
    ctx: &egui::Context,
    state: &mut ShopState,
    sim: &NetworkSim,
    world: &mut World,
    system: &mut SystemState<MessageWriter<UiAction>>,
    events: Vec<egui::Event>,
    size: egui::Vec2,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ui| {
            show(
                ui,
                sim,
                state,
                Selection::None,
                artwork::test_textures(),
                &mut system.get_mut(world).unwrap(),
            )
        },
    );
    output.textures_delta.clear();
    output
}

#[test]
fn buying_emits_one_correlated_order_with_the_selected_quantity_and_variant() {
    let ctx = egui::Context::default();
    let sim = NetworkSim::new();
    let mut world = World::new();
    world.init_resource::<Messages<UiAction>>();
    let mut system = SystemState::<MessageWriter<UiAction>>::new(&mut world);
    let mut state = ShopState {
        open: true,
        category: ShopCategory::Connectivity,
        section: Some(ShopSection::DirectAttach),
        search: "dac".into(),
        ..Default::default()
    };
    state.variants.insert("dac_10g".into(), "dac_10g_3m".into());
    state.quantities.insert("dac_10g_3m".into(), 3);
    let mut position = None;
    for _ in 0..3 {
        let output = frame(
            &ctx,
            &mut state,
            &sim,
            &mut world,
            &mut system,
            vec![],
            egui::vec2(1200.0, 900.0),
        );
        position = output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "Buy — $78" => {
                Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center())
            }
            _ => None,
        });
    }
    let position = position.expect("selected DAC has a visible purchase button");
    for pressed in [true, false] {
        let _ = frame(
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
            egui::vec2(1200.0, 900.0),
        );
    }
    let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
    assert!(
        matches!(actions.as_slice(),[UiAction::ShopPurchase {request_id:1,item:PurchaseItem::Assembly(id),quantity:3}] if id=="dac_10g_3m")
    );
    assert!(state.pending.is_some());
}

#[test]
fn details_render_item_descriptions_in_both_languages_and_narrow_windows() {
    for language in ["en", "ru"] {
        let mut locale = crate::localization::Localization::default();
        locale.select(language).unwrap();
        let _scope = locale.enter();
        for size in [egui::vec2(500.0, 650.0), egui::vec2(1600.0, 900.0)] {
            let ctx = egui::Context::default();
            let sim = NetworkSim::new();
            let mut world = World::new();
            world.init_resource::<Messages<UiAction>>();
            let mut system = SystemState::<MessageWriter<UiAction>>::new(&mut world);
            let mut state = ShopState {
                open: true,
                category: ShopCategory::Compute,
                section: Some(ShopSection::Storage),
                selected_offer: Some("enterprise_ssd_960gb".into()),
                details_open: true,
                ..Default::default()
            };
            let mut texts = Vec::new();
            for _ in 0..3 {
                let output = frame(
                    &ctx,
                    &mut state,
                    &sim,
                    &mut world,
                    &mut system,
                    vec![],
                    size,
                );
                texts = output
                    .shapes
                    .iter()
                    .filter_map(|shape| {
                        if let egui::Shape::Text(text) = &shape.shape {
                            Some(text.galley.text().to_string())
                        } else {
                            None
                        }
                    })
                    .collect();
            }
            assert!(
                texts
                    .iter()
                    .any(|text| text == &offer("enterprise_ssd_960gb").description()),
                "{language} {size:?}"
            );
            assert!(
                !texts.iter().any(|text| text.starts_with("[shop.")),
                "missing translation"
            );
        }
    }
}

#[test]
fn rack_connector_counts_describe_front_panel_pairs_and_actual_cages() {
    assert_eq!(offer("patch_panel").numeric("ports"), 24);
    assert!(offer("patch_panel").values("speed").next().is_none());
    assert_eq!(offer("fiber_panel").numeric("lc-pairs"), 24);
    assert_eq!(offer("switch_10g").numeric("ports"), 24);
    assert_eq!(offer("switch_10g").numeric("cages"), 4);
    assert_eq!(offer("switch_10g").numeric("speed"), 10_000);
    assert_eq!(offer("rack_pdu").numeric("outlets"), 8);
}

#[test]
fn comparison_groups_equivalent_products_across_catalog_sources() {
    for (first, second) in [
        ("cisco_catalyst_c1000", "switch_10g"),
        ("patch_panel", "fiber_panel"),
        ("dell_r360", "dell_r360_full_pack"),
        ("sfp_1g_sx", "sfpplus_10g_sr"),
    ] {
        assert!(
            offer(first).can_compare_with(offer(second)),
            "{first}, {second}"
        );
        assert!(
            offer(second).can_compare_with(offer(first)),
            "{second}, {first}"
        );
    }
    for (first, second) in [
        ("patch_panel", "cable_manager"),
        ("ethernet_cable_box", "rj45_connectors"),
        ("sfp_1g_sx", "fiber_om3_2_3m"),
        ("cisco_catalyst_c1000", "cisco_isr_c1111"),
    ] {
        assert!(
            !offer(first).can_compare_with(offer(second)),
            "{first}, {second}"
        );
    }
}

#[test]
fn shown_variant_controls_sorting_without_discarding_filtered_preferences() {
    let sim = NetworkSim::new();
    let mut state = ShopState {
        category: ShopCategory::Connectivity,
        section: Some(ShopSection::FiberCables),
        sort: ShopSort::PriceAscending,
        ..Default::default()
    };
    state
        .variants
        .insert("fiber_os2_1".into(), "fiber_os2_1_10000m".into());
    let offers = query::filtered(&state, &sim);
    let mut groups = query::grouped(&offers);
    groups.sort_by(|a, b| {
        query::order(
            query::selected_variant(a, &state),
            query::selected_variant(b, &state),
            state.sort,
        )
    });
    assert_eq!(
        query::selected_variant(groups.last().unwrap(), &state).price(),
        2010
    );
    state.max_price = Some(50);
    let filtered = query::filtered(&state, &sim);
    let shorter = query::grouped(&filtered);
    let group = shorter
        .iter()
        .find(|group| group[0].family == "fiber_os2_1")
        .unwrap();
    assert!(query::selected_variant(group, &state).price() <= 50);
    assert_eq!(state.variants["fiber_os2_1"], "fiber_os2_1_10000m");
}

#[test]
fn cached_search_follows_language_switches_and_same_language_catalog_reload() {
    use crate::localization::Localization;
    let sim = NetworkSim::new();
    let mut language = Localization::default();
    let mut state = ShopState {
        all_categories: true,
        search: "Ethernet cable box".into(),
        ..Default::default()
    };
    {
        let _scope = language.enter();
        assert!(
            query::filtered(&state, &sim)
                .iter()
                .any(|o| o.id == "ethernet_cable_box")
        );
    }
    language.select("ru").unwrap();
    {
        let _scope = language.enter();
        let item = offer("ethernet_cable_box");
        state.search = item.name();
        assert!(
            query::filtered(&state, &sim)
                .iter()
                .any(|o| o.id == item.id)
        );
    }
    let directory =
        std::env::temp_dir().join(format!("shop-search-localization-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut catalog: serde_json::Value =
        serde_json::from_str(include_str!("../../../../../assets/locales/en.json")).unwrap();
    for label in ["UniqueCableSearchAlpha", "UniqueCableSearchBeta"] {
        catalog["messages"]["shop.spec.type"] = label.into();
        std::fs::write(
            directory.join("en.json"),
            serde_json::to_vec(&catalog).unwrap(),
        )
        .unwrap();
        let localization = Localization::load(&directory);
        let _scope = localization.enter();
        state.search = label.into();
        assert!(!query::filtered(&state, &sim).is_empty());
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn virtual_product_rows_skip_offscreen_cards_and_keep_scrolling_interactive() {
    for list in [false, true] {
        let ctx = egui::Context::default();
        let sim = NetworkSim::new();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut system = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut state = ShopState {
            open: true,
            all_categories: true,
            list_view: list,
            ..Default::default()
        };
        let size = egui::vec2(1200.0, 900.0);
        for _ in 0..3 {
            let _ = frame(
                &ctx,
                &mut state,
                &sim,
                &mut world,
                &mut system,
                vec![],
                size,
            );
        }
        let initial = state.quantities.len();
        assert!(
            initial > 0 && initial < query::grouped(&query::filtered(&state, &sim)).len(),
            "offscreen products must not be constructed"
        );
        let pointer = egui::pos2(850.0, 500.0);
        for _ in 0..6 {
            let _ = frame(
                &ctx,
                &mut state,
                &sim,
                &mut world,
                &mut system,
                vec![
                    egui::Event::PointerMoved(pointer),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -800.0),
                        phase: egui::TouchPhase::Move,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                size,
            );
        }
        assert!(
            state.quantities.len() > initial,
            "scrolling must construct newly visible products"
        );
        assert!(
            world.resource::<Messages<UiAction>>().is_empty(),
            "scrolling must not purchase items"
        );
    }
}
