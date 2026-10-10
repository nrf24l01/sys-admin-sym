use cloud_provider_sim::*;

fn installed(sim: &mut NetworkSim, kind: DeviceTemplate, unit: u8) -> DeviceId {
    let device = match sim.execute(Command::BuyDevice { kind }).unwrap()[0] {
        SimEvent::DeviceAdded(id) => id,
        _ => unreachable!(),
    };
    sim.execute(Command::PlaceDevice {
        device,
        rack: RackId(1),
        unit,
    })
    .unwrap();
    let outlet = (0..RACK_C13_OUTLETS as u8)
        .map(|index| OutletId {
            source: SourceId::Rack(RackId(1)),
            index,
        })
        .find(|outlet| !sim.power.connections.contains_key(outlet))
        .expect("rack has a free C13 outlet");
    sim.execute(Command::ConnectPower {
        outlet,
        endpoint: PowerEndpoint::Device(device),
    })
    .unwrap();
    device
}
fn endpoints(sim: &mut NetworkSim) -> (PortId, PortId) {
    let a = installed(sim, DeviceTemplate::Switch, 1);
    let b = installed(sim, DeviceTemplate::Server, 2);
    (
        sim.device(a).unwrap().ports()[0],
        sim.device(b).unwrap().ports()[0],
    )
}
fn buy(sim: &mut NetworkSim, supply: CableSupply) {
    sim.execute(Command::BuyCableSupply { supply }).unwrap();
}

#[test]
fn colored_leads_are_saved_and_reused_only_with_matching_jacket() {
    let mut sim = NetworkSim::new();
    let (a, b) = endpoints(&mut sim);
    buy(&mut sim, CableSupply::CableBox305m);
    buy(&mut sim, CableSupply::Rj45Pack20);
    sim.execute(Command::ConnectColoredCable {
        a,
        b,
        length_cm: Some(125),
        color: CableColor::Blue,
    })
    .unwrap();
    let link = sim.link_for_port(a).unwrap().clone();
    assert_eq!(link.color, CableColor::Blue);
    sim.execute(Command::Disconnect { link: link.id }).unwrap();
    assert_eq!(
        sim.cable_inventory().patch_cable_colors,
        vec![CableColor::Blue]
    );
    let cable_before = sim.cable_inventory().cable_cm;
    // A requested color mismatch cannot consume the blue reusable lead.
    sim.execute(Command::ConnectColoredCable {
        a,
        b,
        length_cm: Some(125),
        color: CableColor::Red,
    })
    .unwrap();
    assert_eq!(sim.cable_inventory().cable_cm, cable_before - 125);
    let red = sim.link_for_port(a).unwrap().id;
    sim.execute(Command::Disconnect { link: red }).unwrap();
    // The matching blue lead is selected and removed without spending raw cable.
    sim.execute(Command::ConnectColoredCable {
        a,
        b,
        length_cm: Some(125),
        color: CableColor::Blue,
    })
    .unwrap();
    assert_eq!(sim.cable_inventory().cable_cm, cable_before - 125);
    assert!(
        !sim.cable_inventory()
            .patch_cable_colors
            .contains(&CableColor::Blue)
    );
}

#[test]
fn new_game_has_no_free_cable_and_purchases_charge_money() {
    let mut sim = NetworkSim::new();
    assert_eq!(*sim.cable_inventory(), CableInventory::default());
    buy(&mut sim, CableSupply::CableBox305m);
    assert_eq!(sim.cable_inventory().cable_cm, 30500);
    assert_eq!(sim.money, 6000 - CableSupply::CableBox305m.price());
    buy(&mut sim, CableSupply::Rj45Pack20);
    assert_eq!(sim.cable_inventory().connectors, 20);
    assert_eq!(sim.money, 5870);
    let before = sim.cable_inventory().clone();
    sim.money = 0;
    assert!(matches!(
        sim.execute(Command::BuyCableSupply {
            supply: CableSupply::CableBox305m
        }),
        Err(SimError::InsufficientFunds { .. })
    ));
    assert_eq!(*sim.cable_inventory(), before);
}

