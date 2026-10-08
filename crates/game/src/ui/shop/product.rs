use super::{
    artwork::{self, ShopTextures},
    catalog::{Offer, catalog},
};
use crate::app::{PendingPurchase, ShopState, UiAction};
use crate::localization::{UiMessage, tr, tr_args};
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::{MAX_PURCHASE_QUANTITY, NetworkSim, PurchaseItem};

pub(super) fn card(
    ui: &mut egui::Ui,
    variants: &[&Offer],
    state: &mut ShopState,
    sim: &NetworkSim,
    textures: ShopTextures,
    actions: &mut MessageWriter<UiAction>,
    layout: super::layout::ProductLayout,
) {
    let list = layout.is_list();
    let row_height = layout.row_height;
    let family = &variants[0].family;
    let chosen = super::query::selected_variant(variants, state);
    let mut selected = chosen.id.clone();
    ui.push_id(family, |ui| {
        let stroke = if state.selected_offer.as_ref() == Some(&chosen.id) {
            ui.visuals().selection.stroke
        } else {
            ui.visuals().widgets.noninteractive.bg_stroke
        };
        egui::Frame::group(ui.style())
            .inner_margin(12)
            .corner_radius(8)
            .stroke(stroke)
            .show(ui, |ui| {
                ui.set_min_height((row_height - 28.0).max(0.0));
                ui.set_min_width((ui.available_width() - 2.0).max(0.0));
                if list {
                    ui.horizontal_top(|ui| {
                        artwork::image(ui, chosen, textures, egui::vec2(112.0, 102.0));
                        ui.vertical(|ui| {
                            ui.add(
                                egui::Label::new(egui::RichText::new(chosen.name()).strong())
                                    .truncate(),
                            )
                            .on_hover_text(chosen.name());
                            ui.add(egui::Label::new(chosen.description()).truncate())
                                .on_hover_text(chosen.description());
                            summary(ui, chosen);
                        });
                    });
                } else {
                    artwork::image(
                        ui,
                        chosen,
                        textures,
                        egui::vec2(ui.available_width(), 112.0),
                    );
                    ui.add_space(6.0);
                    ui.add(
                        egui::Label::new(egui::RichText::new(chosen.name()).strong()).truncate(),
                    )
                    .on_hover_text(chosen.name());
                    ui.add(egui::Label::new(chosen.description()).truncate())
                        .on_hover_text(chosen.description());
                    summary(ui, chosen);
                }
                if variants.len() > 1 {
                    egui::ComboBox::from_id_salt("variant")
                        .width(ui.available_width().min(280.0))
                        .selected_text(variant_label(chosen))
                        .show_ui(ui, |ui| {
                            for offer in variants {
                                ui.selectable_value(
                                    &mut selected,
                                    offer.id.clone(),
                                    format!("{} · ${}", variant_label(offer), offer.price()),
                                );
                            }
                        });
                }
                let offer = variants
                    .iter()
                    .find(|offer| offer.id == selected)
                    .copied()
                    .unwrap_or(chosen);
                if selected != chosen.id || !state.variants.contains_key(family) {
                    state.variants.insert(family.clone(), selected);
                }
                ownership(ui, offer, sim);
                purchase(ui, offer, state, sim, actions);
                ui.horizontal(|ui| {
                    if ui.small_button(tr("shop.details")).clicked() {
                        state.selected_offer = Some(offer.id.clone());
                        state.details_open = true;
                    }
                    comparison_toggle(ui, offer, state);
                });
            });
    });
}
fn variant_label(offer: &Offer) -> String {
    if let Some(attribute) = offer
        .attributes
        .iter()
        .find(|a| matches!(a.key, "length" | "configuration"))
    {
        attribute.display()
    } else {
        offer.name()
    }
}
fn summary(ui: &mut egui::Ui, offer: &Offer) {
    use crate::app::ShopSection;
    let preferred: &[&str] = match offer.section {
        ShopSection::Routers | ShopSection::Switches => &["ports", "cages", "speed", "rack"],
        ShopSection::PatchPanels | ShopSection::CableManagers | ShopSection::CopperSupplies => {
            &["type", "ports", "lc-pairs", "rack"]
        }
        ShopSection::Transceivers | ShopSection::FiberCables | ShopSection::DirectAttach => {
            &["type", "cage", "speed", "fiber", "strands", "length"]
        }
        ShopSection::Servers => &["socket", "memory-type", "ram-slots"],
        ShopSection::Cpu => &["cores", "frequency", "socket"],
        ShopSection::Ram => &["capacity", "memory-type"],
        ShopSection::PciCards => &["ports", "cages", "speed", "pcie-generation"],
        ShopSection::Storage => &["type", "capacity", "interface"],
        ShopSection::Ups | ShopSection::Pdu => &["rack", "watts", "outlets"],
        ShopSection::PublicIp => &["prefix", "addresses"],
    };
    let mut summaries = Vec::new();
    for key in preferred {
        let values: Vec<_> = offer
            .attributes
            .iter()
            .filter(|a| a.key == *key)
            .map(|a| {
                if matches!(
                    a.key,
                    "ports"
                        | "cages"
                        | "lc-pairs"
                        | "cores"
                        | "outlets"
                        | "addresses"
                        | "pcie-generation"
                        | "ram-slots"
                ) {
                    tr_args(&format!("shop.summary.{}", a.key), &[a.display()])
                } else {
                    a.display()
                }
            })
            .collect();
        if !values.is_empty() {
            summaries.push(values.join(" / "));
        }
        if summaries.len() == 3 {
            break;
        }
    }
    ui.add(egui::Label::new(egui::RichText::new(summaries.join(" · ")).small().weak()).truncate())
        .on_hover_text(summaries.join(" · "));
}

