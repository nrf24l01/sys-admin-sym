use crate::localization::tr;
use crate::wiki::{MarkdownBlock, MarkdownSpan};
use bevy_egui::egui;

pub(super) fn render(
    ui: &mut egui::Ui,
    blocks: &[MarkdownBlock],
    anchor: &mut Option<String>,
) -> Option<String> {
    let mut clicked = None;
    for (index, block) in blocks.iter().enumerate() {
        ui.push_id(index, |ui| match block {
            MarkdownBlock::Heading {
                level,
                anchor: id,
                spans,
            } => {
                ui.add_space(8.0);
                let size = match level {
                    1 => 24.0,
                    2 => 20.0,
                    _ => 17.0,
                };
                let (response, link) = inline(ui, spans, size, false);
                if link.is_some() {
                    clicked = link;
                }
                if anchor.as_ref() == Some(id) {
                    response.scroll_to_me(Some(egui::Align::TOP));
                    *anchor = None;
                }
                ui.add_space(4.0);
            }
            MarkdownBlock::Text {
                marker,
                depth,
                quote,
                spans,
            } => {
                let size = egui::TextStyle::Body.resolve(ui.style()).size;
                ui.horizontal_top(|ui| {
                    ui.add_space(depth.saturating_sub(1) as f32 * 12.0);
                    if *quote {
                        ui.weak("│");
                    }
                    if let Some(marker) = marker {
                        ui.label(marker);
                    }
                    let (_, link) = inline(ui, spans, size, *quote);
                    if link.is_some() {
                        clicked = link;
                    }
                });
                ui.add_space(if marker.is_some() { 2.0 } else { 6.0 });
            }
            MarkdownBlock::Code(code) => {
                egui::Frame::new()
                    .fill(ui.visuals().code_bg_color)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        if ui.small_button(tr("wiki.copy")).clicked() {
                            ui.ctx().copy_text(code.clone());
                        }
                        egui::ScrollArea::horizontal()
                            .id_salt(("wiki-code", index))
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(code.trim_end_matches('\n'))
                                            .monospace(),
                                    )
                                    .wrap_mode(egui::TextWrapMode::Extend),
                                );
                            });
                    });
                ui.add_space(6.0);
            }
            MarkdownBlock::Rule => {
                ui.separator();
            }
        });
    }
    clicked
}

fn inline(
    ui: &mut egui::Ui,
    spans: &[MarkdownSpan],
    size: f32,
    quote: bool,
) -> (egui::Response, Option<String>) {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = ui.available_width().max(1.0);
    for span in spans {
        let color = if span.link.is_some() {
            ui.visuals().hyperlink_color
        } else if span.bold {
            ui.visuals().strong_text_color()
        } else if quote {
            ui.visuals().weak_text_color()
        } else {
            ui.visuals().text_color()
        };
        job.append(
            &span.text,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::new(
                    size,
                    if span.code {
                        egui::FontFamily::Monospace
                    } else {
                        egui::FontFamily::Proportional
                    },
                ),
                color,
                italics: span.italic,
                background: if span.code {
                    ui.visuals().code_bg_color
                } else {
                    egui::Color32::TRANSPARENT
                },
                underline: if span.link.is_some() {
                    egui::Stroke::new(1.0, color)
                } else {
                    egui::Stroke::NONE
                },
                ..Default::default()
            },
        );
    }
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    let has_links = spans.iter().any(|s| s.link.is_some());
    let response = ui.add(egui::Label::new(galley.clone()).sense(if has_links {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    }));
    let hovered_link = response.hover_pos().and_then(|pos| {
        let cursor = galley.cursor_from_pos(pos - response.rect.min);
        let mut end = 0;
        spans.iter().find_map(|span| {
            let start = end;
            end += span.text.chars().count();
            (start..end)
                .contains(&cursor.index.0)
                .then_some(span.link.as_ref())
                .flatten()
        })
    });
    if let Some(link) = hovered_link {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        response.clone().on_hover_text(link);
    }
    let clicked = response.clicked().then(|| hovered_link.cloned()).flatten();
    (response, clicked)
}
