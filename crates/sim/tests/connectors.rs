use cloud_provider_sim::*;

fn switch(model: SwitchModel) -> DeviceKind {
    DeviceKind::Switch(Switch {
        model,
        ports: vec![],
        vlans: vec![],
        services: Default::default(),
    })
}

#[test]
fn network_and_power_share_the_connector_parent() {
    let network = PortConnector::Rj45;
    let power = PowerInletConnector::IecC14;
    let outlet = ConnectorType::IecC13;
    let connectors: [&dyn Connector; 3] = [&network, &power, &outlet];
    assert_eq!(connectors[0].definition().family, ConnectorFamily::Network);
    assert_eq!(connectors[1].definition().role, ConnectorRole::Inlet);
    assert_eq!(connectors[2].definition().role, ConnectorRole::Outlet);
    assert!(connectors[1].can_mate(connectors[2]));
    assert!(connectors[2].can_mate(connectors[1]));
    assert!(!connectors[0].can_mate(connectors[1]));
}

#[test]
fn switch_sfp_geometry_and_legacy_centers_share_the_json_map() {
    for model in [SwitchModel::Catalyst24T4G, SwitchModel::Catalyst24T4X] {
        let device = switch(model);
        for index in 24..28 {
            let socket = device
                .connector_socket(&PortConnector::Sfp, RackSide::Front, index)
                .unwrap();
            assert_eq!(socket.connector_type(), ConnectorType::Sfp);
            assert_eq!(socket.center(), device.port_position_normalized(index));
            assert!(socket.rect[0] < socket.rect[2] && socket.rect[1] < socket.rect[3]);
            assert!(socket.rect.iter().all(|v| (0.0..=1.0).contains(v)));
        }
        assert!(
            device
                .connector_socket(&PortConnector::Sfp, RackSide::Front, 0)
                .is_none()
        );
        assert!(
            device
                .connector_socket(&PortConnector::Sfp, RackSide::Rear, 24)
                .is_none()
        );
        assert!(
            device
                .connector_socket(&PortConnector::Sfp, RackSide::Front, 28)
                .is_none()
        );
        let power = device.power_connector_socket(0).unwrap();
        assert_eq!(power.center(), device.power_inlet().unwrap().position);
    }
    assert_ne!(
        switch(SwitchModel::Catalyst24T4G)
            .connector_socket(&PortConnector::Sfp, RackSide::Front, 24)
            .unwrap()
            .rect,
        switch(SwitchModel::Catalyst24T4X)
            .connector_socket(&PortConnector::Sfp, RackSide::Front, 24)
            .unwrap()
            .rect
    );
}

#[test]
fn router_power_socket_is_configured_as_front_four_pin() {
    let mut sim = NetworkSim::new();
    let SimEvent::DeviceAdded(id) = sim
        .execute(Command::BuyDevice {
            kind: DeviceTemplate::Router,
        })
        .unwrap()[0]
    else {
        panic!()
    };
    let device = &sim.device(id).unwrap().kind;
    let socket = device.power_connector_socket(0).unwrap();
    assert_eq!(socket.connector, ConnectorType::CiscoFourPin);
    assert_eq!(socket.side, RackSide::Front);
    assert_eq!(socket.center(), device.power_inlet().unwrap().position);
    assert!(
        device
            .connector_socket(&ConnectorType::IecC14, RackSide::Rear, 0)
            .is_none()
    );
}

#[test]
fn every_server_model_resolves_its_json_socket_types_and_rectangles() {
    for model in &server_catalog().models {
        let device = DeviceKind::Server(Server {
            model: ServerModelId::parse(&model.id).unwrap(),
            hostname: "host".into(),
            ports: vec![],
            hardware: None,
        });
        for (index, inlet) in model.power_inlets.iter().enumerate() {
            let socket = device.power_connector_socket(index).unwrap();
            assert_eq!(socket.rect, inlet.rect);
            assert_eq!(socket.connector, inlet.connector.unwrap());
        }
        for (index, port) in model.onboard_ports.iter().enumerate() {
            let socket = device
                .connector_socket(&PortConnector::Rj45, RackSide::Rear, index)
                .unwrap();
            assert_eq!(socket.rect, port.socket.rect);
            assert!(
                device
                    .connector_socket(&PortConnector::Sfp, RackSide::Rear, index)
                    .is_none()
            );
        }
        assert!(
            device
                .connector_socket(&ConnectorType::IecC14, RackSide::Front, 0)
                .is_none()
        );
    }
}

#[test]
fn shared_schema_preserves_metadata_and_rejects_invalid_socket_maps() {
    let config = switch(SwitchModel::default())
        .connector_config()
        .unwrap()
        .clone();
    let roundtrip: EquipmentSocketConfig =
        serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
    assert_eq!(roundtrip.metadata, config.metadata);
    assert_eq!(roundtrip.model, config.model);
    assert_eq!(roundtrip.ports["sfp"][0].index, Some(24));
    let mut invalid = config.clone();
    invalid.ports.get_mut("sfp").unwrap()[0].left_down[0] = -0.1;
    assert!(invalid.validate().is_err());
    let mut duplicate = config;
    duplicate.ports.get_mut("sfp").unwrap()[1].index = Some(24);
    assert!(duplicate.validate().is_err());
}
