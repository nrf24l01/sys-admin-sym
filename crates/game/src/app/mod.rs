mod messages;
mod shop;
mod state;

pub use messages::*;
pub use shop::*;
pub use state::*;

use crate::plugins::{PersistencePlugin, SimulationPlugin, UiPlugin};
use bevy::prelude::*;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (
                GameSet::Commands,
                GameSet::Simulation,
                GameSet::Presentation,
            )
                .chain(),
        )
        .add_plugins((SimulationPlugin, PersistencePlugin, UiPlugin));
    }
}

#[derive(SystemSet, Debug, Clone, Hash, PartialEq, Eq)]
pub enum GameSet {
    Commands,
    Simulation,
    Presentation,
}
