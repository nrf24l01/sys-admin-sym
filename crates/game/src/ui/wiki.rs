use crate::app::WikiWindowState;
use crate::localization::{device_name, tr, tr_args};
use crate::wiki::{WikiLibrary, resolve_article_link};
use bevy_egui::egui;
use cloud_provider_sim::*;
mod markdown;
#[cfg(test)]
mod tests;

pub(super) fn show(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut WikiWindowState,
    library: &mut WikiLibrary,
    language: &str,
    refresh: bool,
) {
    if !state.open {
        state.report = None;
        return;
    }
    if refresh || state.report.is_none() {
        refresh_report(sim, state);
    }
    let mut open = true;
    egui::Window::new(tr("wiki.title"))
        .id(egui::Id::new("wiki-window"))
        .open(&mut open)
        .collapsible(false)
        .default_size(egui::vec2(1040.0, 720.0))
        .min_size(egui::vec2(620.0, 420.0))
        .show(viewport, |ui| {
            let report = state.report.as_ref().unwrap();
            let detected = (report.controller, report.node);
            let previous = (state.controller, state.node);
            ui.horizontal_wrapped(|ui| {
                role_selector(
                    ui,
                    sim,
                    "wiki.controller",
                    &mut state.controller,
                    detected.0,
                    None,
                );
                role_selector(
                    ui,
                    sim,
                    "wiki.node",
                    &mut state.node,
                    detected.1,
                    detected.0,
                );
                if ui.button(tr("wiki.reload")).clicked() {
                    library.reload();
                }
                if ui
                    .add_enabled(
                        !state.history.is_empty(),
                        egui::Button::new(tr("wiki.back")),
                    )
                    .clicked()
                {
                    state.article = state.history.pop().unwrap();
                    state.anchor = None;
                    state.step = None;
                    state.link_error = None;
                }
            });
            if previous != (state.controller, state.node) {
                refresh_report(sim, state);
            }
            let report = state.report.as_ref().unwrap();
            let complete = report.checks.iter().filter(|c| c.complete).count();
            ui.horizontal_wrapped(|ui| {
                ui.label(tr_args(
                    "wiki.progress",
                    &[complete.to_string(), report.checks.len().to_string()],
                ));
                if report.sold {
                    ui.colored_label(egui::Color32::LIGHT_GREEN, tr("wiki.completed"));
                }
            });
            ui.weak(tr("wiki.live-help"));
            if let Some(error) = &state.link_error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
            if !library.errors.is_empty() {
                ui.collapsing(tr("wiki.load-error"), |ui| {
                    for error in &library.errors {
                        ui.label(error);
                    }
                });
            }
            ui.separator();
            let height = ui.available_height().max(200.0);
            let width = ui.available_width();
            let nav_width = (width * 0.30).clamp(180.0, 260.0);
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(nav_width, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(nav_width);
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut state.show_articles, false, tr("wiki.guide"));
                            ui.selectable_value(
                                &mut state.show_articles,
                                true,
                                tr("wiki.articles"),
                            );
                        });
                        egui::ScrollArea::vertical()
                            .id_salt("wiki-navigation")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if ui
                                    .add(
                                        egui::Button::selectable(
                                            state.article == "tutorial/first-vm",
                                            tr("wiki.home"),
                                        )
                                        .wrap(),
                                    )
                                    .clicked()
                                {
                                    state.navigate("tutorial/first-vm");
                                }
                                ui.separator();
                                if state.show_articles {
                                    for article in library.articles(language) {
                                        if ui
                                            .add(
                                                egui::Button::selectable(
                                                    state.article == article.id,
                                                    &article.title,
                                                )
                                                .wrap(),
                                            )
                                            .clicked()
                                        {
                                            state.navigate(&article.id);
                                        }
                                    }
                                } else {
                                    for step in FirstVmStep::ALL {
                                        let checked = state.report.as_ref().unwrap().check(step);
                                        let label = format!(
                                            "{} {}",
                                            if checked.complete { "✓" } else { "○" },
                                            tr(&format!("wiki.task.{}", step.slug()))
                                        );
                                        let color = if checked.complete {
                                            egui::Color32::LIGHT_GREEN
                                        } else {
                                            ui.visuals().text_color()
                                        };
                                        if ui
                                            .add(
                                                egui::Button::selectable(
                                                    state.step == Some(step),
                                                    egui::RichText::new(label).color(color),
                                                )
                                                .wrap(),
                                            )
                                            .on_hover_text(tr(&format!(
                                                "wiki.hint.{}",
                                                step.slug()
                                            )))
                                            .clicked()
                                        {
                                            state.navigate(step_article(step));
                                            state.step = Some(step);
                                        }
                                    }
                                }
                            });
                    },
                );
                ui.separator();
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width().max(1.0), height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt(("wiki-article", &state.article))
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if let Some(step) = state.step {
                                    let check = state.report.as_ref().unwrap().check(step);
                                    ui.strong(tr(&format!("wiki.task.{}", step.slug())));
                                    ui.colored_label(
                                        if check.complete {
                                            egui::Color32::LIGHT_GREEN
                                        } else {
                                            egui::Color32::from_rgb(255, 196, 64)
                                        },
                                        tr(if check.complete {
                                            "wiki.correct"
                                        } else {
                                            "wiki.pending"
                                        }),
                                    );
                                    if !check.complete {
                                        ui.label(tr(&format!("wiki.hint.{}", step.slug())));
                                        if let Some(detail) = &check.detail {
                                            ui.label(detail);
                                        }
                                    }
                                    ui.separator();
                                }
                                if let Some(article) = library.article(language, &state.article) {
                                    if let Some(link) =
                                        markdown::render(ui, &article.blocks, &mut state.anchor)
                                    {
                                        follow_link(ui, state, library, language, &link);
                                    }
                                } else {
                                    ui.colored_label(
                                        egui::Color32::LIGHT_RED,
                                        tr_args(
                                            "wiki.missing",
                                            std::slice::from_ref(&state.article),
                                        ),
                                    );
                                    if library.articles(language).is_empty() {
                                        ui.label(tr("wiki.empty"));
                                    }
                                }
                            });
                    },
                );
            });
        });
    state.open = open;
    if !open {
        state.report = None;
    }
}

