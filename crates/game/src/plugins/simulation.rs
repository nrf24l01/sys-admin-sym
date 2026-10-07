use crate::app::*;
use crate::console::{ConsoleService, LocalConsoleServer};
use crate::settings::{ConsoleSettings, GameSettings};
use bevy::prelude::*;
use cloud_provider_sim::{
    Command, DeviceKind, Ipv4InterfaceConfig, NetworkSim, RemoteRequest, RemoteResponse,
    SwitchPortMode, TerminalOutput, Vlan, VlanId,
};
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::net::Ipv4Addr;
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Resource)]
pub struct SimulationWorker {
    pub tx: Sender<WorkerRequest>,
    pub rx: Receiver<WorkerResponse>,
    handle: Option<JoinHandle<()>>,
    remote: LocalConsoleServer,
}

impl SimulationWorker {
    pub fn configure_console(&mut self, config: ConsoleSettings) -> Result<(), String> {
        self.remote.reconfigure(config, self.tx.clone())
    }

    pub fn console_error(&self) -> Option<&str> {
        self.remote.error()
    }
}

impl Drop for SimulationWorker {
    fn drop(&mut self) {
        self.remote.stop();
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
        let console = app.world().resource::<GameSettings>().console.clone();
        let remote = LocalConsoleServer::start(console, request_tx.clone());
        app.world_mut().resource_mut::<GameSettings>().console_error =
            remote.error().map(str::to_owned);
        app.insert_resource(SimulationWorker {
            tx: request_tx,
            rx: response_rx,
            handle: Some(handle),
            remote,
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
                    let _ = responses.send(WorkerResponse::Error(error));
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
            WorkerRequest::Remote { request, reply } => {
                let console = match &request {
                    RemoteRequest::Run { device, input } => {
                        Some((*device, input.clone(), sim.terminal_prompt(*device)))
                    }
                    _ => None,
                };
                let response = ConsoleService::new(&mut sim).handle(request);
                if let (
                    Some((device, input, prompt)),
                    RemoteResponse::Output { lines, success, .. },
                ) = (&console, &response)
                {
                    let _ = responses.send(WorkerResponse::Terminal {
                        device: *device,
                        input: input.clone(),
                        prompt: prompt.clone(),
                        output: TerminalOutput {
                            lines: lines.clone(),
                            success: *success,
                        },
                    });
                }
                let _ = reply.send(response);
                if console.is_some() {
                    let _ = responses.send(WorkerResponse::Snapshot(Box::new(sim.clone())));
                }
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
            UiAction::SelectPowerCable(outlet) => {
                state.pending_cable = None;
                state.pending_assembly = None;
                state.pending_cable_route.clear();
                state.pending_power_outlet = None;
                state.pending_power_inlet = None;
                state.pending_power_route.clear();
                state.selected = Selection::PowerCable(*outlet);
                None
            }
            UiAction::Buy(kind) => Some(Command::BuyDevice { kind: *kind }),
            UiAction::BuyServerChassis => Some(Command::BuyServerChassis),
            UiAction::BuyServerFullPack => Some(Command::BuyServerFullPack),
            UiAction::BuyPublicIpv4Pool => Some(Command::BuyPublicIpv4Pool),
            UiAction::AssignPublicIpv4 { port, network } => Some(Command::AssignPublicIpv4 {
                port: *port,
                network: *network,
            }),
            UiAction::AssignLanIpv4 { port } => Some(Command::AssignLanIpv4 { port: *port }),
            UiAction::BuyServerPart(part_id) => Some(Command::BuyServerPart {
                part_id: part_id.clone(),
            }),
            UiAction::BuyDrive(drive_id) => Some(Command::BuyDrive {
                drive_id: drive_id.clone(),
            }),
            UiAction::InstallDrive {
                device,
                drive_id,
                bay,
            } => Some(Command::InstallDrive {
                device: *device,
                drive_id: drive_id.clone(),
                bay: *bay,
            }),
            UiAction::RemoveDrive { device, bay } => Some(Command::RemoveDrive {
                device: *device,
                bay: *bay,
            }),
            UiAction::InstallServerPart {
                device,
                part_id,
                slot,
            } => Some(Command::InstallServerPart {
                device: *device,
                part_id: part_id.clone(),
                slot: *slot,
            }),
            UiAction::RemoveServerPart {
                device,
                part_id,
                slot,
            } => Some(Command::RemoveServerPart {
                device: *device,
                part_id: part_id.clone(),
                slot: *slot,
            }),
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
            UiAction::PowerSocket(socket) => power_socket_action(&snapshot.0, &mut state, *socket),
            UiAction::AddPendingPowerRoutePoint(point) => {
                if let Some(index) = state
                    .pending_power_route
                    .iter()
                    .position(|candidate| candidate == point)
                {
                    state.pending_power_route.remove(index);
                } else {
                    state.pending_power_route.push(*point);
                }
                None
            }
            UiAction::DisconnectPower(outlet) => Some(Command::DisconnectPower { outlet: *outlet }),
            UiAction::ReroutePowerCable { outlet, route } => Some(Command::ReroutePowerCable {
                outlet: *outlet,
                route: route.clone(),
            }),
            UiAction::ResetPower(source) => Some(Command::ResetPowerBreaker { source: *source }),
            UiAction::RackMains(rack, on) => Some(Command::SetRackMains {
                rack: *rack,
                on: *on,
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
                    set_error(&mut state, "ui.invalid-vlan-id");
                    None
                }
            },
            UiAction::StartAssembly { port, assembly } => {
                begin_ethernet_gesture(&mut state);
                state.pending_cable = Some(*port);
                state.pending_assembly = Some(*assembly);
                state.pending_cable_route.clear();
                state.notice = Some(("optics.choose-destination".into(), true));
                None
            }
            UiAction::CablePort(port) => {
                if let Some(assembly) = state.pending_assembly {
                    if let Some(first) = state.pending_cable
                        && first != *port
                    {
                        commands.write(SimCommandMessage(Command::Optics(
                            cloud_provider_sim::OpticsCommand::ConnectAssembly {
                                assembly,
                                a: first,
                                b: *port,
                                route: state.pending_cable_route.clone(),
                            },
                        )));
                        state.pending_cable = None;
                        state.pending_assembly = None;
                        state.pending_cable_route.clear();
                    }
                    continue;
                }
                if !snapshot.0.port_is_copper(*port) {
                    state.selected = Selection::Port(*port);
                    continue;
                }
                begin_ethernet_gesture(&mut state);
                if snapshot.0.link_for_port(*port).is_some() {
                    state.notice =
                        Some(("ui.this-port-already-has-a-cable-disconnect".into(), false));
                    continue;
                }
                if let Some(first) = state.pending_cable {
                    if first != *port {
                        let length_cm = state.cable_length_cm;
                        match snapshot.0.quote_routed_colored_cable(
                            first,
                            *port,
                            length_cm,
                            state.cable_color,
                            &state.pending_cable_route,
                        ) {
                            Ok(quote) => {
                                let stock = snapshot.0.cable_inventory();
                                if stock.cable_cm < quote.cable_required()
                                    || stock.connectors < quote.connectors_required()
                                {
                                    set_error(
                                        &mut state,
                                        "ui.buy-cable-and-rj45-connectors-in-the",
                                    );
                                    continue;
                                }
                                let route = std::mem::take(&mut state.pending_cable_route);
                                state.pending_cable = None;
                                state.pending_assembly = None;
                                Some(Command::ConnectRoutedColoredCable {
                                    a: first,
                                    b: *port,
                                    length_cm: if length_cm.is_none() {
                                        None
                                    } else {
                                        Some(quote.length_cm)
                                    },
                                    color: state.cable_color,
                                    route,
                                })
                            }
                            Err(error) => {
                                set_error(&mut state, error);
                                None
                            }
                        }
                    } else {
                        state.pending_cable = None;
                        state.pending_assembly = None;
                        state.pending_cable_route.clear();
                        None
                    }
                } else {
                    state.pending_cable = Some(*port);
                    state.pending_cable_route.clear();
                    state.notice =
                        Some(("ui.select-anchors-then-the-destination-port".into(), true));
                    None
                }
            }
            UiAction::AddPendingCableRoutePoint(point) => {
                if state.pending_cable.is_some() {
                    if let Some(index) = state
                        .pending_cable_route
                        .iter()
                        .position(|candidate| candidate == point)
                    {
                        state.pending_cable_route.remove(index);
                    } else {
                        state.pending_cable_route.push(*point);
                    }
                    state.notice =
                        Some(("ui.route-updated-select-another-anchor-or-the".into(), true));
                }
                None
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
                    set_error(&mut state, "ui.missing-server-configuration");
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
                            "ui.invalid-vlan-list-use-comma-separated-vlan".into(),
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
                        draft
                            .vlan
                            .parse::<u16>()
                            .ok()
                            .map(|vlan| Command::SetSwitchPortMode {
                                port: *port,
                                mode: SwitchPortMode::Access {
                                    vlan: Some(VlanId(vlan)),
                                },
                            })
                    };
                    if result.is_none() {
                        set_error(&mut state, "ui.invalid-access-vlan-enter-a-vlan-id.2");
                    }
                    result
                }
            }),
            UiAction::ApplyRouter(port) => drafts.routers.get(port).and_then(|draft| {
                let prefix = match draft.prefix.parse() {
                    Ok(prefix) if prefix <= 32 => prefix,
                    Err(_) => {
                        state.notice =
                            Some(("ui.invalid-router-prefix-enter-a-number-from".into(), false));
                        return None;
                    }
                    _ => {
                        set_error(&mut state, "ui.invalid-router-prefix-enter-a-number-from");
                        return None;
                    }
                };
                let vlan = if draft.vlan.trim().is_empty() {
                    None
                } else {
                    match draft.vlan.parse::<u16>() {
                        Ok(vlan) if (1..=4094).contains(&vlan) => Some(VlanId(vlan)),
                        Err(_) => {
                            set_error(&mut state, "ui.invalid-router-vlan-enter-a-vlan-id");
                            return None;
                        }
                        _ => {
                            set_error(&mut state, "ui.invalid-router-vlan-enter-a-vlan-id");
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
                            set_error(&mut state, "ui.invalid-router-ipv4-address-enter-a-host");
                            return None;
                        }
                        _ => {
                            set_error(&mut state, "ui.invalid-router-ipv4-address-enter-a-host");
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
            UiAction::OpenIpRanges => {
                state.ranges_open = true;
                None
            }
            UiAction::OpenRouting(device) => {
                state.routing_device = Some(*device);
                None
            }
            UiAction::NetworkCommand(command) => Some(command.clone()),
            UiAction::RunTerminal(device, input) => {
                let _ = worker.tx.send(WorkerRequest::Terminal {
                    device: *device,
                    input: input.clone(),
                });
                None
            }
            UiAction::FlushPortConfig(port) => Some(Command::ResetPortConfig { port: *port }),
            UiAction::RemoveCableRoutePoint { link, index } => {
                Some(Command::RemoveCableRoutePoint {
                    link: *link,
                    index: *index,
                })
            }
            UiAction::MoveCableRoutePoint { link, index, point } => {
                Some(Command::MoveCableRoutePoint {
                    link: *link,
                    index: *index,
                    point: *point,
                })
            }
            UiAction::RerouteCable { link, route } => Some(Command::RerouteCable {
                link: *link,
                route: route.clone(),
            }),
            UiAction::LaunchExternalTerminal(device) => {
                if snapshot.0.device(*device).is_some_and(|d| {
                    matches!(
                        d.kind,
                        DeviceKind::Server(_) | DeviceKind::Switch(_) | DeviceKind::Router(_)
                    )
                }) {
                    state.terminal_windows.insert(*device);
                    state.terminal_window_focus.insert(*device);
                    state.notice = Some(("ui.terminal-window-opened".into(), true));
                }
                None
            }
            UiAction::Save => {
                persistence.write(PersistenceRequest::Save);
                None
            }
            UiAction::ApplyConsoleSettings(_) | UiAction::SelectLanguage(_) => None,
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

fn set_error(state: &mut UiState, message: impl Into<crate::localization::UiMessage>) {
    let message = message.into();
    state.error_dialog = Some(message.clone());
    state.notice = Some((message, false));
}

fn begin_ethernet_gesture(state: &mut UiState) {
    state.pending_power_outlet = None;
    state.pending_power_inlet = None;
    state.pending_power_route.clear();
}

/// Applies one power-socket click to the local gesture state.  The returned
/// command is only emitted after the cloned simulation accepts it.
fn power_socket_action(
    snapshot: &NetworkSim,
    state: &mut UiState,
    socket: PowerSocket,
) -> Option<Command> {
    state.pending_cable = None;
    state.pending_assembly = None;
    state.pending_cable_route.clear();
    match socket {
        PowerSocket::Outlet(outlet) => {
            if state.pending_power_outlet == Some(outlet) {
                state.pending_power_outlet = None;
                state.pending_power_route.clear();
                state.notice = Some(("ui.power-cable-selection-cancelled".into(), true));
            } else if snapshot.power.connections.contains_key(&outlet) {
                state.pending_power_outlet = None;
                state.pending_power_inlet = None;
                state.pending_power_route.clear();
                state.selected = Selection::PowerCable(outlet);
                state.notice = Some(("ui.power-cable-selected".into(), true));
            } else if state.pending_power_inlet.is_none() && state.pending_power_outlet.is_some() {
                set_error(state, "ui.select-a-power-inlet-to-finish-the.2");
            } else if let Some(endpoint) = state.pending_power_inlet.take() {
                state.pending_power_outlet = None;
                let mut route = std::mem::take(&mut state.pending_power_route);
                // Gestures run inlet -> outlet; stored cords run outlet -> inlet.
                route.reverse();
                let command = Command::ConnectPowerRouted {
                    outlet,
                    endpoint,
                    route,
                };
                let mut check = snapshot.clone();
                match check.execute(command.clone()) {
                    Ok(_) => return Some(command),
                    Err(error) => {
                        state.pending_power_inlet = Some(endpoint);
                        if let Command::ConnectPowerRouted { mut route, .. } = command {
                            route.reverse();
                            state.pending_power_route = route;
                        }
                        set_error(state, error);
                    }
                }
            } else {
                state.pending_power_outlet = Some(outlet);
                state.notice = Some(("ui.select-a-power-inlet-to-finish-the".into(), true));
            }
        }
        PowerSocket::Inlet(endpoint) => {
            if state.pending_power_inlet == Some(endpoint) {
                state.pending_power_inlet = None;
                state.pending_power_route.clear();
                state.notice = Some(("ui.power-cable-selection-cancelled".into(), true));
            } else if let Some(outlet) = snapshot
                .power
                .connections
                .iter()
                .find_map(|(outlet, target)| (*target == endpoint).then_some(*outlet))
            {
                state.pending_power_outlet = None;
                state.pending_power_inlet = None;
                state.pending_power_route.clear();
                state.selected = Selection::PowerCable(outlet);
                state.notice = Some(("ui.power-cable-selected".into(), true));
            } else if state.pending_power_outlet.is_none() && state.pending_power_inlet.is_some() {
                set_error(state, "ui.select-a-power-outlet-to-finish-the");
            } else if let Some(outlet) = state.pending_power_outlet.take() {
                state.pending_power_inlet = None;
                let command = Command::ConnectPowerRouted {
                    outlet,
                    endpoint,
                    route: std::mem::take(&mut state.pending_power_route),
                };
                let mut check = snapshot.clone();
                match check.execute(command.clone()) {
                    Ok(_) => return Some(command),
                    Err(error) => {
                        state.pending_power_outlet = Some(outlet);
                        if let Command::ConnectPowerRouted { route, .. } = command {
                            state.pending_power_route = route;
                        }
                        set_error(state, error);
                    }
                }
            } else {
                state.pending_power_inlet = Some(endpoint);
                state.notice = Some(("ui.select-a-free-power-outlet-to-finish".into(), true));
            }
        }
    }
    None
}

fn parse_server_draft(draft: &ServerDraft) -> Result<(String, Ipv4InterfaceConfig), &'static str> {
    let address: Ipv4Addr = draft
        .address
        .parse()
        .map_err(|_| "ui.invalid-server-ipv4-address-enter-a-host")?;
    if address.is_unspecified() {
        return Err("ui.invalid-server-ipv4-address-0-0-0");
    }
    let prefix: u8 = draft
        .prefix
        .parse()
        .map_err(|_| "ui.invalid-server-prefix-enter-a-number-from")?;
    if prefix > 32 {
        return Err("ui.invalid-server-prefix-enter-a-number-from");
    }
    let gateway = if draft.gateway.trim().is_empty() {
        None
    } else {
        Some(
            draft
                .gateway
                .parse()
                .map_err(|_| "ui.invalid-server-gateway-enter-an-ipv4-address")?,
        )
    };
    let vlan = if draft.vlan.trim().is_empty() {
        None
    } else {
        Some(VlanId(
            draft
                .vlan
                .parse()
                .map_err(|_| "ui.invalid-access-vlan-enter-a-vlan-id")?,
        ))
    };
    if vlan.is_some_and(|v| !(1..=4094).contains(&v.0)) {
        return Err("ui.invalid-access-vlan-enter-a-vlan-id");
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
                    Selection::PowerCable(outlet) => {
                        !snapshot.0.power.connections.contains_key(&outlet)
                    }
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
                drafts
                    .routes
                    .retain(|id, _| snapshot.0.device(*id).is_some());
            }
            WorkerResponse::Events(events) => {
                state.notice = events.first().map(|event| {
                    let message = match event {
                        cloud_provider_sim::SimEvent::CableSuppliesPurchased(_) => {
                            "ui.cable-supplies-purchased".into()
                        }
                        cloud_provider_sim::SimEvent::LinkCreated(_) => {
                            "ui.rj45-lead-connected".into()
                        }
                        cloud_provider_sim::SimEvent::LinkRemoved(_) => {
                            "ui.lead-unplugged-and-returned-to-cable-inventory".into()
                        }
                        cloud_provider_sim::SimEvent::RouterRoutesChanged(_) => {
                            "ui.router-routing-table-updated".into()
                        }
                        cloud_provider_sim::SimEvent::DeviceAdded(_) => "ui.device-added".into(),
                        cloud_provider_sim::SimEvent::DeviceMoved { .. } => {
                            "ui.device-placed".into()
                        }
                        cloud_provider_sim::SimEvent::DeviceRemoved(_) => {
                            "ui.device-removed".into()
                        }
                        cloud_provider_sim::SimEvent::DevicePowerChanged { .. } => {
                            "ui.device-power-changed".into()
                        }
                        cloud_provider_sim::SimEvent::PortConfigChanged(_) => {
                            "ui.interface-configuration-updated".into()
                        }
                        cloud_provider_sim::SimEvent::TopologyChanged { .. } => {
                            "ui.topology-updated".into()
                        }
                        cloud_provider_sim::SimEvent::ConnectivityChanged => {
                            "ui.connectivity-updated".into()
                        }
                        cloud_provider_sim::SimEvent::PowerChanged => {
                            "ui.power-configuration-updated".into()
                        }
                    };
                    (message, true)
                });
                for event in &events {
                    if let cloud_provider_sim::SimEvent::RouterRoutesChanged(id) = event {
                        drafts.routes.remove(id);
                    }
                    if matches!(event, cloud_provider_sim::SimEvent::PowerChanged) {
                        state.pending_power_outlet = None;
                        state.pending_power_inlet = None;
                        state.pending_power_route.clear();
                    }
                    if matches!(
                        event,
                        cloud_provider_sim::SimEvent::DeviceMoved { .. }
                            | cloud_provider_sim::SimEvent::DeviceRemoved(_)
                    ) {
                        state.pending_cable = None;
                        state.pending_assembly = None;
                        state.pending_cable_route.clear();
                        state.pending_power_outlet = None;
                        state.pending_power_inlet = None;
                        state.pending_power_route.clear();
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
                for line in output.lines {
                    if line == "\u{1b}[2J\u{1b}[H" {
                        console.lines.clear();
                    } else {
                        console.lines.push(line);
                    }
                }
                if console.lines.len() > 1000 {
                    console.lines.drain(..console.lines.len() - 1000);
                }
            }
            WorkerResponse::ConsolesReset => {
                state.network_summary = NetworkSummaryCache::default();
                state.routing_device = None;
                state.selected_range = None;
                state.range_loaded_for = None;
                state.range_uplink = None;
                state.terminals.clear();
                state.pending_cable = None;
                state.pending_assembly = None;
                state.pending_cable_route.clear();
                state.pending_power_outlet = None;
                state.pending_power_inlet = None;
                state.selected = Selection::None;
            }
            WorkerResponse::Error(error) => set_error(&mut state, error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloud_provider_sim::{
        Command, DeviceTemplate, OutletId, PowerEndpoint, RackId, SimEvent, SourceId,
    };

    fn outlet() -> OutletId {
        OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 0,
        }
    }

    fn device_sim() -> (NetworkSim, cloud_provider_sim::DeviceId) {
        let mut sim = NetworkSim::new();
        let device = match sim
            .execute(Command::BuyDevice {
                kind: DeviceTemplate::Router,
            })
            .unwrap()[0]
        {
            SimEvent::DeviceAdded(id) => id,
            _ => unreachable!(),
        };
        sim.execute(Command::PlaceDevice {
            device,
            rack: RackId(1),
            unit: 1,
        })
        .unwrap();
        (sim, device)
    }

    #[test]
    fn power_socket_pairs_in_both_directions_and_sim_infers_cisco_cord() {
        let (sim, device) = device_sim();
        let endpoint = PowerEndpoint::Device(device);
        let mut state = UiState::default();
        assert!(power_socket_action(&sim, &mut state, PowerSocket::Outlet(outlet())).is_none());
        assert_eq!(state.pending_power_outlet, Some(outlet()));
        let command = power_socket_action(&sim, &mut state, PowerSocket::Inlet(endpoint)).unwrap();
        let mut connected = sim.clone();
        connected.execute(command).unwrap();
        assert_eq!(
            connected.power.cord_kind(outlet()),
            cloud_provider_sim::PowerCordKind::Cisco66WAdapter
        );

        let mut reverse = UiState::default();
        assert!(power_socket_action(&sim, &mut reverse, PowerSocket::Inlet(endpoint)).is_none());
        assert!(power_socket_action(&sim, &mut reverse, PowerSocket::Outlet(outlet())).is_some());
    }

    #[test]
    fn pending_power_anchor_route_is_committed_with_the_connection() {
        let (sim, device) = device_sim();
        let endpoint = PowerEndpoint::Device(device);
        let anchor = cloud_provider_sim::CableRoutePoint {
            rack: RackId(1),
            unit: 2,
            side: cloud_provider_sim::RackSide::Rear,
            offset_cm: 24,
        };
        let mut state = UiState::default();
        power_socket_action(&sim, &mut state, PowerSocket::Outlet(outlet()));
        state.pending_power_route.push(anchor);
        let command = power_socket_action(&sim, &mut state, PowerSocket::Inlet(endpoint)).unwrap();
        assert!(state.pending_power_route.is_empty());
        let mut connected = sim.clone();
        connected.execute(command).unwrap();
        assert_eq!(connected.power.cord_routes[&outlet()], vec![anchor]);
    }

    #[test]
    fn inlet_first_power_route_preserves_the_gesture_order() {
        let (sim, device) = device_sim();
        let endpoint = PowerEndpoint::Device(device);
        let first = cloud_provider_sim::CableRoutePoint {
            rack: RackId(1),
            unit: 3,
            side: cloud_provider_sim::RackSide::Front,
            offset_cm: 48,
        };
        let last = cloud_provider_sim::CableRoutePoint {
            unit: 9,
            side: cloud_provider_sim::RackSide::Rear,
            ..first
        };
        let mut state = UiState::default();
        power_socket_action(&sim, &mut state, PowerSocket::Inlet(endpoint));
        state.pending_power_route = vec![first, last];
        let command = power_socket_action(&sim, &mut state, PowerSocket::Outlet(outlet())).unwrap();
        let mut connected = sim.clone();
        connected.execute(command).unwrap();
        assert_eq!(connected.power.cord_routes[&outlet()], vec![last, first]);
        let invalid = PowerEndpoint::Source(SourceId::Ups(9999));
        power_socket_action(&sim, &mut state, PowerSocket::Inlet(invalid));
        state.pending_power_route = vec![first, last];
        assert!(power_socket_action(&sim, &mut state, PowerSocket::Outlet(outlet())).is_none());
        assert_eq!(state.pending_power_route, vec![first, last]);
    }

    #[test]
    fn power_socket_same_role_and_invalid_connection_preserves_pending_selection() {
        let sim = NetworkSim::new();
        let first = outlet();
        let second = OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 1,
        };
        let mut state = UiState::default();
        power_socket_action(&sim, &mut state, PowerSocket::Outlet(first));
        assert!(power_socket_action(&sim, &mut state, PowerSocket::Outlet(second)).is_none());
        assert_eq!(state.pending_power_outlet, Some(first));

        let endpoint = PowerEndpoint::Device(cloud_provider_sim::DeviceId(99));
        let mut inlet_state = UiState::default();
        power_socket_action(&sim, &mut inlet_state, PowerSocket::Inlet(endpoint));
        assert!(
            power_socket_action(
                &sim,
                &mut inlet_state,
                PowerSocket::Inlet(PowerEndpoint::Device(cloud_provider_sim::DeviceId(98)))
            )
            .is_none()
        );
        assert_eq!(inlet_state.pending_power_inlet, Some(endpoint));
        assert!(power_socket_action(&sim, &mut inlet_state, PowerSocket::Outlet(first)).is_none());
        assert_eq!(inlet_state.pending_power_inlet, Some(endpoint));
    }

    #[test]
    fn power_socket_same_socket_cancels_and_occupied_selects_cable() {
        let (mut sim, device) = device_sim();
        let endpoint = PowerEndpoint::Device(device);
        sim.execute(Command::ConnectPower {
            outlet: outlet(),
            endpoint,
        })
        .unwrap();
        let mut state = UiState {
            pending_power_outlet: Some(outlet()),
            ..Default::default()
        };
        power_socket_action(&sim, &mut state, PowerSocket::Outlet(outlet()));
        assert_eq!(state.pending_power_outlet, None);
        state.pending_power_inlet = Some(endpoint);
        power_socket_action(&sim, &mut state, PowerSocket::Inlet(endpoint));
        assert_eq!(state.pending_power_inlet, None);
        power_socket_action(&sim, &mut state, PowerSocket::Inlet(endpoint));
        assert_eq!(state.selected, Selection::PowerCable(outlet()));
        assert_eq!(state.pending_power_inlet, None);
    }

    #[test]
    fn starting_ethernet_gesture_clears_power_pending_state() {
        let mut state = UiState {
            pending_power_outlet: Some(outlet()),
            pending_power_inlet: Some(PowerEndpoint::Device(cloud_provider_sim::DeviceId(1))),
            ..Default::default()
        };
        begin_ethernet_gesture(&mut state);
        assert!(state.pending_power_outlet.is_none());
        assert!(state.pending_power_inlet.is_none());
        assert!(state.pending_power_route.is_empty());
    }

    fn console_fixture() -> (NetworkSim, Vec<cloud_provider_sim::DeviceId>) {
        let mut sim = NetworkSim::new();
        let mut devices = vec![];
        for kind in [DeviceTemplate::Switch, DeviceTemplate::Router] {
            let device = match sim.execute(Command::BuyDevice { kind }).unwrap()[0] {
                SimEvent::DeviceAdded(id) => id,
                _ => unreachable!(),
            };
            sim.execute(Command::PlaceDevice {
                device,
                rack: RackId(1),
                unit: devices.len() as u8 + 1,
            })
            .unwrap();
            sim.execute(Command::ConnectPower {
                outlet: OutletId {
                    source: SourceId::Rack(RackId(1)),
                    index: devices.len() as u8,
                },
                endpoint: PowerEndpoint::Device(device),
            })
            .unwrap();
            sim.execute(Command::SetPower {
                device,
                powered: true,
            })
            .unwrap();
            devices.push(device);
        }
        (sim, devices)
    }

    #[test]
    fn console_worker_keeps_device_identity_and_stops_scripts_on_error() {
        let (sim, devices) = console_fixture();
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

    #[test]
    fn local_console_changes_live_state_and_updates_gui_history() {
        use std::io::{BufRead, BufReader, Write};
        use std::net::{TcpListener, TcpStream};

        fn exchange(address: std::net::SocketAddr, json: &str) -> RemoteResponse {
            let mut stream = TcpStream::connect(address).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let line = match serde_json::from_str::<RemoteRequest>(json) {
                Ok(request) => serde_json::to_string(&cloud_provider_sim::RemoteEnvelope {
                    password: "test-password".into(),
                    request,
                })
                .unwrap(),
                Err(_) => json.into(),
            };
            writeln!(stream, "{line}").unwrap();
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).unwrap();
            serde_json::from_str(&line).unwrap()
        }

        let (sim, devices) = console_fixture();
        let (tx, requests) = unbounded();
        let (responses, rx) = unbounded();
        tx.send(WorkerRequest::Replace(Box::new(sim))).unwrap();
        let worker = thread::spawn(move || worker_loop(requests, responses));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let config =
            ConsoleSettings::new("127.0.0.1".into(), address.port(), "test-password".into())
                .unwrap();
        let mut server = LocalConsoleServer::from_listener(listener, tx.clone(), config).unwrap();

        assert!(matches!(
            exchange(address, "invalid json"),
            RemoteResponse::Error(_)
        ));
        let denied = cloud_provider_sim::RemoteEnvelope {
            password: "wrong-password".into(),
            request: RemoteRequest::Run {
                device: devices[0],
                input: "enable".into(),
            },
        };
        assert!(
            matches!(exchange(address, &serde_json::to_string(&denied).unwrap()),
            RemoteResponse::Error(error) if error == "Authentication failed")
        );
        let connect = RemoteRequest::Connect {
            target: devices[0].to_string(),
        };
        assert!(
            matches!(exchange(address, &serde_json::to_string(&connect).unwrap()),
            RemoteResponse::Connected { device, .. } if device == devices[0])
        );
        for input in ["enable", "configure terminal", "hostname External", "exit"] {
            let request = RemoteRequest::Run {
                device: devices[0],
                input: input.into(),
            };
            assert!(matches!(
                exchange(address, &serde_json::to_string(&request).unwrap()),
                RemoteResponse::Output { success: true, .. }
            ));
        }
        let complete = RemoteRequest::Complete {
            device: devices[0],
            input: "conf".into(),
        };
        assert!(matches!(
            exchange(address, &serde_json::to_string(&complete).unwrap()),
            RemoteResponse::Completions(result) if result.candidates == ["configure"]
        ));
        let connect = RemoteRequest::Connect {
            target: "External".into(),
        };
        assert!(
            matches!(exchange(address, &serde_json::to_string(&connect).unwrap()),
            RemoteResponse::Connected { prompt, .. } if prompt == "External#")
        );
        let list = exchange(
            address,
            &serde_json::to_string(&RemoteRequest::List).unwrap(),
        );
        assert!(
            matches!(list, RemoteResponse::Devices(devices) if devices[0].hostname == "External")
        );

        let rotated = ConsoleSettings::new(
            "127.0.0.1".into(),
            address.port(),
            "rotated-password".into(),
        )
        .unwrap();
        server.reconfigure(rotated.clone(), tx.clone()).unwrap();
        assert!(
            matches!(exchange(address, &serde_json::to_string(&RemoteRequest::List).unwrap()),
            RemoteResponse::Error(error) if error == "Authentication failed")
        );
        let authenticated = serde_json::to_string(&cloud_provider_sim::RemoteEnvelope {
            password: "rotated-password".into(),
            request: RemoteRequest::List,
        })
        .unwrap();
        assert!(matches!(
            exchange(address, &authenticated),
            RemoteResponse::Devices(_)
        ));

        let occupied = TcpListener::bind("127.0.0.1:0").unwrap();
        let blocked = ConsoleSettings::new(
            "127.0.0.1".into(),
            occupied.local_addr().unwrap().port(),
            "another-password".into(),
        )
        .unwrap();
        assert!(server.reconfigure(blocked, tx.clone()).is_err());
        assert!(matches!(
            exchange(address, &authenticated),
            RemoteResponse::Devices(_)
        ));

        let next_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let next_address = next_listener.local_addr().unwrap();
        drop(next_listener);
        let next = ConsoleSettings::new(
            "127.0.0.1".into(),
            next_address.port(),
            "rotated-password".into(),
        )
        .unwrap();
        server.reconfigure(next, tx.clone()).unwrap();
        assert!(matches!(
            exchange(next_address, &authenticated),
            RemoteResponse::Devices(_)
        ));
        assert!(TcpStream::connect(address).is_err());

        server.stop();
        tx.send(WorkerRequest::Stop).unwrap();
        worker.join().unwrap();
        let mut console_lines = Vec::new();
        let mut latest = None;
        for response in rx.try_iter() {
            match response {
                WorkerResponse::Terminal { input, .. } => console_lines.push(input),
                WorkerResponse::Snapshot(sim) => latest = Some(sim),
                _ => {}
            }
        }
        assert_eq!(
            console_lines,
            ["enable", "configure terminal", "hostname External", "exit"]
        );
        assert_eq!(latest.unwrap().terminal_prompt(devices[0]), "External#");
    }
}
