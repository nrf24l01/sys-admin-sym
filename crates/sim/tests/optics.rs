use cloud_provider_sim::*;

fn optics(sim: &mut NetworkSim, command: OpticsCommand) -> Vec<SimEvent> {
    sim.execute(Command::Optics(command)).unwrap()
}
fn device(sim: &mut NetworkSim, command: Command, unit: u8) -> DeviceId {
    let SimEvent::DeviceAdded(id) = sim.execute(command).unwrap()[0] else {
        panic!()
    };
    sim.execute(Command::PlaceDevice {
        device: id,
        rack: RackId(1),
        unit,
    })
    .unwrap();
    if !matches!(sim.device(id).unwrap().kind, DeviceKind::PatchPanel(_)) {
        let outlet = (0..RACK_C13_OUTLETS as u8)
            .map(|index| OutletId {
                source: SourceId::Rack(RackId(1)),
                index,
            })
            .find(|o| !sim.power.connections.contains_key(o))
            .unwrap();
        sim.execute(Command::ConnectPower {
            outlet,
            endpoint: PowerEndpoint::Device(id),
        })
        .unwrap();
        sim.execute(Command::SetPower {
            device: id,
            powered: true,
        })
        .unwrap();
    }
    id
}
fn hardware(model: &str) -> Command {
    Command::Optics(OpticsCommand::BuyHardware {
        model: model.into(),
    })
}
fn pair() -> (NetworkSim, DeviceId, DeviceId, PortId, PortId) {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    let a = device(&mut sim, hardware("switch_10g"), 1);
    let b = device(&mut sim, hardware("switch_10g"), 2);
    let ap = sim.device(a).unwrap().ports()[24];
    let bp = sim.device(b).unwrap().ports()[24];
    (sim, a, b, ap, bp)
}
fn module(sim: &mut NetworkSim, port: PortId, model: &str) -> TransceiverId {
    optics(
        sim,
        OpticsCommand::BuyTransceiver {
            model: model.into(),
        },
    );
    let id = *sim.optics.transceivers.keys().next_back().unwrap();
    optics(sim, OpticsCommand::InstallTransceiver { port, module: id });
    id
}
fn cable(sim: &mut NetworkSim, a: PortId, b: PortId, model: &str) -> CableAssemblyId {
    optics(
        sim,
        OpticsCommand::BuyAssembly {
            model: model.into(),
        },
    );
    let assembly = *sim.optics.assemblies.keys().next_back().unwrap();
    optics(
        sim,
        OpticsCommand::ConnectAssembly {
            assembly,
            a,
            b,
            route: vec![],
        },
    );
    assembly
}

#[test]
fn host_cage_rate_and_power_limits_are_enforced_without_consuming_inventory() {
    let (mut sim, _, _, _, _) = pair();
    let sw = device(
        &mut sim,
        Command::BuyDevice {
            kind: DeviceTemplate::Switch,
        },
        3,
    );
    let port = sim.device(sw).unwrap().ports()[24];
    optics(
        &mut sim,
        OpticsCommand::BuyTransceiver {
            model: "sfpplus_10g_sr".into(),
        },
    );
    let id = *sim.optics.transceivers.keys().next_back().unwrap();
    assert_eq!(
        sim.execute(Command::Optics(OpticsCommand::InstallTransceiver {
            port,
            module: id
        })),
        Err(SimError::Optics(OpticsError::IncompatibleHost))
    );
    assert!(sim.optics.transceivers[&id].port.is_none());
    assert_eq!(sim.port(port).unwrap().max_speed, LinkSpeed::Gbps1);
    let mut cage = CageProfile::for_speed(LinkSpeed::Gbps10);
    cage.max_power_mw = 1000;
    assert!(!cage.supports(optics_catalog().module("sfpplus_10g_sr").unwrap()));
    cage.max_power_mw = 2000;
    cage.modes[1].fec = Fec::Rs;
    assert!(!cage.supports(optics_catalog().module("sfpplus_10g_sr").unwrap()));
}

