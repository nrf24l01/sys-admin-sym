//! Catalyst hardware specifications and deterministic switching services.
mod catalog;
mod channels;
mod migration;
mod qos;
mod snmp;
pub use catalog::*;
pub use channels::*;
pub use qos::*;
pub use snmp::*;

use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SwitchServices {
    pub etherchannel: EtherChannelConfig,
    pub qos: QosConfig,
    pub snmp: SnmpConfig,
}

impl NetworkSim {
    pub(crate) fn switch_services(&self, device: DeviceId) -> Option<&SwitchServices> {
        match &self.device(device)?.kind {
            DeviceKind::Switch(switch) => Some(&switch.services),
            _ => None,
        }
    }

    pub(crate) fn switch_services_mut(
        &mut self,
        device: DeviceId,
    ) -> Result<&mut SwitchServices, String> {
        match &mut self
            .devices
            .get_mut(&device)
            .ok_or("% Device not found.")?
            .kind
        {
            DeviceKind::Switch(switch) => Ok(&mut switch.services),
            _ => Err("% This command requires a switch.".into()),
        }
    }
}