#[test]
fn shortages_invalid_endpoints_and_short_cuts_never_consume_materials() {
    let mut sim = NetworkSim::new();
    let (a, b) = endpoints(&mut sim);
    assert!(matches!(
        sim.execute(Command::Connect { a, b }),
        Err(SimError::InsufficientCable { .. })
    ));
    buy(&mut sim, CableSupply::CableBox305m);
    assert!(matches!(
        sim.execute(Command::Connect { a, b }),
        Err(SimError::InsufficientConnectors { .. })
    ));
    assert_eq!(sim.cable_inventory().cable_cm, 30500);
    buy(&mut sim, CableSupply::Rj45Pack20);
    let before = sim.cable_inventory().clone();
    assert!(matches!(
        sim.execute(Command::ConnectCable {
            a,
            b,
            length_cm: 10
        }),
        Err(SimError::CableTooShort { .. })
    ));
    assert!(matches!(
        sim.execute(Command::ConnectCable {
            a,
            b,
            length_cm: 10001
        }),
        Err(SimError::CableTooLong)
    ));
    assert_eq!(
        sim.execute(Command::Connect { a, b: a }),
        Err(SimError::SamePort)
    );
    assert_eq!(*sim.cable_inventory(), before);
    assert_eq!(sim.links().count(), 0);
    assert!(sim.quote_cable(a, b, None).is_ok());
    assert_eq!(*sim.cable_inventory(), before);
}

#[test]
fn crimping_consumes_exact_length_and_two_plugs_and_reuse_is_free() {
    let mut sim = NetworkSim::new();
    let (a, b) = endpoints(&mut sim);
    buy(&mut sim, CableSupply::CableBox305m);
    buy(&mut sim, CableSupply::Rj45Pack20);
    sim.execute(Command::ConnectCable {
        a,
        b,
        length_cm: 125,
    })
    .unwrap();
    assert_eq!(sim.cable_inventory().cable_cm, 30375);
    assert_eq!(sim.cable_inventory().connectors, 18);
    let link = sim.link_for_port(a).unwrap().clone();
    assert_eq!(link.length_cm, 125);
    assert!(matches!(
        sim.execute(Command::Connect { a, b }),
        Err(SimError::PortAlreadyConnected(_))
    ));
    assert_eq!(sim.cable_inventory().connectors, 18);
    sim.execute(Command::Disconnect { link: link.id }).unwrap();
    assert_eq!(sim.cable_inventory().patch_cables_cm, vec![125]);
    assert_eq!(sim.cable_inventory().connectors, 18);
    assert_eq!(sim.cable_inventory().cable_cm, 30375);
    assert!(sim.execute(Command::Disconnect { link: link.id }).is_err());
    assert_eq!(sim.cable_inventory().patch_cables_cm, vec![125]);
    sim.money = 0;
    assert!(!sim.quote_cable(a, b, None).unwrap().reused);
    assert!(sim.quote_cable(a, b, Some(125)).unwrap().reused);
    sim.execute(Command::ConnectCable {
        a,
        b,
        length_cm: 125,
    })
    .unwrap();
    assert_eq!(sim.link_for_port(a).unwrap().length_cm, 125);
    assert!(sim.cable_inventory().patch_cables_cm.is_empty());
    assert_eq!(sim.cable_inventory().cable_cm, 30375);
    assert_eq!(sim.cable_inventory().connectors, 18);
}

#[test]
fn taking_device_out_of_rack_recovers_leads_only_once() {
    let mut sim = NetworkSim::new();
    let (a, b) = endpoints(&mut sim);
    buy(&mut sim, CableSupply::CableBox305m);
    buy(&mut sim, CableSupply::Rj45Pack20);
    sim.execute(Command::Connect { a, b }).unwrap();
    let owner = sim.port(b).unwrap().device;
    let length = sim.link_for_port(a).unwrap().length_cm;
    sim.execute(Command::RemoveDevice { device: owner })
        .unwrap();
    assert!(sim.link_for_port(a).is_none());
    assert_eq!(sim.cable_inventory().patch_cables_cm, vec![length]);
    sim.execute(Command::SellDevice { device: owner }).unwrap();
    assert_eq!(sim.cable_inventory().patch_cables_cm, vec![length]);
}