#[test]
fn hot_swap_preserves_vlan_configuration_and_updates_power_and_carrier() {
    let (mut sim, a, _, ap, bp) = pair();
    let base = sim.power.device_status(a).unwrap().load.watts;
    sim.execute(Command::SetSwitchPortMode {
        port: ap,
        mode: SwitchPortMode::Trunk {
            allowed: vec![VlanId(40)],
            native_vlan: None,
        },
    })
    .unwrap();
    let config = sim.port(ap).unwrap().config.clone();
    let first = module(&mut sim, ap, "sfpplus_10g_sr");
    module(&mut sim, bp, "sfpplus_10g_sr");
    cable(&mut sim, ap, bp, "fiber_om3_2_3m");
    assert_eq!(sim.port_link_speed(ap), Some(LinkSpeed::Gbps10));
    assert_eq!(sim.power.device_status(a).unwrap().load.watts, base + 2);
    optics(&mut sim, OpticsCommand::RemoveTransceiver { port: ap });
    assert_eq!(sim.link_status(ap).fault, Some(LinkFault::EmptyCage));
    assert_eq!(sim.power.device_status(a).unwrap().load.watts, base);
    assert_eq!(sim.port(ap).unwrap().config, config);
    optics(
        &mut sim,
        OpticsCommand::InstallTransceiver {
            port: ap,
            module: first,
        },
    );
    assert!(sim.physical_link_up(ap));
    sim.execute(Command::SetPortEnabled {
        port: bp,
        enabled: false,
    })
    .unwrap();
    assert_eq!(sim.link_status(ap).fault, Some(LinkFault::Disabled));
    sim.execute(Command::SetPortEnabled {
        port: bp,
        enabled: true,
    })
    .unwrap();
    sim.execute(Command::SetPower {
        device: a,
        powered: false,
    })
    .unwrap();
    assert_eq!(sim.link_status(bp).fault, Some(LinkFault::Unpowered));
}

#[test]
fn mismatched_optics_and_fiber_report_physical_faults() {
    for (left, right, lead, fault) in [
        (
            "sfpplus_10g_sr",
            "sfpplus_10g_lr",
            "fiber_om3_2_3m",
            LinkFault::WavelengthMismatch,
        ),
        (
            "sfpplus_10g_sr",
            "sfpplus_10g_sr",
            "fiber_os2_2_3m",
            LinkFault::FiberMismatch,
        ),
        (
            "sfp_1g_sx",
            "sfpplus_10g_sr",
            "fiber_om3_2_3m",
            LinkFault::ModeMismatch,
        ),
        (
            "sfp_1g_bidi_u",
            "sfp_1g_bidi_u",
            "fiber_os2_1_3m",
            LinkFault::WavelengthMismatch,
        ),
        (
            "sfp_1g_bidi_u",
            "sfp_1g_bidi_d",
            "fiber_os2_2_3m",
            LinkFault::FiberMismatch,
        ),
    ] {
        let (mut sim, _, _, ap, bp) = pair();
        module(&mut sim, ap, left);
        module(&mut sim, bp, right);
        cable(&mut sim, ap, bp, lead);
        assert_eq!(
            sim.link_status(ap).fault,
            Some(fault),
            "{left}/{right}/{lead}"
        );
        assert!(!sim.port_link_up(bp));
    }
}

#[test]
fn complementary_bidi_optics_use_one_os2_strand() {
    let (mut sim, _, _, ap, bp) = pair();
    module(&mut sim, ap, "sfp_1g_bidi_u");
    module(&mut sim, bp, "sfp_1g_bidi_d");
    cable(&mut sim, ap, bp, "fiber_os2_1_10000m");
    assert_eq!(sim.link_status(ap).speed, Some(LinkSpeed::Gbps1));
    assert_eq!(sim.link_status(ap).optical.len(), 2);
}

