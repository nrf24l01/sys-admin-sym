use cloud_provider_sim::{
    Command, DeviceKind, DeviceTemplate, NetworkSim, ServerHardware, ServerPartKind, server_catalog,
};

#[test]
fn memory_score_accounts_for_ddr_generation_and_module_type() {
    let score = |memory_type: &str| {
        ServerPartKind::Ram {
            memory_type: memory_type.into(),
            capacity_gb: 30,
        }
        .memory_score_gb()
        .unwrap()
    };
    assert_eq!(score("DDR3 UDIMM"), 30);
    assert_eq!(score("DDR4 RDIMM"), 48);
    assert_eq!(score("DDR5 RDIMM"), 60);
    assert_eq!(score("DDR5 DIMM"), 45);
}

#[test]
fn integrated_power_supply_allows_assembled_server_to_be_ready() {
    let hardware = ServerHardware {
        cpus: vec!["xeon_e_2434".into()],
        ram: vec!["ddr5_ecc_16gb".into()],
        ..Default::default()
    };
    assert!(hardware.ready());
    assert_eq!(hardware.compute_mhz(), 4 * 3400);
    assert_eq!(hardware.memory_score_gb(), 16 * 5 * 10 / 30);
    assert_eq!(server_catalog().chassis.integrated_psu_watts, 600);
}

#[test]
fn server_has_dedicated_management_nic_and_no_network_credit_when_disconnected() {
    let mut sim = NetworkSim::new();
    let id = match sim
        .execute(Command::BuyDevice {
            kind: DeviceTemplate::Server,
        })
        .unwrap()
        .as_slice()
    {
        [cloud_provider_sim::SimEvent::DeviceAdded(id)] => *id,
        _ => panic!("server purchase event missing"),
    };
    let device = sim.device(id).unwrap();
    let DeviceKind::Server(server) = &device.kind else {
        panic!("expected server")
    };
    assert!(
        server
            .ports
            .iter()
            .any(|id| sim.port(*id).unwrap().name == "mgmt0")
    );
    assert_eq!(sim.datacenter_resources().lan.compute_mhz, 0);
    assert_eq!(sim.datacenter_resources().global.compute_mhz, 0);
    let serialized = ron::to_string(&sim).unwrap();
    let mut reloaded: NetworkSim = ron::from_str(&serialized).unwrap();
    reloaded.rebuild_indexes();
    assert_eq!(
        reloaded.network_outlets().count(),
        sim.network_outlets().count()
    );
}

#[test]
fn empty_datacenter_has_no_transit_capacity() {
    let sim = NetworkSim::new();
    assert_eq!(sim.network_capacity().active_transit_mbps, 0);
    assert_eq!(sim.network_capacity().largest_transit_failure_mbps, 0);
}