#[test]
fn connector_pack_limits_number_of_new_leads() {
    let mut sim = NetworkSim::new();
    let a = installed(&mut sim, DeviceTemplate::Switch, 1);
    let b = installed(&mut sim, DeviceTemplate::Switch, 12);
    buy(&mut sim, CableSupply::CableBox305m);
    buy(&mut sim, CableSupply::Rj45Pack20);
    let ap = sim.device(a).unwrap().ports().to_vec();
    let bp = sim.device(b).unwrap().ports().to_vec();
    for i in 0..10 {
        sim.execute(Command::Connect { a: ap[i], b: bp[i] })
            .unwrap();
    }
    assert_eq!(sim.cable_inventory().connectors, 0);
    let cable = sim.cable_inventory().cable_cm;
    assert!(matches!(
        sim.execute(Command::Connect {
            a: ap[10],
            b: bp[10]
        }),
        Err(SimError::InsufficientConnectors { available: 0 })
    ));
    assert_eq!(sim.cable_inventory().cable_cm, cable);
    assert_eq!(sim.links().count(), 10);
    buy(&mut sim, CableSupply::Rj45Pack20);
    sim.execute(Command::Connect {
        a: ap[10],
        b: bp[10],
    })
    .unwrap();
    assert_eq!(sim.links().count(), 11);
}

#[test]
fn automatic_length_accounts_for_rack_distance_and_custom_length_is_exact() {
    let mut sim = NetworkSim::new();
    let a = installed(&mut sim, DeviceTemplate::Switch, 1);
    let b = installed(&mut sim, DeviceTemplate::Server, 12);
    let (a, b) = (
        sim.device(a).unwrap().ports()[0],
        sim.device(b).unwrap().ports()[0],
    );
    // Direct leads get 5% installation slack; routed leads use their measured path.
    assert_eq!(sim.minimum_cable_length(a, b).unwrap(), 58);
    assert_eq!(sim.quote_cable(a, b, None).unwrap().length_cm, 58);
    assert_eq!(sim.quote_cable(a, b, Some(200)).unwrap().length_cm, 200);
    assert!(sim.quote_cable(a, b, Some(57)).is_err());
}

fn settings(sim: &mut NetworkSim, settings: CableSettings) {
    sim.execute(Command::SetCableSettings { settings }).unwrap();
}

fn rack_anchor(unit: u8) -> CableRoutePoint {
    CableRoutePoint {
        rack: RackId(1),
        unit,
        side: RackSide::Rear,
        offset_cm: 0,
    }
}

#[test]
fn automatic_allowance_uses_the_full_route_and_consumes_the_quoted_materials() {
    let mut sim = NetworkSim::new();
    let (a, b) = endpoints(&mut sim);
    buy(&mut sim, CableSupply::CableBox305m);
    buy(&mut sim, CableSupply::Rj45Pack20);
    settings(
        &mut sim,
        CableSettings {
            extra_percent: 10,
            extra_cm: 50,
            reuse_longer_leads: false,
        },
    );
    let route = vec![rack_anchor(20), rack_anchor(10)];
    let minimum = sim.minimum_routed_cable_length(a, b, &route).unwrap();
    let quote = sim
        .quote_routed_colored_cable(a, b, None, CableColor::Blue, &route)
        .unwrap();
    assert_eq!(quote.length_cm, minimum + (minimum * 10).div_ceil(100) + 50);
    assert!(!quote.reused);
    let before = sim.cable_inventory().clone();
    // Quoting and choosing route anchors never consume materials.
    assert_eq!(*sim.cable_inventory(), before);
    sim.execute(Command::ConnectRoutedColoredCable {
        a,
        b,
        length_cm: None,
        color: CableColor::Blue,
        route: route.clone(),
    })
    .unwrap();
    let link = sim.link_for_port(a).unwrap();
    assert_eq!(link.length_cm, quote.length_cm);
    assert_eq!(link.route, route);
    assert_eq!(
        sim.cable_inventory().cable_cm,
        before.cable_cm - quote.length_cm
    );
    assert_eq!(sim.cable_inventory().connectors, before.connectors - 2);
}