#[test]
fn polarity_is_independent_from_network_configuration() {
    let (mut sim, _, _, ap, bp) = pair();
    module(&mut sim, ap, "sfpplus_10g_sr");
    module(&mut sim, bp, "sfpplus_10g_sr");
    let assembly = cable(&mut sim, ap, bp, "fiber_om4_2_3m");
    assert!(sim.port_link_up(ap));
    optics(&mut sim, OpticsCommand::FlipPolarity { assembly });
    assert_eq!(sim.link_status(bp).fault, Some(LinkFault::PolarityMismatch));
    optics(&mut sim, OpticsCommand::FlipPolarity { assembly });
    assert!(sim.port_link_up(bp));
}

#[test]
fn fiber_panel_is_passive_and_total_channel_reach_includes_both_cords() {
    let (mut sim, _, _, ap, bp) = pair();
    let panel = device(&mut sim, hardware("fiber_panel"), 3);
    let ports = sim.device(panel).unwrap().ports().to_vec();
    module(&mut sim, ap, "sfpplus_10g_sr");
    module(&mut sim, bp, "sfpplus_10g_sr");
    let first = cable(&mut sim, ap, ports[0], "fiber_om3_2_100m");
    cable(&mut sim, ports[1], ports[2], "fiber_om3_2_100m");
    cable(&mut sim, ports[3], ports[4], "fiber_om3_2_100m");
    cable(&mut sim, ports[5], bp, "fiber_om3_2_100m");
    optics(&mut sim, OpticsCommand::FlipPolarity { assembly: first });
    assert!(!sim.device(panel).unwrap().powered);
    assert_eq!(sim.link_status(ports[0]).fault, Some(LinkFault::TooLong));
    // OM4 permits this same 400 m channel while OM3 permits only 300 m.
    for cable in sim.optics.assemblies.values_mut() {
        cable.model_id = "fiber_om4_2_100m".into();
    }
    assert_eq!(sim.link_status(bp).speed, Some(LinkSpeed::Gbps10));
    assert_eq!(sim.physical_path(ap).len(), 8);
    assert!(sim.link_status(ap).optical[0].rx_mdbm < -3000);
}

#[test]
fn connector_losses_can_exhaust_optical_budget_below_nominal_reach() {
    let (mut sim, _, _, ap, bp) = pair();
    let panel = device(&mut sim, hardware("fiber_panel"), 3);
    let ports = sim.device(panel).unwrap().ports().to_vec();
    module(&mut sim, ap, "sfpplus_10g_sr");
    module(&mut sim, bp, "sfpplus_10g_sr");
    let first = cable(&mut sim, ap, ports[0], "fiber_om3_2_3m");
    for i in 0..6 {
        cable(
            &mut sim,
            ports[i * 2 + 1],
            ports[(i + 1) * 2],
            "fiber_om3_2_3m",
        );
    }
    cable(&mut sim, ports[13], bp, "fiber_om3_2_3m");
    optics(&mut sim, OpticsCommand::FlipPolarity { assembly: first });
    let status = sim.link_status(ap);
    assert_eq!(status.fault, Some(LinkFault::LowLight));
    assert!(status.optical[0].rx_mdbm < -9900);
}

#[test]
fn dac_and_aoc_are_whole_assemblies_and_never_consume_rj45_materials() {
    for model in ["dac_10g_3m", "aoc_10g_10m"] {
        let (mut sim, a, _, ap, bp) = pair();
        let stock = sim.cable_inventory().clone();
        let watts = sim.power.device_status(a).unwrap().load.watts;
        let assembly = cable(&mut sim, ap, bp, model);
        assert_eq!(sim.port_link_speed(ap), Some(LinkSpeed::Gbps10));
        assert!(sim.power.device_status(a).unwrap().load.watts > watts);
        let id = sim.link_for_port(ap).unwrap().id;
        assert_eq!(
            sim.execute(Command::Optics(OpticsCommand::RemoveTransceiver {
                port: ap
            })),
            Err(SimError::Optics(OpticsError::NotOwned))
        );
        sim.execute(Command::Disconnect { link: id }).unwrap();
        assert!(sim.optics.assemblies[&assembly].link.is_none());
        assert_eq!(sim.cable_inventory(), &stock);
        assert_eq!(sim.power.device_status(a).unwrap().load.watts, watts);
        optics(
            &mut sim,
            OpticsCommand::ConnectAssembly {
                assembly,
                a: ap,
                b: bp,
                route: vec![],
            },
        );
        assert!(sim.port_link_up(ap));
    }
}

