use crate::app::{ShopCategory, ShopSection, ShopState, UiAction};
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::{
    CableSupply, DeviceTemplate, DriveModel, NetworkOutletKind, NetworkSim, PciCard,
    PublicIpv4Block, ServerFullPack, ServerPart, ServerPartKind, drive_catalog, server_catalog,
};

fn drive_matches(drive: &DriveModel, state: &ShopState, money: i64) -> bool {
    state.category == ShopCategory::Compute
        && state.section.is_none_or(|s| s == ShopSection::Storage)
        && state.rack_units.is_none()
        && state.ports.is_none()
        && state.outlets.is_none()
        && (state.search.trim().is_empty()
            || format!("{} {}", drive.name, drive.id)
                .to_lowercase()
                .contains(&state.search.trim().to_lowercase()))
        && (!state.affordable_only || drive.price <= money)
        && state.max_price.is_none_or(|max| drive.price <= max)
}

fn part_matches(part: &ServerPart, state: &ShopState, money: i64) -> bool {
    let section = match part.kind {
        ServerPartKind::Cpu { .. } => ShopSection::Cpu,
        ServerPartKind::Ram { .. } => ShopSection::Ram,
        ServerPartKind::PowerSupply { .. } => return false,
        ServerPartKind::PciCard { .. } => ShopSection::PciCards,
    };
    state.category == ShopCategory::Compute
        && state.section.is_none_or(|s| s == section)
        && state.rack_units.is_none()
        && state.ports.is_none()
        && state.outlets.is_none()
        && (state.search.trim().is_empty()
            || format!("{} {}", part.name, part.id)
                .to_lowercase()
                .contains(&state.search.trim().to_lowercase()))
        && (!state.affordable_only || part.price <= money)
        && state.max_price.is_none_or(|max| part.price <= max)
}

fn part_description(part: &ServerPart) -> String {
    match &part.kind {
        ServerPartKind::Cpu {
            socket,
            pcie_lanes,
            tdp_w,
            cores,
            frequency_mhz,
        } => format!(
            "{socket} · {cores} cores @ {} MHz · {pcie_lanes} PCIe lanes · {tdp_w} W TDP",
            frequency_mhz
        ),
        ServerPartKind::Ram {
            memory_type,
            capacity_gb,
        } => format!("{capacity_gb} GB · {memory_type}"),
        ServerPartKind::PowerSupply { capacity_w } => format!("{capacity_w} W power supply"),
        ServerPartKind::PciCard {
            card:
                PciCard::Ethernet {
                    lanes,
                    generation,
                    rj45_ports,
                    speed_mbps,
                    ..
                },
        } => format!("{rj45_ports} × RJ45 · {speed_mbps} Mb/s · PCIe Gen {generation} x{lanes}"),
    }
}

#[derive(Clone, Copy)]
enum Purchase {
    Equipment(DeviceTemplate),
    ServerChassis,
    Supply(CableSupply),
}

struct Product {
    name: &'static str,
    description: &'static str,
    section: ShopSection,
    purchase: Purchase,
    ports: Option<u16>,
    outlets: Option<u16>,
}

impl Product {
    fn price(&self) -> i64 {
        match self.purchase {
            Purchase::Equipment(template) => template.price(),
            Purchase::ServerChassis => DeviceTemplate::Server.price(),
            Purchase::Supply(supply) => supply.price(),
        }
    }

    fn rack_units(&self) -> Option<u8> {
        match self.purchase {
            Purchase::Equipment(template) => Some(template.rack_units()),
            Purchase::ServerChassis => Some(1),
            Purchase::Supply(_) => None,
        }
    }

    fn matches(&self, filters: &ShopState, money: i64) -> bool {
        let search = filters.search.trim().to_lowercase();
        self.section.category() == filters.category
            && filters
                .section
                .is_none_or(|section| section == self.section)
            && (search.is_empty()
                || format!("{} {}", self.name, self.description)
                    .to_lowercase()
                    .contains(&search))
            && (!filters.affordable_only || self.price() <= money)
            && filters.max_price.is_none_or(|price| self.price() <= price)
            && filters
                .rack_units
                .is_none_or(|units| self.rack_units() == Some(units))
            && filters.ports.is_none_or(|ports| self.ports == Some(ports))
            && filters
                .outlets
                .is_none_or(|outlets| self.outlets == Some(outlets))
    }

