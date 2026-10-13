use super::*;

pub(super) fn open_shop(
    sim: &NetworkSim,
    state: &mut ShopState,
    device: DeviceId,
    socket: ServerSocket,
) {
    let Some(device_data) = sim.device(device) else {
        return;
    };
    let DeviceKind::Server(server) = &device_data.kind else {
        return;
    };
    let chassis = &server.model().chassis;
    state.select(ShopCategory::Compute, Some(section(socket)));
    state.clear_filters();
    let facet = match socket {
        ServerSocket::Cpu(_) => Some(("socket", vec![chassis.cpu_socket.clone()])),
        ServerSocket::Dimm(_) => Some((
            "memory-type",
            std::iter::once(chassis.memory_type.clone())
                .chain(chassis.additional_memory_types.clone())
                .collect(),
        )),
        ServerSocket::Drive(index) => chassis
            .drive_bays
            .get(index)
            .map(|bay| ("interface", vec![bay.interface.clone()])),
        ServerSocket::Pcie(_) => None,
    };
    if let Some((key, values)) = facet {
        state
            .facets
            .insert(key.into(), values.into_iter().collect());
    }
    state.target = Some(ShopTarget::Socket(device, socket));
    state.compatible_only = true;
    state.selected_offer = None;
    state.details_open = false;
    state.comparison_open = false;
    state.open = true;
}

pub(in crate::ui) fn picker(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut UiState,
    actions: &mut MessageWriter<UiAction>,
) {
    let Some((device, socket)) = state.hardware_picker else {
        return;
    };
    let Some(device_data) = sim.device(device) else {
        state.hardware_picker = None;
        return;
    };
    let DeviceKind::Server(server) = &device_data.kind else {
        state.hardware_picker = None;
        return;
    };
    let Some(hardware) = &server.hardware else {
        state.hardware_picker = None;
        return;
    };
    let editable = !device_data.powered || server.model().chassis.parts_hot_swappable;
    let installed = sim.server_socket_item(device, socket);
    let mut open = true;
    let mut close = false;
    egui::Window::new(format!(
        "{} · {}",
        crate::localization::device_name(device_data),
        socket_name(sim, device, socket)
    ))
    .id(egui::Id::new("server-hardware-picker"))
    .open(&mut open)
    .default_width(460.0)
    .resizable(true)
    .show(ui.ctx(), |ui| {
        if let Some(id) = installed {
            ui.label(crate::localization::item_name(id, id));
            // CPUs are stored in physical order; removing an earlier CPU would shift sockets.
            let removable =
                !matches!(socket, ServerSocket::Cpu(index) if index + 1 < hardware.cpus.len());
            if ui
                .add_enabled(
                    (editable || matches!(socket, ServerSocket::Drive(_))) && removable,
                    egui::Button::new(tr("ui.remove")),
                )
                .on_disabled_hover_text(if !removable {
                    tr("hardware.remove-later-cpu")
                } else {
                    tr("hardware.power-off-parts")
                })
                .clicked()
            {
                match socket {
                    ServerSocket::Drive(bay) => {
                        actions.write(UiAction::RemoveDrive { device, bay });
                    }
                    ServerSocket::Cpu(index)
                    | ServerSocket::Dimm(index)
                    | ServerSocket::Pcie(index) => {
                        actions.write(UiAction::RemoveServerPart {
                            device,
                            part_id: id.into(),
                            slot: Some(index),
                        });
                    }
                }
            }
            ui.weak(tr("hardware.remove-first"));
            ui.separator();
        }
        ui.strong(tr("hardware.socket-inventory"));
        let candidates = inventory(sim, device, socket);
        if candidates.is_empty() {
            ui.weak(tr("hardware.no-compatible-inventory"));
        }
        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                for (id, name, count) in candidates {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{name} ×{count}"));
                        let result = match socket {
                            ServerSocket::Drive(index) => sim
                                .drive_installation_bay(device, &id, Some(index))
                                .map(|_| ()),
                            ServerSocket::Cpu(index)
                            | ServerSocket::Dimm(index)
                            | ServerSocket::Pcie(index) => sim
                                .server_part_installation_slot(device, &id, Some(index))
                                .map(|_| ()),
                        };
                        let enabled = installed.is_none()
                            && result.is_ok()
                            && (editable || matches!(socket, ServerSocket::Drive(_)));
                        if ui
                            .add_enabled(enabled, egui::Button::new(tr("ui.install")))
                            .on_disabled_hover_text(if installed.is_some() {
                                tr("hardware.remove-first")
                            } else if !editable && !matches!(socket, ServerSocket::Drive(_)) {
                                tr("hardware.power-off-parts")
                            } else {
                                result
                                    .err()
                                    .map(|error| error.to_string())
                                    .unwrap_or_default()
                            })
                            .clicked()
                        {
                            match socket {
                                ServerSocket::Drive(bay) => {
                                    actions.write(UiAction::InstallDrive {
                                        device,
                                        drive_id: id,
                                        bay: Some(bay),
                                    });
                                }
                                ServerSocket::Cpu(index)
                                | ServerSocket::Dimm(index)
                                | ServerSocket::Pcie(index) => {
                                    actions.write(UiAction::InstallServerPart {
                                        device,
                                        part_id: id,
                                        slot: Some(index),
                                    });
                                }
                            }
                            close = true;
                        }
                    });
                }
            });
        ui.separator();
        if ui.button(tr("hardware.browse-compatible-shop")).clicked() {
            open_shop(sim, &mut state.shop, device, socket);
            close = true;
        }
    });
    if !open || close || ui.input(|input| input.key_pressed(egui::Key::Escape)) {
        state.hardware_picker = None;
    }
}

