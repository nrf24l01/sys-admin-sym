mod messages;
mod shop;
mod settings;
mod state;

pub use messages::*;
pub use shop::*;
pub use settings::*;
pub use state::*;

use crate::plugins::{PersistencePlugin, SettingsPlugin, SimulationPlugin, UiPlugin};
use bevy::prelude::*;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (
                GameSet::Commands,
                GameSet::Settings,
                GameSet::Simulation,
                GameSet::Presentation,
            )
                .chain(),
        )
        .add_plugins((SettingsPlugin, SimulationPlugin, PersistencePlugin, UiPlugin));
    }
}

#[derive(SystemSet, Debug, Clone, Hash, PartialEq, Eq)]
pub enum GameSet {
    Commands,
    Settings,
    Simulation,
    Presentation,
}