    fn buy(&self) -> UiAction {
        match self.purchase {
            Purchase::Equipment(template) => UiAction::Buy(template),
            Purchase::ServerChassis => UiAction::BuyServerChassis,
            Purchase::Supply(supply) => UiAction::BuyCableSupply(supply),
        }
    }
}

const PRODUCTS: &[Product] = &[
    Product {
        name: "Cisco ISR C1111-8P",
        description: "2 WAN + 8 LAN RJ45 ports · supplied 66 W adapter",
        section: ShopSection::Routers,
        purchase: Purchase::Equipment(DeviceTemplate::Router),
        ports: Some(10),
        outlets: None,
    },
    Product {
        name: "Cisco Catalyst C1000-24T-4G-L",
        description: "24 RJ45 ports · 4 SFP cages (not yet active)",
        section: ShopSection::Switches,
        purchase: Purchase::Equipment(DeviceTemplate::Switch),
        ports: Some(24),
        outlets: None,
    },
    Product {
        name: "Dell PowerEdge R360",
        description: "1U R360 chassis · integrated 600 W PSU · full pack available with CPU, RAM, NIC and SSD",
        section: ShopSection::DellServers,
        purchase: Purchase::ServerChassis,
        ports: Some(3),
        outlets: None,
    },
    Product {
        name: "APC Smart-UPS SMT1500RMI2U",
        description: "1000 W / 1500 VA · battery backup · 4 C13 outlets",
        section: ShopSection::Ups,
        purchase: Purchase::Equipment(DeviceTemplate::Ups),
        ports: None,
        outlets: Some(4),
    },
    Product {
        name: "Rack PDU 8×C13",
        description: "2300 W / 10 A · 8 C13 outlets · feed from UPS or mains",
        section: ShopSection::Pdu,
        purchase: Purchase::Equipment(DeviceTemplate::Pdu),
        ports: None,
        outlets: Some(8),
    },
    Product {
        name: "24-port RJ45 patch panel",
        description: "24 passive ports · front/rear paired",
        section: ShopSection::Cabling,
        purchase: Purchase::Equipment(DeviceTemplate::PatchPanel),
        ports: Some(24),
        outlets: None,
    },
    Product {
        name: "Horizontal cable manager",
        description: "Front routing anchors · no network logic",
        section: ShopSection::Cabling,
        purchase: Purchase::Equipment(DeviceTemplate::CableManager),
        ports: None,
        outlets: None,
    },
    Product {
        name: "305 m Ethernet cable box",
        description: "Bulk cable stock for new patch leads",
        section: ShopSection::Cabling,
        purchase: Purchase::Supply(CableSupply::CableBox305m),
        ports: None,
        outlets: None,
    },
    Product {
        name: "20 × RJ45 connectors",
        description: "Two plugs are needed for each new Ethernet lead",
        section: ShopSection::Cabling,
        purchase: Purchase::Supply(CableSupply::Rj45Pack20),
        ports: None,
        outlets: None,
    },
];

