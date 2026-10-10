use super::*;
use bevy::{
    ecs::system::SystemState,
    prelude::{Messages, World},
};

fn render(
    ctx: &egui::Context,
    sim: &NetworkSim,
    state: &mut BankWindowState,
    world: &mut World,
    system: &mut SystemState<MessageWriter<UiAction>>,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000.0, 900.0),
            )),
            events,
            ..Default::default()
        },
        |ui| show(ui, sim, state, &mut system.get_mut(world).unwrap()),
    );
    output.textures_delta.clear();
    output
}

fn click_button(
    label: &str,
    ctx: &egui::Context,
    sim: &NetworkSim,
    state: &mut BankWindowState,
    world: &mut World,
    system: &mut SystemState<MessageWriter<UiAction>>,
) {
    let mut position = None;
    let mut rendered = Vec::new();
    for _ in 0..2 {
        let output = render(ctx, sim, state, world, system, Vec::new());
        rendered = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    Some(text.galley.text().to_string())
                } else {
                    None
                }
            })
            .collect();
        position = output.shapes.iter().find_map(|shape| {
            if let egui::Shape::Text(text) = &shape.shape
                && text.galley.text() == label
            {
                Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center())
            } else {
                None
            }
        });
    }
    let pos = position
        .unwrap_or_else(|| panic!("bank action {label:?} is visible; rendered: {rendered:?}"));
    for pressed in [true, false] {
        render(
            ctx,
            sim,
            state,
            world,
            system,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

#[test]
fn bank_buttons_borrow_and_settle_through_domain_commands() {
    let mut sim = NetworkSim::new();
    let mut state = BankWindowState {
        open: true,
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let mut world = World::new();
    world.init_resource::<Messages<UiAction>>();
    let mut system = SystemState::<MessageWriter<UiAction>>::new(&mut world);
    click_button(
        "Borrow $5000",
        &ctx,
        &sim,
        &mut state,
        &mut world,
        &mut system,
    );
    let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
    assert_eq!(actions.len(), 1);
    let UiAction::NetworkCommand(command) = &actions[0] else {
        panic!()
    };
    sim.execute(command.clone()).unwrap();
    assert_eq!(sim.money, 11000);
    assert_eq!(sim.bank().active().unwrap().remaining(), 5500);
    click_button(
        "Repay all · $5500",
        &ctx,
        &sim,
        &mut state,
        &mut world,
        &mut system,
    );
    let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
    assert_eq!(actions.len(), 1);
    let UiAction::NetworkCommand(command) = &actions[0] else {
        panic!()
    };
    sim.execute(command.clone()).unwrap();
    assert!(sim.bank().active().is_none());
    assert_eq!(sim.money, 5500);
}

#[test]
fn unaffordable_early_repayment_is_disabled_and_overdue_debt_is_visible() {
    let mut sim = NetworkSim::new();
    sim.execute(Command::Bank(BankCommand::Borrow {
        amount: 5000,
        term_id: "standard".into(),
    }))
    .unwrap();
    sim.money = 0;
    sim.advance_time(300_000);
    let mut state = BankWindowState {
        open: true,
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let mut world = World::new();
    world.init_resource::<Messages<UiAction>>();
    let mut system = SystemState::<MessageWriter<UiAction>>::new(&mut world);
    click_button(
        "Repay all · $5500",
        &ctx,
        &sim,
        &mut state,
        &mut world,
        &mut system,
    );
    assert_eq!(
        world.resource_mut::<Messages<UiAction>>().drain().count(),
        0
    );
    let output = render(&ctx, &sim, &mut state, &mut world, &mut system, Vec::new());
    assert!(output.shapes.iter().any(|shape| {
        matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Overdue: $459")
    }));
    assert_eq!(summary(&sim), "Bank · overdue $459");
}
