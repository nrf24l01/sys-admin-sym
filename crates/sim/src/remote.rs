use crate::DeviceId;
use serde::{Deserialize, Serialize};

pub const CONSOLE_ADDRESS: &str = "127.0.0.1:47655";

#[derive(Serialize, Deserialize)]
pub struct RemoteEnvelope {
    pub password: String,
    pub request: RemoteRequest,
}

/// One JSON request per local TCP connection.
#[derive(Debug, Serialize, Deserialize)]
pub enum RemoteRequest {
    List,
    Connect { target: String },
    Run { device: DeviceId, input: String },
    Complete { device: DeviceId, input: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoteDevice {
    pub id: DeviceId,
    pub name: String,
    pub hostname: String,
    pub kind: String,
    pub addresses: Vec<String>,
    pub powered: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RemoteResponse {
    Devices(Vec<RemoteDevice>),
    Connected {
        device: DeviceId,
        prompt: String,
    },
    Output {
        lines: Vec<String>,
        prompt: String,
        success: bool,
    },
    Error(String),
    Completions(crate::ConsoleCompletion),
}
