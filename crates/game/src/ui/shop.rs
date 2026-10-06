use crate::app::{ShopCategory, ShopSection, ShopState, UiAction};
use crate::localization::tr;
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::{
    CableSupply, DeviceTemplate, DriveModel, NetworkSim, PublicIpv4Block, ServerFullPack,
    ServerPart, ServerPartKind, drive_catalog, server_catalog,
};

fn drive_matches(drive: &DriveModel, state: &ShopState, money: i64) -> bool {
    state.category == ShopCategory::Compute
        && state.section.is_none_or(|s| s == ShopSection::Storage)
        && state.rack_units.is_none()
        && state.ports.is_none()
        && state.outlets.is_none()
        && (state.search.trim().is_empty()
            || format!(
                "{} {} {} {}",
                drive.name,
                crate::localization::item_name(&drive.id, &drive.name),
                drive.id,
                crate::localization::item_description(&drive.id)
            )
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
            || format!(
                "{} {} {} {}",
                part.name,
                crate::localization::item_name(&part.id, &part.name),
                part.id,
                crate::localization::item_description(&part.id)
            )
            .to_lowercase()
            .contains(&state.search.trim().to_lowercase()))
        && (!state.affordable_only || part.price <= money)
        && state.max_price.is_none_or(|max| part.price <= max)
}

#[derive(Clone, Copy)]
enum Purchase {
    Equipment(DeviceTemplate),
    ServerChassis,
    Supply(CableSupply),
}

struct Product {
    id: &'static str,
    section: ShopSection,
    purchase: Purchase,
    ports: Option<u16>,
    outlets: Option<u16>,
}