fn inventory(
    sim: &NetworkSim,
    device: DeviceId,
    socket: ServerSocket,
) -> Vec<(String, String, u32)> {
    match socket {
        ServerSocket::Drive(_) => drive_catalog()
            .drives
            .iter()
            .filter_map(|item| {
                let count = sim.drive_inventory.get(&item.id).copied().unwrap_or(0);
                (count > 0
                    && sim
                        .server_socket_compatibility(device, socket, &item.id)
                        .is_ok())
                .then(|| {
                    (
                        item.id.clone(),
                        crate::localization::item_name(&item.id, &item.name),
                        count,
                    )
                })
            })
            .collect(),
        _ => server_catalog()
            .parts
            .iter()
            .filter_map(|item| {
                let count = sim.server_parts.get(&item.id).copied().unwrap_or(0);
                (count > 0
                    && sim
                        .server_socket_compatibility(device, socket, &item.id)
                        .is_ok())
                .then(|| {
                    (
                        item.id.clone(),
                        crate::localization::item_name(&item.id, &item.name),
                        count,
                    )
                })
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(configuration: ServerConfiguration) -> (NetworkSim, DeviceId) {
        let mut sim = NetworkSim::new();
        sim.money = 1_000_000;
        let SimEvent::DeviceAdded(device) = sim
            .execute(cloud_provider_sim::Command::BuyServer {
                model_id: "dell_r760".into(),
                configuration,
            })
            .unwrap()[0]
        else {
            panic!()
        };
        (sim, device)
    }

    #[test]
    fn socket_click_selects_physical_cpu_and_shop_clears_stale_filters() {
        let (sim, device) = server(ServerConfiguration::Chassis);
        let ctx = egui::Context::default();
        let mut state = UiState::default();
        let mut frame = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    socket_tile(ui, &sim, device, ServerSocket::Cpu(0), &mut state);
                },
            );
            output.textures_delta.clear();
            output
        };
        let output = frame(vec![]);
        crate::ui::test_windows::click_text(&output, "CPU 1", &mut frame);
        assert_eq!(state.hardware_picker, Some((device, ServerSocket::Cpu(0))));
        state.shop.search = "wrong model".into();
        state.shop.max_price = Some(1);
        state
            .shop
            .facets
            .insert("memory-type".into(), ["DDR4".into()].into());
        open_shop(&sim, &mut state.shop, device, ServerSocket::Cpu(0));
        assert!(state.shop.open && state.shop.compatible_only);
        assert!(state.shop.search.is_empty());
        assert_eq!(state.shop.max_price, None);
        assert_eq!(state.shop.section, Some(ShopSection::Cpu));
        assert_eq!(
            state.shop.target,
            Some(ShopTarget::Socket(device, ServerSocket::Cpu(0)))
        );
        assert_eq!(state.shop.facets["socket"], ["LGA4677".into()].into());
        assert!(!state.shop.facets.contains_key("memory-type"));
    }

    #[test]
    fn inventory_groups_compatible_stock_and_supports_occupied_socket_replacements() {
        let (mut sim, device) = server(ServerConfiguration::FullPack);
        let cpu = sim
            .server_socket_item(device, ServerSocket::Cpu(0))
            .unwrap()
            .to_owned();
        sim.server_parts.insert(cpu.clone(), 3);
        sim.server_parts.insert("xeon_e_2434".into(), 4);
        let items = inventory(&sim, device, ServerSocket::Cpu(0));
        assert_eq!(items.len(), 1);
        assert_eq!((&items[0].0, items[0].2), (&cpu, 3));
        assert!(inventory(&sim, device, ServerSocket::Dimm(0)).is_empty());
        open_shop(
            &sim,
            &mut ShopState::default(),
            device,
            ServerSocket::Cpu(0),
        );
        sim.drive_inventory.insert("enterprise_ssd_960gb".into(), 2);
        let drives = inventory(&sim, device, ServerSocket::Drive(0));
        assert_eq!(drives.len(), 1);
        assert_eq!(drives[0].2, 2);
    }

    #[test]
    fn picker_install_click_emits_the_selected_socket_and_shop_click_sets_filters() {
        let (mut sim, device) = server(ServerConfiguration::Chassis);
        let cpu = server_catalog()
            .model("dell_r760")
            .unwrap()
            .chassis
            .supported_cpu_ids[0]
            .clone();
        sim.server_parts.insert(cpu.clone(), 2);
        let ctx = egui::Context::default();
        let mut world = World::new();
        world.init_resource::<Messages<UiAction>>();
        let mut writer = bevy::ecs::system::SystemState::<MessageWriter<UiAction>>::new(&mut world);
        let mut state = UiState {
            hardware_picker: Some((device, ServerSocket::Cpu(0))),
            ..Default::default()
        };
        let mut frame = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 900.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    picker(
                        ui,
                        &sim,
                        &mut state,
                        &mut writer.get_mut(&mut world).unwrap(),
                    )
                },
            );
            output.textures_delta.clear();
            output
        };
        frame(vec![]);
        let output = frame(vec![]);
        crate::ui::test_windows::click_text(&output, &tr("ui.install"), &mut frame);
        assert!(state.hardware_picker.is_none());
        let actions: Vec<_> = world.resource_mut::<Messages<UiAction>>().drain().collect();
        assert!(
            matches!(actions.as_slice(), [UiAction::InstallServerPart { device: id, part_id, slot: Some(0) }] if *id == device && *part_id == cpu)
        );
        state.hardware_picker = Some((device, ServerSocket::Dimm(0)));
        let mut frame = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 900.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    picker(
                        ui,
                        &sim,
                        &mut state,
                        &mut writer.get_mut(&mut world).unwrap(),
                    )
                },
            );
            output.textures_delta.clear();
            output
        };
        frame(vec![]);
        let output = frame(vec![]);
        crate::ui::test_windows::click_text(
            &output,
            &tr("hardware.browse-compatible-shop"),
            &mut frame,
        );
        assert!(state.hardware_picker.is_none());
        assert!(state.shop.open && state.shop.compatible_only);
        assert_eq!(state.shop.section, Some(ShopSection::Ram));
        assert_eq!(
            state.shop.target,
            Some(ShopTarget::Socket(device, ServerSocket::Dimm(0)))
        );
        assert_eq!(
            state.shop.facets["memory-type"],
            ["DDR5 ECC RDIMM".into()].into()
        );
    }

    #[test]
    fn each_socket_shop_keeps_its_target_in_the_target_controls() {
        let (sim, device) = server(ServerConfiguration::FullPack);
        for socket in [
            ServerSocket::Cpu(0),
            ServerSocket::Dimm(0),
            ServerSocket::Pcie(0),
            ServerSocket::Drive(0),
        ] {
            let mut state = ShopState::default();
            open_shop(&sim, &mut state, device, socket);
            let ctx = egui::Context::default();
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                crate::ui::shop::filters::compatibility_target(
                    ui,
                    &mut state,
                    &sim,
                    Selection::Device(device),
                );
            });
            output.textures_delta.clear();
            assert_eq!(state.target, Some(ShopTarget::Socket(device, socket)));
            assert!(state.compatible_only);
        }
    }
}