#[test]
fn copper_sfp_requires_a_module_and_can_connect_to_rj45() {
    let (mut sim, _, b, ap, _) = pair();
    let bp = sim.device(b).unwrap().ports()[0];
    module(&mut sim, ap, "sfp_1g_copper");
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::CableBox305m,
    })
    .unwrap();
    sim.execute(Command::BuyCableSupply {
        supply: CableSupply::Rj45Pack20,
    })
    .unwrap();
    sim.execute(Command::Connect { a: ap, b: bp }).unwrap();
    assert_eq!(sim.port_link_speed(bp), Some(LinkSpeed::Gbps1));
    optics(&mut sim, OpticsCommand::RemoveTransceiver { port: ap });
    assert_eq!(sim.link_status(bp).fault, Some(LinkFault::EmptyCage));
    optics(
        &mut sim,
        OpticsCommand::BuyTransceiver {
            model: "sfpplus_10g_sr".into(),
        },
    );
    let id = *sim.optics.transceivers.keys().next_back().unwrap();
    assert_eq!(
        sim.execute(Command::Optics(OpticsCommand::InstallTransceiver {
            port: ap,
            module: id
        })),
        Err(SimError::Optics(OpticsError::ConnectorMismatch))
    );
}

#[test]
fn too_short_assembly_and_reroute_fail_atomically() {
    let (mut sim, _, _, ap, bp) = pair();
    let assembly = cable(&mut sim, ap, bp, "dac_10g_3m");
    let link = sim.link_for_port(ap).unwrap().id;
    let route = vec![CableRoutePoint {
        rack: RackId(1),
        unit: 42,
        side: RackSide::Front,
        offset_cm: 48,
    }];
    assert!(matches!(
        sim.execute(Command::RerouteCable {
            link,
            route: route.clone()
        }),
        Err(SimError::CableTooShort { .. })
    ));
    assert!(sim.link(link).unwrap().route.is_empty());
    sim.execute(Command::Disconnect { link }).unwrap();
    assert!(matches!(
        sim.execute(Command::Optics(OpticsCommand::ConnectAssembly {
            assembly,
            a: ap,
            b: bp,
            route
        })),
        Err(SimError::CableTooShort { .. })
    ));
    assert!(sim.optics.assemblies[&assembly].link.is_none());
    assert!(sim.link_for_port(ap).is_none());
}

#[test]
fn optics_save_roundtrip_preserves_inventory_links_and_existing_save_defaults() {
    let (mut sim, _, _, ap, bp) = pair();
    module(&mut sim, ap, "sfpplus_10g_lr");
    module(&mut sim, bp, "sfpplus_10g_lr");
    let assembly = cable(&mut sim, ap, bp, "fiber_os2_2_30m");
    let saved = ron::to_string(&sim).unwrap();
    let mut loaded: NetworkSim = ron::from_str(&saved).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.port_link_speed(ap), Some(LinkSpeed::Gbps10));
    assert!(loaded.optics.assemblies[&assembly].link.is_some());
    let count = loaded.optics.transceivers.len();
    optics(
        &mut loaded,
        OpticsCommand::BuyTransceiver {
            model: "sfp_1g_sx".into(),
        },
    );
    assert_eq!(loaded.optics.transceivers.len(), count + 1);
    let mut legacy = serde_json::to_value(NetworkSim::new()).unwrap();
    legacy.as_object_mut().unwrap().remove("optics");
    let mut legacy: NetworkSim = serde_json::from_value(legacy).unwrap();
    legacy.rebuild_indexes();
    assert!(legacy.optics.transceivers.is_empty());
}