pub(super) fn show(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut ShopState,
    actions: &mut MessageWriter<UiAction>,
) {
    if !state.open {
        return;
    }
    let mut open = state.open;
    egui::Window::new("Equipment shop")
        .id(egui::Id::new("equipment-shop-window"))
        .open(&mut open)
        .resizable(true)
        .default_size(egui::vec2(760.0, 540.0))
        .min_size(egui::vec2(620.0, 360.0))
        .show(viewport, |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("Available: ${}", sim.money));
                ui.separator();
                ui.label("Purchased equipment goes to inventory.");
            });
            ui.separator();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(135.0);
                    categories(ui, state);
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.set_min_width(420.0);
                    filters(ui, state);
                    let products: Vec<_> = PRODUCTS
                        .iter()
                        .filter(|product| product.matches(state, sim.money))
                        .collect();
                    let parts: Vec<_> = server_catalog().parts.iter().filter(|part| part_matches(part, state, sim.money)).collect();
                    let drives: Vec<_> = drive_catalog().drives.iter().filter(|drive| drive_matches(drive, state, sim.money)).collect();
                    let uplink_offers: Vec<_> = sim.network_outlets().filter(|outlet| {
                        matches!(outlet.kind, NetworkOutletKind::Uplink { .. })
                            && state.category == ShopCategory::Network
                            && state.section.is_none_or(|section| section == ShopSection::PublicIp)
                            && state.rack_units.is_none() && state.ports.is_none() && state.outlets.is_none()
                            && (state.search.trim().is_empty() || format!("public IPv4 /29 range {}", sim.port(outlet.port).map_or("", |port| port.name.as_str())).to_lowercase().contains(&state.search.trim().to_lowercase()))
                            && (!state.affordable_only || sim.money >= PublicIpv4Block::PRICE)
                            && state.max_price.is_none_or(|max| PublicIpv4Block::PRICE <= max)
                    }).collect();
                    ui.label(format!(
                        "{} product{}",
                        products.len() + parts.len() + drives.len() + uplink_offers.len(),
                        if products.len() + parts.len() + drives.len() + uplink_offers.len() == 1 { "" } else { "s" }
                    ));
                    egui::ScrollArea::vertical()
                        .id_salt("shop-products")
                        .max_height(ui.available_height().max(120.0))
                        .show(ui, |ui| {
                            if products.is_empty() && parts.is_empty() && drives.is_empty() && uplink_offers.is_empty() {
                                ui.weak("No products match these filters.");
                                if ui.button("Clear filters").clicked() {
                                    state.clear_filters();
                                }
                            }
                            for product in products {
                                ui.group(|ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.strong(product.name);
                                    ui.label(product.description);
                                    ui.horizontal(|ui| {
                                        if let Some(units) = product.rack_units() {
                                            ui.weak(format!("{units}U"));
                                        }
                                        if ui
                                            .add_enabled(
                                                sim.money >= product.price(),
                                                egui::Button::new(format!(
                                                    "{} — ${}",
                                                    if matches!(product.purchase, Purchase::ServerChassis) { "Buy chassis" } else { "Buy" },
                                                    product.price()
                                                )),
                                            )
                                            .clicked()
                                        {
                                            actions.write(product.buy());
                                        }
                                        if matches!(product.purchase, Purchase::ServerChassis)
                                            && ui.add_enabled(
                                                sim.money >= ServerFullPack::price(),
                                                egui::Button::new(format!(
                                                    "Order full pack — ${}",
                                                    ServerFullPack::price()
                                                )),
                                            ).on_hover_text("Chassis, CPU, 16 GB RAM, 4-port network card and 960 GB SSD; PSU included")
                                            .clicked()
                                        {
                                            actions.write(UiAction::BuyServerFullPack);
                                        }
                                    });
                                    if matches!(product.purchase, Purchase::ServerChassis) {
                                        ui.weak("Full pack: CPU, 16 GB RAM, 4-port NIC and 960 GB SSD. Integrated PSU included.");
                                    }
                                });
                            }
                            for outlet in uplink_offers {
                                ui.group(|ui| {
                                    let name = sim.port(outlet.port).map_or("UPLINK", |port| port.name.as_str());
                                    ui.strong(format!("Public IPv4 /29 · {name}"));
                                    ui.weak("Five server addresses and one provider gateway. The range is routed to this uplink port.");
                                    let count = sim.public_ipv4_blocks().iter().filter(|block| block.uplink == outlet.port).count();
                                    ui.label(format!("{count} range{} on {name}", if count == 1 { "" } else { "s" }));
                                    if ui.add_enabled(sim.money >= PublicIpv4Block::PRICE && sim.public_ipv4_blocks().len() < 32,
                                        egui::Button::new(format!("Order on {name} — ${}", PublicIpv4Block::PRICE))).clicked() {
                                        actions.write(UiAction::BuyPublicIpv4Block { uplink: outlet.port });
                                    }
                                });
                            }
                            {
                                for part in parts {
                                    ui.group(|ui| {
                                        ui.set_min_width(ui.available_width());
                                        ui.strong(&part.name);
                                        ui.weak(part_description(part));
                                        if ui.add_enabled(sim.money >= part.price, egui::Button::new(format!("Buy — ${}", part.price))).clicked() {
                                            actions.write(UiAction::BuyServerPart(part.id.clone()));
                                        }
                                    });
                                }
                            }
                            for drive in drives {
                                ui.group(|ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.strong(&drive.name);
                                    ui.weak(format!("{} GB · {} · {} · read {} MB/s / {} IOPS · write {} MB/s / {} IOPS",
                                        drive.capacity_gb, if drive.kind == cloud_provider_sim::DriveKind::Ssd { "SSD" } else { "HDD" }, drive.interface,
                                        drive.read_mb_s, drive.read_iops, drive.write_mb_s, drive.write_iops));
                                    if ui.add_enabled(sim.money >= drive.price, egui::Button::new(format!("Buy — ${}", drive.price))).clicked() {
                                        actions.write(UiAction::BuyDrive(drive.id.clone()));
                                    }
                                });
                            }
                        });
                });
            });
        });
    state.open = open;
}

