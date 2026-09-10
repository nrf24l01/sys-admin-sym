use crate::app::{PersistenceRequest, SimSnapshot, UiState, WorkerRequest};
use crate::persistence::SaveStore;
use crate::plugins::simulation::SimulationWorker;
use bevy::prelude::*;

#[derive(Resource)]
struct PersistenceState(SaveStore);

pub struct PersistencePlugin;

impl Plugin for PersistencePlugin {
    fn build(&self, app: &mut App) {
        let path = std::env::current_dir()
            .unwrap_or_default()
            .join("cloud-provider-save.db");
        app.insert_resource(PersistenceState(SaveStore::new(path)))
            .add_systems(Update, handle_persistence_requests);
    }
}

fn handle_persistence_requests(
    mut requests: MessageReader<PersistenceRequest>,
    store: Res<PersistenceState>,
    snapshot: Res<SimSnapshot>,
    worker: Res<SimulationWorker>,
    mut ui: ResMut<UiState>,
) {
    for request in requests.read() {
        match request {
            PersistenceRequest::Save => match store.0.save(&snapshot.0) {
                Ok(()) => {
                    ui.notice = Some((format!("Saved to {}", store.0.path().display()), true))
                }
                Err(error) => ui.notice = Some((error.to_string(), false)),
            },
            PersistenceRequest::Load => match store.0.load() {
                Ok(sim) => {
                    let _ = worker.tx.send(WorkerRequest::Replace(Box::new(sim)));
                    ui.notice = Some(("Save loaded".into(), true));
                }
                Err(error) => ui.notice = Some((error.to_string(), false)),
            },
        }
    }
}
