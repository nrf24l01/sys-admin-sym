use crate::app::UiAction;
use crate::localization::tr;
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::{Command, NetworkSim, SimulationSpeed};

pub(super) fn show(ui: &mut egui::Ui, sim: &NetworkSim, actions: &mut MessageWriter<UiAction>) {
    ui.label(tr("ui.simulation-speed"));
    let mut selected = sim.simulation_speed();
    egui::ComboBox::from_id_salt("simulation-speed")
        .width(65.0)
        .selected_text(label(selected))
        .show_ui(ui, |ui| {
            for speed in SimulationSpeed::ALL {
                ui.selectable_value(&mut selected, speed, label(speed));
            }
        })
        .response
        .on_hover_text(tr("ui.simulation-speed-help"));
    if selected != sim.simulation_speed() {
        actions.write(UiAction::NetworkCommand(Command::SetSimulationSpeed(
            selected,
        )));
    }
}

fn label(speed: SimulationSpeed) -> String {
    format!("×{}", speed.multiplier())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::SystemState;
    use bevy::prelude::*;

    fn render(
        ctx: &egui::Context,
        world: &mut World,
        writer: &mut SystemState<MessageWriter<UiAction>>,
        sim: &NetworkSim,
        events: Vec<egui::Event>,
    ) -> Vec<(String, egui::Pos2)> {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(500.0, 500.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                ui.horizontal(|ui| show(ui, sim, &mut writer.get_mut(world).unwrap()));
            },
        );
        let text = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(t) => Some((
                    t.galley.text().to_string(),
                    egui::Rect::from_min_size(t.pos, t.galley.size()).center(),
                )),
                _ => None,
            })
            .collect();
        output.textures_delta.clear();
        text
    }

    fn click(
        ctx: &egui::Context,
        world: &mut World,
        writer: &mut SystemState<MessageWriter<UiAction>>,
        sim: &NetworkSim,
        text: &str,
    ) {
        let shapes = render(ctx, world, writer, sim, vec![]);
        let pos = shapes
            .iter()
            .rev()
            .find(|(label, _)| label == text)
            .unwrap_or_else(|| panic!("Missing {text}: {shapes:?}"))
            .1;
        for pressed in [true, false] {
            render(
                ctx,
                world,
                writer,
                sim,
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
    fn speed_selector_changes_the_authoritative_clock_for_every_preset() {
        let ctx = egui::Context::default();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut writer = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut sim = NetworkSim::new();
        for speed in SimulationSpeed::ALL
            .into_iter()
            .skip(1)
            .chain([SimulationSpeed::X1])
        {
            render(&ctx, &mut world, &mut writer, &sim, vec![]);
            click(
                &ctx,
                &mut world,
                &mut writer,
                &sim,
                &label(sim.simulation_speed()),
            );
            render(&ctx, &mut world, &mut writer, &sim, vec![]);
            click(&ctx, &mut world, &mut writer, &sim, &label(speed));
            let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
            let [UiAction::NetworkCommand(command)] = actions.as_slice() else {
                panic!("expected speed command")
            };
            sim.execute(command.clone()).unwrap();
            assert_eq!(sim.simulation_speed(), speed);
            let before = sim.simulation_time_ms();
            sim.advance_time(10);
            assert_eq!(sim.simulation_time_ms() - before, 10 * speed.multiplier());
        }
    }
}