pub(super) fn ownership(ui: &mut egui::Ui, offer: &Offer, sim: &NetworkSim) {
    let text = match offer.item {
        PurchaseItem::Supply(cloud_provider_sim::CableSupply::CableBox305m) => tr_args(
            "ui.bulk-cable-m",
            &[format!(
                "{:.2}",
                sim.cable_inventory().cable_cm as f64 / 100.0
            )],
        ),
        PurchaseItem::Supply(cloud_provider_sim::CableSupply::Rj45Pack20) => tr_args(
            "ui.rj45-connectors",
            &[sim.cable_inventory().connectors.to_string()],
        ),
        PurchaseItem::PublicIpv4Pool => tr_args(
            "shop.ipv4-owned",
            &[
                sim.public_ipv4_blocks().len().to_string(),
                sim.available_public_ipv4_pools().to_string(),
            ],
        ),
        _ => {
            let (free, installed) = offer.ownership(sim);
            tr_args("shop.ownership", &[free.to_string(), installed.to_string()])
        }
    };
    ui.add(egui::Label::new(egui::RichText::new(text).small().weak()).truncate());
}

pub(super) fn purchase(
    ui: &mut egui::Ui,
    offer: &Offer,
    state: &mut ShopState,
    sim: &NetworkSim,
    actions: &mut MessageWriter<UiAction>,
) {
    let quantity = state.quantities.entry(offer.id.clone()).or_insert(1);
    ui.horizontal_wrapped(|ui| {
        ui.strong(tr_args("shop.unit-price", &[offer.price().to_string()]));
        ui.label(tr("shop.quantity"));
        ui.add(
            egui::DragValue::new(quantity)
                .range(1..=MAX_PURCHASE_QUANTITY)
                .speed(1),
        );
    });
    let quantity = *quantity;
    let quote = sim.quote_purchase(&offer.item, quantity);
    let total = offer.price() * i64::from(quantity);
    let label = if state
        .pending
        .as_ref()
        .is_some_and(|pending| pending.offer_id == offer.id)
    {
        tr("shop.purchasing")
    } else {
        tr_args("shop.buy", &[total.to_string()])
    };
    let button = ui.add_enabled(
        state.pending.is_none() && quote.is_ok(),
        egui::Button::new(label).min_size(egui::vec2(ui.available_width().min(260.0), 28.0)),
    );
    let button = if let Err(error) = &quote {
        button.on_hover_text(UiMessage::from(error.clone()).render())
    } else {
        button.on_hover_text(tr_args(
            "shop.balance-after",
            &[(sim.money - total).to_string()],
        ))
    };
    if button.clicked() {
        state.next_request_id = state.next_request_id.wrapping_add(1);
        let request_id = state.next_request_id;
        state.pending = Some(PendingPurchase {
            request_id,
            offer_id: offer.id.clone(),
            quantity,
        });
        state.feedback = None;
        actions.write(UiAction::ShopPurchase {
            request_id,
            item: offer.item.clone(),
            quantity,
        });
    }
    if let Err(error) = quote {
        ui.add(
            egui::Label::new(
                egui::RichText::new(UiMessage::from(error).render())
                    .small()
                    .color(ui.visuals().warn_fg_color),
            )
            .truncate(),
        );
    }
}

pub(super) fn comparison_toggle(ui: &mut egui::Ui, offer: &Offer, state: &mut ShopState) {
    let mut checked = state.compared.contains(&offer.id);
    let same_family = state
        .compared
        .first()
        .and_then(|id| catalog().iter().find(|o| &o.id == id))
        .is_none_or(|first| first.can_compare_with(offer));
    let enabled = checked || (state.compared.len() < 3 && same_family);
    let response = ui
        .add_enabled(
            enabled,
            egui::Checkbox::new(&mut checked, tr("shop.compare")),
        )
        .on_hover_text(tr("shop.compare-hint"));
    if response.changed() {
        if checked {
            state.compared.push(offer.id.clone());
        } else {
            state.compared.retain(|id| id != &offer.id);
        }
    }
}
