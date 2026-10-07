use super::{
    catalog::{catalog, display_value},
    query,
};
use crate::app::{Selection, ShopCategory, ShopSection, ShopSort, ShopState, ShopTarget};
use crate::localization::{tr, tr_args};
use bevy_egui::egui;
use cloud_provider_sim::{DeviceKind, NetworkSim, PortConnector, PurchaseItem};
use std::collections::BTreeSet;

type CategoryEntry = (
    ShopCategory,
    &'static str,
    &'static [(ShopSection, &'static str)],
);
pub(super) const CATEGORIES: &[CategoryEntry] = &[
    (
        ShopCategory::Network,
        "ui.network",
        &[
            (ShopSection::Routers, "ui.routers"),
            (ShopSection::Switches, "ui.switches"),
            (ShopSection::Cabling, "ui.cabling"),
            (ShopSection::Optics, "optics.shop"),
            (ShopSection::PublicIp, "ui.public-ipv4"),
        ],
    ),
    (
        ShopCategory::Compute,
        "ui.compute",
        &[
            (ShopSection::DellServers, "shop.servers"),
            (ShopSection::Cpu, "ui.cpu"),
            (ShopSection::Ram, "ui.ram"),
            (ShopSection::PciCards, "ui.pcie-cards"),
            (ShopSection::Storage, "ui.storage-drives"),
        ],
    ),
    (
        ShopCategory::Power,
        "ui.power",
        &[(ShopSection::Ups, "ui.ups"), (ShopSection::Pdu, "ui.pdu")],
    ),
];

pub(super) fn category_label(state: &ShopState) -> String {
    if state.all_categories {
        return tr("shop.all-products");
    }
    for &(category, label, sections) in CATEGORIES {
        if category == state.category {
            return tr(state
                .section
                .and_then(|s| {
                    sections
                        .iter()
                        .find(|(section, _)| *section == s)
                        .map(|(_, label)| *label)
                })
                .unwrap_or(label));
        }
    }
    tr("shop.all-products")
}

