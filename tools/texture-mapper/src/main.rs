use bevy::{asset::AssetPlugin, prelude::*};
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, EguiTextureHandle, egui};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fs};

#[derive(Clone, Serialize, Deserialize)]
struct Config {
    #[serde(rename = "type")]
    kind: String,
    texture: String,
    #[serde(default)]
    textures: HashMap<String, String>,
    #[serde(default)]
    ports: HashMap<String, Vec<Port>>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Port {
    left_down: [f32; 2],
    right_up: [f32; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    speed: Option<String>,
    side: String,
}
#[derive(Resource)]
struct State {
    path: String,
    config_choices: Vec<String>,
    texture_path: String,
    output: String,
    config: Config,
    images: HashMap<String, Handle<Image>>,
    tex: HashMap<String, egui::TextureId>,
    side: String,
    group: String,
    selected: Vec<usize>,
    vertical: bool,
    status: String,
    zoom: f32,
    undo: Vec<Config>,
    redo: Vec<Config>,
}

fn main() {
    let a: Vec<_> = env::args().collect();
    let arg = |n: &str| a.windows(2).find(|x| x[0] == n).map(|x| x[1].clone());
    let path = arg("--config").unwrap_or("assets/equipment/switch_config.json".into());
    let output = arg("--output").unwrap_or(path.clone());
    let config_choices = fs::read_dir(format!(
        "{}/../../assets/equipment",
        env!("CARGO_MANIFEST_DIR")
    ))
    .into_iter()
    .flatten()
    .filter_map(|entry| entry.ok())
    .filter(|entry| entry.path().extension().is_some_and(|x| x == "json"))
    .map(|entry| format!("assets/equipment/{}", entry.file_name().to_string_lossy()))
    .collect();
    let config: Config = serde_json::from_str(
        &fs::read_to_string(project_path(&path)).expect("cannot read --config"),
    )
    .expect("invalid config JSON");
    let texture = asset_path(arg("--texture").unwrap_or(config.texture.clone()));
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: format!("{}/../../assets", env!("CARGO_MANIFEST_DIR")),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Equipment Texture Mapper".into(),
                        resolution: (1500, 900).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(EguiPlugin::default())
        .insert_resource(State {
            path,
            config_choices,
            texture_path: texture.clone(),
            output,
            config,
            images: HashMap::new(),
            tex: HashMap::new(),
            side: "front".into(),
            group: "rj-45".into(),
            selected: vec![],
            vertical: false,
            status: "Shift-click ports to select several".into(),
            zoom: 1.0,
            undo: Vec::new(),
            redo: Vec::new(),
        })
        .add_systems(
            Startup,
            move |mut s: ResMut<State>, assets: Res<AssetServer>| {
                s.images
                    .insert("front".into(), assets.load(texture.clone()));
                let back = s
                    .config
                    .textures
                    .get("back")
                    .cloned()
                    .unwrap_or_else(|| texture.clone());
                s.images
                    .insert("back".into(), assets.load(asset_path(back)));
            },
        )
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
        })
        .add_systems(EguiPrimaryContextPass, ui)
        .run();
}

fn asset_path(path: String) -> String {
    path.strip_prefix("assets/").unwrap_or(&path).to_owned()
}

fn project_path(path: &str) -> String {
    if path.starts_with("assets/") {
        format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"))
    } else {
        path.into()
    }
}