#[test]
fn optical_uplink_forwards_frames_and_cli_uses_same_carrier() {
    let (mut sim, a, b, ap, bp) = pair();
    module(&mut sim, ap, "sfpplus_10g_sr");
    module(&mut sim, bp, "sfpplus_10g_sr");
    let assembly = cable(&mut sim, ap, bp, "fiber_om4_2_3m");
    let frame = EthernetFrame {
        qos: FrameQos::default(),
        source: MacAddress([2, 0, 0, 0, 0, 1]),
        destination: MacAddress([255; 6]),
        vlan: None,
        payload: EthernetPayload::DhcpDiscover,
    };
    // Delivery to another switch causes forwarding on its copper ports only when they have live links.
    assert!(sim.physical_link_up(bp));
    assert!(
        sim.execute_console(a, "show interfaces transceiver")
            .lines
            .join("\n")
            .contains("10000 Mb/s")
    );
    assert!(
        sim.execute_console(b, "show interfaces Te1/0/1 transceiver")
            .success
    );
    assert!(
        sim.execute_console(a, "show inventory")
            .lines
            .join("\n")
            .contains("sfpplus_10g_sr")
    );
    sim.transmit_frame(ap, frame);
    assert!(sim.port_telemetry(ap).tx_frames > 0);
    optics(&mut sim, OpticsCommand::FlipPolarity { assembly });
    let before = sim.port_telemetry(ap).tx_frames;
    sim.transmit_frame(ap, frame);
    assert_eq!(sim.port_telemetry(ap).tx_frames, before);
    assert!(
        sim.execute_console(a, "show interfaces Te1/0/1 transceiver")
            .lines
            .join("\n")
            .contains("PolarityMismatch")
    );
}

#[test]
fn server_sfpplus_nic_adds_real_interfaces_and_removal_returns_modules_and_cable() {
    let part = server_catalog()
        .parts
        .iter()
        .find(|p| p.id == "intel_x520_da2")
        .unwrap();
    assert_eq!(part.display_name.get("en"), "Intel X520-DA2");
    assert!(matches!(
        &part.kind,
        ServerPartKind::PciCard {
            card: PciCard::Ethernet {
                lanes: 8,
                generation: 2,
                width: 8,
                ports: 2,
                connector: PortConnector::Sfp,
                speed_mbps: 10000,
                ..
            }
        }
    ));
    let (mut sim, _, _, ap, _) = pair();
    let server = device(&mut sim, Command::BuyServerFullPack, 3);
    sim.execute(Command::BuyServerPart {
        part_id: "intel_x520_da2".into(),
    })
    .unwrap();
    sim.execute(Command::InstallServerPart {
        device: server,
        part_id: "intel_x520_da2".into(),
        slot: Some(1),
    })
    .unwrap();
    let DeviceKind::Server(data) = &sim.device(server).unwrap().kind else {
        panic!()
    };
    let port = data.hardware.as_ref().unwrap().card_ports[1][0];
    let name = sim.port(port).unwrap().name.clone();
    assert_eq!(sim.port(port).unwrap().connector, PortConnector::Sfp);
    assert_eq!(sim.port(port).unwrap().max_speed, LinkSpeed::Gbps10);
    let installed = module(&mut sim, port, "sfpplus_10g_sr");
    module(&mut sim, ap, "sfpplus_10g_sr");
    let assembly = cable(&mut sim, port, ap, "fiber_om3_2_3m");
    assert_eq!(sim.server_resources(server).network_mbps, 10_000);
    assert!(
        sim.execute_console(server, &format!("ethtool -m {name}"))
            .lines
            .join("\n")
            .contains("RX optical power")
    );
    assert!(
        sim.execute_console(server, &format!("ethtool {name}"))
            .lines
            .join("\n")
            .contains("10000Mb/s")
    );
    // A live optical server interface actually receives Ethernet frames from the switch.
    let deliveries = sim.transmit_frame(
        ap,
        EthernetFrame {
            qos: FrameQos::default(),
            source: MacAddress::for_port(ap),
            destination: MacAddress([255; 6]),
            vlan: None,
            payload: EthernetPayload::DhcpDiscover,
        },
    );
    assert!(deliveries.iter().any(|delivery| delivery.port == port));
    sim.execute(Command::RemoveServerPart {
        device: server,
        part_id: "intel_x520_da2".into(),
        slot: Some(1),
    })
    .unwrap();
    assert!(sim.port(port).is_none());
    assert!(sim.optics.transceivers[&installed].port.is_none());
    assert!(sim.optics.assemblies[&assembly].link.is_none());
    assert!(!sim.optics.cages.contains_key(&port));
    assert!(!sim.port_link_up(ap));
    assert_eq!(sim.server_resources(server).network_mbps, 0);
}

