use crate::app::*;
use bevy::prelude::*;
use cloud_provider_sim::{Command, Ipv4InterfaceConfig, NetworkSim, SwitchPortMode, Vlan, VlanId};
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::net::Ipv4Addr;
use std::thread::{self, JoinHandle};
use std::time::Duration;

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
    let mut last_tick = std::time::Instant::now();
    if responses
        .send(WorkerResponse::Snapshot(Box::new(sim.clone())))
        .is_err()
    {
        return;
    }
    loop {
        let elapsed = last_tick.elapsed().as_millis().min(u64::MAX as u128) as u64;
        if elapsed > 0 {
            sim.advance_time(elapsed);
            last_tick = std::time::Instant::now();
        }
        let request = match requests.recv_timeout(Duration::from_millis(50)) {
            Ok(request) => request,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                if responses
                    .send(WorkerResponse::Snapshot(Box::new(sim.clone())))
                    .is_err()
                {
                    break;
                }
                continue;
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        };
        match request {
            WorkerRequest::Execute(command) => match sim.execute(command) {
                Ok(events) => {
                    if responses.send(WorkerResponse::Events(events)).is_err()
                        || responses
                            .send(WorkerResponse::Snapshot(Box::new(sim.clone())))
                            .is_err()
                    {
                        break;
                    }
                }
                Err(error) => {
                    let _ = responses.send(WorkerResponse::Error(error.to_string()));
                }
            },
            WorkerRequest::Terminal { device, input } => {
                for line in input
                    .lines()
                    .chain(if input.is_empty() { Some("") } else { None })
                {
                    let prompt = sim.terminal_prompt(device);
                    let output = sim.execute_console(device, line);
                    let success = output.success;
                    let _ = responses.send(WorkerResponse::Terminal {
                        device,
                        input: line.into(),
                        prompt,
                        output,
                    });
                    if !success {
                        break;
                    }
                }
                let _ = responses.send(WorkerResponse::Snapshot(Box::new(sim.clone())));
            }
            WorkerRequest::Replace(mut replacement) => {
                replacement.rebuild_indexes();
                sim = *replacement;
                let _ = responses.send(WorkerResponse::ConsolesReset);
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
            UiAction::BuyCableSupply(supply) => Some(Command::BuyCableSupply { supply: *supply }),
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
                    set_error(&mut state, "Invalid VLAN ID");
                    None
                }
            },
            UiAction::CablePort(port) => {
                if snapshot.0.link_for_port(*port).is_some() {
                    state.notice = Some((
                        "This port already has a cable. Disconnect it first.".into(),
                        false,
                    ));
                    continue;
                }
                if let Some(first) = state.pending_cable {
                    if first != *port {
                        let length_cm = state.cable_length_cm;
                        match snapshot.0.quote_colored_cable(
                            first,
                            *port,
                            length_cm,
                            state.cable_color,
                        ) {
                            Ok(quote) => {
                                let stock = snapshot.0.cable_inventory();
                                if stock.cable_cm < quote.cable_required()
                                    || stock.connectors < quote.connectors_required()
                                {
                                    set_error(&mut state, "Buy cable and RJ45 connectors in the shop before making this lead.");
                                    continue;
                                }
                                state.pending_cable = None;
                                Some(Command::ConnectColoredCable {
                                    a: first,
                                    b: *port,
                                    length_cm: if length_cm.is_none() {
                                        None
                                    } else {
                                        Some(quote.length_cm)
                                    },
                                    color: state.cable_color,
                                })
                            }
                            Err(error) => {
                                set_error(&mut state, error.to_string());
                                None
                            }
                        }
                    } else {
                        state.pending_cable = None;
                        None
                    }
                } else {
                    state.pending_cable = Some(*port);
                    state.notice = Some(("Select the destination port".into(), true));
                    None
                }
            }
            UiAction::ApplyServer(port) => match drafts.servers.get(port).map(parse_server_draft) {
                Some(Ok((hostname, config))) => {
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
                Some(Err(error)) => {
                    set_error(&mut state, error);
                    None
                }
                None => {
                    set_error(&mut state, "Missing server configuration");
                    None
                }
            },
            UiAction::ApplySwitch(port) => drafts.switches.get(port).and_then(|draft| {
                if draft.trunk {
                    let allowed: Option<Vec<_>> = draft
                        .allowed
                        .split(',')
                        .filter(|v| !v.trim().is_empty())
                        .map(|v| v.trim().parse::<u16>().ok().map(VlanId))
                        .collect();
                    if allowed.is_none() {
                        state.notice = Some((
                            "Invalid VLAN list: use comma-separated VLAN IDs.".into(),
                            false,
                        ));
                    }
                    allowed.map(|allowed| Command::SetSwitchPortMode {
                        port: *port,
                        mode: SwitchPortMode::Trunk {
                            native_vlan: None,
                            allowed,
                        },
                    })
                } else {
                    let result = if draft.vlan.trim().is_empty() {
                        Some(Command::SetSwitchPortMode {
                            port: *port,
                            mode: SwitchPortMode::Access { vlan: None },
                        })
                    } else {
                        draft.vlan.parse::<u16>().ok().map(|vlan| Command::SetSwitchPortMode {
                            port: *port,
                            mode: SwitchPortMode::Access { vlan: Some(VlanId(vlan)) },
                        })
                    };
                    if result.is_none() {
                        set_error(&mut state, "Invalid access VLAN: enter a VLAN ID or leave it blank.");
                    }
                    result
                }
            }),
            UiAction::ApplyRouter(port) => drafts.routers.get(port).and_then(|draft| {
                let prefix = match draft.prefix.parse() {
                    Ok(prefix) if prefix <= 32 => prefix,
                    Err(_) => {
                        state.notice = Some((
                            "Invalid router prefix: enter a number from 0 to 32.".into(),
                            false,
                        ));
                        return None;
                    }
                    _ => {
                        set_error(&mut state, "Invalid router prefix: enter a number from 0 to 32.");
                        return None;
                    }
                };
                let vlan = if draft.vlan.trim().is_empty() {
                    None
                } else {
                    match draft.vlan.parse::<u16>() {
                        Ok(vlan) if (1..=4094).contains(&vlan) => Some(VlanId(vlan)),
                        Err(_) => {
                            set_error(&mut state, "Invalid router VLAN: enter a VLAN ID from 1 to 4094 or leave it blank.");
                            return None;
                        }
                        _ => {
                            set_error(&mut state, "Invalid router VLAN: enter a VLAN ID from 1 to 4094 or leave it blank.");
                            return None;
                        }
                    }
                };
                let address = if draft.address.trim().is_empty() {
                    None
                } else {
                    match draft.address.parse::<Ipv4Addr>() {
                        Ok(address) if !address.is_unspecified() => Some(address),
                        Err(_) => {
                            set_error(&mut state, "Invalid router IPv4 address: enter a host address.");
                            return None;
                        }
                        _ => {
                            set_error(&mut state, "Invalid router IPv4 address: enter a host address.");
                            return None;
                        }
                    }
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
            UiAction::FlushPortConfig(port) => Some(Command::ResetPortConfig { port: *port }),
            UiAction::AddCableRoutePoint { link, point } => Some(Command::AddCableRoutePoint { link: *link, point: *point }),
            UiAction::RemoveCableRoutePoint { link, index } => Some(Command::RemoveCableRoutePoint { link: *link, index: *index }),
            UiAction::MoveCableRoutePoint { link, index, point } => Some(Command::MoveCableRoutePoint { link: *link, index: *index, point: *point }),
            UiAction::RerouteCable { link, route } => Some(Command::RerouteCable { link: *link, route: route.clone() }),
            UiAction::LaunchExternalTerminal(device) => {
                state.terminal_windows.insert(*device);
                state.terminal_window_focus.insert(*device);
                state.notice = Some(("Terminal window opened".into(), true));
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

fn set_error(state: &mut UiState, message: impl Into<String>) {
    let message = message.into();
    state.error_dialog = Some(message.clone());
    state.notice = Some((message, false));
}

fn parse_server_draft(draft: &ServerDraft) -> Result<(String, Ipv4InterfaceConfig), &'static str> {
    let address: Ipv4Addr = draft
        .address
        .parse()
        .map_err(|_| "Invalid server IPv4 address: enter a host address.")?;
    if address.is_unspecified() {
        return Err("Invalid server IPv4 address: 0.0.0.0 is not a host address.");
    }
    let prefix: u8 = draft
        .prefix
        .parse()
        .map_err(|_| "Invalid server prefix: enter a number from 0 to 32.")?;
    if prefix > 32 {
        return Err("Invalid server prefix: enter a number from 0 to 32.");
    }
    let gateway = if draft.gateway.trim().is_empty() {
        None
    } else {
        Some(
            draft
                .gateway
                .parse()
                .map_err(|_| "Invalid server gateway: enter an IPv4 address or leave it blank.")?,
        )
    };
    let vlan = if draft.vlan.trim().is_empty() {
        None
    } else {
        Some(VlanId(draft.vlan.parse().map_err(
            |_| "Invalid access VLAN: enter a VLAN ID from 1 to 4094 or leave it blank.",
        )?))
    };
    if vlan.is_some_and(|v| !(1..=4094).contains(&v.0)) {
        return Err("Invalid access VLAN: enter a VLAN ID from 1 to 4094 or leave it blank.");
    }
    let mut config = Ipv4InterfaceConfig::new(address, prefix, gateway, vlan.unwrap_or(VlanId(1)));
    config.vlan = vlan;
    Ok((draft.hostname.clone(), config))
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
    mut drafts: ResMut<EditorDrafts>,
) {
    while let Ok(response) = worker.rx.try_recv() {
        match response {
            WorkerResponse::Snapshot(new_snapshot) => {
                snapshot.0 = *new_snapshot;
                let selection_gone = match state.selected {
                    Selection::Port(port) => snapshot.0.port(port).is_none(),
                    Selection::Link(link) => snapshot.0.link(link).is_none(),
                    _ => false,
                };
                if selection_gone {
                    state.selected = Selection::None;
                }
                drafts
                    .servers
                    .retain(|port, _| snapshot.0.port(*port).is_some());
                drafts
                    .switches
                    .retain(|port, _| snapshot.0.port(*port).is_some());
                drafts
                    .routers
                    .retain(|port, _| snapshot.0.port(*port).is_some());
            }
            WorkerResponse::Events(events) => {
                state.notice = events.first().map(|event| {
                    let message = match event {
                        cloud_provider_sim::SimEvent::CableSuppliesPurchased(_) => {
                            "Cable supplies purchased".into()
                        }
                        cloud_provider_sim::SimEvent::LinkCreated(_) => {
                            "RJ45 lead connected".into()
                        }
                        cloud_provider_sim::SimEvent::LinkRemoved(_) => {
                            "Lead unplugged and returned to cable inventory".into()
                        }
                        _ => format!("{event:?}"),
                    };
                    (message, true)
                });
                for event in &events {
                    if matches!(
                        event,
                        cloud_provider_sim::SimEvent::DeviceMoved { .. }
                            | cloud_provider_sim::SimEvent::DeviceRemoved(_)
                    ) {
                        state.pending_cable = None;
                    }
                }
            }
            WorkerResponse::Terminal {
                device,
                input,
                prompt,
                output,
            } => {
                if output.success {
                    *drafts = EditorDrafts::default();
                }
                let console = state.terminals.entry(device).or_default();
                console.lines.push(format!("{prompt} {input}"));
                console.lines.extend(output.lines);
                if console.lines.len() > 1000 {
                    console.lines.drain(..console.lines.len() - 1000);
                }
            }
            WorkerResponse::ConsolesReset => {
                state.terminals.clear();
                state.pending_cable = None;
                state.selected = Selection::None;
            }
            WorkerResponse::Error(error) => set_error(&mut state, error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloud_provider_sim::{DeviceTemplate, SimEvent};

    #[test]
    fn console_worker_keeps_device_identity_and_stops_scripts_on_error() {
        let mut sim = NetworkSim::new();
        let mut devices = vec![];
        for kind in [DeviceTemplate::Switch, DeviceTemplate::Router] {
            let device = match sim.execute(Command::BuyDevice { kind }).unwrap()[0] {
                SimEvent::DeviceAdded(id) => id,
                _ => unreachable!(),
            };
            sim.execute(Command::SetPower {
                device,
                powered: true,
            })
            .unwrap();
            devices.push(device);
        }
        let (requests_tx, requests_rx) = unbounded();
        let (responses_tx, responses_rx) = unbounded();
        requests_tx
            .send(WorkerRequest::Replace(Box::new(sim)))
            .unwrap();
        requests_tx.send(WorkerRequest::Terminal {
            device: devices[0],
            input: "enable\nconfigure terminal\nhostname Core\ninvalid command\nhostname ShouldNotRun".into(),
        }).unwrap();
        requests_tx
            .send(WorkerRequest::Terminal {
                device: devices[1],
                input: "show version".into(),
            })
            .unwrap();
        requests_tx.send(WorkerRequest::Stop).unwrap();
        worker_loop(requests_rx, responses_tx);
        let mut consoles = vec![];
        let mut snapshot = None;
        let mut resets = 0;
        for response in responses_rx.try_iter() {
            match response {
                WorkerResponse::Terminal {
                    device,
                    prompt,
                    output,
                    ..
                } => consoles.push((device, prompt, output.success)),
                WorkerResponse::Snapshot(sim) => snapshot = Some(sim),
                WorkerResponse::ConsolesReset => resets += 1,
                _ => {}
            }
        }
        assert_eq!(resets, 1);
        assert_eq!(consoles.len(), 5);
        assert_eq!(consoles[0], (devices[0], "Switch>".into(), true));
        assert_eq!(consoles[3], (devices[0], "Core(config)#".into(), false));
        assert_eq!(consoles[4], (devices[1], "Router>".into(), true));
        let snapshot = snapshot.unwrap();
        assert_eq!(snapshot.terminal_prompt(devices[0]), "Core(config)#");
        assert_eq!(snapshot.terminal_prompt(devices[1]), "Router>");
    }
}
