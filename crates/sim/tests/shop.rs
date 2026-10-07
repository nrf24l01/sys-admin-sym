use cloud_provider_sim::*;

fn purchase(
    sim: &mut NetworkSim,
    item: PurchaseItem,
    quantity: u32,
) -> Result<Vec<SimEvent>, SimError> {
    sim.execute(Command::Purchase { item, quantity })
}

#[test]
fn quantity_purchase_charges_the_domain_price_and_adds_every_item() {
    let mut sim = NetworkSim::new();
    let money = sim.money;
    let item = PurchaseItem::Drive("enterprise_ssd_960gb".into());
    let receipt = sim.quote_purchase(&item, 3).unwrap();
    purchase(&mut sim, item, 3).unwrap();
    assert_eq!(receipt.total, 3 * 230);
    assert_eq!(sim.money, money - receipt.total);
    assert_eq!(sim.drive_inventory["enterprise_ssd_960gb"], 3);
}

#[test]
fn unaffordable_and_invalid_quantity_purchases_leave_the_world_unchanged() {
    let mut sim = NetworkSim::new();
    sim.money = 400;
    let before = ron::ser::to_string(&sim).unwrap();
    for quantity in [0, 2, MAX_PURCHASE_QUANTITY + 1, u32::MAX] {
        assert!(
            purchase(
                &mut sim,
                PurchaseItem::Device(DeviceTemplate::Switch),
                quantity
            )
            .is_err()
        );
        assert_eq!(ron::ser::to_string(&sim).unwrap(), before);
    }
    assert!(purchase(&mut sim, PurchaseItem::Drive("missing".into()), 1).is_err());
    assert_eq!(ron::ser::to_string(&sim).unwrap(), before);
}

#[test]
fn complete_optical_assemblies_have_distinct_ids_and_never_consume_copper_stock() {
    let mut sim = NetworkSim::new();
    let copper = sim.cable_inventory().clone();
    purchase(&mut sim, PurchaseItem::Assembly("dac_10g_3m".into()), 4).unwrap();
    assert_eq!(sim.optics.assemblies.len(), 4);
    assert!(
        sim.optics
            .assemblies
            .values()
            .all(|c| c.model_id == "dac_10g_3m" && c.link.is_none())
    );
    assert_eq!(sim.cable_inventory(), &copper);
    assert!(purchase(&mut sim, PurchaseItem::Transceiver("dac_10g_end".into()), 1).is_err());
}

#[test]
fn multiple_full_packs_are_built_with_all_included_parts() {
    let mut sim = NetworkSim::new();
    sim.money = 10_000;
    let events = purchase(&mut sim, PurchaseItem::ServerFullPack, 2).unwrap();
    let ids: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let SimEvent::DeviceAdded(id) = e {
                Some(*id)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(ids.len(), 2);
    for id in ids {
        let DeviceKind::Server(server) = &sim.device(id).unwrap().kind else {
            panic!("server")
        };
        let hardware = server.hardware.as_ref().unwrap();
        assert_eq!(hardware.cpus, [ServerFullPack::CPU]);
        assert_eq!(hardware.ram, [ServerFullPack::RAM]);
        assert!(
            hardware
                .pcie
                .iter()
                .flatten()
                .any(|id| id == ServerFullPack::NIC)
        );
        assert!(
            hardware
                .drives
                .iter()
                .flatten()
                .any(|id| id == ServerFullPack::DRIVE)
        );
    }
    assert_eq!(sim.money, 10_000 - 2 * ServerFullPack::price());
}

#[test]
fn address_availability_accounts_for_overlapping_provider_pools() {
    let mut sim = NetworkSim::new();
    sim.money = 100_000;
    assert_eq!(sim.available_public_ipv4_pools(), 32);
    sim.execute(Command::Provider(ProviderCommand::AddPool(AddressPool {
        prefix: Ipv4Prefix::new("203.0.113.0".parse().unwrap(), 28).unwrap(),
        description: "Reserved provider pool".into(),
    })))
    .unwrap();
    assert_eq!(sim.available_public_ipv4_pools(), 30);
    let before = ron::ser::to_string(&sim).unwrap();
    assert_eq!(
        purchase(&mut sim, PurchaseItem::PublicIpv4Pool, 31),
        Err(SimError::PublicIpv4Exhausted)
    );
    assert_eq!(ron::ser::to_string(&sim).unwrap(), before);
    purchase(&mut sim, PurchaseItem::PublicIpv4Pool, 2).unwrap();
    assert_eq!(sim.available_public_ipv4_pools(), 28);
}

#[test]
fn compatibility_preview_uses_installation_rules_without_requiring_inventory() {
    let mut sim = NetworkSim::new();
    let events = purchase(&mut sim, PurchaseItem::ServerChassis, 1).unwrap();
    let id = events
        .iter()
        .find_map(|e| {
            if let SimEvent::DeviceAdded(id) = e {
                Some(*id)
            } else {
                None
            }
        })
        .unwrap();
    let before = ron::ser::to_string(&sim).unwrap();
    assert!(
        sim.server_part_installation_slot(id, "xeon_e_2434", None)
            .is_ok()
    );
    assert!(
        sim.drive_installation_bay(id, "enterprise_ssd_960gb", None)
            .is_ok()
    );
    assert!(
        sim.server_part_installation_slot(id, "intel_x520_da2", None)
            .is_err()
    );
    assert_eq!(ron::ser::to_string(&sim).unwrap(), before);
    purchase(&mut sim, PurchaseItem::ServerPart("xeon_e_2434".into()), 1).unwrap();
    sim.execute(Command::InstallServerPart {
        device: id,
        part_id: "xeon_e_2434".into(),
        slot: None,
    })
    .unwrap();
    let slot = sim
        .server_part_installation_slot(id, "intel_x520_da2", None)
        .unwrap();
    purchase(
        &mut sim,
        PurchaseItem::ServerPart("intel_x520_da2".into()),
        1,
    )
    .unwrap();
    sim.execute(Command::InstallServerPart {
        device: id,
        part_id: "intel_x520_da2".into(),
        slot,
    })
    .unwrap();
}