#[test]
fn failed_optical_purchase_does_not_change_money_or_ownership() {
    let mut sim = NetworkSim::new();
    sim.money = 0;
    for command in [
        OpticsCommand::BuyTransceiver {
            model: "sfpplus_10g_sr".into(),
        },
        OpticsCommand::BuyAssembly {
            model: "dac_10g_3m".into(),
        },
        OpticsCommand::BuyHardware {
            model: "switch_10g".into(),
        },
    ] {
        assert!(matches!(
            sim.execute(Command::Optics(command)),
            Err(SimError::InsufficientFunds { .. })
        ));
    }
    assert_eq!(sim.money, 0);
    assert!(sim.optics.transceivers.is_empty());
    assert!(sim.optics.assemblies.is_empty());
    assert_eq!(sim.devices().count(), 0);
}

#[test]
fn fiber_connections_require_optical_modules_at_both_active_endpoints() {
    let (mut sim, _, _, ap, bp) = pair();
    optics(
        &mut sim,
        OpticsCommand::BuyAssembly {
            model: "fiber_om3_2_3m".into(),
        },
    );
    let assembly = *sim.optics.assemblies.keys().next_back().unwrap();
    for installed in [false, true] {
        if installed {
            module(&mut sim, ap, "sfpplus_10g_sr");
        }
        let money = sim.money;
        for (a, b) in [(ap, bp), (bp, ap)] {
            assert!(matches!(
                sim.quote_assembly(assembly, a, b, &[]),
                Err(SimError::Optics(OpticsError::MissingTransceiver))
            ));
            assert_eq!(
                sim.execute(Command::Optics(OpticsCommand::ConnectAssembly {
                    assembly,
                    a,
                    b,
                    route: vec![]
                })),
                Err(SimError::Optics(OpticsError::MissingTransceiver))
            );
        }
        assert_eq!(sim.money, money);
        assert!(sim.optics.assemblies[&assembly].link.is_none());
        assert!(sim.link_for_port(ap).is_none());
        assert!(sim.link_for_port(bp).is_none());
    }
    module(&mut sim, bp, "sfpplus_10g_sr");
    optics(
        &mut sim,
        OpticsCommand::ConnectAssembly {
            assembly,
            a: ap,
            b: bp,
            route: vec![],
        },
    );
    assert_eq!(sim.port_link_speed(ap), Some(LinkSpeed::Gbps10));
}