pub(super) fn sidebar(ui: &mut egui::Ui, state: &mut ShopState, sim: &NetworkSim) {
    ui.strong(tr("shop.browse"));
    let count = |category: Option<ShopCategory>, section: Option<ShopSection>| {
        catalog()
            .iter()
            .filter(|offer| {
                category.is_none_or(|c| offer.section.category() == c)
                    && section.is_none_or(|s| offer.section == s)
                    && query::global_matches(offer, state, sim)
            })
            .map(|o| o.family.as_str())
            .collect::<BTreeSet<_>>()
            .len()
    };
    let all = count(None, None);
    let category_counts: Vec<_> = CATEGORIES
        .iter()
        .map(|&(category, _, sections)| {
            (
                count(Some(category), None),
                sections
                    .iter()
                    .map(|&(section, _)| count(Some(category), Some(section)))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    if ui
        .selectable_label(
            state.all_categories,
            format!("{}  {all}", tr("shop.all-products")),
        )
        .clicked()
    {
        state.select_all();
    }
    for (index, &(category, label, sections)) in CATEGORIES.iter().enumerate() {
        ui.add_space(8.0);
        if ui
            .selectable_label(
                !state.all_categories && state.category == category && state.section.is_none(),
                format!("{}  {}", tr(label), category_counts[index].0),
            )
            .clicked()
        {
            state.select(category, None);
        }
        if !state.all_categories && state.category == category {
            ui.indent(label, |ui| {
                for (i, &(section, label)) in sections.iter().enumerate() {
                    if ui
                        .selectable_label(
                            !state.all_categories && state.section == Some(section),
                            format!("{}  {}", tr(label), category_counts[index].1[i]),
                        )
                        .clicked()
                    {
                        state.select(category, Some(section));
                    }
                }
            });
        }
    }
    ui.add_space(12.0);
    ui.separator();
    ui.strong(tr("shop.filters"));
    ui.checkbox(&mut state.affordable_only, tr("ui.affordable-only"));
    price_range(ui, state);
    if ui.small_button(tr("ui.clear-filters")).clicked() {
        state.clear_filters();
    }
    ui.add_space(8.0);
    for key in query::facet_keys(state) {
        let values = query::facet_values(key, state, sim);
        let selected = state.facets.get(key).is_some_and(|v| !v.is_empty());
        if values.len() < 2 && !selected {
            continue;
        }
        egui::CollapsingHeader::new(tr(&format!("shop.spec.{key}")))
            .id_salt(("shop-facet", key))
            .default_open(
                selected
                    || (!state.all_categories
                        && matches!(
                            key,
                            "type" | "speed" | "capacity" | "fiber" | "length" | "rack"
                        )),
            )
            .show(ui, |ui| {
                for (value, count) in values {
                    let mut checked = state.facets.get(key).is_some_and(|v| v.contains(&value));
                    if ui
                        .add_enabled(
                            count > 0 || checked,
                            egui::Checkbox::new(
                                &mut checked,
                                format!("{}  {count}", display_value(key, &value)),
                            ),
                        )
                        .changed()
                    {
                        let selected = state.facets.entry(key.into()).or_default();
                        if checked {
                            selected.insert(value);
                        } else {
                            selected.remove(&value);
                        }
                    }
                }
            });
    }
}

fn price_range(ui: &mut egui::Ui, state: &mut ShopState) {
    egui::CollapsingHeader::new(tr("shop.price-range"))
        .default_open(true)
        .show(ui, |ui| {
            for (label, value) in [
                ("shop.minimum", &mut state.min_price),
                ("shop.maximum", &mut state.max_price),
            ] {
                ui.horizontal(|ui| {
                    let mut enabled = value.is_some();
                    if ui.checkbox(&mut enabled, tr(label)).changed() {
                        *value = enabled.then_some(0);
                    }
                    if let Some(value) = value {
                        ui.add(
                            egui::DragValue::new(value)
                                .range(0..=1_000_000)
                                .speed(10)
                                .prefix("$"),
                        );
                    }
                });
            }
        });
}

pub(super) fn toolbar(ui: &mut egui::Ui, state: &mut ShopState) {
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.search)
                .desired_width((ui.available_width() - 240.0).max(140.0))
                .hint_text(tr("ui.search-products")),
        );
        egui::ComboBox::from_id_salt("shop-sort")
            .width(130.0)
            .selected_text(sort_label(state.sort))
            .show_ui(ui, |ui| {
                for sort in [
                    ShopSort::Category,
                    ShopSort::Name,
                    ShopSort::PriceAscending,
                    ShopSort::PriceDescending,
                    ShopSort::CapacityDescending,
                    ShopSort::SpeedDescending,
                    ShopSort::LengthAscending,
                ] {
                    ui.selectable_value(&mut state.sort, sort, sort_label(sort));
                }
            });
    });
    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.list_view, false, tr("shop.grid"));
        ui.selectable_value(&mut state.list_view, true, tr("shop.list"));
        if !state.compared.is_empty()
            && ui
                .button(tr_args(
                    "shop.compare-count",
                    &[state.compared.len().to_string()],
                ))
                .clicked()
        {
            state.comparison_open = true;
        }
    });
    ui.horizontal_wrapped(|ui| {
        if !state.search.is_empty() && ui.small_button(format!("{} ×", state.search)).clicked() {
            state.search.clear();
        }
        if state.affordable_only
            && ui
                .small_button(format!("{} ×", tr("ui.affordable-only")))
                .clicked()
        {
            state.affordable_only = false;
        }
        if let Some(min) = state.min_price
            && ui.small_button(format!("≥ ${min} ×")).clicked()
        {
            state.min_price = None;
        }
        if let Some(max) = state.max_price
            && ui.small_button(format!("≤ ${max} ×")).clicked()
        {
            state.max_price = None;
        }
        let chips: Vec<_> = state
            .facets
            .iter()
            .flat_map(|(key, values)| values.iter().map(move |value| (key.clone(), value.clone())))
            .collect();
        for (key, value) in chips {
            if ui
                .small_button(format!(
                    "{}: {} ×",
                    tr(&format!("shop.spec.{key}")),
                    display_value(&key, &value)
                ))
                .clicked()
            {
                state.facets.get_mut(&key).unwrap().remove(&value);
            }
        }
    });
}
fn sort_label(sort: ShopSort) -> String {
    tr(match sort {
        ShopSort::Category => "shop.sort.category",
        ShopSort::Name => "shop.sort.name",
        ShopSort::PriceAscending => "shop.sort.price-up",
        ShopSort::PriceDescending => "shop.sort.price-down",
        ShopSort::CapacityDescending => "shop.sort.capacity",
        ShopSort::SpeedDescending => "shop.sort.speed",
        ShopSort::LengthAscending => "shop.sort.length",
    })
}

