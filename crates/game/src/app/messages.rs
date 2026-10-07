use bevy::prelude::*;
use cloud_provider_sim::{
    CableRoutePoint, Command, DeviceId, DeviceTemplate, LinkId, NetworkSim, OutletId, PortId,
    PowerEndpoint, RackId, SimEvent, SourceId, TerminalOutput,
};

#[derive(Message, Debug, Clone)]
pub enum UiAction {
    SelectLanguage(String),
    SelectDevice(DeviceId),
    SelectPort(PortId),
    SelectLink(LinkId),
    SelectPowerCable(OutletId),
    Buy(DeviceTemplate),
    BuyServerChassis,
    BuyServerFullPack,
    BuyPublicIpv4Pool,
    AssignPublicIpv4 {
        port: PortId,
        network: std::net::Ipv4Addr,
    },
    AssignLanIpv4 {
        port: PortId,
    },
    BuyServerPart(String),
    BuyDrive(String),
    InstallDrive {
        device: DeviceId,
        drive_id: String,
        bay: Option<usize>,
    },
    RemoveDrive {
        device: DeviceId,
        bay: usize,
    },
    InstallServerPart {
        device: DeviceId,
        part_id: String,
        slot: Option<usize>,
    },
    RemoveServerPart {
        device: DeviceId,
        part_id: String,
        slot: Option<usize>,
    },
    BuyCableSupply(cloud_provider_sim::CableSupply),
    Place {
        device: DeviceId,
        rack: RackId,
        unit: u8,
    },
    Remove(DeviceId),
    TogglePower(DeviceId, bool),
    /// Click a power socket. A second complementary click completes the lead.
    PowerSocket(PowerSocket),
    AddPendingPowerRoutePoint(CableRoutePoint),
    DisconnectPower(OutletId),
    ReroutePowerCable {
        outlet: OutletId,
        route: Vec<CableRoutePoint>,
    },
    ResetPower(SourceId),
    RackMains(RackId, bool),
    CablePort(PortId),
    StartAssembly {
        port: PortId,
        assembly: cloud_provider_sim::CableAssemblyId,
    },
    AddPendingCableRoutePoint(CableRoutePoint),
    Disconnect(LinkId),
    CreateVlan(DeviceId),
    ApplyServer(PortId),
    ApplySwitch(PortId),
    ApplyRouter(PortId),
    FlushPortConfig(PortId),
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
    OpenRouting(DeviceId),
    OpenIpRanges,
    NetworkCommand(Command),
    RunTerminal(DeviceId, String),
    LaunchExternalTerminal(DeviceId),
    ApplyConsoleSettings(crate::settings::ConsoleSettings),
    Save,
    Load,
    NewGame,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerSocket {
    Outlet(OutletId),
    Inlet(PowerEndpoint),
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
    Terminal {
        device: DeviceId,
        input: String,
    },
    Replace(Box<NetworkSim>),
    Remote {
        request: cloud_provider_sim::RemoteRequest,
        reply: crossbeam_channel::Sender<cloud_provider_sim::RemoteResponse>,
    },
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
    Error(cloud_provider_sim::SimError),
}