fn load_selection(state: &mut State, assets: &AssetServer) -> Result<(), String> {
    let config: Config = serde_json::from_str(
        &fs::read_to_string(project_path(&state.path)).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    state.config = config;
    state.output = state.path.clone();
    state.texture_path = asset_path(state.config.texture.clone());
    state.images.clear();
    state.tex.clear();
    state
        .images
        .insert("front".into(), assets.load(state.texture_path.clone()));
    let back = state
        .config
        .textures
        .get("back")
        .cloned()
        .unwrap_or_else(|| state.texture_path.clone());
    state
        .images
        .insert("back".into(), assets.load(asset_path(back)));
    state.group = state
        .config
        .ports
        .keys()
        .next()
        .cloned()
        .unwrap_or_else(|| "rj-45".into());
    state.side = "front".into();
    state.selected.clear();
    state.undo.clear();
    state.redo.clear();
    Ok(())
}
#[allow(clippy::needless_range_loop)]
fn ui(
    mut c: EguiContexts,
    images: Res<Assets<Image>>,
    assets: Res<AssetServer>,
    mut s: ResMut<State>,
) -> Result {
    let side_key = s.side.clone();
    if !s.tex.contains_key(&side_key)
        && let Some(image) = s.images.get(&side_key)
    {
        let id = c.add_image(EguiTextureHandle::Strong(image.clone()));
        s.tex.insert(side_key.clone(), id);
    }
    let ctx = c.ctx_mut()?;
    if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z))
        && let Some(previous) = s.undo.pop()
    {
        let current = s.config.clone();
        s.redo.push(current);
        s.config = previous;
        s.selected.clear();
    }
    if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Y))
        && let Some(next) = s.redo.pop()
    {
        let current = s.config.clone();
        s.undo.push(current);
        s.config = next;
        s.selected.clear();
    }
    let Some(texture_id) = s.tex.get(&s.side).copied() else {
        let mut loading = egui::Ui::new(
            ctx.clone(),
            "mapper_loading".into(),
            egui::UiBuilder::new().max_rect(ctx.viewport_rect()),
        );
        egui::CentralPanel::default().show(&mut loading, |ui| {
            ui.heading("Loading texture…");
            ui.label("Waiting for the selected image to become available.");
        });
        ctx.request_repaint();
        return Ok(());
    };
    let mut root = egui::Ui::new(
        ctx.clone(),
        "mapper".into(),
        egui::UiBuilder::new().max_rect(ctx.viewport_rect()),
    );
    egui::Panel::left("controls").show(&mut root, |u| {
        u.heading("Texture mapper");
        let mut selected_config = false;
        egui::ComboBox::from_label("Config file")
            .selected_text(s.path.rsplit('/').next().unwrap_or(&s.path))
            .show_ui(u, |u| {
                for path in s.config_choices.clone() {
                    let label = path.rsplit('/').next().unwrap_or(&path).to_owned();
                    if u.selectable_label(s.path == path, label).clicked() {
                        s.path = path;
                        selected_config = true;
                    }
                }
            });
        u.label(format!("Texture: {}", s.texture_path));
        if selected_config || u.button("Reload selected config").clicked() {
            s.status = match load_selection(&mut s, &assets) {
                Ok(()) => "Reloaded".into(),
                Err(error) => format!("Load failed: {error}"),
            };
        }
        u.horizontal(|u| {
            for x in ["front", "back"] {
                if u.selectable_label(s.side == x, x).clicked() {
                    s.side = x.into();
                    s.selected.clear();
                }
            }
        });
        egui::ComboBox::from_label("Port type")
            .selected_text(&s.group)
            .show_ui(u, |u| {
                let mut groups: Vec<_> = s.config.ports.keys().cloned().collect();
                groups.sort();
                for group in groups {
                    u.selectable_value(&mut s.group, group.clone(), group);
                }
            });
        let g = s.group.clone();
        let side = s.side.clone();
        let side_for_new = side.clone();
        let add = u.button("Add port").clicked();
        u.checkbox(&mut s.vertical, "Vertical distribute");
        u.add(egui::Slider::new(&mut s.zoom, 0.25..=4.0).text("Zoom"));
        let selected = s.selected.clone();
        let vertical = s.vertical;
        let distribute_clicked = u.button("Distribute selected").clicked();
        if add || distribute_clicked {
            let previous = s.config.clone();
            s.undo.push(previous);
            s.redo.clear();
        }
        let p = s.config.ports.entry(g).or_default();
        u.label(format!("{} mapped ports", p.len()));
        if add {
            p.push(Port {
                left_down: [0.45, 0.45],
                right_up: [0.55, 0.55],
                speed: None,
                side: side_for_new,
            });
        }
        if distribute_clicked {
            distribute(p, &selected, vertical);
        }
        if u.button("Save JSON").clicked() {
            s.status = match serde_json::to_string_pretty(&s.config)
                .and_then(|x| fs::write(project_path(&s.output), x).map_err(serde_json::Error::io))
            {
                Ok(_) => "Saved".into(),
                Err(e) => e.to_string(),
            };
        }
        u.separator();
        u.label(&s.status);
        u.small("Drag a port box to move it. Shift-click selects multiple ports.");
    });
    egui::CentralPanel::default().show(&mut root, |u| {
        let uv = face_uv(&s.config.kind, &s.side);
        let image = s.images.get(&s.side);
        let aspect = image
            .and_then(|handle| images.get(handle))
            .map(|image| {
                let size = image.texture_descriptor.size;
                size.width as f32 / (size.height as f32 * uv.height())
            })
            .unwrap_or(16.0 / 9.0);
        let max = u.available_size().min(egui::vec2(1100.0, 760.0)) * s.zoom;
        let size = if max.x / max.y > aspect {
            egui::vec2(max.y * aspect, max.y)
        } else {
            egui::vec2(max.x, max.x / aspect)
        };
        let (r, _) = u.allocate_exact_size(size, egui::Sense::hover());
        u.painter().image(texture_id, r, uv, egui::Color32::WHITE);
        let g = s.group.clone();
        let side = s.side.clone();
        let selected = s.selected.clone();
        let p = s.config.ports.entry(g.clone()).or_default();
        let mut clicked = None;
        for i in 0..p.len() {
            if p[i].side != side {
                continue;
            }
            let b = rect(r, &p[i]);
            let hit = u.interact(
                b,
                u.make_persistent_id(("port", g.as_str(), side.as_str(), i)),
                egui::Sense::click_and_drag(),
            );
            u.painter().rect_stroke(
                b,
                1.0,
                egui::Stroke::new(
                    if selected.contains(&i) { 3.0 } else { 1.0 },
                    if selected.contains(&i) {
                        egui::Color32::YELLOW
                    } else {
                        egui::Color32::LIGHT_BLUE
                    },
                ),
                egui::StrokeKind::Inside,
            );
            u.painter().text(
                b.center(),
                egui::Align2::CENTER_CENTER,
                i.to_string(),
                egui::FontId::monospace(12.0),
                egui::Color32::WHITE,
            );
            if selected.contains(&i) {
                for (corner, point) in [(0u8, b.left_top()), (1, b.right_bottom())] {
                    u.painter().circle_filled(point, 5.0, egui::Color32::YELLOW);
                    let handle = egui::Rect::from_center_size(point, egui::vec2(12.0, 12.0));
                    let resize = u.interact(
                        handle,
                        u.make_persistent_id(("resize", g.as_str(), side.as_str(), i, corner)),
                        egui::Sense::drag(),
                    );
                    if resize.dragged()
                        && let Some(pointer) = resize.interact_pointer_pos()
                    {
                        let x = ((pointer.x - r.left()) / r.width()).clamp(0.0, 1.0);
                        let y = ((pointer.y - r.top()) / r.height()).clamp(0.0, 1.0);
                        if corner == 0 {
                            p[i].left_down = [x, y];
                        } else {
                            p[i].right_up = [x, y];
                        }
                    }
                }
            }
            if hit.clicked() {
                clicked = Some((i, u.input(|x| x.modifiers.shift)));
            }
            if hit.dragged() {
                move_port(&mut p[i], hit.drag_delta(), r)
            }
        }
        if let Some((i, shift)) = clicked {
            if !shift {
                s.selected.clear();
            }
            if let Some(x) = s.selected.iter().position(|x| *x == i) {
                s.selected.remove(x);
            } else {
                s.selected.push(i);
            }
        }
    });
    Ok(())
}

