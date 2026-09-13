use crate::{
    CableRoutePoint, DeviceId, DeviceTemplate, Ipv4InterfaceConfig, LinkId, LinkSpeed, PortId,
    RackId, Route, SwitchPortMode, Vlan, VlanId,
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
    ConnectColoredCable {
        a: PortId,
        b: PortId,
        length_cm: Option<u32>,
        color: crate::CableColor,
    },
    ConnectRoutedColoredCable {
        a: PortId,
        b: PortId,
        length_cm: Option<u32>,
        color: crate::CableColor,
        route: Vec<CableRoutePoint>,
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
    SetSwitchPortMode {
        port: PortId,
        mode: SwitchPortMode,
    },
    SetPortSpeed {
        port: PortId,
        speed: LinkSpeed,
    },
    SetPortEnabled {
        port: PortId,
        enabled: bool,
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
    SetStaticRoute {
        router: DeviceId,
        route: Route,
    },
    RemoveStaticRoute {
        router: DeviceId,
        route: Route,
    },
    SetHostname {
        device: DeviceId,
        hostname: String,
    },
    SetPower {
        device: DeviceId,
        powered: bool,
    },
    ConnectPower {
        outlet: crate::OutletId,
        endpoint: crate::PowerEndpoint,
    },
    ConnectPowerCord {
        outlet: crate::OutletId,
        endpoint: crate::PowerEndpoint,
        kind: crate::PowerCordKind,
    },
    DisconnectPower {
        outlet: crate::OutletId,
    },
    ResetPowerBreaker {
        source: crate::SourceId,
    },
    SetRackMains {
        rack: RackId,
        on: bool,
    },
    ResetPortConfig {
        port: PortId,
    },
}
