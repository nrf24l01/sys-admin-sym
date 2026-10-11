//! Real pointer-driven window resizing for layout regression tests.
use bevy_egui::egui;

pub(super) fn rect(ctx: &egui::Context, id: &str) -> egui::Rect {
    ctx.memory(|m| m.area_rect(egui::Id::new(id))).unwrap()
}

pub(super) fn resize(
    ctx: &egui::Context,
    id: &str,
    size: egui::Vec2,
    mut frame: impl FnMut(Vec<egui::Event>) -> egui::FullOutput,
) -> egui::FullOutput {
    let old = rect(ctx, id);
    let start = old.right_bottom() - egui::vec2(2.0, 2.0);
    let end = start + size - old.size();
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(vec![egui::Event::PointerMoved(start)]);
    frame(vec![button(start, true)]);
    frame(vec![egui::Event::PointerMoved(end)]);
    frame(vec![button(end, false)]);
    for _ in 0..5 {
        frame(vec![]);
    }
    let output = frame(vec![]);
    let actual = rect(ctx, id).size();
    assert!(
        (actual - size).abs().max_elem() < 35.0,
        "Window requested {size:?}, got {actual:?}"
    );
    output
}

pub(super) fn text_clip(output: &egui::FullOutput, label: &str) -> egui::Rect {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text().starts_with(label) => {
                Some(shape.clip_rect)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing {label}"))
}

pub(super) fn click_text(
    output: &egui::FullOutput,
    label: &str,
    mut frame: impl FnMut(Vec<egui::Event>) -> egui::FullOutput,
) {
    let pos = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text().starts_with(label) => {
                Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing {label}"));
    for pressed in [true, false] {
        frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }
}

pub(super) fn fills_window_bottom(window: egui::Rect, clip: egui::Rect) {
    let gap = window.bottom() - clip.bottom();
    assert!(
        (0.0..30.0).contains(&gap),
        "Scroll viewport {clip:?} leaves {gap} px below it in {window:?}"
    );
}
