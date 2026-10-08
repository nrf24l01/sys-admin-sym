use super::*;

pub(super) fn offers() -> Vec<Offer> {
    let mut offers = Vec::new();
    for module in &optics_catalog().modules {
        if matches!(module.medium, ModuleMedium::DirectAttach) {
            continue;
        }
        let mut offer = Offer::new(
            &module.id,
            ShopSection::Transceivers,
            PurchaseItem::Transceiver(module.id.clone()),
        );
        offer.attributes = vec![
            Attribute::text("type", "transceiver"),
            Attribute::text("cage", cage_name(module.cage)),
            Attribute::number("power", module.power_mw),
            Attribute::text("dom", if module.dom { "yes" } else { "no" }),
        ];
        offer.attributes.extend(
            module
                .modes
                .iter()
                .map(|mode| Attribute::number("speed", mode.speed.mbps())),
        );
        match &module.medium {
            ModuleMedium::Copper => offer.attributes.push(Attribute::text("medium", "RJ45")),
            ModuleMedium::Optical {
                strands,
                tx_nm,
                rx_nm,
                reaches,
                ..
            } => {
                offer.attributes.extend([
                    Attribute::text("medium", "LC"),
                    Attribute::text("strands", if *strands == 1 { "simplex" } else { "duplex" }),
                    Attribute::number("tx", *tx_nm),
                    Attribute::number("rx", *rx_nm),
                ]);
                for reach in reaches {
                    offer.attributes.extend([
                        Attribute::text("fiber", fiber_name(reach.fiber)),
                        Attribute::number("reach", reach.max_length_cm),
                    ]);
                }
                offer.guidance.push(if *strands == 1 {
                    "shop.guide.bidi"
                } else {
                    "shop.guide.fiber"
                });
            }
            ModuleMedium::DirectAttach => unreachable!(),
        }
        offers.push(offer);
    }
    for cable in &optics_catalog().cables {
        let mut offer = Offer::new(
            &cable.id,
            if matches!(cable.medium, AssemblyMedium::Fiber { .. }) {
                ShopSection::FiberCables
            } else {
                ShopSection::DirectAttach
            },
            PurchaseItem::Assembly(cable.id.clone()),
        );
        offer.family = cable.id.rsplit_once('_').unwrap().0.into();
        offer
            .attributes
            .push(Attribute::number("length", cable.length_cm));
        match &cable.medium {
            AssemblyMedium::Fiber { fiber, strands, .. } => {
                offer.attributes.extend([
                    Attribute::text("type", "fiber"),
                    Attribute::text("fiber", fiber_name(*fiber)),
                    Attribute::text("strands", if *strands == 1 { "simplex" } else { "duplex" }),
                    Attribute::text("medium", "LC"),
                ]);
                offer.guidance.push("shop.guide.fiber");
            }
            AssemblyMedium::Dac { transceiver } | AssemblyMedium::Aoc { transceiver } => {
                offer.attributes.push(Attribute::text(
                    "type",
                    if matches!(cable.medium, AssemblyMedium::Dac { .. }) {
                        "dac"
                    } else {
                        "aoc"
                    },
                ));
                if let Some(module) = optics_catalog().module(transceiver) {
                    offer
                        .attributes
                        .push(Attribute::text("cage", cage_name(module.cage)));
                    offer.attributes.extend(
                        module
                            .modes
                            .iter()
                            .map(|mode| Attribute::number("speed", mode.speed.mbps())),
                    );
                }
                offer.guidance.push("shop.guide.assembly");
            }
        }
        offers.push(offer);
    }
    offers
}
