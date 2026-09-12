use cloud_provider_sim::*;

fn buy(sim: &mut NetworkSim, kind: DeviceTemplate) -> DeviceId {
    match &sim.execute(Command::BuyDevice { kind }).unwrap()[0] {
        SimEvent::DeviceAdded(id) => *id,
        event => panic!("unexpected event: {event:?}"),
    }
}

fn place(sim: &mut NetworkSim, device: DeviceId, unit: u8) {
    sim.execute(Command::PlaceDevice {
        device,
        rack: RackId(1),
        unit,
    })
    .unwrap();
}

fn connect(sim: &mut NetworkSim, source: SourceId, index: u8, endpoint: PowerEndpoint) {
    sim.execute(Command::ConnectPower {
        outlet: OutletId { source, index },
        endpoint,
    })
    .unwrap();
}

fn source(sim: &NetworkSim, device: DeviceId) -> SourceId {
    match &sim.device(device).unwrap().kind {
        DeviceKind::Ups(ups) => ups.source.expect("source assigned"),
        DeviceKind::Pdu(pdu) => pdu.source.expect("source assigned"),
        _ => panic!("not a power source device"),
    }
}

fn remove_top_level_field(mut text: String, field: &str) -> String {
    let needle = format!("{field}:");
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut quoted = false;
    let mut escaped = false;
    let mut start = None;
    for i in 0..bytes.len() {
        let c = bytes[i] as char;
        if quoted {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                quoted = false;
            }
            continue;
        }
        if c == '"' {
            quoted = true;
            continue;
        }
        if depth == 1 && text[i..].starts_with(&needle) {
            start = Some(i);
            break;
        }
        if matches!(c, '(' | '[' | '{') {
            depth += 1;
        }
        if matches!(c, ')' | ']' | '}') {
            depth -= 1;
        }
    }
    let start = start.expect("serialized field exists");
    let mut i = start + needle.len();
    let value_start = i;
    let mut value_depth = 0i32;
    quoted = false;
    escaped = false;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if quoted {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                quoted = false;
            }
        } else if c == '"' {
            quoted = true;
        } else if matches!(c, '(' | '[' | '{') {
            value_depth += 1;
        } else if matches!(c, ')' | ']' | '}') {
            value_depth -= 1;
        } else if c == ',' && value_depth == 0 {
            i += 1;
            break;
        }
        i += 1;
    }
    text.replace_range(start..i, "");
    let _ = value_start;
    text
}

#[test]
fn legacy_save_without_power_migrates_requested_flags_without_free_power() {
    let mut sim = NetworkSim::new();
    let on = buy(&mut sim, DeviceTemplate::Server);
    let off = buy(&mut sim, DeviceTemplate::Switch);
    let panel = buy(&mut sim, DeviceTemplate::PatchPanel);
    place(&mut sim, on, 1);
    place(&mut sim, off, 2);
    place(&mut sim, panel, 3);
    connect(
        &mut sim,
        SourceId::Rack(RackId(1)),
        0,
        PowerEndpoint::Device(on),
    );
    sim.execute(Command::SetPower {
        device: on,
        powered: true,
    })
    .unwrap();
    assert!(sim.device(on).unwrap().powered);
    sim.execute(Command::SetPower {
        device: off,
        powered: false,
    })
    .unwrap();
    let legacy = remove_top_level_field(ron::ser::to_string(&sim).unwrap(), "power");
    let mut loaded: NetworkSim = ron::from_str(&legacy).unwrap();
    loaded.rebuild_indexes();
    assert!(loaded.power.devices[&on].requested);
    assert!(!loaded.power.devices[&off].requested);
    assert!(!loaded.device(on).unwrap().powered);
    assert!(!loaded.device(off).unwrap().powered);
    assert!(!loaded.power.devices.contains_key(&panel));
}