fn refresh_report(sim: &NetworkSim, state: &mut WikiWindowState) {
    let report = sim.first_vm_guide(state.controller, state.node);
    // Drop invalid or colliding overrides so labels display the evaluated roles.
    state.controller = state.controller.filter(|id| report.controller == Some(*id));
    state.node = state.node.filter(|id| report.node == Some(*id));
    state.report = Some(report);
}

fn role_selector(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    key: &str,
    selected: &mut Option<DeviceId>,
    detected: Option<DeviceId>,
    exclude: Option<DeviceId>,
) {
    ui.label(tr(key));
    let label = selected
        .or(detected)
        .and_then(|id| sim.device(id))
        .map(|d| format!("{} #{}", device_name(d), d.id.0))
        .unwrap_or_else(|| tr("wiki.no-server"));
    egui::ComboBox::from_id_salt(key)
        .width(170.0)
        .selected_text(label)
        .show_ui(ui, |ui| {
            ui.selectable_value(selected, None, tr("wiki.automatic"));
            let mut servers: Vec<_> = sim
                .devices()
                .filter(|d| matches!(d.kind, DeviceKind::Server(_)) && Some(d.id) != exclude)
                .collect();
            servers.sort_by_key(|d| d.id);
            for device in servers {
                ui.selectable_value(
                    selected,
                    Some(device.id),
                    format!("{} #{}", device_name(device), device.id.0),
                );
            }
        })
        .response
        .on_hover_text(tr("wiki.roles-help"));
}

fn step_article(step: FirstVmStep) -> &'static str {
    match step {
        FirstVmStep::Switch | FirstVmStep::Servers | FirstVmStep::Power => "tutorial/hardware",
        FirstVmStep::ManagementCables => "tutorial/management-cables",
        FirstVmStep::ManagementVlan => "tutorial/management-vlan",
        FirstVmStep::ManagementAddresses => "tutorial/management-addresses",
        FirstVmStep::ManagementPing => "tutorial/management-ping",
        FirstVmStep::ControllerService | FirstVmStep::NodePrepared => "tutorial/hosting-control",
        FirstVmStep::PublicRange => "tutorial/public-range",
        FirstVmStep::DataNetwork => "tutorial/data-network",
        FirstVmStep::Storefront => "tutorial/storefront",
        FirstVmStep::Product => "tutorial/product",
        FirstVmStep::FirstSale => "tutorial/first-sale",
    }
}

fn follow_link(
    ui: &egui::Ui,
    state: &mut WikiWindowState,
    library: &WikiLibrary,
    language: &str,
    url: &str,
) {
    if url.starts_with("https://") || url.starts_with("http://") {
        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
    } else if let Some(link) = resolve_article_link(&state.article, url)
        && let Some(article) = library.article(language, &link.article)
        && link.anchor.as_ref().is_none_or(|anchor| article.blocks.iter().any(|b| matches!(b, crate::wiki::MarkdownBlock::Heading { anchor: a, .. } if a == anchor))) {
        state.navigate(&link.article);
        state.anchor = link.anchor;
    } else { state.link_error = Some(tr_args("wiki.bad-link", &[url.to_owned()])); }
}
