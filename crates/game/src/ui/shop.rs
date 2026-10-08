//! Equipment catalog, discovery and purchase presentation.
mod artwork;
mod catalog;
mod details;
mod filters;
mod layout;
mod navigation;
#[cfg(test)]
mod navigation_tests;
mod product;
mod query;
#[cfg(test)]
mod server_tests;
#[cfg(test)]
mod tests;

use crate::app::{Selection, ShopState, UiAction};
use crate::localization::{tr, tr_args};
pub(super) use artwork::ShopTextures;
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::NetworkSim;

pub(super) fn show(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut ShopState,
    selected: Selection,
    textures: ShopTextures,
    actions: &mut MessageWriter<UiAction>,
) {
    if !state.open {
        return;
    }
    let mut open = true;
    let mut inline_details = false;
    let screen = viewport.ctx().viewport_rect().size();
    let size = egui::vec2(
        1040.0_f32.min(screen.x - 40.0).max(420.0),
        680.0_f32.min(screen.y - 60.0).max(300.0),
    );
    egui::Window::new(tr("shop.title"))
        .id(egui::Id::new("equipment-shop-window"))
        .open(&mut open)
        .resizable(true)
        .default_pos(egui::pos2(
            ((screen.x - size.x) / 2.0).max(0.0),
            ((screen.y - size.y) / 2.0).max(0.0),
        ))
        .default_size(size)
        .min_size(egui::vec2(420.0, 300.0))
        .show(viewport, |ui| {
            style(ui);
            ui.horizontal_wrapped(|ui| {
                ui.heading(tr("shop.catalog"));
                ui.separator();
                ui.strong(tr_args("ui.available", &[sim.money.to_string()]));
                ui.weak(tr("ui.purchased-equipment-goes-to-inventory"));
            });
            details::feedback(ui, state);
            ui.separator();
            let compact = ui.available_width() < 660.0;
            if compact {
                egui::CollapsingHeader::new(tr("shop.categories-filters"))
                    .id_salt("shop-compact-sidebar")
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("shop-compact-filters")
                            .max_height(220.0)
                            .show(ui, |ui| filters::sidebar(ui, state, sim));
                    });
                products(ui, sim, state, selected, textures, actions);
            } else {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(210.0);
                        egui::ScrollArea::vertical()
                            .id_salt("shop-sidebar")
                            .max_height(ui.available_height())
                            .show(ui, |ui| filters::sidebar(ui, state, sim));
                    });
                    ui.separator();
                    let selected_offer = state
                        .selected_offer
                        .as_ref()
                        .and_then(|id| catalog::catalog().iter().find(|o| &o.id == id));
                    inline_details = state.details_open
                        && selected_offer.is_some()
                        && ui.available_width() > 900.0;
                    let width = ui.available_width() - if inline_details { 330.0 } else { 0.0 };
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        products(ui, sim, state, selected, textures, actions);
                    });
                    if inline_details {
                        ui.separator();
                        ui.vertical(|ui| {
                            ui.set_width(310.0);
                            if ui.small_button(tr("shop.close-details")).clicked() {
                                state.details_open = false;
                            }
                            egui::ScrollArea::vertical()
                                .id_salt("shop-inline-details")
                                .max_height(ui.available_height())
                                .show(ui, |ui| {
                                    details::contents(
                                        ui,
                                        selected_offer.unwrap(),
                                        state,
                                        sim,
                                        textures.full_resolution(),
                                        actions,
                                    )
                                });
                        });
                    }
                });
            }
        });
    state.open = open;
    if state.open {
        details::windows(
            viewport,
            state,
            sim,
            textures.full_resolution(),
            actions,
            inline_details,
        );
    }
}

fn style(ui: &mut egui::Ui) {
    ui.visuals_mut().weak_text_color = Some(egui::Color32::from_rgb(153, 163, 171));
    for (kind, size) in [
        (egui::TextStyle::Body, 14.0),
        (egui::TextStyle::Button, 14.0),
        (egui::TextStyle::Small, 12.0),
    ] {
        ui.style_mut()
            .text_styles
            .insert(kind, egui::FontId::proportional(size));
    }
    ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);
}

fn products(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut ShopState,
    selected: Selection,
    textures: ShopTextures,
    actions: &mut MessageWriter<UiAction>,
) {
    navigation::shortcuts(ui, state, sim);
    filters::toolbar(ui, state);
    filters::compatibility_target(ui, state, sim, selected);
    let offers = query::filtered(state, sim);
    let mut groups = query::grouped(&offers);
    groups.sort_by(|a, b| {
        query::order(
            query::selected_variant(a, state),
            query::selected_variant(b, state),
            state.sort,
        )
    });
    ui.horizontal(|ui| {
        ui.strong(navigation::category_label(state));
        ui.weak(tr_args(
            "shop.results",
            &[groups.len().to_string(), offers.len().to_string()],
        ));
    });
    ui.separator();
    let scroll = egui::ScrollArea::vertical()
        .id_salt((
            "shop-products",
            (!state.all_categories).then_some((state.category, state.section)),
        ))
        .max_height(ui.available_height().max(80.0))
        .auto_shrink([false, false]);
    if groups.is_empty() {
        scroll.show(ui, |ui| {
            ui.add_space(30.0);
            ui.heading(tr("ui.no-products-match-these-filters"));
            ui.label(tr("shop.empty-hint"));
            if ui.button(tr("ui.clear-filters")).clicked() {
                state.clear_filters();
            }
        });
        return;
    }
    let layout = layout::ProductLayout::cached(
        ui,
        state.list_view,
        groups.iter().any(|group| group.len() > 1),
    );
    let total_rows = groups.len().div_ceil(layout.columns);
    scroll.show_rows(ui, layout.row_height, total_rows, |ui, rows| {
        for row in rows {
            let first = row * layout.columns;
            let end = (first + layout.columns).min(groups.len());
            ui.push_id(("product-row", row), |ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), layout.row_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.columns(layout.columns, |uis| {
                            for (index, variants) in groups[first..end].iter().enumerate() {
                                product::card(
                                    &mut uis[index],
                                    variants,
                                    state,
                                    sim,
                                    textures,
                                    actions,
                                    layout,
                                );
                            }
                        });
                    },
                );
            });
        }
    });
}
