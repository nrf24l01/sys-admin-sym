use crate::app::*;
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::{Device, DeviceId, DeviceKind, NetworkSim};

pub struct TerminalRenderer;

impl TerminalRenderer {
    fn scroll_height(available_height: f32, controls_height: f32) -> f32 {
        (available_height - controls_height).max(0.0)
    }

    pub(super) fn output_scroll(ui: &egui::Ui, controls_height: f32) -> egui::ScrollArea {
        let height = Self::scroll_height(ui.available_height(), controls_height);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .min_scrolled_height(height)
            .max_height(height)
    }

    pub fn panel(
        viewport: &mut egui::Ui,
        sim: &NetworkSim,
        state: &mut UiState,
        actions: &mut MessageWriter<UiAction>,
    ) {
        // Existing terminal windows remain visible even when the rack selection is
        // a passive device. Passive hardware itself has no console dock or action.
        let windows = state.terminal_windows.iter().copied().collect::<Vec<_>>();
        for window_device in windows.iter().copied() {
            Self::window(viewport, sim, state, window_device, actions);
        }
        let device = match state.selected {
            Selection::Device(id) => sim.device(id).map(|d| d.id),
            Selection::Port(id) => sim.port(id).map(|p| p.device),
            _ => None,
        };
        let Some(device) = device.filter(|id| sim.device(*id).is_some_and(Self::is_console_device))
        else {
            return;
        };
        let dev = sim.device(device).unwrap();
        let server = matches!(dev.kind, DeviceKind::Server(_));
        if state.terminal_windows.contains(&device) {
            return;
        }
        let console = state.terminals.entry(device).or_default();
        egui::Panel::bottom("terminal")
        .resizable(true)
        .default_size(240.0)
        .show(viewport, |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!("{} — Console", dev.name));
                if ui.small_button("Open terminal window").clicked() {
                    actions.write(UiAction::LaunchExternalTerminal(device));
                }
                if ui.small_button("Clear output").clicked() { console.lines.clear(); }
                if !server { ui.checkbox(&mut console.script_mode, "Paste configuration"); }
            });
            if !server { ui.weak("? help · Up/Down history · Tab completion · Ctrl-Z end · Scripts stop at the first error"); }
            if !dev.powered { ui.colored_label(egui::Color32::YELLOW, "Power on this device in the Inspector to use its console."); }
            Self::output_scroll(ui, if console.script_mode { 120.0 } else { 70.0 })
                .id_salt(("console-output", device.0))
                .max_height(Self::scroll_height(
                    ui.available_height(),
                    if console.script_mode { 120.0 } else { 70.0 },
                ).max(60.0))
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if console.lines.is_empty() {
                        ui.monospace(if server { "Type help for server commands." } else { "IOS-style simulator console. Type enable, then configure terminal. Type ? to see supported commands." });
                    }
                    for line in &console.lines { ui.monospace(line); }
                });
            ui.horizontal(|ui| {
                ui.monospace(sim.terminal_prompt(device));
                let input_id = egui::Id::new(("console-input", device.0));
                let focused = ui.memory(|m| m.has_focus(input_id));
                if focused && !console.script_mode {
                    let (up, down, tab, end) = ui.input_mut(|i| (
                        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                        i.consume_key(egui::Modifiers::NONE, egui::Key::Tab),
                        i.consume_key(egui::Modifiers::CTRL, egui::Key::Z),
                    ));
                    if up && !console.history.is_empty() {
                        let pos = console.history_position.unwrap_or(console.history.len()).saturating_sub(1);
                        console.history_position = Some(pos);
                        console.input = console.history[pos].clone();
                    }
                    if down && let Some(pos) = console.history_position {
                        let next = pos + 1;
                        console.history_position = (next < console.history.len()).then_some(next);
                        console.input = console.history.get(next).cloned().unwrap_or_default();
                    }
                    if tab && !server {
                        let completions = sim.console_help(device, &console.input);
                        if completions.len() == 1 && !completions[0].contains('<') { console.input = completions[0].clone(); }
                        else { console.lines.extend(completions); }
                    }
                    if end && !server { actions.write(UiAction::RunTerminal(device, "end".into())); }
                }
                let editor = if console.script_mode {
                    egui::TextEdit::multiline(&mut console.input).desired_rows(3)
                } else { egui::TextEdit::singleline(&mut console.input) };
                let response = ui.add_enabled(dev.powered, editor
                    .id(input_id).font(egui::TextStyle::Monospace)
                    .desired_width((ui.available_width() - 65.0).max(80.0))
                    .hint_text(if server { "help" } else { "Enter command" }));
                let enter = !console.script_mode && response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.add_enabled(dev.powered, egui::Button::new("Run")).clicked() || (dev.powered && enter) {
                    let input = std::mem::take(&mut console.input);
                    if !input.trim().is_empty() {
                        console.history.push(input.clone());
                        if console.history.len() > 100 { console.history.remove(0); }
                    }
                    console.history_position = None;
                    actions.write(UiAction::RunTerminal(device, input));
                    response.request_focus();
                }
            });
        });
    }

    fn is_console_device(device: &Device) -> bool {
        matches!(
            device.kind,
            DeviceKind::Server(_) | DeviceKind::Switch(_) | DeviceKind::Router(_)
        )
    }

    fn window(
        viewport: &mut egui::Ui,
        sim: &NetworkSim,
        state: &mut UiState,
        device: DeviceId,
        actions: &mut MessageWriter<UiAction>,
    ) {
        let Some(dev) = sim.device(device) else {
            state.terminal_windows.remove(&device);
            state.terminal_window_focus.remove(&device);
            return;
        };
        if !Self::is_console_device(dev) {
            state.terminal_windows.remove(&device);
            state.terminal_window_focus.remove(&device);
            return;
        }
        let powered = dev.powered;
        let name = dev.name.clone();
        let server = matches!(dev.kind, DeviceKind::Server(_));
        let console = state.terminals.entry(device).or_default();
        let mut open = true;
        egui::Window::new(format!("Terminal — {name}"))
            .open(&mut open)
            .resizable(true)
            .default_size(egui::vec2(760.0, 480.0))
            .min_size(egui::vec2(420.0, 260.0))
            .frame(egui::Frame::window(viewport.style()).fill(egui::Color32::from_rgb(8, 10, 12)))
            .show(viewport, |ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(125, 190, 145),
                    if server {
                        "LINUX CONSOLE • LIVE"
                    } else {
                        "IOS CONSOLE • LIVE"
                    },
                );
                Self::output_scroll(ui, 58.0)
                    .max_height((ui.available_height() - 58.0).max(0.0))
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        if console.lines.is_empty() {
                            ui.colored_label(
                                egui::Color32::from_rgb(145, 180, 150),
                                if server {
                                    "Linux terminal · type help to begin"
                                } else {
                                    "IOS-style simulator console · type ? for help"
                                },
                            );
                        }
                        for line in &console.lines {
                            ui.colored_label(
                                egui::Color32::from_rgb(190, 205, 195),
                                egui::RichText::new(line).monospace(),
                            );
                        }
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.monospace(sim.terminal_prompt(device));
                    let response = ui.add_enabled(
                        powered,
                        egui::TextEdit::singleline(&mut console.input)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(ui.available_width() - 55.0),
                    );
                    if state.terminal_window_focus.contains(&device) {
                        response.request_focus();
                    }
                    if (ui.button("Run").clicked()
                        || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))))
                        && powered
                    {
                        let input = std::mem::take(&mut console.input);
                        if !input.trim().is_empty() {
                            console.history.push(input.clone());
                        }
                        actions.write(UiAction::RunTerminal(device, input));
                        state.terminal_window_focus.insert(device);
                        response.request_focus();
                    }
                });
            });
        state.terminal_window_focus.remove(&device);
        if !open {
            state.terminal_windows.remove(&device);
        }
    }
}
