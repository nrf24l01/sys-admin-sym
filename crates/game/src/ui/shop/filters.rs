use super::{
    catalog::{catalog, display_value},
    navigation, query,
};
use crate::app::{Selection, ShopSort, ShopState, ShopTarget};
use crate::localization::{tr, tr_args};
use bevy_egui::egui;
use cloud_provider_sim::{DeviceKind, NetworkSim, PortConnector, PurchaseItem};

pub(super) fn sidebar(ui: &mut egui::Ui, state: &mut ShopState, sim: &NetworkSim) {
    navigation::sidebar(ui, state, sim);
    ui.add_space(12.0);
    ui.separator();
    ui.strong(tr("shop.filters"));
    ui.checkbox(&mut state.affordable_only, tr("ui.affordable-only"));
    price_range(ui, state, sim.money);
    if ui.small_button(tr("ui.clear-filters")).clicked() {
        state.clear_filters();
    }
    ui.add_space(8.0);
    if state.all_categories || state.section.is_none() {
        ui.weak(tr("shop.choose-type-filters"));
        return;
    }
    for &key in query::facet_keys(state) {
        let values = query::facet_values(key, state, sim);
        let selected = state.facets.get(key).is_some_and(|v| !v.is_empty());
        if values.is_empty() {
            continue;
        }
        egui::CollapsingHeader::new(facet_label(state, key))
            .id_salt(("shop-facet", state.section, key))
            .default_open(selected || query::facet_keys(state).iter().take(3).any(|k| *k == key))
            .show(ui, |ui| {
                for (value, count) in values {
                    let mut checked = state.facets.get(key).is_some_and(|v| v.contains(&value));
                    if ui
                        .add_enabled(
                            count > 0 || checked,
                            egui::Checkbox::new(
                                &mut checked,
                                format!("{} ({count})", display_value(key, &value)),
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

fn price_range(ui: &mut egui::Ui, state: &mut ShopState, balance: i64) {
    egui::CollapsingHeader::new(tr("shop.price-range"))
        .default_open(state.min_price.is_some() || state.max_price.is_some())
        .show(ui, |ui| {
            for (label, value, initial) in [
                ("shop.minimum", &mut state.min_price, 0),
                ("shop.maximum", &mut state.max_price, balance.max(0)),
            ] {
                ui.horizontal(|ui| {
                    let mut enabled = value.is_some();
                    if ui.checkbox(&mut enabled, tr(label)).changed() {
                        *value = enabled.then_some(initial);
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
    let section = (!state.all_categories).then_some(state.section).flatten();
    if !state.sort.applies_to(section) {
        state.sort = ShopSort::Category;
    }
    ui.horizontal(|ui| {
        let sort_width = (ui.available_width() * 0.4).clamp(130.0, 240.0);
        let search_width =
            (ui.available_width() - sort_width - ui.spacing().item_spacing.x).max(80.0);
        ui.add(
            egui::TextEdit::singleline(&mut state.search)
                .desired_width(search_width)
                .hint_text(tr("ui.search-products")),
        );
        egui::ComboBox::from_id_salt("shop-sort")
            .width(sort_width)
            .height(280.0)
            .truncate()
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
                    ShopSort::CpuSocket,
                    ShopSort::MemoryType,
                    ShopSort::RamSlotsDescending,
                    ShopSort::CpuSocketsDescending,
                    ShopSort::DriveBaysDescending,
                    ShopSort::PcieSlotsDescending,
                ] {
                    if sort.applies_to(section) {
                        ui.selectable_value(&mut state.sort, sort, sort_label(sort));
                    }
                }
            })
            .response
            .on_hover_text(sort_label(state.sort));
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
                    facet_label(state, &key),
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
        ShopSort::CpuSocket => "shop.sort.socket",
        ShopSort::MemoryType => "shop.sort.memory-type",
        ShopSort::RamSlotsDescending => "shop.sort.ram-slots",
        ShopSort::CpuSocketsDescending => "shop.sort.cpu-sockets",
        ShopSort::DriveBaysDescending => "shop.sort.drive-bays",
        ShopSort::PcieSlotsDescending => "shop.sort.pcie-slots",
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
    if state.all_categories || state.section.is_none() {
        state.compatible_only = false;
        return;
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

fn facet_label(state: &ShopState, key: &str) -> String {
    use crate::app::ShopSection;
    let label = match (state.section, key) {
        (Some(ShopSection::Storage), "type") => "shop.filter.drive-type",
        (Some(ShopSection::DirectAttach), "type") => "shop.filter.cable-type",
        (Some(ShopSection::CopperSupplies), "type") => "shop.filter.supply-type",
        (Some(ShopSection::PatchPanels), "type") => "shop.filter.panel-type",
        _ => return tr(&format!("shop.spec.{key}")),
    };
    tr(label)
}
