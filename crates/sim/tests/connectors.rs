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
    for (model, identity) in [
        (SwitchModel::Catalyst24T4G, ConnectorType::Sfp),
        (SwitchModel::Catalyst24T4X, ConnectorType::SfpPlus),
    ] {
        let device = switch(model);
        for index in 24..28 {
            let socket = device
                .connector_socket(&PortConnector::Sfp, RackSide::Front, index)
                .unwrap();
            assert_eq!(socket.connector_type(), identity);
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

#[test]
fn module_sockets_report_their_family_instead_of_a_generic_sfp_identity() {
    let mut sim = NetworkSim::new();
    sim.money = 1_000_000;
    for (model, index, expected) in [
        ("c1000_24t_4g_l", 24, "sfp"),
        ("c1000_24t_4x_l", 24, "sfp-plus"),
        ("c9500_24y4c_e", 0, "sfp28"),
        ("c9500_24y4c_e", 24, "qsfp28"),
        ("c8500_12x4qc", 12, "qsfp-plus"),
    ] {
        let SimEvent::DeviceAdded(id) = sim
            .execute(Command::BuyNetworkDevice {
                model_id: model.into(),
            })
            .unwrap()[0]
        else {
            panic!()
        };
        let kind = &sim.device(id).unwrap().kind;
        let socket = kind
            .connector_socket(&PortConnector::Sfp, RackSide::Front, index)
            .unwrap();
        assert_eq!(socket.connector.key(), expected, "{model} port {index}");
    }
}

#[test]
fn module_connector_parent_enforces_directional_family_compatibility() {
    let kinds = [
        CageKind::Sfp,
        CageKind::SfpPlus,
        CageKind::Sfp28,
        CageKind::QsfpPlus,
        CageKind::Qsfp28,
    ];
    let accepted = [
        [true, false, false, false, false],
        [true, true, false, false, false],
        [true, true, true, false, false],
        [false, false, false, true, false],
        [false, false, false, true, true],
    ];
    for (i, host) in kinds.iter().enumerate() {
        let connector: &dyn ModuleConnector = host;
        let parent: &dyn Connector = connector;
        assert_eq!(parent.definition().role, ConnectorRole::Module);
        for (j, module) in kinds.iter().enumerate() {
            assert_eq!(
                connector.accepts_module(module),
                accepted[i][j],
                "{host:?} <- {module:?}"
            );
        }
    }
}

#[test]
fn qsfp_defaults_support_lr4_modules_and_reject_sfp_modules() {
    let host = CageProfile::for_speed(LinkSpeed::Gbps100);
    assert!(host.supports(optics_catalog().module("qsfp100_lr4").unwrap()));
    assert!(host.supports(optics_catalog().module("qsfp40_lr4").unwrap()));
    assert!(!host.supports(optics_catalog().module("sfpplus_10g_sr").unwrap()));
    assert!(
        !CageProfile::for_speed(LinkSpeed::Gbps40)
            .supports(optics_catalog().module("qsfp100_lr4").unwrap())
    );
}

#[test]
fn typed_module_socket_rejects_a_module_from_the_wrong_family() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let SimEvent::DeviceAdded(id) = sim
        .execute(Command::BuyNetworkDevice {
            model_id: "c9500_24y4c_e".into(),
        })
        .unwrap()[0]
    else {
        panic!()
    };
    let kind = &sim.device(id).unwrap().kind;
    let socket = kind.module_connector_socket(RackSide::Front, 24).unwrap();
    let sfp = optics_catalog().module("sfpplus_10g_sr").unwrap();
    let qsfp = optics_catalog().module("qsfp40_lr4").unwrap();
    assert!(socket.profile.supports(qsfp));
    assert!(!socket.accepts_module(sfp));
    assert!(kind.connector_socket(sfp, RackSide::Front, 24).is_none());
    assert_eq!(
        kind.connector_socket(qsfp, RackSide::Front, 24),
        Some(socket.socket)
    );
}

#[test]
fn combo_ports_map_the_module_cage_before_and_after_installation() {
    let mut sim = NetworkSim::new();
    let SimEvent::DeviceAdded(id) = sim
        .execute(Command::BuyNetworkDevice {
            model_id: "isr_4321".into(),
        })
        .unwrap()[0]
    else {
        panic!()
    };
    let port = sim.device(id).unwrap().ports()[0];
    let kind = &sim.device(id).unwrap().kind;
    let copper = kind
        .connector_socket(&PortConnector::Rj45, RackSide::Front, 0)
        .unwrap();
    let optical = kind
        .connector_socket(&PortConnector::Sfp, RackSide::Front, 0)
        .expect("combo cage has its own physical socket");
    assert_ne!(copper.rect, optical.rect);
    sim.execute(Command::Optics(OpticsCommand::BuyTransceiver {
        model: "sfp_1g_sx".into(),
    }))
    .unwrap();
    let module = *sim.optics.transceivers.keys().next().unwrap();
    sim.execute(Command::Optics(OpticsCommand::InstallTransceiver {
        port,
        module,
    }))
    .unwrap();
    let kind = &sim.device(id).unwrap().kind;
    assert_eq!(
        kind.connector_socket(sim.port(port).unwrap(), RackSide::Front, 0),
        Some(optical)
    );
}
