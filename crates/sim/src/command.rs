use crate::{
    DeviceId, DeviceTemplate, Ipv4InterfaceConfig, LinkId, PortId, RackId, SwitchPortMode, Vlan,
    VlanId,
};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    BuyCableSupply {
        supply: crate::CableSupply,
    },
    ConnectCable {
        a: PortId,
        b: PortId,
        length_cm: u32,
    },
    BuyDevice {
        kind: DeviceTemplate,
    },
    SellDevice {
        device: DeviceId,
    },
    PlaceDevice {
        device: DeviceId,
        rack: RackId,
        unit: u8,
    },
    RemoveDevice {
        device: DeviceId,
    },
    Connect {
        a: PortId,
        b: PortId,
    },
    Disconnect {
        link: LinkId,
    },
    SetSwitchPortMode {
        port: PortId,
        mode: SwitchPortMode,
    },
    CreateVlan {
        switch: DeviceId,
        vlan: Vlan,
    },
    SetIpv4 {
        port: PortId,
        config: Ipv4InterfaceConfig,
    },
    ConfigureRouterInterface {
        port: PortId,
        name: String,
        vlan: Option<VlanId>,
        address: Option<Ipv4Addr>,
        prefix: u8,
        internet_connected: bool,
    },
    SetHostname {
        device: DeviceId,
        hostname: String,
    },
    SetPower {
        device: DeviceId,
        powered: bool,
    },
}
