use bevy::prelude::*;
use cloud_provider_sim::{
    CableRoutePoint, Command, DeviceId, DeviceTemplate, LinkId, NetworkSim, OutletId, PortId,
    PowerEndpoint, RackId, SimEvent, SourceId, TerminalOutput,
};

#[derive(Message, Debug, Clone)]
pub enum UiAction {
    SelectDevice(DeviceId),
    SelectPort(PortId),
    SelectLink(LinkId),
    Buy(DeviceTemplate),
    BuyCableSupply(cloud_provider_sim::CableSupply),
    Place {
        device: DeviceId,
        rack: RackId,
        unit: u8,
    },
    Remove(DeviceId),
    TogglePower(DeviceId, bool),
    #[allow(dead_code)]
    ConnectPower(OutletId, PowerEndpoint),
    #[allow(dead_code)]
    DisconnectPower(OutletId),
    ResetPower(SourceId),
    RackMains(RackId, bool),
    CablePort(PortId),
    AddPendingCableRoutePoint(CableRoutePoint),
    Disconnect(LinkId),
    CreateVlan(DeviceId),
    ApplyServer(PortId),
    ApplySwitch(PortId),
    ApplyRouter(PortId),
    FlushPortConfig(PortId),
    AddCableRoutePoint {
        link: LinkId,
        point: CableRoutePoint,
    },
    RemoveCableRoutePoint {
        link: LinkId,
        index: usize,
    },
    MoveCableRoutePoint {
        link: LinkId,
        index: usize,
        point: CableRoutePoint,
    },
    RerouteCable {
        link: LinkId,
        route: Vec<CableRoutePoint>,
    },
    RunTerminal(DeviceId, String),
    LaunchExternalTerminal(DeviceId),
    Save,
    Load,
    NewGame,
}

#[derive(Message, Debug, Clone)]
pub struct SimCommandMessage(pub Command);

#[derive(Message, Debug, Clone)]
pub enum PersistenceRequest {
    Save,
    Load,
}

#[derive(Debug)]
pub enum WorkerRequest {
    Execute(Command),
    Terminal { device: DeviceId, input: String },
    Replace(Box<NetworkSim>),
    Stop,
}

#[derive(Debug)]
pub enum WorkerResponse {
    Snapshot(Box<NetworkSim>),
    Events(Vec<SimEvent>),
    Terminal {
        device: DeviceId,
        input: String,
        prompt: String,
        output: TerminalOutput,
    },
    ConsolesReset,
    Error(String),
}
