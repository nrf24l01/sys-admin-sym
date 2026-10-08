use super::*;

fn server_attributes(offer: &mut Offer, chassis: &ServerChassis) {
    offer.attributes.extend([
        Attribute::text("socket", chassis.cpu_socket.clone()),
        Attribute::text("memory-type", chassis.memory_type.clone()),
        Attribute::number("cpu-sockets", chassis.cpu_sockets as u64),
        Attribute::number("ram-slots", chassis.dimm_slots as u64),
        Attribute::number("drive-bays", chassis.drive_bays.len() as u64),
        Attribute::number("pcie-slots", chassis.pcie_slots.len() as u64),
        Attribute::number("psu-watts", server_power_profile().psu.capacity_watts),
    ]);
    let interfaces: std::collections::BTreeSet<_> = chassis
        .drive_bays
        .iter()
        .map(|bay| &bay.interface)
        .collect();
    offer.attributes.extend(
        interfaces
            .into_iter()
            .map(|interface| Attribute::text("drive-interface", interface.clone())),
    );
    let generations: std::collections::BTreeSet<_> = chassis
        .pcie_slots
        .iter()
        .map(|slot| slot.generation)
        .collect();
    offer.attributes.extend(
        generations
            .into_iter()
            .map(|generation| Attribute::number("pcie-generation", generation)),
    );
    let widths: std::collections::BTreeSet<_> =
        chassis.pcie_slots.iter().map(|slot| slot.width).collect();
    offer.attributes.extend(
        widths
            .into_iter()
            .map(|width| Attribute::number("pcie-width", width)),
    );
}

