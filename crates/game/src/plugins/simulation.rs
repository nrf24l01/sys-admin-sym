use crate::app::*;
use bevy::prelude::*;
use cloud_provider_sim::{
    Command, Ipv4InterfaceConfig, NetworkSim, SwitchPortMode, Vlan, VlanId, parse_terminal_command,
};
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::net::Ipv4Addr;
use std::thread::{self, JoinHandle};

#[derive(Resource)]
pub struct SimulationWorker {
    pub tx: Sender<WorkerRequest>,
    pub rx: Receiver<WorkerResponse>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for SimulationWorker {
    fn drop(&mut self) {
        let _ = self.tx.send(WorkerRequest::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        let (request_tx, request_rx) = unbounded();
        let (response_tx, response_rx) = unbounded();
        let handle = thread::Builder::new()
            .name("network-simulation".into())
            .spawn(move || worker_loop(request_rx, response_tx))
            .expect("spawn network simulation thread");
        app.insert_resource(SimulationWorker {
            tx: request_tx,
            rx: response_rx,
            handle: Some(handle),
        })
        .init_resource::<SimSnapshot>()
        .init_resource::<UiState>()
        .init_resource::<EditorDrafts>()
        .add_message::<UiAction>()
        .add_message::<SimCommandMessage>()
        .add_message::<PersistenceRequest>()
        .add_systems(Update, translate_ui_actions.in_set(GameSet::Commands))
        .add_systems(
            Update,
            dispatch_sim_commands
                .in_set(GameSet::Commands)
                .after(translate_ui_actions),
        )
        .add_systems(Update, poll_worker.in_set(GameSet::Simulation));
    }
}

fn worker_loop(requests: Receiver<WorkerRequest>, responses: Sender<WorkerResponse>) {
    let mut sim = NetworkSim::new();
    let _ = responses.send(WorkerResponse::Snapshot(Box::new(sim.clone())));
    while let Ok(request) = requests.recv() {
        match request {
            WorkerRequest::Execute(command) => match sim.execute(command) {
                Ok(events) => {
                    let _ = responses.send(WorkerResponse::Events(events));
                    let _ = responses.send(WorkerResponse::Snapshot(Box::new(sim.clone())));
                }
                Err(error) => {
                    let _ = responses.send(WorkerResponse::Error(error.to_string()));
                }
            },
            WorkerRequest::Terminal { device, input } => match parse_terminal_command(&input) {
                Ok(command) => {
                    let _ = responses.send(WorkerResponse::Terminal(
                        sim.execute_terminal(device, command),
                    ));
                }
                Err(error) => {
                    let _ = responses.send(WorkerResponse::Error(error));
                }
            },
            WorkerRequest::Replace(mut replacement) => {
                replacement.rebuild_indexes();
                sim = *replacement;
                let _ = responses.send(WorkerResponse::Snapshot(Box::new(sim.clone())));
            }
            WorkerRequest::Stop => break,
        }
    }
}

fn translate_ui_actions(
    mut actions: MessageReader<UiAction>,
    mut commands: MessageWriter<SimCommandMessage>,
    mut persistence: MessageWriter<PersistenceRequest>,
    worker: Res<SimulationWorker>,
    snapshot: Res<SimSnapshot>,
    mut state: ResMut<UiState>,
    drafts: Res<EditorDrafts>,
) {
    for action in actions.read() {
        let command = match action {
            UiAction::SelectDevice(id) => {
                state.selected = Selection::Device(*id);
                None
            }
            UiAction::SelectPort(id) => {
                state.selected = Selection::Port(*id);
                None
            }
            UiAction::SelectLink(id) => {
                state.selected = Selection::Link(*id);
                None
            }
            UiAction::Buy(kind) => Some(Command::BuyDevice { kind: *kind }),
            UiAction::Place { device, rack, unit } => Some(Command::PlaceDevice {
                device: *device,
                rack: *rack,
                unit: *unit,
            }),
            UiAction::Remove(device) => Some(Command::RemoveDevice { device: *device }),
            UiAction::TogglePower(device, powered) => Some(Command::SetPower {
                device: *device,
                powered: *powered,
            }),
            UiAction::Disconnect(link) => Some(Command::Disconnect { link: *link }),
            UiAction::CreateVlan(device) => match state.new_vlan_id.parse::<u16>() {
                Ok(id) => Some(Command::CreateVlan {
                    switch: *device,
                    vlan: Vlan {
                        id: VlanId(id),
                        name: state.new_vlan_name.clone(),
                    },
                }),
                Err(_) => {
                    state.notice = Some(("Invalid VLAN ID".into(), false));
                    None
                }
            },
            UiAction::CablePort(port) => {
                if let Some(first) = state.pending_cable.take() {
                    if first != *port {
                        Some(Command::Connect { a: first, b: *port })
                    } else {
                        None
                    }
                } else {
                    state.pending_cable = Some(*port);
                    state.notice = Some(("Select the destination port".into(), true));
                    None
                }
            }
            UiAction::ApplyServer(port) => {
                match drafts.servers.get(port).and_then(parse_server_draft) {
                    Some((hostname, config)) => {
                        if let Some(owner) = snapshot.0.port(*port).map(|p| p.device) {
                            commands.write(SimCommandMessage(Command::SetHostname {
                                device: owner,
                                hostname,
                            }));
                        }
                        Some(Command::SetIpv4 {
                            port: *port,
                            config,
                        })
                    }
                    None => {
                        state.notice = Some(("Invalid server configuration".into(), false));
                        None
                    }
                }
            }
            UiAction::ApplySwitch(port) => drafts.switches.get(port).and_then(|draft| {
                if draft.trunk {
                    let allowed: Option<Vec<_>> = draft
                        .allowed
                        .split(',')
                        .filter(|v| !v.trim().is_empty())
                        .map(|v| v.trim().parse::<u16>().ok().map(VlanId))
                        .collect();
                    allowed.map(|allowed| Command::SetSwitchPortMode {
                        port: *port,
                        mode: SwitchPortMode::Trunk {
                            native_vlan: None,
                            allowed,
                        },
                    })
                } else {
                    draft
                        .vlan
                        .parse::<u16>()
                        .ok()
                        .map(|vlan| Command::SetSwitchPortMode {
                            port: *port,
                            mode: SwitchPortMode::Access { vlan: VlanId(vlan) },
                        })
                }
            }),
            UiAction::ApplyRouter(port) => drafts.routers.get(port).and_then(|draft| {
                let prefix = draft.prefix.parse().ok()?;
                let vlan = if draft.vlan.trim().is_empty() {
                    None
                } else {
                    Some(VlanId(draft.vlan.parse().ok()?))
                };
                let address = if draft.address.trim().is_empty() {
                    None
                } else {
                    Some(draft.address.parse().ok()?)
                };
                Some(Command::ConfigureRouterInterface {
                    port: *port,
                    name: draft.name.clone(),
                    vlan,
                    address,
                    prefix,
                    internet_connected: draft.internet,
                })
            }),
            UiAction::RunTerminal(device, input) => {
                let _ = worker.tx.send(WorkerRequest::Terminal {
                    device: *device,
                    input: input.clone(),
                });
                None
            }
            UiAction::Save => {
                persistence.write(PersistenceRequest::Save);
                None
            }
            UiAction::Load => {
                persistence.write(PersistenceRequest::Load);
                None
            }
            UiAction::NewGame => {
                let _ = worker
                    .tx
                    .send(WorkerRequest::Replace(Box::new(NetworkSim::new())));
                state.selected = Selection::None;
                None
            }
        };
        if let Some(command) = command {
            commands.write(SimCommandMessage(command));
        }
    }
}

fn parse_server_draft(draft: &ServerDraft) -> Option<(String, Ipv4InterfaceConfig)> {
    let address: Ipv4Addr = draft.address.parse().ok()?;
    let prefix = draft.prefix.parse().ok()?;
    let gateway = if draft.gateway.trim().is_empty() {
        None
    } else {
        Some(draft.gateway.parse().ok()?)
    };
    let vlan = VlanId(draft.vlan.parse().ok()?);
    Some((
        draft.hostname.clone(),
        Ipv4InterfaceConfig::new(address, prefix, gateway, vlan),
    ))
}

fn dispatch_sim_commands(
    mut messages: MessageReader<SimCommandMessage>,
    worker: Res<SimulationWorker>,
) {
    for message in messages.read() {
        let _ = worker.tx.send(WorkerRequest::Execute(message.0.clone()));
    }
}

fn poll_worker(
    worker: Res<SimulationWorker>,
    mut snapshot: ResMut<SimSnapshot>,
    mut state: ResMut<UiState>,
) {
    while let Ok(response) = worker.rx.try_recv() {
        match response {
            WorkerResponse::Snapshot(new_snapshot) => snapshot.0 = *new_snapshot,
            WorkerResponse::Events(events) => {
                state.notice = events.last().map(|event| (format!("{event:?}"), true))
            }
            WorkerResponse::Terminal(output) => state.terminal_lines = output.lines,
            WorkerResponse::Error(error) => state.notice = Some((error, false)),
        }
    }
}
