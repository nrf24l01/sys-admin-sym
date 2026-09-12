use cloud_provider_sim::*;

fn buy(sim: &mut NetworkSim, kind: DeviceTemplate) -> DeviceId {
    sim.money = i64::MAX / 4;
    match sim.execute(Command::BuyDevice { kind }).unwrap()[0] {
        SimEvent::DeviceAdded(id) => id,
        _ => unreachable!(),
    }
}

#[test]
fn physical_templates_and_port_metadata_are_realistic() {
    let mut sim = NetworkSim::new();
    let server = buy(&mut sim, DeviceTemplate::Server);
    assert_eq!(sim.device(server).unwrap().ports().len(), 3);
    assert!(
        sim.device(server)
            .unwrap()
            .ports()
            .iter()
            .all(|p| sim.port(*p).unwrap().side == RackSide::Rear)
    );

    let switch = buy(&mut sim, DeviceTemplate::Switch);
    assert!(
        sim.device(switch)
            .unwrap()
            .ports()
            .iter()
            .filter(|p| sim.port(**p).unwrap().connector == PortConnector::Rj45)
            .all(|p| sim.port(*p).unwrap().side == RackSide::Front)
    );
    let router = buy(&mut sim, DeviceTemplate::Router);
    assert!(
        sim.device(router)
            .unwrap()
            .ports()
            .iter()
            .all(|p| sim.port(*p).unwrap().side == RackSide::Front)
    );

    let panel = buy(&mut sim, DeviceTemplate::PatchPanel);
    let ports = sim.device(panel).unwrap().ports();
    assert_eq!(ports.len(), 48);
    let mut positions = Vec::new();
    for (index, port) in ports.iter().enumerate() {
        let p = sim.port(*port).unwrap();
        let position =
            DeviceKind::PatchPanel(PatchPanel { ports: vec![] }).port_position_normalized(index);
        if !positions.contains(&position) {
            positions.push(position);
        }
        let pair = p.paired_port.unwrap();
        let pair_index = ports
            .iter()
            .position(|candidate| *candidate == pair)
            .unwrap();
        assert_eq!(
            position,
            DeviceKind::PatchPanel(PatchPanel { ports: vec![] })
                .port_position_normalized(pair_index)
        );
        assert_eq!(
            p.paired_port
                .and_then(|pair| sim.port(pair).unwrap().paired_port),
            Some(*port)
        );
        assert_eq!(
            p.side,
            if index % 2 == 0 {
                RackSide::Rear
            } else {
                RackSide::Front
            }
        );
    }
    assert_eq!(positions.len(), 24);
    let manager = buy(&mut sim, DeviceTemplate::CableManager);
    assert_eq!(sim.device(manager).unwrap().template().rack_units(), 1);
}