fn categories(ui: &mut egui::Ui, state: &mut ShopState) {
    for (category, label, sections) in [
        (
            ShopCategory::Network,
            "Network",
            &[
                (ShopSection::Routers, "Routers"),
                (ShopSection::Switches, "Switches"),
                (ShopSection::Cabling, "Cabling"),
                (ShopSection::PublicIp, "Public IPv4"),
            ][..],
        ),
        (
            ShopCategory::Compute,
            "Compute",
            &[
                (ShopSection::DellServers, "DELL servers"),
                (ShopSection::Cpu, "CPU"),
                (ShopSection::Ram, "RAM"),
                (ShopSection::PciCards, "PCIe cards"),
                (ShopSection::Storage, "Storage drives"),
            ][..],
        ),
        (
            ShopCategory::Power,
            "Power",
            &[(ShopSection::Ups, "UPS"), (ShopSection::Pdu, "PDU")][..],
        ),
    ] {
        if ui
            .selectable_label(state.category == category && state.section.is_none(), label)
            .clicked()
        {
            state.select(category, None);
        }
        ui.indent(label, |ui| {
            for &(section, label) in sections {
                if ui
                    .selectable_label(state.section == Some(section), label)
                    .clicked()
                {
                    state.select(category, Some(section));
                }
            }
        });
        ui.add_space(8.0);
    }
}

fn filters(ui: &mut egui::Ui, state: &mut ShopState) {
    ui.add(egui::TextEdit::singleline(&mut state.search).hint_text("Search products"));
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut state.affordable_only, "Affordable only");
        let mut limited = state.max_price.is_some();
        if ui.checkbox(&mut limited, "Price limit").changed() {
            state.max_price = limited.then_some(1000);
        }
        if let Some(price) = &mut state.max_price {
            ui.add(
                egui::DragValue::new(price)
                    .range(0..=100_000)
                    .prefix("$")
                    .speed(25),
            );
        }
        if ui.small_button("Reset").clicked() {
            state.clear_filters();
        }
    });
    ui.horizontal_wrapped(|ui| {
        count_filter(ui, "Rack size", &mut state.rack_units, &[1, 2], "U");
        match state.category {
            ShopCategory::Network => {
                count_filter(ui, "RJ45 ports", &mut state.ports, &[8, 10, 16, 24, 48], "")
            }
            ShopCategory::Power => count_filter(ui, "C13 outlets", &mut state.outlets, &[4, 8], ""),
            ShopCategory::Compute => {}
        }
    });
    ui.separator();
}

