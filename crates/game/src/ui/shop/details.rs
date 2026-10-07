use super::{
    artwork::{self, ShopTextures},
    catalog::{Offer, catalog},
    product,
};
use crate::app::{ShopState, UiAction};
use crate::localization::{UiMessage, tr, tr_args};
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::NetworkSim;
use std::collections::BTreeSet;

pub(super) fn contents(
    ui: &mut egui::Ui,
    offer: &Offer,
    state: &mut ShopState,
    sim: &NetworkSim,
    textures: ShopTextures,
    actions: &mut MessageWriter<UiAction>,
) {
    ui.heading(offer.name());
    ui.weak(&offer.id);
    artwork::image(ui, offer, textures, egui::vec2(ui.available_width(), 160.0));
    ui.label(offer.description());
    ui.add_space(8.0);
    if let Some(target) = state.target
        && let Some(compatibility) = offer.compatibility(sim, target)
    {
        match compatibility {
            Ok(()) => {
                ui.colored_label(ui.visuals().selection.stroke.color, tr("shop.compatible"));
            }
            Err(error) => {
                ui.colored_label(ui.visuals().warn_fg_color, tr("shop.incompatible"));
                ui.label(UiMessage::from(error).render());
            }
        }
        ui.weak(tr("shop.compatibility-hint"));
    }
    product::ownership(ui, offer, sim);
    ui.separator();
    let keys: BTreeSet<_> = offer.attributes.iter().map(|a| a.key).collect();
    egui::Grid::new(("shop-specifications", &offer.id))
        .num_columns(2)
        .spacing(egui::vec2(12.0, 7.0))
        .striped(true)
        .show(ui, |ui| {
            for key in keys {
                let attributes: Vec<_> = offer.attributes.iter().filter(|a| a.key == key).collect();
                ui.weak(attributes[0].label());
                let values: BTreeSet<_> = attributes.iter().map(|a| a.display()).collect();
                ui.label(values.into_iter().collect::<Vec<_>>().join(" / "));
                ui.end_row();
            }
        });
    if !offer.guidance.is_empty() {
        ui.add_space(12.0);
        ui.strong(tr("shop.before-buying"));
        for key in &offer.guidance {
            ui.label(tr(key));
        }
    }
    ui.separator();
    product::purchase(ui, offer, state, sim, actions);
    product::comparison_toggle(ui, offer, state);
}

pub(super) fn windows(
    viewport: &mut egui::Ui,
    state: &mut ShopState,
    sim: &NetworkSim,
    textures: ShopTextures,
    actions: &mut MessageWriter<UiAction>,
    inline_details: bool,
) {
    if state.details_open
        && !inline_details
        && let Some(offer) = state
            .selected_offer
            .as_ref()
            .and_then(|id| catalog().iter().find(|offer| &offer.id == id))
    {
        let mut open = true;
        egui::Window::new(tr("shop.product-details"))
            .id(egui::Id::new("shop-product-details"))
            .open(&mut open)
            .default_size(egui::vec2(460.0, 600.0))
            .show(viewport, |ui| {
                super::style(ui);
                egui::ScrollArea::vertical()
                    .show(ui, |ui| contents(ui, offer, state, sim, textures, actions));
            });
        state.details_open = open;
    }
    if state.comparison_open {
        let mut open = true;
        let offers: Vec<_> = state
            .compared
            .iter()
            .filter_map(|id| catalog().iter().find(|o| &o.id == id))
            .collect();
        egui::Window::new(tr("shop.comparison"))
            .id(egui::Id::new("shop-comparison"))
            .open(&mut open)
            .default_size(egui::vec2(840.0, 530.0))
            .show(viewport, |ui| {
                super::style(ui);
                if offers.len() < 2 {
                    ui.label(tr("shop.compare-select-more"));
                }
                if ui.small_button(tr("shop.clear-comparison")).clicked() {
                    state.compared.clear();
                    state.comparison_open = false;
                }
                egui::ScrollArea::both().show(ui, |ui| {
                    let keys: BTreeSet<_> = offers
                        .iter()
                        .flat_map(|offer| offer.attributes.iter().map(|a| a.key))
                        .collect();
                    egui::Grid::new("shop-comparison-grid")
                        .num_columns(offers.len() + 1)
                        .spacing(egui::vec2(16.0, 8.0))
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("");
                            for offer in &offers {
                                ui.vertical(|ui| {
                                    ui.set_width(210.0);
                                    artwork::image(ui, offer, textures, egui::vec2(210.0, 110.0));
                                    ui.strong(offer.name());
                                });
                            }
                            ui.end_row();
                            ui.weak(tr("shop.price"));
                            for offer in &offers {
                                ui.strong(format!("${}", offer.price()));
                            }
                            ui.end_row();
                            for key in keys {
                                ui.weak(tr(&format!("shop.spec.{key}")));
                                for offer in &offers {
                                    let values: BTreeSet<_> = offer
                                        .attributes
                                        .iter()
                                        .filter(|a| a.key == key)
                                        .map(|a| a.display())
                                        .collect();
                                    ui.label(if values.is_empty() {
                                        "—".into()
                                    } else {
                                        values.into_iter().collect::<Vec<_>>().join(" / ")
                                    });
                                }
                                ui.end_row();
                            }
                            ui.label("");
                            for offer in &offers {
                                if ui.button(tr("shop.details")).clicked() {
                                    state.selected_offer = Some(offer.id.clone());
                                    state.details_open = true;
                                }
                            }
                            ui.end_row();
                            ui.weak(tr("shop.remove"));
                            for offer in &offers {
                                if ui.small_button("×").clicked() {
                                    state.compared.retain(|id| id != &offer.id);
                                }
                            }
                            ui.end_row();
                        });
                });
            });
        state.comparison_open = state.comparison_open && open;
    }
}

pub(super) fn feedback(ui: &mut egui::Ui, state: &ShopState) {
    if let Some(pending) = &state.pending {
        ui.horizontal(|ui| {
            ui.spinner();
            let name = catalog()
                .iter()
                .find(|o| o.id == pending.offer_id)
                .map(|o| o.name())
                .unwrap_or_else(|| pending.offer_id.clone());
            ui.label(tr_args(
                "shop.pending-order",
                &[pending.quantity.to_string(), name],
            ));
        });
    }
    if let Some(feedback) = &state.feedback {
        match &feedback.result {
            Ok(receipt) => {
                let name = catalog()
                    .iter()
                    .find(|o| o.id == feedback.offer_id)
                    .map(|o| o.name())
                    .unwrap_or_else(|| feedback.offer_id.clone());
                ui.label(tr_args(
                    "shop.purchase-success",
                    &[
                        receipt.quantity.to_string(),
                        name,
                        receipt.total.to_string(),
                    ],
                ));
            }
            Err(error) => {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    UiMessage::from(error.clone()).render(),
                );
            }
        }
    }
}