impl Product {
    fn name(&self) -> String {
        crate::localization::item_name(self.id, self.id)
    }
    fn description(&self) -> String {
        crate::localization::item_description(self.id)
    }
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
                || format!("{} {} {}", self.id, self.name(), self.description())
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
        id: "cisco_isr_c1111",
        section: ShopSection::Routers,
        purchase: Purchase::Equipment(DeviceTemplate::Router),
        ports: Some(10),
        outlets: None,
    },
    Product {
        id: "cisco_catalyst_c1000",
        section: ShopSection::Switches,
        purchase: Purchase::Equipment(DeviceTemplate::Switch),
        ports: Some(24),
        outlets: None,
    },
    Product {
        id: "dell_r360",
        section: ShopSection::DellServers,
        purchase: Purchase::ServerChassis,
        ports: Some(3),
        outlets: None,
    },
    Product {
        id: "apc_smt1500",
        section: ShopSection::Ups,
        purchase: Purchase::Equipment(DeviceTemplate::Ups),
        ports: None,
        outlets: Some(4),
    },
    Product {
        id: "rack_pdu",
        section: ShopSection::Pdu,
        purchase: Purchase::Equipment(DeviceTemplate::Pdu),
        ports: None,
        outlets: Some(8),
    },
    Product {
        id: "patch_panel",
        section: ShopSection::Cabling,
        purchase: Purchase::Equipment(DeviceTemplate::PatchPanel),
        ports: Some(24),
        outlets: None,
    },
    Product {
        id: "cable_manager",
        section: ShopSection::Cabling,
        purchase: Purchase::Equipment(DeviceTemplate::CableManager),
        ports: None,
        outlets: None,
    },
    Product {
        id: "ethernet_cable_box",
        section: ShopSection::Cabling,
        purchase: Purchase::Supply(CableSupply::CableBox305m),
        ports: None,
        outlets: None,
    },
    Product {
        id: "rj45_connectors",
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
    egui::Window::new(tr("shop.title"))
        .id(egui::Id::new("equipment-shop-window"))
        .open(&mut open)
        .resizable(true)
        .default_size(egui::vec2(760.0, 540.0))
        .min_size(egui::vec2(620.0, 360.0))
        .show(viewport, |ui| {
            ui.horizontal(|ui| {
                ui.strong(crate::localization::tr_args(
                    "ui.available",
                    &[(sim.money).to_string()],
                ));
                ui.separator();
                ui.label(tr("ui.purchased-equipment-goes-to-inventory"));
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
                    let parts: Vec<_> = server_catalog()
                        .parts
                        .iter()
                        .filter(|part| part_matches(part, state, sim.money))
                        .collect();
                    let drives: Vec<_> = drive_catalog()
                        .drives
                        .iter()
                        .filter(|drive| drive_matches(drive, state, sim.money))
                        .collect();
                    let public_pool_offer = state.category == ShopCategory::Network
                        && state
                            .section
                            .is_none_or(|section| section == ShopSection::PublicIp)
                        && state.rack_units.is_none()
                        && state.ports.is_none()
                        && state.outlets.is_none()
                        && (state.search.trim().is_empty()
                            || tr("ui.public-ipv4-29-address-pool.2")
                                .to_lowercase()
                                .contains(&state.search.trim().to_lowercase()))
                        && (!state.affordable_only || sim.money >= PublicIpv4Block::PRICE)
                        && state
                            .max_price
                            .is_none_or(|max| PublicIpv4Block::PRICE <= max);
                    ui.label(crate::localization::tr_args(
                        "ui.product",
                        &[(products.len()
                            + parts.len()
                            + drives.len()
                            + usize::from(public_pool_offer))
                        .to_string()],
                    ));
                    egui::ScrollArea::vertical()
                        .id_salt("shop-products")
                        .max_height(ui.available_height().max(120.0))
                        .show(ui, |ui| {
                            if products.is_empty()
                                && parts.is_empty()
                                && drives.is_empty()
                                && !public_pool_offer
                            {
                                ui.weak(tr("ui.no-products-match-these-filters"));
                                if ui.button(tr("ui.clear-filters")).clicked() {
                                    state.clear_filters();
                                }
                            }
                            for product in products {
                                ui.group(|ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.strong(product.name());
                                    ui.label(product.description());
                                    ui.horizontal(|ui| {
                                        if let Some(units) = product.rack_units() {
                                            ui.weak(crate::localization::tr_args(
                                                "rack.units",
                                                &[(units).to_string()],
                                            ));
                                        }
                                        if ui
                                            .add_enabled(
                                                sim.money >= product.price(),
                                                egui::Button::new(crate::localization::tr_args(
                                                    if matches!(
                                                        product.purchase,
                                                        Purchase::ServerChassis
                                                    ) {
                                                        "shop.buy-chassis"
                                                    } else {
                                                        "shop.buy"
                                                    },
                                                    &[product.price().to_string()],
                                                )),
                                            )
                                            .clicked()
                                        {
                                            actions.write(product.buy());
                                        }
                                        if matches!(product.purchase, Purchase::ServerChassis)
                                            && ui
                                                .add_enabled(
                                                    sim.money >= ServerFullPack::price(),
                                                    egui::Button::new(
                                                        crate::localization::tr_args(
                                                            "ui.order-full-pack",
                                                            &[(ServerFullPack::price())
                                                                .to_string()],
                                                        ),
                                                    ),
                                                )
                                                .on_hover_text(tr(
                                                    "ui.chassis-cpu-16-gb-ram-4-port",
                                                ))
                                                .clicked()
                                        {
                                            actions.write(UiAction::BuyServerFullPack);
                                        }
                                    });
                                    if matches!(product.purchase, Purchase::ServerChassis) {
                                        ui.weak(tr("ui.full-pack-cpu-16-gb-ram-4"));
                                    }
                                });
                            }
                            if public_pool_offer {
                                ui.group(|ui| {
                                    ui.strong(tr("ui.public-ipv4-29-address-pool"));
                                    ui.weak(tr("ui.open-ip-ranges-to-select-its-uplink"));
                                    let count = sim.public_ipv4_blocks().len();
                                    ui.label(crate::localization::tr_args(
                                        "ui.allocation-owned",
                                        &[(count).to_string()],
                                    ));
                                    if ui
                                        .add_enabled(
                                            sim.money >= PublicIpv4Block::PRICE && count < 32,
                                            egui::Button::new(crate::localization::tr_args(
                                                "ui.order-ipv4-pool",
                                                &[(PublicIpv4Block::PRICE).to_string()],
                                            )),
                                        )
                                        .clicked()
                                    {
                                        actions.write(UiAction::BuyPublicIpv4Pool);
                                    }
                                });
                            }
                            {
                                for part in parts {
                                    ui.group(|ui| {
                                        ui.set_min_width(ui.available_width());
                                        ui.strong(crate::localization::item_name(
                                            &part.id, &part.name,
                                        ));
                                        ui.weak(crate::localization::item_description(&part.id));
                                        if ui
                                            .add_enabled(
                                                sim.money >= part.price,
                                                egui::Button::new(crate::localization::tr_args(
                                                    "shop.buy",
                                                    &[(part.price).to_string()],
                                                )),
                                            )
                                            .clicked()
                                        {
                                            actions.write(UiAction::BuyServerPart(part.id.clone()));
                                        }
                                    });
                                }
                            }
                            for drive in drives {
                                ui.group(|ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.strong(crate::localization::item_name(
                                        &drive.id,
                                        &drive.name,
                                    ));
                                    ui.weak(crate::localization::item_description(&drive.id));
                                    ui.weak(crate::localization::tr_args(
                                        "ui.gb-read-mb-s-iops-write-mb",
                                        &[
                                            (drive.capacity_gb).to_string(),
                                            (if drive.kind == cloud_provider_sim::DriveKind::Ssd {
                                                "SSD"
                                            } else {
                                                "HDD"
                                            })
                                            .to_string(),
                                            (drive.interface).to_string(),
                                            (drive.read_mb_s).to_string(),
                                            (drive.read_iops).to_string(),
                                            (drive.write_mb_s).to_string(),
                                            (drive.write_iops).to_string(),
                                        ],
                                    ));
                                    if ui
                                        .add_enabled(
                                            sim.money >= drive.price,
                                            egui::Button::new(crate::localization::tr_args(
                                                "shop.buy",
                                                &[(drive.price).to_string()],
                                            )),
                                        )
                                        .clicked()
                                    {
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
            "ui.network",
            &[
                (ShopSection::Routers, "ui.routers"),
                (ShopSection::Switches, "ui.switches"),
                (ShopSection::Cabling, "ui.cabling"),
                (ShopSection::PublicIp, "ui.public-ipv4"),
            ][..],
        ),
        (
            ShopCategory::Compute,
            "ui.compute",
            &[
                (ShopSection::DellServers, "ui.dell-servers"),
                (ShopSection::Cpu, "ui.cpu"),
                (ShopSection::Ram, "ui.ram"),
                (ShopSection::PciCards, "ui.pcie-cards"),
                (ShopSection::Storage, "ui.storage-drives"),
            ][..],
        ),
        (
            ShopCategory::Power,
            "ui.power",
            &[(ShopSection::Ups, "ui.ups"), (ShopSection::Pdu, "ui.pdu")][..],
        ),
    ] {
        if ui
            .selectable_label(
                state.category == category && state.section.is_none(),
                tr(label),
            )
            .clicked()
        {
            state.select(category, None);
        }
        ui.indent(label, |ui| {
            for &(section, label) in sections {
                if ui
                    .selectable_label(state.section == Some(section), tr(label))
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
    ui.add(egui::TextEdit::singleline(&mut state.search).hint_text(tr("ui.search-products")));
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut state.affordable_only, tr("ui.affordable-only"));
        let mut limited = state.max_price.is_some();
        if ui.checkbox(&mut limited, tr("ui.price-limit")).changed() {
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
        if ui.small_button(tr("ui.reset")).clicked() {
            state.clear_filters();
        }
    });
    ui.horizontal_wrapped(|ui| {
        count_filter(ui, "ui.rack-size", &mut state.rack_units, &[1, 2], "U");
        match state.category {
            ShopCategory::Network => count_filter(
                ui,
                "ui.rj45-ports",
                &mut state.ports,
                &[8, 10, 16, 24, 48],
                "",
            ),
            ShopCategory::Power => {
                count_filter(ui, "ui.c13-outlets", &mut state.outlets, &[4, 8], "")
            }
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
    ui.label(tr(label));
    egui::ComboBox::from_id_salt(label)
        .selected_text(selected.map_or_else(
            || tr("ui.any"),
            |count| {
                crate::localization::tr_args(
                    "format.count-with-unit",
                    &[(count).to_string(), (suffix).to_string()],
                )
            },
        ))
        .show_ui(ui, |ui| {
            ui.selectable_value(selected, None, tr("ui.any"));
            for &count in values {
                ui.selectable_value(
                    selected,
                    Some(count),
                    crate::localization::tr_args(
                        "format.count-with-unit",
                        &[(count).to_string(), (suffix).to_string()],
                    ),
                );
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_pool_order_is_independent_of_uplink_ports() {
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
            category: ShopCategory::Network,
            section: Some(ShopSection::PublicIp),
            ..Default::default()
        };
        let label = format!("Order IPv4 pool — ${}", PublicIpv4Block::PRICE);
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
        let position = position.expect("IPv4 pool purchase button is visible");
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
        assert!(matches!(purchases[0], UiAction::BuyPublicIpv4Pool));
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

#[cfg(test)]
#[test]
fn russian_product_search_uses_localized_names_and_preserves_model_search() {
    let mut locale = crate::localization::Localization::default();
    locale.select("ru").unwrap();
    let _scope = locale.enter();
    let mut state = ShopState::default();
    state.select(ShopCategory::Network, Some(ShopSection::Cabling));
    state.search = "патч".into();
    assert!(
        PRODUCTS
            .iter()
            .any(|product| product.matches(&state, 100_000))
    );
    state.search = "кабель".into();
    assert!(
        PRODUCTS
            .iter()
            .any(|product| product.matches(&state, 100_000))
    );
    state.select(ShopCategory::Network, Some(ShopSection::Routers));
    state.search = "C1111".into();
    assert!(
        PRODUCTS
            .iter()
            .any(|product| product.matches(&state, 100_000))
    );
    state.select(ShopCategory::Compute, Some(ShopSection::Storage));
    state.search = "твердотельный".into();
    let matches: Vec<_> = drive_catalog()
        .drives
        .iter()
        .filter(|drive| drive_matches(drive, &state, 6000))
        .collect();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, "enterprise_ssd_960gb");
    assert_eq!(
        crate::localization::tr_args("shop.buy-chassis", &["500".into()]),
        "Купить корпус — $500"
    );
}

#[cfg(test)]
#[test]
fn shop_renders_item_owned_drive_descriptions_in_both_languages() {
    use bevy::{
        ecs::system::SystemState,
        prelude::{Messages, World},
    };
    for (language, expected) in [
        ("en", "Enterprise solid-state drive for fast storage."),
        (
            "ru",
            "Серверный твердотельный накопитель для быстрого хранения данных.",
        ),
    ] {
        let mut locale = crate::localization::Localization::default();
        locale.select(language).unwrap();
        let _scope = locale.enter();
        let ctx = egui::Context::default();
        let sim = NetworkSim::new();
        let mut state = ShopState {
            open: true,
            category: ShopCategory::Compute,
            section: Some(ShopSection::Storage),
            ..Default::default()
        };
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut system = SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut visible = false;
        for _ in 0..2 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 900.0),
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
            visible = output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == expected));
            output.textures_delta.clear();
        }
        assert!(visible, "missing {language} item description");
    }
}
