use super::*;

fn frame(
    ctx: &egui::Context,
    sim: &NetworkSim,
    state: &mut WikiWindowState,
    library: &mut WikiLibrary,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 900.0),
            )),
            events,
            ..Default::default()
        },
        |ui| show(ui, sim, state, library, "en", true),
    );
    output.textures_delta.clear();
    output
}
fn texts(output: &egui::FullOutput) -> Vec<String> {
    output
        .shapes
        .iter()
        .filter_map(|s| {
            if let egui::Shape::Text(t) = &s.shape {
                Some(t.galley.text().to_owned())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn wiki_opens_guide_navigates_and_keeps_current_checks() {
    let ctx = egui::Context::default();
    let sim = NetworkSim::new();
    let mut state = WikiWindowState {
        open: true,
        ..Default::default()
    };
    let mut library = WikiLibrary::load(&crate::asset_root().join("wiki"));
    frame(&ctx, &sim, &mut state, &mut library, vec![]);
    let output = frame(&ctx, &sim, &mut state, &mut library, vec![]);
    let rendered = texts(&output).join("\n");
    assert!(rendered.contains("Sell your first VM"), "{rendered}");
    assert!(rendered.contains("0 / 14"), "{rendered}");
    super::super::test_windows::click_text(&output, "Articles", |events| {
        frame(&ctx, &sim, &mut state, &mut library, events)
    });
    let output = frame(&ctx, &sim, &mut state, &mut library, vec![]);
    super::super::test_windows::click_text(&output, "Networks and physical", |events| {
        frame(&ctx, &sim, &mut state, &mut library, events)
    });
    assert_eq!(state.article, "reference/network");
    assert!(state.report.as_ref().is_some_and(|r| !r.sold));
    let output = frame(&ctx, &sim, &mut state, &mut library, vec![]);
    super::super::test_windows::click_text(&output, "Back", |events| {
        frame(&ctx, &sim, &mut state, &mut library, events)
    });
    assert_eq!(state.article, "tutorial/first-vm");
    state.open = false;
    assert!(texts(&frame(&ctx, &sim, &mut state, &mut library, vec![])).is_empty());
    state.open = true;
    assert!(!texts(&frame(&ctx, &sim, &mut state, &mut library, vec![])).is_empty());
}

#[test]
fn wiki_resizes_and_code_copy_preserves_commands() {
    let ctx = egui::Context::default();
    let sim = NetworkSim::new();
    let mut state = WikiWindowState {
        open: true,
        article: "tutorial/management-ping".into(),
        ..Default::default()
    };
    let mut library = WikiLibrary::load(&crate::asset_root().join("wiki"));
    for _ in 0..3 {
        frame(&ctx, &sim, &mut state, &mut library, vec![]);
    }
    let output = super::super::test_windows::resize(
        &ctx,
        "wiki-window",
        egui::vec2(700.0, 550.0),
        |events| frame(&ctx, &sim, &mut state, &mut library, events),
    );
    let window = super::super::test_windows::rect(&ctx, "wiki-window");
    let clip = super::super::test_windows::text_clip(&output, "Check management");
    assert!(clip.width() < window.width(), "{clip:?} {window:?}");
    let copied = std::cell::RefCell::new(Vec::new());
    super::super::test_windows::click_text(&output, "Copy", |events| {
        let output = frame(&ctx, &sim, &mut state, &mut library, events);
        copied
            .borrow_mut()
            .extend(output.platform_output.commands.clone());
        output
    });
    assert!(
        copied.borrow().iter().any(
            |c| matches!(c, egui::OutputCommand::CopyText(s) if s.contains("ping -c 3 -I eth1"))
        )
    );
}

#[test]
fn wiki_missing_article_reports_the_problem_without_discarding_guide() {
    let ctx = egui::Context::default();
    let sim = NetworkSim::new();
    let mut state = WikiWindowState {
        open: true,
        article: "missing".into(),
        ..Default::default()
    };
    let mut library = WikiLibrary::load(&crate::asset_root().join("wiki"));
    frame(&ctx, &sim, &mut state, &mut library, vec![]);
    let output = frame(&ctx, &sim, &mut state, &mut library, vec![]);
    assert!(texts(&output).join("\n").contains("Article not found"));
    assert!(state.report.is_some());
}

#[test]
fn wiki_reopening_refreshes_changes_that_happened_while_closed() {
    let ctx = egui::Context::default();
    let mut sim = NetworkSim::new();
    let mut state = WikiWindowState {
        open: true,
        ..Default::default()
    };
    let mut library = WikiLibrary::load(&crate::asset_root().join("wiki"));
    frame(&ctx, &sim, &mut state, &mut library, vec![]);
    assert!(
        !state
            .report
            .as_ref()
            .unwrap()
            .check(FirstVmStep::Switch)
            .complete
    );
    state.open = false;
    sim.execute(Command::BuyDevice {
        kind: DeviceTemplate::Switch,
    })
    .unwrap();
    frame(&ctx, &sim, &mut state, &mut library, vec![]);
    state.open = true;
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        show(ui, &sim, &mut state, &mut library, "en", false)
    });
    output.textures_delta.clear();
    assert!(
        state
            .report
            .as_ref()
            .unwrap()
            .check(FirstVmStep::Switch)
            .complete
    );
}

#[test]
fn wiki_pinned_roles_are_reconciled_with_the_distinct_roles_being_checked() {
    let ctx = egui::Context::default();
    let mut sim = NetworkSim::new();
    sim.money = 100000;
    let SimEvent::DeviceAdded(controller) = sim.execute(Command::BuyServerFullPack).unwrap()[0]
    else {
        panic!()
    };
    sim.execute(Command::BuyServerFullPack).unwrap();
    let mut state = WikiWindowState {
        open: true,
        controller: Some(controller),
        node: Some(controller),
        ..Default::default()
    };
    let mut library = WikiLibrary::load(&crate::asset_root().join("wiki"));
    frame(&ctx, &sim, &mut state, &mut library, vec![]);
    let report = state.report.as_ref().unwrap();
    assert_eq!(report.controller, state.controller);
    assert_ne!(report.controller, report.node);
    assert!(state.node.is_none() || state.node == report.node);
}

#[test]
fn wiki_articles_do_not_expand_the_window_while_reading_or_navigating() {
    let ctx = egui::Context::default();
    let sim = NetworkSim::new();
    let mut state = WikiWindowState {
        open: true,
        ..Default::default()
    };
    let mut library = WikiLibrary::load(&crate::asset_root().join("wiki"));
    for _ in 0..4 {
        frame(&ctx, &sim, &mut state, &mut library, vec![]);
    }
    let before = super::super::test_windows::rect(&ctx, "wiki-window").size();
    for article in [
        "tutorial/first-vm",
        "tutorial/management-addresses",
        "tutorial/management-ping",
    ] {
        state.navigate(article);
        for _ in 0..35 {
            frame(&ctx, &sim, &mut state, &mut library, vec![]);
        }
        let after = super::super::test_windows::rect(&ctx, "wiki-window").size();
        assert!(
            (after - before).abs().max_elem() < 3.0,
            "{article}: window grew from {before:?} to {after:?}"
        );
    }
}
