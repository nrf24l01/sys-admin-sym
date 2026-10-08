use super::{catalog::catalog, query};
use crate::app::{ShopCategory, ShopSection, ShopState};
use crate::localization::tr;
use bevy_egui::egui;
use cloud_provider_sim::NetworkSim;
use std::collections::BTreeSet;

type CategoryEntry = (
    ShopCategory,
    &'static str,
    &'static [(ShopSection, &'static str)],
);

pub(super) const CATEGORIES: &[CategoryEntry] = &[
    (
        ShopCategory::Network,
        "shop.category.network",
        &[
            (ShopSection::Routers, "ui.routers"),
            (ShopSection::Switches, "ui.switches"),
        ],
    ),
    (
        ShopCategory::Compute,
        "shop.category.compute",
        &[
            (ShopSection::Servers, "shop.servers"),
            (ShopSection::Cpu, "shop.section.cpu"),
            (ShopSection::Ram, "shop.section.ram"),
            (ShopSection::PciCards, "shop.section.nic"),
            (ShopSection::Storage, "ui.storage-drives"),
        ],
    ),
    (
        ShopCategory::Connectivity,
        "shop.category.connectivity",
        &[
            (ShopSection::Transceivers, "shop.section.transceivers"),
            (ShopSection::FiberCables, "shop.section.fiber"),
            (ShopSection::DirectAttach, "shop.section.direct-attach"),
            (ShopSection::CopperSupplies, "shop.section.copper"),
        ],
    ),
    (
        ShopCategory::Rack,
        "shop.category.rack",
        &[
            (ShopSection::PatchPanels, "shop.section.panels"),
            (ShopSection::CableManagers, "shop.section.managers"),
        ],
    ),
    (
        ShopCategory::Power,
        "ui.power",
        &[(ShopSection::Ups, "ui.ups"), (ShopSection::Pdu, "ui.pdu")],
    ),
    (
        ShopCategory::Services,
        "shop.category.services",
        &[(ShopSection::PublicIp, "ui.public-ipv4")],
    ),
];

pub(super) fn category_label(state: &ShopState) -> String {
    if state.all_categories {
        return tr("shop.all-products");
    }
    let (_, label, sections) = CATEGORIES
        .iter()
        .find(|(c, _, _)| *c == state.category)
        .unwrap();
    tr(state
        .section
        .and_then(|s| {
            sections
                .iter()
                .find(|(section, _)| *section == s)
                .map(|(_, label)| *label)
        })
        .unwrap_or(label))
}

fn count(
    state: &ShopState,
    sim: &NetworkSim,
    category: Option<ShopCategory>,
    section: Option<ShopSection>,
) -> usize {
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
}

pub(super) fn sidebar(ui: &mut egui::Ui, state: &mut ShopState, sim: &NetworkSim) {
    ui.strong(tr("shop.browse"));
    if ui
        .selectable_label(
            state.all_categories,
            format!(
                "{}  {}",
                tr("shop.all-products"),
                count(state, sim, None, None)
            ),
        )
        .clicked()
    {
        state.select_all();
    }
    if state.all_categories {
        for &(category, label, _) in CATEGORIES {
            if ui
                .selectable_label(
                    false,
                    format!("{}  {}", tr(label), count(state, sim, Some(category), None)),
                )
                .clicked()
            {
                state.select(category, None);
            }
        }
        return;
    }
    let selected_label = CATEGORIES
        .iter()
        .find(|(category, _, _)| *category == state.category)
        .unwrap()
        .1;
    egui::ComboBox::from_id_salt("shop-category")
        .width(ui.available_width())
        .selected_text(tr(selected_label))
        .show_ui(ui, |ui| {
            for &(category, label, _) in CATEGORIES {
                if ui
                    .selectable_label(
                        state.category == category,
                        format!("{}  {}", tr(label), count(state, sim, Some(category), None)),
                    )
                    .clicked()
                {
                    state.select(category, None);
                }
            }
        });
    let (category, _, sections) = *CATEGORIES
        .iter()
        .find(|(category, _, _)| *category == state.category)
        .unwrap();
    if ui
        .selectable_label(
            state.section.is_none(),
            format!(
                "{}  {}",
                tr("shop.all-in-category"),
                count(state, sim, Some(category), None)
            ),
        )
        .clicked()
    {
        state.select(category, None);
    }
    ui.indent("shop-sections", |ui| {
        for &(section, label) in sections {
            if ui
                .selectable_label(
                    state.section == Some(section),
                    format!(
                        "{}  {}",
                        tr(label),
                        count(state, sim, Some(category), Some(section))
                    ),
                )
                .clicked()
            {
                state.select(category, Some(section));
            }
        }
    });
}

/// Direct choices keep product types discoverable when the narrow sidebar is collapsed.
pub(super) fn shortcuts(ui: &mut egui::Ui, state: &mut ShopState, sim: &NetworkSim) {
    ui.horizontal_wrapped(|ui| {
        if state.all_categories {
            for &(category, label, _) in CATEGORIES {
                if ui
                    .button(format!(
                        "{} ({})",
                        tr(label),
                        count(state, sim, Some(category), None)
                    ))
                    .clicked()
                {
                    state.select(category, None);
                }
            }
        } else {
            if ui.small_button(tr("shop.all-products")).clicked() {
                state.select_all();
                return;
            }
            ui.weak("›");
            let (category, label, sections) = *CATEGORIES
                .iter()
                .find(|(c, _, _)| *c == state.category)
                .unwrap();
            if state.section.is_some() {
                if ui.small_button(tr(label)).clicked() {
                    state.select(category, None);
                }
                ui.weak("›");
                ui.strong(category_label(state));
            } else {
                for &(section, label) in sections {
                    if ui
                        .button(format!(
                            "{} ({})",
                            tr(label),
                            count(state, sim, Some(category), Some(section))
                        ))
                        .clicked()
                    {
                        state.select(category, Some(section));
                    }
                }
            }
        }
    });
}