fn count_filter<T: Copy + PartialEq + std::fmt::Display>(
    ui: &mut egui::Ui,
    label: &str,
    selected: &mut Option<T>,
    values: &[T],
    suffix: &str,
) {
    ui.label(label);
    egui::ComboBox::from_id_salt(label)
        .selected_text(selected.map_or_else(|| "Any".into(), |count| format!("{count}{suffix}")))
        .show_ui(ui, |ui| {
            ui.selectable_value(selected, None, "Any");
            for &count in values {
                ui.selectable_value(selected, Some(count), format!("{count}{suffix}"));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_range_order_targets_the_chosen_uplink_port() {
        use bevy::{
            ecs::system::SystemState,
            prelude::{Messages, World},
        };
        let sim = NetworkSim::new();
        let uplink = sim
            .network_outlets()
            .find(|outlet| {
                sim.port(outlet.port)
                    .is_some_and(|port| port.name == "UPLINK 2")
            })
            .unwrap()
            .port;
        let ctx = egui::Context::default();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut system = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut state = ShopState {
            open: true,
            category: ShopCategory::Network,
            section: Some(ShopSection::PublicIp),
            ..Default::default()
        };
        let label = format!("Order on UPLINK 2 — ${}", PublicIpv4Block::PRICE);
        let mut position = None;
        for _ in 0..2 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 800.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    show(
                        ui,
                        &sim,
                        &mut state,
                        &mut system.get_mut(&mut world).unwrap(),
                    );
                },
            );
            position = output.shapes.iter().find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.text() == label
                {
                    return Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center());
                }
                None
            });
            output.textures_delta.clear();
        }
        let position = position.expect("UPLINK 2 purchase button is visible");
        for pressed in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| {
                    show(
                        ui,
                        &sim,
                        &mut state,
                        &mut system.get_mut(&mut world).unwrap(),
                    );
                },
            );
            output.textures_delta.clear();
        }
        let purchases: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        assert_eq!(purchases.len(), 1);
        assert!(
            matches!(purchases[0], UiAction::BuyPublicIpv4Block { uplink: chosen } if chosen == uplink)
        );
    }

    #[test]
    fn shop_window_filters_products_and_buy_emits_the_selected_purchase() {
        use bevy::{
            ecs::system::SystemState,
            prelude::{Messages, World},
        };
        let sim = NetworkSim::new();
        let ctx = egui::Context::default();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut system = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut state = ShopState {
            open: true,
            section: Some(ShopSection::Switches),
            ports: Some(24),
            ..Default::default()
        };
        let mut buy_position = None;
        for _ in 0..2 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 800.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    show(
                        ui,
                        &sim,
                        &mut state,
                        &mut system.get_mut(&mut world).unwrap(),
                    )
                },
            );
            buy_position = output.shapes.iter().find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.text() == "Buy — $500"
                {
                    return Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center());
                }
                None
            });
            output.textures_delta.clear();
        }
        assert_eq!(world.resource::<Messages<UiAction>>().len(), 0);
        let position = buy_position.expect("filtered switch has a visible buy button");
        for pressed in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| {
                    show(
                        ui,
                        &sim,
                        &mut state,
                        &mut system.get_mut(&mut world).unwrap(),
                    )
                },
            );
            output.textures_delta.clear();
        }
        let purchases: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        assert_eq!(purchases.len(), 1);
        assert!(matches!(
            purchases[0],
            UiAction::Buy(DeviceTemplate::Switch)
        ));
        state.select(ShopCategory::Compute, Some(ShopSection::DellServers));
        state.clear_filters();
        state.section = Some(ShopSection::DellServers);
        let label = format!("Order full pack — ${}", ServerFullPack::price());
        let mut full_pack_position = None;
        for _ in 0..2 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 800.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    show(
                        ui,
                        &sim,
                        &mut state,
                        &mut system.get_mut(&mut world).unwrap(),
                    )
                },
            );
            full_pack_position = output.shapes.iter().find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.text() == label
                {
                    return Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center());
                }
                None
            });
            output.textures_delta.clear();
        }
        let position = full_pack_position.expect("full pack order button is visible");
        for pressed in [true, false] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| {
                    show(
                        ui,
                        &sim,
                        &mut state,
                        &mut system.get_mut(&mut world).unwrap(),
                    )
                },
            );
            output.textures_delta.clear();
        }
        let purchases: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        assert_eq!(purchases.len(), 1);
        assert!(matches!(purchases[0], UiAction::BuyServerFullPack));
        state.open = false;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            show(
                ui,
                &sim,
                &mut state,
                &mut system.get_mut(&mut world).unwrap(),
            );
        });
        output.textures_delta.clear();
        assert_eq!(world.resource::<Messages<UiAction>>().len(), 0);
    }

    #[test]
    fn filters_combine_category_ports_price_and_search() {
        let mut state = ShopState {
            section: Some(ShopSection::Switches),
            ports: Some(24),
            ..Default::default()
        };
        let matches = || {
            PRODUCTS
                .iter()
                .filter(|product| product.matches(&state, 6000))
                .count()
        };
        assert_eq!(matches(), 1);
        state.ports = Some(48);
        assert!(
            PRODUCTS
                .iter()
                .all(|product| !product.matches(&state, 6000))
        );
        state.ports = Some(24);
        state.max_price = Some(499);
        assert!(
            PRODUCTS
                .iter()
                .all(|product| !product.matches(&state, 6000))
        );
        state.max_price = None;
        state.search = " C1000 ".into();
        assert_eq!(
            PRODUCTS
                .iter()
                .filter(|product| product.matches(&state, 6000))
                .count(),
            1
        );
        state.affordable_only = true;
        assert!(PRODUCTS.iter().all(|product| !product.matches(&state, 499)));
    }

    #[test]
    fn power_filters_and_category_changes_do_not_leak() {
        let mut state = ShopState {
            category: ShopCategory::Power,
            outlets: Some(4),
            rack_units: Some(2),
            ..Default::default()
        };
        let products: Vec<_> = PRODUCTS
            .iter()
            .filter(|product| product.matches(&state, 6000))
            .collect();
        assert_eq!(products.len(), 1);
        assert!(matches!(
            products[0].buy(),
            UiAction::Buy(DeviceTemplate::Ups)
        ));
        state.select(ShopCategory::Compute, Some(ShopSection::DellServers));
        assert_eq!(state.outlets, None);
        state.clear_filters();
        let products: Vec<_> = PRODUCTS
            .iter()
            .filter(|product| product.matches(&state, 6000))
            .collect();
        assert_eq!(products.len(), 1);
        assert!(matches!(products[0].buy(), UiAction::BuyServerChassis));
    }

    #[test]
    fn storage_section_lists_drives_separately_from_server_parts() {
        let mut state = ShopState {
            category: ShopCategory::Compute,
            section: Some(ShopSection::Storage),
            ..Default::default()
        };
        assert!(
            server_catalog()
                .parts
                .iter()
                .all(|part| !part_matches(part, &state, 6000))
        );
        assert_eq!(
            drive_catalog()
                .drives
                .iter()
                .filter(|drive| drive_matches(drive, &state, 6000))
                .count(),
            2
        );
        state.search = "SSD".into();
        assert_eq!(
            drive_catalog()
                .drives
                .iter()
                .filter(|drive| drive_matches(drive, &state, 6000))
                .count(),
            1
        );
    }

    #[test]
    fn compute_sections_only_show_their_server_parts() {
        let catalog = server_catalog();
        let mut state = ShopState {
            category: ShopCategory::Compute,
            ..Default::default()
        };
        assert_eq!(
            catalog
                .parts
                .iter()
                .filter(|part| part_matches(part, &state, 6000))
                .count(),
            catalog.parts.len()
        );
        for section in [
            ShopSection::DellServers,
            ShopSection::Cpu,
            ShopSection::Ram,
            ShopSection::PciCards,
        ] {
            state.section = Some(section);
            let matching: Vec<_> = catalog
                .parts
                .iter()
                .filter(|part| part_matches(part, &state, 6000))
                .collect();
            if section == ShopSection::DellServers {
                assert!(matching.is_empty());
            } else {
                assert_eq!(matching.len(), 1, "{section:?}");
            }
        }
    }
}