pub(super) fn compatibility_target(
    ui: &mut egui::Ui,
    state: &mut ShopState,
    sim: &NetworkSim,
    selected: Selection,
) {
    if state.target.is_some_and(|target| match target {
        ShopTarget::Server(id) => sim.device(id).is_none(),
        ShopTarget::Port(id) => sim.port(id).is_none(),
    }) {
        state.target = None;
        state.compatible_only = false;
    }
    let scope = |offer: &&super::catalog::Offer| {
        state.all_categories
            || (offer.section.category() == state.category
                && state.section.is_none_or(|section| section == offer.section))
    };
    let server_scope = catalog().iter().filter(scope).any(|offer| {
        matches!(
            offer.item,
            PurchaseItem::ServerPart(_) | PurchaseItem::Drive(_)
        )
    });
    let port_scope = catalog().iter().filter(scope).any(|offer| {
        matches!(
            offer.item,
            PurchaseItem::Transceiver(_) | PurchaseItem::Assembly(_)
        )
    });
    if !server_scope && !port_scope {
        state.compatible_only = false;
        return;
    }
    let mut targets = Vec::new();
    for device in sim.devices() {
        if server_scope
            && matches!(&device.kind,DeviceKind::Server(server) if server.hardware.is_some())
        {
            targets.push((
                ShopTarget::Server(device.id),
                crate::localization::device_name(device),
            ));
        }
        for &port_id in device.ports() {
            if port_scope
                && let Some(port) = sim.port(port_id)
                && matches!(port.connector, PortConnector::Sfp | PortConnector::Lc)
            {
                targets.push((
                    ShopTarget::Port(port_id),
                    format!(
                        "{} · {}",
                        crate::localization::device_name(device),
                        port.name
                    ),
                ));
            }
        }
    }
    if state
        .target
        .is_some_and(|target| !targets.iter().any(|(candidate, _)| *candidate == target))
    {
        state.target = None;
        state.compatible_only = false;
    }
    if targets.is_empty() {
        return;
    }
    targets.sort_by(|a, b| a.1.cmp(&b.1));
    let selected_target = match selected {
        Selection::Device(id) => Some(ShopTarget::Server(id)),
        Selection::Port(id) => Some(ShopTarget::Port(id)),
        _ => None,
    };
    ui.horizontal_wrapped(|ui| {
        ui.label(tr("shop.target"));
        egui::ComboBox::from_id_salt("shop-target")
            .width(230.0)
            .selected_text(
                targets
                    .iter()
                    .find(|(target, _)| Some(*target) == state.target)
                    .map(|(_, name)| name.clone())
                    .unwrap_or_else(|| tr("ui.any")),
            )
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(&mut state.target, None, tr("ui.any"))
                    .changed()
                {
                    state.compatible_only = false;
                }
                for (target, name) in &targets {
                    ui.selectable_value(&mut state.target, Some(*target), name);
                }
            });
        if let Some(target) = selected_target
            && targets.iter().any(|(candidate, _)| *candidate == target)
            && ui.small_button(tr("shop.use-selected")).clicked()
        {
            state.target = Some(target);
        }
        ui.add_enabled(
            state.target.is_some(),
            egui::Checkbox::new(&mut state.compatible_only, tr("shop.compatible-only")),
        )
        .on_hover_text(tr("shop.compatibility-hint"));
    });
}
