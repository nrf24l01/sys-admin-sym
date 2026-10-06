//! Built-in equipment labels use item identities. Player names remain unchanged.
use super::{item_name, tr_args};
use cloud_provider_sim::{Device, DeviceTemplate, Rack};

pub fn device_name(device: &Device) -> String {
    let (id, original) = match device.template() {
        DeviceTemplate::Server => ("dell_r360", "Dell PowerEdge R360"),
        DeviceTemplate::Switch => ("cisco_catalyst_c1000", "Cisco Catalyst C1000-24T-4G-L"),
        DeviceTemplate::Router => ("cisco_isr_c1111", "Cisco ISR C1111-8P"),
        DeviceTemplate::PatchPanel => ("patch_panel", "24-port Patch Panel"),
        DeviceTemplate::CableManager => ("cable_manager", "1U Horizontal Cable Manager"),
        DeviceTemplate::Ups => ("apc_smt1500", "APC Smart-UPS SMT1500RMI2U"),
        DeviceTemplate::Pdu => ("rack_pdu", "Rack PDU 8x C13"),
    };
    if let Some(number) = device
        .name
        .strip_prefix(original)
        .and_then(|name| name.strip_prefix(" #"))
        && number.chars().all(|c| c.is_ascii_digit())
    {
        tr_args(
            "device.numbered-name",
            &[item_name(id, original), number.into()],
        )
    } else {
        device.name.clone()
    }
}

pub fn rack_name(rack: &Rack) -> String {
    if let Some(number) = rack.name.strip_prefix("Rack ")
        && !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
    {
        tr_args("rack.name", &[number.into()])
    } else {
        rack.name.clone()
    }
}

pub fn room_name(room: &cloud_provider_sim::DataCenterRoom) -> String {
    if room.name == "Room 01" {
        tr_args("room.name", &["01".into()])
    } else {
        room.name.clone()
    }
}