#[test]
fn manual_cuts_ignore_allowance_and_reuse_requires_an_exact_match() {
    let mut sim = NetworkSim::new();
    let (a, b) = endpoints(&mut sim);
    buy(&mut sim, CableSupply::CableBox305m);
    buy(&mut sim, CableSupply::Rj45Pack20);
    sim.execute(Command::ConnectCable {
        a,
        b,
        length_cm: 200,
    })
    .unwrap();
    let link = sim.link_for_port(a).unwrap().id;
    sim.execute(Command::Disconnect { link }).unwrap();
    settings(
        &mut sim,
        CableSettings {
            extra_percent: 100,
            extra_cm: 10_000,
            reuse_longer_leads: true,
        },
    );
    assert_eq!(sim.quote_cable(a, b, None), Err(SimError::CableTooLong));
    assert_eq!(sim.quote_cable(a, b, Some(125)).unwrap().length_cm, 125);
    assert!(!sim.quote_cable(a, b, Some(125)).unwrap().reused);
    assert!(sim.quote_cable(a, b, Some(200)).unwrap().reused);
    let route = [rack_anchor(10)];
    assert_eq!(
        sim.quote_routed_colored_cable(a, b, Some(200), CableColor::White, &route)
            .unwrap()
            .length_cm,
        200
    );
}

#[test]
fn automatic_reuse_chooses_the_shortest_suitable_lead_of_the_selected_color() {
    let mut sim = NetworkSim::new();
    let (a, b) = endpoints(&mut sim);
    buy(&mut sim, CableSupply::CableBox305m);
    buy(&mut sim, CableSupply::Rj45Pack20);
    let minimum = sim.minimum_cable_length(a, b).unwrap();
    for (length_cm, color) in [
        (minimum + 30, CableColor::Blue),
        (minimum + 10, CableColor::Blue),
        (minimum + 5, CableColor::Red),
        (minimum + 1, CableColor::Blue),
    ] {
        sim.execute(Command::ConnectColoredCable {
            a,
            b,
            length_cm: Some(length_cm),
            color,
        })
        .unwrap();
        let link = sim.link_for_port(a).unwrap().id;
        sim.execute(Command::Disconnect { link }).unwrap();
    }
    settings(
        &mut sim,
        CableSettings {
            extra_cm: 5,
            ..Default::default()
        },
    );
    let exact = sim
        .quote_colored_cable(a, b, None, CableColor::Blue)
        .unwrap();
    assert!(!exact.reused);
    settings(
        &mut sim,
        CableSettings {
            extra_cm: 5,
            reuse_longer_leads: true,
            ..Default::default()
        },
    );
    let quote = sim
        .quote_colored_cable(a, b, None, CableColor::Blue)
        .unwrap();
    assert!(quote.reused);
    assert_eq!(quote.length_cm, minimum + 10);
    let before = sim.cable_inventory().clone();
    sim.execute(Command::ConnectRoutedColoredCable {
        a,
        b,
        length_cm: None,
        color: CableColor::Blue,
        route: Vec::new(),
    })
    .unwrap();
    assert_eq!(sim.link_for_port(a).unwrap().length_cm, minimum + 10);
    assert_eq!(sim.cable_inventory().cable_cm, before.cable_cm);
    assert_eq!(sim.cable_inventory().connectors, before.connectors);
    assert_eq!(sim.cable_inventory().patch_cables_cm.len(), 3);
    let serialized = ron::to_string(&sim).unwrap();
    let mut loaded: NetworkSim = ron::from_str(&serialized).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.link_for_port(a).unwrap().length_cm, minimum + 10);
}

#[test]
fn saved_allowance_and_cut_lengths_survive_rerouting_and_changing_defaults() {
    let mut sim = NetworkSim::new();
    let (a, b) = endpoints(&mut sim);
    buy(&mut sim, CableSupply::CableBox305m);
    buy(&mut sim, CableSupply::Rj45Pack20);
    let configured = CableSettings {
        extra_percent: 20,
        extra_cm: 150,
        reuse_longer_leads: true,
    };
    settings(&mut sim, configured);
    sim.execute(Command::ConnectRoutedColoredCable {
        a,
        b,
        length_cm: None,
        color: CableColor::Orange,
        route: vec![rack_anchor(20)],
    })
    .unwrap();
    let original = sim.link_for_port(a).unwrap().clone();
    let stock = sim.cable_inventory().clone();
    sim.execute(Command::RerouteCable {
        link: original.id,
        route: Vec::new(),
    })
    .unwrap();
    let mut loaded: NetworkSim = ron::from_str(&ron::to_string(&sim).unwrap()).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.cable_settings(), configured);
    assert_eq!(
        loaded.link_for_port(a).unwrap().length_cm,
        original.length_cm
    );
    settings(&mut loaded, CableSettings::default());
    assert_eq!(
        loaded.link_for_port(a).unwrap().length_cm,
        original.length_cm
    );
    loaded
        .execute(Command::Disconnect { link: original.id })
        .unwrap();
    assert_eq!(loaded.cable_inventory().cable_cm, stock.cable_cm);
    assert_eq!(loaded.cable_inventory().connectors, stock.connectors);
    assert_eq!(
        loaded.cable_inventory().patch_cables_cm,
        vec![original.length_cm]
    );
    assert_eq!(
        loaded.cable_inventory().patch_cable_colors,
        vec![CableColor::Orange]
    );
}

