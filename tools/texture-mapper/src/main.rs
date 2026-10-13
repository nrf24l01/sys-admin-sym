mod document;
mod ui;
use bevy::{
    asset::{AssetPlugin, LoadState},
    prelude::*,
    window::WindowCloseRequested,
};
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, EguiTextureHandle, egui};
use document::*;
use serde_json::json;
use std::{collections::HashMap, env, path::PathBuf};

#[derive(Clone)]
enum Pending {
    Open(Entry),
    Reload,
    Exit,
}
#[derive(Resource)]
struct Editor {
    root: PathBuf,
    entries: Vec<Entry>,
    filter: String,
    warnings: Vec<String>,
    doc: Option<Document>,
    side: String,
    group: String,
    selected: Vec<usize>,
    images: HashMap<String, Handle<Image>>,
    textures: HashMap<String, egui::TextureId>,
    zoom: f32,
    canvas_rect: Option<egui::Rect>,
    canvas_viewport: Option<egui::Rect>,
    canvas_content_size: egui::Vec2,
    pending_scroll_offset: Option<egui::Vec2>,
    vertical: bool,
    status: String,
    pending: Option<Pending>,
    texture_override: Option<String>,
}
impl Editor {
    fn open(&mut self, entry: &Entry) {
        match Document::load(&entry.path, &entry.pointer) {
            Ok(doc) => {
                self.side = faces(doc.model())
                    .first()
                    .map(|f| f.side.clone())
                    .unwrap_or_default();
                self.group = groups(doc.model(), &self.side)
                    .first()
                    .map(|g| g.key())
                    .unwrap_or_default();
                self.texture_override = None;
                self.doc = Some(doc);
                self.selected.clear();
                self.zoom = 1.;
                self.pending_scroll_offset = Some(egui::Vec2::ZERO);
                self.status = format!("Opened {}", entry.label);
            }
            Err(e) => self.status = format!("Open failed: {e}"),
        }
    }
    fn face(&self) -> Option<Face> {
        let mut face = faces(self.doc.as_ref()?.model())
            .into_iter()
            .find(|f| f.side == self.side)?;
        if self.side == "front"
            && let Some(path) = &self.texture_override
        {
            face.path = path.clone();
        }
        Some(face)
    }
    fn save(&mut self) -> bool {
        let Some(doc) = &mut self.doc else {
            return false;
        };
        match doc.save() {
            Ok(()) => {
                self.status = format!("Saved {}", doc.output.display());
                true
            }
            Err(e) => {
                self.status = format!("Save failed: {e}");
                false
            }
        }
    }
    fn active_group(&self) -> Option<Group> {
        groups(self.doc.as_ref()?.model(), &self.side)
            .into_iter()
            .find(|g| g.key() == self.group || g.pointer == self.group)
    }
    fn delete(&mut self) {
        let Some(group) = self.active_group() else {
            return;
        };
        if self.selected.is_empty() || group.fixed_count() {
            return;
        }
        let doc = self.doc.as_mut().unwrap();
        doc.checkpoint();
        let array = doc
            .model_mut()
            .pointer_mut(&group.pointer)
            .unwrap()
            .as_array_mut()
            .unwrap();
        self.selected.sort_unstable();
        self.selected.dedup();
        for &i in self.selected.iter().rev() {
            if i < array.len() {
                array.remove(i);
            }
        }
        self.selected.clear();
    }
    fn add(&mut self) {
        let Some(group) = self.active_group() else {
            return;
        };
        if group.fixed_count() {
            return;
        }
        let template = {
            let model = self.doc.as_ref().unwrap().model();
            visible_indices(model, &group, &self.side)
                .last()
                .map(|&i| model.pointer(&group.pointer).unwrap()[i].clone())
        };
        let doc = self.doc.as_mut().unwrap();
        doc.checkpoint();
        let array = doc
            .model_mut()
            .pointer_mut(&group.pointer)
            .unwrap()
            .as_array_mut()
            .unwrap();
        let mut port = if group.legacy {
            json!({"left_down": [0.45,0.45], "right_up": [0.55,0.55], "side": self.side})
        } else if let Some(port) = template {
            port
        } else {
            match group.pointer.as_str() {
                "/onboard_ports" => {
                    json!({"name": "eth0", "speed_mbps": 1000, "socket": {"rect": [0.45,0.45,0.55,0.55]}})
                }
                "/power_inlets" => json!({"rect": [0.45,0.45,0.55,0.55]}),
                // Bay and PCIe counts are hardware specifications; edit their existing geometry.
                _ => {
                    doc.undo.pop();
                    return;
                }
            }
        };
        if !group.legacy && group.pointer == "/onboard_ports" {
            let mut number = 0;
            while array
                .iter()
                .any(|p| p["name"].as_str() == Some(&format!("eth{number}")))
            {
                number += 1;
            }
            port["name"] = json!(format!("eth{number}"));
        }
        let i = array.len();
        array.push(port);
        set_bounds(doc.model_mut(), &group, i, [0.45, 0.45, 0.55, 0.55]);
        self.selected = vec![i];
    }
}
fn main() {
    let args: Vec<_> = env::args().collect();
    let arg = |key: &str| {
        args.windows(2)
            .find(|pair| pair[0] == key)
            .map(|pair| pair[1].clone())
    };
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = arg("--assets")
        .map(PathBuf::from)
        .unwrap_or_else(|| project.join("assets"));
    let root = root.canonicalize().unwrap_or(root);
    let (entries, warnings) = discover(&root);
    let mut editor = Editor {
        root: root.clone(),
        entries,
        warnings,
        filter: String::new(),
        doc: None,
        side: String::new(),
        group: String::new(),
        selected: vec![],
        images: HashMap::new(),
        textures: HashMap::new(),
        zoom: 1.,
        canvas_rect: None,
        canvas_viewport: None,
        canvas_content_size: egui::Vec2::ZERO,
        pending_scroll_offset: None,
        vertical: false,
        status: "Choose a texture from the library to start editing.".into(),
        pending: None,
        texture_override: None,
    };
    if let Some(path) = arg("--config") {
        let path = PathBuf::from(path);
        let path = if path.exists() {
            path
        } else {
            project.join(path)
        };
        let pointer = editor
            .entries
            .iter()
            .find(|e| e.path.canonicalize().ok() == path.canonicalize().ok())
            .map(|e| e.pointer.clone())
            .unwrap_or_default();
        editor.open(&Entry {
            path,
            pointer,
            label: "configuration".into(),
        });
        if let Some(doc) = &mut editor.doc
            && let Some(output) = arg("--output")
        {
            doc.output = PathBuf::from(output);
        }
    }
    editor.texture_override = arg("--texture");
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: root.to_string_lossy().into_owned(),
                    ..default()
                })
                .set(WindowPlugin {
                    close_when_requested: false,
                    primary_window: Some(Window {
                        title: "Equipment Texture Editor".into(),
                        resolution: (1500, 900).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(EguiPlugin::default())
        .insert_resource(editor)
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
        })
        .add_systems(EguiPrimaryContextPass, ui::ui)
        .run();
}

#[cfg(test)]
mod tests {
    use super::*;
    fn editor(model: serde_json::Value, side: &str, group: &str) -> Editor {
        static NEXT_FILE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root = env::temp_dir();
        let path = root.join(format!(
            "mapper-editor-{}-{}.json",
            std::process::id(),
            NEXT_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::write(&path, model.to_string()).unwrap();
        let doc = Document::load(&path, "").unwrap();
        std::fs::remove_file(path).unwrap();
        Editor {
            root,
            entries: vec![],
            filter: String::new(),
            warnings: vec![],
            doc: Some(doc),
            side: side.into(),
            group: group.into(),
            selected: vec![0],
            images: HashMap::new(),
            textures: HashMap::new(),
            zoom: 1.,
            canvas_rect: None,
            canvas_viewport: None,
            canvas_content_size: egui::Vec2::ZERO,
            pending_scroll_offset: None,
            vertical: false,
            status: String::new(),
            pending: None,
            texture_override: None,
        }
    }
    #[test]
    fn delete_shortcut_does_not_change_hardware_slot_counts() {
        let model = json!({"chassis":{"drive_bays":[{"face_rect":[0.1,0.2,0.3,0.4]}]}});
        let mut editor = editor(model.clone(), "front", "/chassis/drive_bays");
        editor.delete();
        assert_eq!(editor.doc.unwrap().data, model);
    }
    #[test]
    fn add_port_after_deletion_chooses_an_unused_name() {
        let mut editor = editor(
            json!({"onboard_ports":[{"name":"eth1","speed_mbps":1000,"socket":{"rect":[0.1,0.2,0.3,0.4]}}]}),
            "rear",
            "/onboard_ports",
        );
        editor.add();
        let ports = &editor.doc.unwrap().data["onboard_ports"];
        assert_ne!(ports[0]["name"], ports[1]["name"]);
    }
    #[test]
    fn add_port_ignores_malformed_existing_geometry() {
        let mut editor = editor(
            json!({"power_inlets":[],"onboard_ports":[{"name":"eth1"}]}),
            "rear",
            "/onboard_ports",
        );
        editor.add();
        assert!(
            bounds(
                editor.doc.as_ref().unwrap().model(),
                &editor.active_group().unwrap(),
                1
            )
            .is_some()
        );
    }
}
