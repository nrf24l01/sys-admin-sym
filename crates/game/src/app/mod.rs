mod bank;
mod messages;
mod network;
mod settings;
mod shop;
mod state;

pub use bank::*;
pub use messages::*;
pub use network::*;
pub use settings::*;
pub use shop::*;
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
        .add_plugins((
            SettingsPlugin,
            SimulationPlugin,
            PersistencePlugin,
            UiPlugin,
        ));
    }
}

#[derive(SystemSet, Debug, Clone, Hash, PartialEq, Eq)]
pub enum GameSet {
    Commands,
    Settings,
    Simulation,
    Presentation,
}