#[test]
fn invalid_settings_routes_and_oversized_automatic_cuts_leave_stock_untouched() {
    let mut sim = NetworkSim::new();
    let (a, b) = endpoints(&mut sim);
    buy(&mut sim, CableSupply::CableBox305m);
    buy(&mut sim, CableSupply::Rj45Pack20);
    let before = sim.cable_inventory().clone();
    for invalid in [
        CableSettings {
            extra_percent: 101,
            ..Default::default()
        },
        CableSettings {
            extra_cm: u32::MAX,
            ..Default::default()
        },
    ] {
        assert_eq!(
            sim.execute(Command::SetCableSettings { settings: invalid }),
            Err(SimError::InvalidCableSettings)
        );
        assert_eq!(sim.cable_settings(), CableSettings::default());
    }
    let mut invalid_anchor = rack_anchor(1);
    invalid_anchor.offset_cm = 49;
    assert_eq!(
        sim.quote_routed_colored_cable(a, b, None, CableColor::White, &[invalid_anchor]),
        Err(SimError::RackPlacementOutOfBounds)
    );
    let long_route: Vec<_> = (0..100)
        .map(|i| rack_anchor(if i % 2 == 0 { 42 } else { 1 }))
        .collect();
    assert_eq!(
        sim.execute(Command::ConnectRoutedColoredCable {
            a,
            b,
            length_cm: None,
            color: CableColor::White,
            route: long_route
        }),
        Err(SimError::CableTooLong)
    );
    settings(
        &mut sim,
        CableSettings {
            extra_cm: 10_000,
            ..Default::default()
        },
    );
    assert_eq!(
        sim.execute(Command::Connect { a, b }),
        Err(SimError::CableTooLong)
    );
    assert_eq!(*sim.cable_inventory(), before);
    assert_eq!(sim.links().count(), 0);
}

#[test]
fn older_saves_default_to_minimum_cuts_and_exact_reuse() {
    let mut sim = NetworkSim::new();
    let (a, b) = endpoints(&mut sim);
    let encoded = ron::to_string(&sim).unwrap();
    let legacy = encoded.replace(
        "cable_settings:(extra_percent:0,extra_cm:0,reuse_longer_leads:false),",
        "",
    );
    assert_ne!(encoded, legacy);
    let mut loaded: NetworkSim = ron::from_str(&legacy).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.cable_settings(), CableSettings::default());
    assert_eq!(
        loaded.quote_cable(a, b, None).unwrap().length_cm,
        sim.minimum_cable_length(a, b).unwrap()
    );
}

#[test]
fn overstretched_copper_routes_drop_the_link_until_the_route_fits_again() {
    let mut sim = NetworkSim::new();
    let (a, b) = endpoints(&mut sim);
    buy(&mut sim, CableSupply::CableBox305m);
    buy(&mut sim, CableSupply::Rj45Pack20);
    sim.execute(Command::ConnectCable {
        a,
        b,
        length_cm: 125,
    })
    .unwrap();
    let link = sim.link_for_port(a).unwrap().id;
    assert!(sim.port_link_up(a));
    let stock = sim.cable_inventory().clone();
    sim.execute(Command::RerouteCable {
        link,
        route: vec![rack_anchor(42)],
    })
    .unwrap();
    assert_eq!(sim.link_status(a).fault, Some(LinkFault::CableTooShort));
    assert_eq!(sim.link_status(b).fault, Some(LinkFault::CableTooShort));
    assert_eq!(sim.port_link_speed(a), None);
    assert_eq!(sim.link(link).unwrap().length_cm, 125);
    sim.execute(Command::RerouteCable {
        link,
        route: Vec::new(),
    })
    .unwrap();
    assert!(sim.port_link_up(a));
    assert_eq!(sim.link(link).unwrap().length_cm, 125);
    assert_eq!(*sim.cable_inventory(), stock);
}
