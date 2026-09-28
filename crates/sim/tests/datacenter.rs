use cloud_provider_sim::{
    CableColor, CableRoutePoint, CableSupply, Command, DeviceTemplate, NetworkSim, RackId,
    RoomPosition, SimError, SimEvent,
};

#[test]
fn room_racks_and_managers_persist_and_price_cross_rack_routes() {
    let mut sim = NetworkSim::new();
    sim.execute(Command::AddRack {
        name: "Rack 02".into(),
        units: 12,
    })
    .unwrap();
    assert_eq!(sim.money, 5_500);
    let rack2 = RackId(2);
    assert_eq!(sim.rack(rack2).unwrap().name, "Rack 02");
    sim.execute(Command::AddRoomCableAnchor {
        position: RoomPosition {
            x_cm: 240,
            y_cm: 160,
        },
    })
    .unwrap();
    let server = match sim
        .execute(Command::BuyDevice {
            kind: DeviceTemplate::Server,
        })
        .unwrap()[0]
    {
        SimEvent::DeviceAdded(id) => id,
        _ => unreachable!(),
    };
    let switch = match sim
        .execute(Command::BuyDevice {
            kind: DeviceTemplate::Switch,
        })
        .unwrap()[0]
    {
        SimEvent::DeviceAdded(id) => id,
        _ => unreachable!(),
    };
    sim.execute(Command::PlaceDevice {
        device: server,
        rack: RackId(1),
        unit: 1,
    })
    .unwrap();
    sim.execute(Command::PlaceDevice {
        device: switch,
        rack: rack2,
        unit: 1,
    })
    .unwrap();
    let a = sim.device(server).unwrap().ports()[0];
    let b = sim.device(switch).unwrap().ports()[0];
    let direct = sim.minimum_cable_length(a, b).unwrap();
    let route = [CableRoutePoint::room_anchor(1)];
    let routed = sim.minimum_routed_cable_length(a, b, &route).unwrap();
    assert!(
        routed > direct,
        "ceiling cable manager must add a vertical run"
    );
    assert_eq!(
        sim.quote_routed_colored_cable(a, b, None, CableColor::White, &route)
            .unwrap()
            .length_cm,
        routed
    );
    assert!(matches!(
        sim.quote_routed_colored_cable(a, b, Some(direct), CableColor::White, &route),
        Err(SimError::CableTooShort { .. })
    ));
    sim.execute(Command::MoveRackInRoom {
        rack: rack2,
        position: RoomPosition {
            x_cm: 600,
            y_cm: 160,
        },
    })
    .unwrap();
    assert!(sim.minimum_cable_length(a, b).unwrap() > direct);
    let routed = sim.minimum_routed_cable_length(a, b, &route).unwrap();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::CableBox305m,
    })
    .unwrap();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::Rj45Pack20,
    })
    .unwrap();
    let link = match sim
        .execute(Command::ConnectRoutedColoredCable {
            a,
            b,
            length_cm: None,
            color: CableColor::White,
            route: route.to_vec(),
        })
        .unwrap()[0]
    {
        SimEvent::LinkCreated(id) => id,
        _ => unreachable!(),
    };
    assert_eq!(sim.link(link).unwrap().route, route);
    assert_eq!(sim.link(link).unwrap().length_cm, routed);
    assert!(matches!(
        sim.execute(Command::MoveRackInRoom {
            rack: rack2,
            position: RoomPosition {
                x_cm: 900,
                y_cm: 160
            },
        }),
        Err(SimError::CableTooShort { .. })
    ));
    assert_eq!(sim.rack_room_position(rack2).x_cm, 600);
    assert!(matches!(
        sim.execute(Command::MoveRoomCableAnchor {
            id: 1,
            position: RoomPosition {
                x_cm: 950,
                y_cm: 160
            },
        }),
        Err(SimError::CableTooShort { .. })
    ));
    assert_eq!(sim.room.cable_anchors[0].position.x_cm, 240);
    let restored: NetworkSim = ron::from_str(&ron::to_string(&sim).unwrap()).unwrap();
    assert_eq!(restored.room.cable_anchors.len(), 1);
    assert_eq!(restored.rack_room_position(rack2).x_cm, 600);
}

#[test]
fn room_rejects_invalid_positions_and_missing_managers() {
    let mut sim = NetworkSim::new();
    assert!(matches!(
        sim.execute(Command::MoveRackInRoom {
            rack: RackId(1),
            position: RoomPosition { x_cm: 0, y_cm: 0 }
        }),
        Err(SimError::InvalidRoomPosition)
    ));
    assert!(matches!(
        sim.minimum_routed_cable_length(
            cloud_provider_sim::PortId(1),
            cloud_provider_sim::PortId(2),
            &[CableRoutePoint::room_anchor(42)]
        ),
        Err(SimError::RoomAnchorNotFound(42)) | Err(SimError::PortNotFound(_))
    ));
}
