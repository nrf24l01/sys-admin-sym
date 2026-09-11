use bevy::prelude::*;
use cloud_provider_sim::{
    Command, DeviceId, DeviceTemplate, LinkId, NetworkSim, PortId, RackId, SimEvent, TerminalOutput,
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
    CablePort(PortId),
    Disconnect(LinkId),
    CreateVlan(DeviceId),
    ApplyServer(PortId),
    ApplySwitch(PortId),
    ApplyRouter(PortId),
    RunTerminal(DeviceId, String),
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