fn face_uv(kind: &str, side: &str) -> egui::Rect {
    let (left, top, right, bottom) = match (kind, side) {
        ("switch", "front") => (0.0, 210.0 / 666.0, 1.0, 434.0 / 666.0),
        ("switch", "back") => (0.002, 0.45, 0.997, 0.755),
        ("router", "front") => (0.0, 193.0 / 683.0, 1.0, 480.0 / 683.0),
        ("router", "back") => (0.0, 0.31, 1.0, 0.70),
        ("server", "front") => (0.009, 0.48, 0.995, 0.84),
        ("ups", "front") | ("pdu", "front") => (0.0, 0.0, 1.0, 0.5),
        ("ups", "back") | ("pdu", "back") => (0.0, 0.5, 1.0, 1.0),
        _ => (0.0, 0.0, 1.0, 1.0),
    };
    egui::Rect::from_min_max(egui::pos2(left, top), egui::pos2(right, bottom))
}
fn rect(r: egui::Rect, p: &Port) -> egui::Rect {
    let f = |x: [f32; 2]| egui::pos2(r.left() + r.width() * x[0], r.top() + r.height() * x[1]);
    egui::Rect::from_two_pos(f(p.left_down), f(p.right_up))
}
fn move_port(p: &mut Port, d: egui::Vec2, r: egui::Rect) {
    let (x, y) = (d.x / r.width(), d.y / r.height());
    for q in [&mut p.left_down, &mut p.right_up] {
        q[0] = (q[0] + x).clamp(0.0, 1.0);
        q[1] = (q[1] + y).clamp(0.0, 1.0)
    }
}
fn distribute(p: &mut [Port], ids: &[usize], vertical: bool) {
    if ids.len() < 2 {
        return;
    }
    let mut x = ids.to_vec();
    x.sort_unstable();
    let a = p[x[0]].left_down;
    let b = p[*x.last().unwrap()].left_down;
    for (n, i) in x.into_iter().enumerate() {
        let t = n as f32 / (ids.len() - 1) as f32;
        if vertical {
            p[i].left_down[1] = a[1] + (b[1] - a[1]) * t;
            p[i].right_up[1] = p[i].left_down[1] + (p[i].right_up[1] - p[i].left_down[1])
        } else {
            p[i].left_down[0] = a[0] + (b[0] - a[0]) * t;
            p[i].right_up[0] = p[i].left_down[0] + (p[i].right_up[0] - p[i].left_down[0])
        }
    }
}
