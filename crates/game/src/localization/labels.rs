//! Built-in equipment labels use item identities. Player names remain unchanged.
use super::{item_name, tr_args};
use cloud_provider_sim::{Device, DeviceTemplate, Rack};

pub fn device_name(device: &Device) -> String {
    let (id, original) = match device.template() {
        DeviceTemplate::Server => {
            let cloud_provider_sim::DeviceKind::Server(server) = &device.kind else {
                unreachable!()
            };
            (server.model.as_str(), server.model().chassis.model.as_str())
        }
        DeviceTemplate::Switch | DeviceTemplate::Router => {
            let model = device.kind.network_model().unwrap();
            (model.id.as_str(), model.name.as_str())
        }
        DeviceTemplate::PatchPanel => ("patch_panel", "24-port Patch Panel"),
        DeviceTemplate::CableManager => ("cable_manager", "1U Horizontal Cable Manager"),
        DeviceTemplate::Ups => ("apc_smt1500", "APC Smart-UPS SMT1500RMI2U"),
        DeviceTemplate::Pdu => ("rack_pdu", "Rack PDU 8x C13"),
    };
    // Profile models retain their own item translations; only default names are translated.
    for model in &cloud_provider_sim::optics_catalog().hardware {
        if let Some(number) = device
            .name
            .strip_prefix(model.display_name.get("en"))
            .and_then(|name| name.strip_prefix(" #"))
            && number.chars().all(|c| c.is_ascii_digit())
        {
            return tr_args(
                "device.numbered-name",
                &[item_name(&model.id, &device.name), number.into()],
            );
        }
    }
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