#[test]
fn ups_pdu_chain_survives_outage_then_depletes_and_restores() {
    let mut sim = NetworkSim::new();
    let ups = buy(&mut sim, DeviceTemplate::Ups);
    let pdu = buy(&mut sim, DeviceTemplate::Pdu);
    let server = buy(&mut sim, DeviceTemplate::Server);
    place(&mut sim, ups, 1);
    place(&mut sim, pdu, 3);
    place(&mut sim, server, 4);
    let ups_source = source(&sim, ups);
    let pdu_source = source(&sim, pdu);
    connect(
        &mut sim,
        SourceId::Rack(RackId(1)),
        0,
        PowerEndpoint::Source(ups_source),
    );
    connect(&mut sim, ups_source, 0, PowerEndpoint::Source(pdu_source));
    connect(&mut sim, pdu_source, 0, PowerEndpoint::Device(server));
    sim.execute(Command::SetPower {
        device: server,
        powered: true,
    })
    .unwrap();
    assert!(sim.device(server).unwrap().powered);
    let before = sim.power.source_telemetry(ups_source).unwrap().battery_mwh;
    sim.execute(Command::SetRackMains {
        rack: RackId(1),
        on: false,
    })
    .unwrap();
    assert!(sim.device(server).unwrap().powered);
    assert!(sim.power.source_telemetry(ups_source).unwrap().available);
    assert!(sim.execute_console(server, "ip addr").success);
    let revision = sim.topology_revision;
    sim.advance_time(8 * 60 * 60 * 1000);
    assert!(!sim.device(server).unwrap().powered);
    assert!(sim.power.source_telemetry(ups_source).unwrap().battery_mwh < before);
    assert!(sim.topology_revision > revision);
    assert!(!sim.execute_console(server, "ip addr").success);
    sim.execute(Command::SetRackMains {
        rack: RackId(1),
        on: true,
    })
    .unwrap();
    assert!(sim.device(server).unwrap().powered);
    assert!(sim.topology_revision > revision);
}

#[test]
fn unplugging_device_loses_effective_power() {
    let mut sim = NetworkSim::new();
    let server = buy(&mut sim, DeviceTemplate::Server);
    place(&mut sim, server, 1);
    connect(
        &mut sim,
        SourceId::Rack(RackId(1)),
        0,
        PowerEndpoint::Device(server),
    );
    sim.execute(Command::SetPower {
        device: server,
        powered: true,
    })
    .unwrap();
    assert!(sim.device(server).unwrap().powered);
    sim.execute(Command::DisconnectPower {
        outlet: OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 0,
        },
    })
    .unwrap();
    assert!(!sim.device(server).unwrap().powered);
}

#[test]
fn invalid_power_connections_are_atomic() {
    let mut sim = NetworkSim::new();
    let server = buy(&mut sim, DeviceTemplate::Server);
    let result = sim.execute(Command::ConnectPower {
        outlet: OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 0,
        },
        endpoint: PowerEndpoint::Device(server),
    });
    assert!(result.is_err());
    assert!(sim.power.connections.is_empty());
    assert!(!sim.device(server).unwrap().powered);
    let ups = buy(&mut sim, DeviceTemplate::Ups);
    let result = sim.execute(Command::ConnectPower {
        outlet: OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 0,
        },
        endpoint: PowerEndpoint::Source(source(&sim, ups)),
    });
    assert!(
        result.is_err(),
        "an unplaced UPS cannot be used as a source"
    );
    assert!(sim.power.connections.is_empty());
    let panel = buy(&mut sim, DeviceTemplate::PatchPanel);
    place(&mut sim, panel, 1);
    let result = sim.execute(Command::ConnectPower {
        outlet: OutletId {
            source: SourceId::Rack(RackId(1)),
            index: 0,
        },
        endpoint: PowerEndpoint::Device(panel),
    });
    assert!(result.is_err());
    assert!(sim.power.connections.is_empty());
}

#[test]
fn selling_power_source_removes_its_downstream_connection() {
    let mut sim = NetworkSim::new();
    let ups = buy(&mut sim, DeviceTemplate::Ups);
    let server = buy(&mut sim, DeviceTemplate::Server);
    place(&mut sim, ups, 1);
    place(&mut sim, server, 3);
    let ups_source = source(&sim, ups);
    connect(
        &mut sim,
        SourceId::Rack(RackId(1)),
        0,
        PowerEndpoint::Source(ups_source),
    );
    connect(&mut sim, ups_source, 0, PowerEndpoint::Device(server));
    sim.execute(Command::SetPower {
        device: server,
        powered: true,
    })
    .unwrap();
    assert!(sim.device(server).unwrap().powered);
    sim.execute(Command::RemoveDevice { device: ups }).unwrap();
    sim.execute(Command::SellDevice { device: ups }).unwrap();
    assert!(!sim.device(server).unwrap().powered);
    assert!(sim.power.connections.is_empty());
}