fn rack_attributes(offer: &mut Offer, command: Command) {
    // Derive connector counts/cage modes from a domain-created device, not UI constants.
    let mut preview = NetworkSim::new();
    preview.money = i64::MAX;
    let device = preview
        .execute(command)
        .unwrap()
        .into_iter()
        .find_map(|event| {
            if let SimEvent::DeviceAdded(id) = event {
                Some(id)
            } else {
                None
            }
        })
        .unwrap();
    let device = preview.device(device).unwrap();
    offer
        .attributes
        .push(Attribute::number("rack", device.template().rack_units()));
    let ports: Vec<_> = device
        .ports()
        .iter()
        .filter_map(|p| preview.port(*p))
        .collect();
    let copper = ports
        .iter()
        .filter(|p| {
            p.connector == PortConnector::Rj45
                && (p.paired_port.is_none() || p.side == RackSide::Front)
        })
        .count();
    if copper > 0 {
        offer
            .attributes
            .push(Attribute::number("ports", copper as u64));
    }
    let lc_pairs = ports
        .iter()
        .filter(|p| p.connector == PortConnector::Lc && p.side == RackSide::Front)
        .count();
    if lc_pairs > 0 {
        offer
            .attributes
            .push(Attribute::number("lc-pairs", lc_pairs as u64));
    }
    let cages = ports
        .iter()
        .filter(|p| p.connector == PortConnector::Sfp)
        .count();
    if cages > 0 {
        offer
            .attributes
            .push(Attribute::number("cages", cages as u64));
    }
    for port in &ports {
        if !matches!(device.kind, DeviceKind::PatchPanel(_))
            && matches!(port.connector, PortConnector::Rj45 | PortConnector::Sfp)
        {
            offer
                .attributes
                .push(Attribute::number("speed", port.max_speed.mbps()));
        }
        if let Some(cage) = preview.cage_profile(port.id) {
            offer
                .attributes
                .push(Attribute::text("cage", cage_name(cage.kind)));
            for mode in cage.modes {
                offer
                    .attributes
                    .push(Attribute::number("speed", mode.speed.mbps()));
            }
        }
    }
    match &device.kind {
        DeviceKind::Switch(switch) => {
            let spec = switch.model.spec();
            offer.attributes.extend([
                Attribute::number("switching-capacity", spec.switching_mbps),
                Attribute::number("forwarding-rate", spec.forwarding_kpps),
                Attribute::number("switch-idle-power", spec.power.idle_mw),
                Attribute::number("switch-traffic-power", spec.power.peak_mw),
                Attribute::text("poe", "no"),
            ]);
        }
        DeviceKind::Ups(ups) => {
            if let Some(source) = ups.source {
                offer.attributes.push(Attribute::number(
                    "outlets",
                    preview.power.outlets(source) as u64,
                ));
            }
        }
        DeviceKind::Pdu(pdu) => {
            if let Some(source) = pdu.source {
                offer.attributes.push(Attribute::number(
                    "outlets",
                    preview.power.outlets(source) as u64,
                ));
            }
        }
        _ => {}
    }
    offer
        .attributes
        .sort_by(|a, b| a.key.cmp(b.key).then(a.value.cmp(&b.value)));
    offer
        .attributes
        .dedup_by(|a, b| a.key == b.key && a.value == b.value);
}
pub(super) fn offers() -> Vec<Offer> {
    let mut offers = Vec::new();
    for (id, section, template) in [
        (
            "cisco_isr_c1111",
            ShopSection::Routers,
            DeviceTemplate::Router,
        ),
        (
            "cisco_catalyst_c1000",
            ShopSection::Switches,
            DeviceTemplate::Switch,
        ),
        ("apc_smt1500", ShopSection::Ups, DeviceTemplate::Ups),
        ("rack_pdu", ShopSection::Pdu, DeviceTemplate::Pdu),
        (
            "patch_panel",
            ShopSection::PatchPanels,
            DeviceTemplate::PatchPanel,
        ),
        (
            "cable_manager",
            ShopSection::CableManagers,
            DeviceTemplate::CableManager,
        ),
    ] {
        let mut offer = Offer::new(id, section, PurchaseItem::Device(template));
        rack_attributes(&mut offer, Command::BuyDevice { kind: template });
        let kind = match template {
            DeviceTemplate::Router => "router",
            DeviceTemplate::Switch => "switch",
            DeviceTemplate::Ups => "ups",
            DeviceTemplate::Pdu => "pdu",
            DeviceTemplate::PatchPanel => "patch-panel",
            DeviceTemplate::CableManager => "cable-manager",
            DeviceTemplate::Server => "server",
        };
        offer.attributes.push(Attribute::text("type", kind));
        if template == DeviceTemplate::Ups {
            let spec = UpsSpec::default();
            offer.attributes.extend([
                Attribute::text("outlet-type", "IEC C13"),
                Attribute::number("watts", spec.watts),
                Attribute::number("va", spec.va),
                Attribute::number("battery", spec.battery_wh),
            ]);
        } else if template == DeviceTemplate::Pdu {
            let spec = PduState::default();
            offer.attributes.extend([
                Attribute::text("outlet-type", "IEC C13"),
                Attribute::number("watts", spec.watts),
            ]);
        }
        offers.push(offer);
    }
    for full in [false, true] {
        let item = if full {
            PurchaseItem::ServerFullPack
        } else {
            PurchaseItem::ServerChassis
        };
        let mut offer = Offer::new(
            if full {
                "dell_r360_full_pack"
            } else {
                "dell_r360"
            },
            ShopSection::Servers,
            item,
        );
        offer.attributes.push(Attribute::text("type", "server"));
        offer.display_id = "dell_r360".into();
        offer.family = "dell_r360".into();
        rack_attributes(
            &mut offer,
            if full {
                Command::BuyServerFullPack
            } else {
                Command::BuyServerChassis
            },
        );
        offer.attributes.push(Attribute::text(
            "configuration",
            if full { "full-pack" } else { "chassis" },
        ));
        server_attributes(&mut offer, &server_catalog().chassis);
        offer.guidance.push(if full {
            "shop.guide.full-pack"
        } else {
            "shop.guide.chassis"
        });
        offers.push(offer);
    }
    for (id, supply, kind) in [
        (
            "ethernet_cable_box",
            CableSupply::CableBox305m,
            "bulk-cable",
        ),
        ("rj45_connectors", CableSupply::Rj45Pack20, "connector-pack"),
    ] {
        let mut offer = Offer::new(
            id,
            ShopSection::CopperSupplies,
            PurchaseItem::Supply(supply),
        );
        offer.attributes.push(Attribute::text("type", kind));
        offer.guidance.push("shop.guide.copper");
        offers.push(offer);
    }
    for hardware in &optics_catalog().hardware {
        let section = if matches!(hardware.profile, OpticalHardwareProfile::Switch { .. }) {
            ShopSection::Switches
        } else {
            ShopSection::PatchPanels
        };
        let mut offer = Offer::new(
            &hardware.id,
            section,
            PurchaseItem::OpticalHardware(hardware.id.clone()),
        );
        rack_attributes(
            &mut offer,
            Command::Optics(OpticsCommand::BuyHardware {
                model: hardware.id.clone(),
            }),
        );
        offer.attributes.push(Attribute::text(
            "type",
            if matches!(hardware.profile, OpticalHardwareProfile::FiberPanel) {
                "fiber-panel"
            } else {
                "switch"
            },
        ));
        if matches!(hardware.profile, OpticalHardwareProfile::FiberPanel) {
            offer.attributes.push(Attribute::text("medium", "LC"));
        } else {
            offer.guidance.push("shop.guide.modules");
        }
        offers.push(offer);
    }
    let mut ipv4 = Offer::new(
        "public_ipv4_pool",
        ShopSection::PublicIp,
        PurchaseItem::PublicIpv4Pool,
    );
    ipv4.attributes = vec![
        Attribute::text("prefix", "/29"),
        Attribute::number("addresses", 8_u64),
    ];
    ipv4.attributes
        .push(Attribute::text("type", "address-pool"));
    ipv4.guidance.push("shop.guide.ipv4");
    offers.push(ipv4);
    offers
}
