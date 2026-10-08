use super::*;

pub(super) fn offers() -> Vec<Offer> {
    let mut offers = Vec::new();
    for part in &server_catalog().parts {
        let (section, attributes) = match &part.kind {
            ServerPartKind::Cpu {
                socket,
                cores,
                frequency_mhz,
                tdp_w,
                ..
            } => (
                ShopSection::Cpu,
                vec![
                    Attribute::text("socket", socket.clone()),
                    Attribute::number("cores", *cores),
                    Attribute::number("frequency", *frequency_mhz),
                    Attribute::number("watts", *tdp_w),
                ],
            ),
            ServerPartKind::Ram {
                memory_type,
                capacity_gb,
            } => (
                ShopSection::Ram,
                vec![
                    Attribute::text("memory-type", memory_type.clone()),
                    Attribute::number("capacity", *capacity_gb),
                ],
            ),
            ServerPartKind::PciCard {
                card:
                    PciCard::Ethernet {
                        ports,
                        connector,
                        speed_mbps,
                        lanes,
                        generation,
                        width,
                        cage,
                    },
            } => {
                let mut attributes = vec![
                    Attribute::number("speed", *speed_mbps),
                    Attribute::number("pcie-generation", *generation),
                    Attribute::number("pcie-width", *width),
                    Attribute::number("lanes", *lanes),
                    Attribute::number("watts", part.power.peak_watts()),
                ];
                if *connector == PortConnector::Sfp {
                    attributes.extend([
                        Attribute::number("cages", *ports),
                        Attribute::text("cage", cage_name(cage.as_ref().unwrap().kind)),
                    ]);
                    attributes.extend(
                        cage.as_ref()
                            .unwrap()
                            .modes
                            .iter()
                            .map(|m| Attribute::number("speed", m.speed.mbps())),
                    );
                } else {
                    attributes.push(Attribute::number("ports", *ports));
                }
                (ShopSection::PciCards, attributes)
            }
            ServerPartKind::PowerSupply { .. } => continue,
        };
        let mut offer = Offer::new(&part.id, section, PurchaseItem::ServerPart(part.id.clone()));
        offer.attributes = attributes;
        offer.attributes.push(Attribute::text(
            "type",
            match section {
                ShopSection::Cpu => "cpu",
                ShopSection::Ram => "ram",
                _ => "nic",
            },
        ));
        if part.id == "intel_x520_da2" {
            offer.guidance.push("shop.guide.modules");
        }
        offers.push(offer);
    }
    for drive in &drive_catalog().drives {
        let mut offer = Offer::new(
            &drive.id,
            ShopSection::Storage,
            PurchaseItem::Drive(drive.id.clone()),
        );
        offer.attributes = vec![
            Attribute::text(
                "type",
                if drive.kind == DriveKind::Ssd {
                    "ssd"
                } else {
                    "hdd"
                },
            ),
            Attribute::text("interface", drive.interface.clone()),
            Attribute::number("capacity", drive.capacity_gb),
            Attribute::number("read", drive.read_mb_s),
            Attribute::number("write", drive.write_mb_s),
            Attribute::number("read-iops", drive.read_iops),
            Attribute::number("write-iops", drive.write_iops),
            Attribute::number("watts", drive.power.peak_watts()),
        ];
        offers.push(offer);
    }
    offers
}
