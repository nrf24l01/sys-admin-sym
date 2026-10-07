use crate::app::GameSet;
use crate::ui::{EquipmentImages, load_equipment_images, main_ui, prepare_equipment_textures};
use bevy::prelude::*;
use bevy_egui::{EguiContext, EguiInput, EguiPreUpdateSet, EguiPrimaryContextPass, egui};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EquipmentImages>()
            .add_systems(Startup, (setup_camera, load_equipment_images))
            .add_systems(
                PreUpdate,
                filter_unchanged_modifiers
                    .after(EguiPreUpdateSet::ProcessInput)
                    .before(EguiPreUpdateSet::BeginPass),
            )
            .add_systems(EguiPrimaryContextPass, main_ui)
            .add_systems(
                Update,
                (prepare_equipment_textures, clear_stale_notice).in_set(GameSet::Presentation),
            );
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn clear_stale_notice() {
    // Notices intentionally remain visible until the next action in the MVP.
}

// bevy_egui 0.42 emits ModifiersChanged every frame, including when no key
// changed. egui treats any event as a request to repaint, defeating reactive
// window updates. Preserve real transitions and every other input event.
fn filter_unchanged_modifiers(mut contexts: Query<(&mut EguiContext, &mut EguiInput)>) {
    for (mut context, mut input) in &mut contexts {
        let current = context.get_mut().input(|input| input.modifiers);
        retain_modifier_changes(&mut input.events, current);
    }
}

fn retain_modifier_changes(events: &mut Vec<egui::Event>, mut current: egui::Modifiers) {
    events.retain(|event| {
        let egui::Event::ModifiersChanged(modifiers) = event else {
            return true;
        };
        let changed = *modifiers != current;
        current = *modifiers;
        changed
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_modifier_state_does_not_keep_idle_ui_repainting() {
        let ctx = egui::Context::default();
        for frame in 0..5 {
            let mut input = egui::RawInput {
                time: Some(frame as f64),
                events: vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)],
                ..Default::default()
            };
            retain_modifier_changes(&mut input.events, ctx.input(|i| i.modifiers));
            let mut output = ctx.run_ui(input, |_| {});
            output.textures_delta.clear();
        }
        assert!(!ctx.has_requested_repaint());
    }

    #[test]
    fn modifier_press_release_and_pointer_events_are_preserved() {
        let ctx = egui::Context::default();
        let shift = egui::Modifiers {
            shift: true,
            ..Default::default()
        };
        for modifiers in [shift, egui::Modifiers::NONE] {
            let pointer = egui::Event::PointerMoved(egui::pos2(42.0, 17.0));
            let mut input = egui::RawInput {
                events: vec![
                    egui::Event::ModifiersChanged(modifiers),
                    pointer.clone(),
                    egui::Event::ModifiersChanged(modifiers),
                ],
                ..Default::default()
            };
            retain_modifier_changes(&mut input.events, ctx.input(|i| i.modifiers));
            assert_eq!(input.events.len(), 2);
            assert_eq!(input.events[1], pointer);
            let mut output = ctx.run_ui(input, |_| {});
            output.textures_delta.clear();
            assert_eq!(ctx.input(|i| i.modifiers), modifiers);
        }
    }
}
