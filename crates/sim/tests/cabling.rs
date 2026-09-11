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
    // 9.02462 cm sideways, 53.9505 cm vertically, then 10% slack.
    assert_eq!(sim.minimum_cable_length(a, b).unwrap(), 61);
    assert_eq!(sim.quote_cable(a, b, None).unwrap().length_cm, 61);
    assert_eq!(sim.quote_cable(a, b, Some(200)).unwrap().length_cm, 200);
    assert!(sim.quote_cable(a, b, Some(60)).is_err());
}
