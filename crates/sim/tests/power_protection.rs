use cloud_provider_sim::*;

fn rack(p: &mut PowerSystem) -> RackId {
    let id = RackId(1);
    p.add_rack(id);
    id
}
fn outlet(source: SourceId, index: u8) -> OutletId {
    OutletId { source, index }
}

#[test]
fn ups_trips_on_watts_or_va_while_on_battery() {
    for (watts, pf, expect_trip) in [(1_100, 100, true), (900, 50, true)] {
        let mut p = PowerSystem::new();
        let r = rack(&mut p);
        let d = DeviceId(1);
        p.add_device(d, watts, pf);
        let u = p.add_ups(UpsSpec::default());
        p.connect(
            outlet(SourceId::Rack(r), 0),
            PowerEndpoint::Source(SourceId::Ups(u)),
        )
        .unwrap();
        p.connect(outlet(SourceId::Ups(u), 0), PowerEndpoint::Device(d))
            .unwrap();
        p.set_rack_mains(r, false);
        assert_eq!(p.ups[&u].tripped, expect_trip, "load {watts}W pf {pf}");
    }
}

#[test]
fn pdu_outlet_and_shared_rack_breakers_protect_current() {
    let mut p = PowerSystem::new();
    let r = rack(&mut p);
    let pdu = p.add_pdu(PduState::default());
    let d = DeviceId(1);
    p.add_device(d, 2_400, 100);
    p.connect(
        outlet(SourceId::Rack(r), 0),
        PowerEndpoint::Source(SourceId::Pdu(pdu)),
    )
    .unwrap();
    p.connect(outlet(SourceId::Pdu(pdu), 0), PowerEndpoint::Device(d))
        .unwrap();
    assert!(p.pdus[&pdu].tripped);
    let mut p = PowerSystem::new();
    let r = rack(&mut p);
    for (i, id) in [DeviceId(1), DeviceId(2)].into_iter().enumerate() {
        p.add_device(id, 2_000, 100);
        p.connect(
            outlet(SourceId::Rack(r), i as u8),
            PowerEndpoint::Device(id),
        )
        .unwrap();
    }
    assert!(!p.racks[&r].breaker_on);
}

#[test]
fn disabled_sources_do_not_trip_from_phantom_load() {
    let mut p = PowerSystem::new();
    let r = rack(&mut p);
    let u = p.add_ups(UpsSpec::default());
    let d = DeviceId(1);
    p.add_device(d, 2_000, 100);
    p.set_source_enabled(SourceId::Ups(u), false);
    p.connect(
        outlet(SourceId::Rack(r), 0),
        PowerEndpoint::Source(SourceId::Ups(u)),
    )
    .unwrap();
    p.connect(outlet(SourceId::Ups(u), 0), PowerEndpoint::Device(d))
        .unwrap();
    assert!(!p.ups[&u].tripped);
}

#[test]
fn indirect_source_cycles_are_rejected_in_both_orders() {
    for reverse in [false, true] {
        let mut p = PowerSystem::new();
        let r = rack(&mut p);
        let u = p.add_ups(UpsSpec::default());
        let q = p.add_pdu(PduState::default());
        let first = if reverse {
            (SourceId::Ups(u), SourceId::Pdu(q))
        } else {
            (SourceId::Pdu(q), SourceId::Ups(u))
        };
        p.connect(outlet(SourceId::Rack(r), 0), PowerEndpoint::Source(first.0))
            .unwrap();
        p.connect(outlet(first.0, 0), PowerEndpoint::Source(first.1))
            .unwrap();
        assert!(
            p.connect(outlet(first.1, 0), PowerEndpoint::Source(first.0))
                .is_err()
        );
    }
}
