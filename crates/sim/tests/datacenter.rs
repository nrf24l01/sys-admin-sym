use cloud_provider_sim::{
    CableColor, CableRoutePoint, CableSupply, Command, DeviceTemplate, NetworkSim, RackId,
    RoomCableLayout, RoomPosition, SimError, SimEvent,
};

#[test]
fn predefined_room_has_ten_rows_of_five_racks() {
    let sim = NetworkSim::new();
    assert_eq!(sim.racks().count(), 50);
    assert_eq!(sim.money, 6_000);
    assert!(sim.racks().all(|rack| rack.units == 42));
    assert_eq!(
        sim.rack_room_position(RackId(1)),
        RoomPosition {
            x_cm: 180,
            y_cm: 180
        }
    );
    assert_eq!(
        sim.rack_room_position(RackId(5)),
        RoomPosition {
            x_cm: 1380,
            y_cm: 180
        }
    );
    assert_eq!(
        sim.rack_room_position(RackId(6)),
        RoomPosition {
            x_cm: 180,
            y_cm: 450
        }
    );
    assert_eq!(
        sim.rack_room_position(RackId(50)),
        RoomPosition {
            x_cm: 1380,
            y_cm: 2610
        }
    );
    assert_eq!(sim.room.cable_anchors.len(), RoomCableLayout::ANCHOR_COUNT);
    for (column, x_cm) in [630, 330, 930, 1230].into_iter().enumerate() {
        for row in 0..10u8 {
            let id = column as u8 * 10 + row + 1;
            let anchor = sim
                .room
                .cable_anchors
                .iter()
                .find(|anchor| anchor.id == id)
                .unwrap();
            assert_eq!(
                anchor.position,
                RoomPosition {
                    x_cm,
                    y_cm: 180 + u16::from(row) * 270
                }
            );
        }
    }
}

#[test]
fn cable_managers_cover_both_sides_of_each_rack_and_every_tray_crossing() {
    let sim = NetworkSim::new();
    let anchors = &sim.room.cable_anchors;
    let positions: std::collections::HashSet<_> = anchors
        .iter()
        .map(|anchor| (anchor.position.x_cm, anchor.position.y_cm))
        .collect();
    let ids: std::collections::HashSet<_> = anchors.iter().map(|anchor| anchor.id).collect();
    assert_eq!(positions.len(), RoomCableLayout::ANCHOR_COUNT);
    assert_eq!(ids.len(), RoomCableLayout::ANCHOR_COUNT);
    for rack in sim.racks() {
        for position in RoomCableLayout::rack_access_positions(rack.id) {
            assert!(positions.contains(&(position.x_cm, position.y_cm)));
        }
    }
    for y_cm in RoomCableLayout::horizontal_rows_cm() {
        for x_cm in cloud_provider_sim::DATACENTER_CABLE_COLUMNS_CM {
            assert!(positions.contains(&(x_cm, y_cm)));
        }
    }
    assert!(
        anchors
            .iter()
            .all(|anchor| anchor.position.x_cm < sim.room.width_cm
                && anchor.position.y_cm < sim.room.depth_cm)
    );
    let mut loaded: NetworkSim = ron::from_str(&ron::to_string(&sim).unwrap()).unwrap();
    loaded.rebuild_indexes();
    loaded.rebuild_indexes();
    assert_eq!(loaded.room.cable_anchors, *anchors);
}

#[test]
fn cable_manager_routes_between_fixed_racks_and_survives_save() {
    let mut sim = NetworkSim::new();
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
        rack: RackId(2),
        unit: 1,
    })
    .unwrap();
    let a = sim.device(server).unwrap().ports()[0];
    let b = sim.device(switch).unwrap().ports()[0];
    let direct = sim.minimum_cable_length(a, b).unwrap();
    let route = [CableRoutePoint::room_anchor(1)];
    let routed = sim.minimum_routed_cable_length(a, b, &route).unwrap();
    assert!(routed > direct);
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
    assert_eq!(sim.room.cable_anchors[0].position.x_cm, 630);
    let restored: NetworkSim = ron::from_str(&ron::to_string(&sim).unwrap()).unwrap();
    assert_eq!(restored.racks().count(), 50);
    assert_eq!(restored.link(link).unwrap().route, route);
}