#[test]
fn loading_prototype_nics_migrates_inventory_and_preserves_live_interfaces() {
    let (mut sim, _, _, ap, _) = pair();
    let server = device(&mut sim, Command::BuyServerFullPack, 3);
    sim.execute(Command::BuyServerPart {
        part_id: "intel_x520_da2".into(),
    })
    .unwrap();
    sim.execute(Command::InstallServerPart {
        device: server,
        part_id: "intel_x520_da2".into(),
        slot: Some(1),
    })
    .unwrap();
    let DeviceKind::Server(data) = &sim.device(server).unwrap().kind else {
        panic!()
    };
    let original_ports = data.hardware.as_ref().unwrap().card_ports[1].clone();
    let port = original_ports[0];
    let module_id = module(&mut sim, port, "sfpplus_10g_sr");
    module(&mut sim, ap, "sfpplus_10g_sr");
    let assembly = cable(&mut sim, port, ap, "fiber_om3_2_3m");
    sim.server_parts.insert("intel_x520_da2".into(), 2);
    let legacy = ron::to_string(&sim)
        .unwrap()
        .replace("intel_x520_da2", "dual_sfpplus_10g");
    let mut loaded: NetworkSim = ron::from_str(&legacy).unwrap();
    loaded.server_parts.insert("intel_x520_da2".into(), 1);
    for _ in 0..2 {
        loaded.rebuild_indexes();
        assert_eq!(loaded.server_parts["intel_x520_da2"], 3);
        assert!(!loaded.server_parts.contains_key("dual_sfpplus_10g"));
        let DeviceKind::Server(data) = &loaded.device(server).unwrap().kind else {
            panic!()
        };
        let hardware = data.hardware.as_ref().unwrap();
        assert_eq!(hardware.pcie[1].as_deref(), Some("intel_x520_da2"));
        assert_eq!(hardware.card_ports[1], original_ports);
        assert_eq!(loaded.optics.transceivers[&module_id].port, Some(port));
        assert!(loaded.optics.assemblies[&assembly].link.is_some());
        assert_eq!(loaded.port_link_speed(port), Some(LinkSpeed::Gbps10));
    }
}

#[test]
fn inventory_picker_filters_media_strands_host_support_and_ownership() {
    let (mut sim, _, _, ap, bp) = pair();
    let mut stock = std::collections::BTreeMap::new();
    for model in [
        "fiber_om3_2_3m",
        "fiber_om4_2_3m",
        "fiber_os2_2_3m",
        "fiber_os2_1_3m",
        "dac_10g_3m",
        "aoc_10g_3m",
    ] {
        optics(
            &mut sim,
            OpticsCommand::BuyAssembly {
                model: model.into(),
            },
        );
        stock.insert(model, *sim.optics.assemblies.keys().next_back().unwrap());
    }
    assert!(sim.assembly_supported_at_port(stock["dac_10g_3m"], ap));
    assert!(sim.assembly_supported_at_port(stock["aoc_10g_3m"], ap));
    assert!(!sim.assembly_supported_at_port(stock["fiber_om3_2_3m"], ap));
    module(&mut sim, ap, "sfpplus_10g_sr");
    for model in stock.keys() {
        assert_eq!(
            sim.assembly_supported_at_port(stock[model], ap),
            matches!(*model, "fiber_om3_2_3m" | "fiber_om4_2_3m"),
            "{model}"
        );
    }
    module(&mut sim, bp, "sfp_1g_bidi_d");
    for model in stock.keys() {
        assert_eq!(
            sim.assembly_supported_at_port(stock[model], bp),
            *model == "fiber_os2_1_3m",
            "{model}"
        );
    }
    optics(&mut sim, OpticsCommand::RemoveTransceiver { port: bp });
    module(&mut sim, bp, "sfpplus_10g_sr");
    optics(
        &mut sim,
        OpticsCommand::ConnectAssembly {
            assembly: stock["fiber_om3_2_3m"],
            a: ap,
            b: bp,
            route: vec![],
        },
    );
    assert!(!sim.assembly_supported_at_port(stock["fiber_om4_2_3m"], ap));
    let spare_port = sim.device(sim.port(ap).unwrap().device).unwrap().ports()[25];
    module(&mut sim, spare_port, "sfpplus_10g_sr");
    assert!(!sim.assembly_supported_at_port(stock["fiber_om3_2_3m"], spare_port));
    assert!(sim.assembly_supported_at_port(stock["fiber_om4_2_3m"], spare_port));
    assert!(!sim.assembly_supported_at_port(CableAssemblyId(u64::MAX), spare_port));
}
