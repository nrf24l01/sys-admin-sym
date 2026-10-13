//! Lossless JSON documents and schema-based discovery. Model IDs are never enumerated.
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub pointer: String,
    pub label: String,
}
#[derive(Clone)]
pub struct Face {
    pub side: String,
    pub path: String,
    pub uv: [f32; 4],
}
#[derive(Clone)]
pub struct Group {
    pub label: String,
    pub pointer: String,
    pub rect_pointer: String,
    pub legacy: bool,
}
impl Group {
    pub fn key(&self) -> String {
        format!("{}:{}", self.pointer, self.rect_pointer)
    }
    pub fn fixed_count(&self) -> bool {
        self.pointer.starts_with("/chassis/")
            || self.pointer == "/power_inlets"
            || (!self.legacy && self.pointer == "/ports")
    }
}
pub struct Document {
    pub path: PathBuf,
    pub output: PathBuf,
    pub pointer: String,
    pub data: Value,
    saved: Value,
    pub undo: Vec<Value>,
    pub redo: Vec<Value>,
}
impl Document {
    pub fn load(path: &Path, pointer: &str) -> Result<Self, String> {
        let path = path.canonicalize().map_err(|e| e.to_string())?;
        let data: Value =
            serde_json::from_str(&fs::read_to_string(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if data.pointer(pointer).is_none() {
            return Err("Selected model no longer exists; refresh the library".into());
        }
        Ok(Self {
            path: path.clone(),
            output: path,
            pointer: pointer.into(),
            saved: data.clone(),
            data,
            undo: vec![],
            redo: vec![],
        })
    }
    pub fn model(&self) -> &Value {
        self.data.pointer(&self.pointer).unwrap()
    }
    pub fn model_mut(&mut self) -> &mut Value {
        self.data.pointer_mut(&self.pointer).unwrap()
    }
    pub fn dirty(&self) -> bool {
        self.data != self.saved
    }
    pub fn checkpoint(&mut self) {
        self.undo.push(self.data.clone());
        self.redo.clear();
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
    }
    pub fn undo(&mut self) {
        if let Some(data) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.data, data));
        }
    }
    pub fn redo(&mut self) {
        if let Some(data) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.data, data));
        }
    }
    pub fn save(&mut self) -> Result<(), String> {
        validate_mapping(self.model())?;
        let text = serde_json::to_string_pretty(&self.data).map_err(|e| e.to_string())? + "\n";
        fs::write(&self.output, text).map_err(|e| e.to_string())?;
        self.saved = self.data.clone();
        Ok(())
    }
}
fn escaped(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}
pub fn entries_in(data: &Value) -> Vec<Entry> {
    fn walk(data: &Value, pointer: String, entries: &mut Vec<Entry>) {
        if !faces(data).is_empty() {
            let label = data
                .pointer("/display_name/en")
                .and_then(Value::as_str)
                .or_else(|| data.get("name").and_then(Value::as_str))
                .or_else(|| data.get("id").and_then(Value::as_str))
                .or_else(|| data.get("type").and_then(Value::as_str))
                .unwrap_or("Texture")
                .to_owned();
            entries.push(Entry {
                path: PathBuf::new(),
                pointer,
                label,
            });
            return;
        }
        match data {
            Value::Object(map) => {
                for (key, value) in map {
                    walk(value, format!("{pointer}/{}", escaped(key)), entries);
                }
            }
            Value::Array(array) => {
                for (index, value) in array.iter().enumerate() {
                    walk(value, format!("{pointer}/{index}"), entries);
                }
            }
            _ => {}
        }
    }
    let mut entries = vec![];
    walk(data, String::new(), &mut entries);
    entries
}
pub fn discover(root: &Path) -> (Vec<Entry>, Vec<String>) {
    fn walk(root: &Path, entries: &mut Vec<Entry>, errors: &mut Vec<String>) {
        let paths = match fs::read_dir(root) {
            Ok(paths) => paths,
            Err(e) => {
                errors.push(format!("{}: {e}", root.display()));
                return;
            }
        };
        for path in paths {
            let path = match path {
                Ok(path) => path.path(),
                Err(e) => {
                    errors.push(e.to_string());
                    continue;
                }
            };
            if path.is_dir() {
                walk(&path, entries, errors);
            } else if path.extension().is_some_and(|ext| ext == "json") {
                match fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|text| {
                        serde_json::from_str::<Value>(&text).map_err(|e| e.to_string())
                    }) {
                    Ok(data) => {
                        for mut entry in entries_in(&data) {
                            entry.path = path.canonicalize().unwrap_or_else(|_| path.clone());
                            entries.push(entry);
                        }
                    }
                    Err(e) => errors.push(format!("{}: {e}", path.display())),
                }
            }
        }
    }
    let mut entries = vec![];
    let mut errors = vec![];
    walk(root, &mut entries, &mut errors);
    entries.sort_by(|a, b| {
        a.label
            .cmp(&b.label)
            .then(a.path.cmp(&b.path))
            .then(a.pointer.cmp(&b.pointer))
    });
    (entries, errors)
}
fn four(value: &Value) -> Option<[f32; 4]> {
    let a = value.as_array()?;
    if a.len() != 4 {
        return None;
    }
    Some([
        a[0].as_f64()? as f32,
        a[1].as_f64()? as f32,
        a[2].as_f64()? as f32,
        a[3].as_f64()? as f32,
    ])
}
pub fn faces(model: &Value) -> Vec<Face> {
    let mut result = vec![];
    if let Some(artwork) = model.get("artwork").and_then(Value::as_object) {
        for (side, face) in artwork {
            if let Some(path) = face
                .get("path")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                result.push(Face {
                    side: side.clone(),
                    path: path.into(),
                    uv: four(&face["uv"]).unwrap_or([0., 0., 1., 1.]),
                });
            }
        }
    } else if let Some(texture) = model
        .get("texture")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        let mut sides = vec!["front".to_owned(), "back".to_owned()];
        if let Some(textures) = model.get("textures").and_then(Value::as_object) {
            sides.extend(textures.keys().cloned());
        }
        sides.sort();
        sides.dedup();
        for side in sides {
            let path = model["textures"][&side]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(texture);
            result.push(Face {
                uv: legacy_uv(model["type"].as_str().unwrap_or(""), &side),
                side,
                path: path.into(),
            });
        }
    }
    result.sort_by_key(|face| (face.side != "front", face.side.clone()));
    result
}
fn legacy_uv(kind: &str, side: &str) -> [f32; 4] {
    match (kind, side) {
        ("switch", "front") => [0., 210. / 666., 1., 434. / 666.],
        ("switch", "back") => [0.002, 0.45, 0.997, 0.755],
        ("router", "front") => [0., 193. / 683., 1., 480. / 683.],
        ("router", "back") => [0., 0.31, 1., 0.70],
        ("server", "front") => [0.009, 0.48, 0.995, 0.84],
        ("ups" | "pdu", "front") => [0., 0., 1., 0.5],
        ("ups" | "pdu", "back") => [0., 0.5, 1., 1.],
        _ => [0., 0., 1., 1.],
    }
}
pub fn groups(model: &Value, side: &str) -> Vec<Group> {
    let mut result = vec![];
    if let Some(ports) = model.get("ports").and_then(Value::as_object) {
        for (key, value) in ports {
            if !value.is_array() {
                continue;
            }
            result.push(Group {
                label: key.clone(),
                pointer: format!("/ports/{}", escaped(key)),
                rect_pointer: String::new(),
                legacy: true,
            });
        }
    }
    if let Some(ports) = model.get("ports").and_then(Value::as_array) {
        for (label, rect_pointer) in [
            ("ports", "/socket/rect"),
            ("combo optical sockets", "/optical_socket/rect"),
        ] {
            if rect_pointer == "/socket/rect"
                || ports.iter().any(|p| p.pointer(rect_pointer).is_some())
            {
                result.push(Group {
                    label: label.into(),
                    pointer: "/ports".into(),
                    rect_pointer: rect_pointer.into(),
                    legacy: false,
                });
            }
        }
    }
    if let Some(inlets) = model.get("power_inlets").and_then(Value::as_array)
        && inlets.iter().any(|p| p.get("socket").is_some())
    {
        result.push(Group {
            label: "power_inlets".into(),
            pointer: "/power_inlets".into(),
            rect_pointer: "/socket/rect".into(),
            legacy: false,
        });
    }
    // These are schema fields, not a list of equipment or models.
    for (pointer, rect, face) in [
        ("/onboard_ports", "/socket/rect", "rear"),
        ("/power_inlets", "/rect", "rear"),
        ("/chassis/drive_bays", "/face_rect", "front"),
        ("/chassis/pcie_slots", "/face_rect", "rear"),
    ] {
        if side == face
            && model.pointer(pointer).is_some_and(Value::is_array)
            && !(pointer == "/power_inlets" && result.iter().any(|g| g.pointer == pointer))
        {
            result.push(Group {
                label: pointer.rsplit('/').next().unwrap().into(),
                pointer: pointer.into(),
                rect_pointer: rect.into(),
                legacy: false,
            });
        }
    }
    result.sort_by_key(|group| visible_indices(model, group, side).is_empty());
    result
}
pub fn bounds(model: &Value, group: &Group, index: usize) -> Option<[f32; 4]> {
    let port = model.pointer(&group.pointer)?.get(index)?;
    let rect = if group.legacy {
        [
            port["left_down"][0].as_f64()? as f32,
            port["left_down"][1].as_f64()? as f32,
            port["right_up"][0].as_f64()? as f32,
            port["right_up"][1].as_f64()? as f32,
        ]
    } else {
        four(port.pointer(&group.rect_pointer)?)?
    };
    (rect
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        && rect[0] < rect[2]
        && rect[1] < rect[3])
        .then_some(rect)
}
pub fn set_bounds(model: &mut Value, group: &Group, index: usize, b: [f32; 4]) {
    let port = &mut model.pointer_mut(&group.pointer).unwrap()[index];
    if group.legacy {
        port["left_down"] = json!([b[0], b[1]]);
        port["right_up"] = json!([b[2], b[3]]);
    } else {
        *port.pointer_mut(&group.rect_pointer).unwrap() = json!(b);
    }
}
pub fn visible_indices(model: &Value, group: &Group, side: &str) -> Vec<usize> {
    model
        .pointer(&group.pointer)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter(|(_, port)| {
            port["side"]
                .as_str()
                .map_or(!group.legacy || side == "front", |face| {
                    face.eq_ignore_ascii_case(side)
                        || (face.eq_ignore_ascii_case("rear") && side == "back")
                        || (face == "back" && side == "rear")
                })
        })
        .filter_map(|(i, _)| bounds(model, group, i).map(|_| i))
        .collect()
}
pub fn translated(b: [f32; 4], delta: [f32; 2]) -> [f32; 4] {
    let x = delta[0].clamp(-b[0], 1. - b[2]);
    let y = delta[1].clamp(-b[1], 1. - b[3]);
    [b[0] + x, b[1] + y, b[2] + x, b[3] + y]
}
/// Resize from the lower-right corner while keeping the box valid and inside the texture.
pub fn resized(b: [f32; 4], delta: [f32; 2]) -> [f32; 4] {
    [
        b[0],
        b[1],
        (b[2] + delta[0]).clamp(b[0] + 0.001, 1.),
        (b[3] + delta[1]).clamp(b[1] + 0.001, 1.),
    ]
}
pub fn distribute(model: &mut Value, group: &Group, indices: &[usize], vertical: bool) {
    let axis = usize::from(vertical);
    let mut boxes: Vec<_> = indices
        .iter()
        .filter_map(|&i| bounds(model, group, i).map(|b| (i, b)))
        .collect();
    boxes.sort_by(|a, b| a.1[axis].total_cmp(&b.1[axis]));
    if boxes.len() < 3 {
        return;
    }
    let start = boxes[0].1[axis];
    let end = boxes.last().unwrap().1[axis];
    let count = boxes.len();
    for (n, (i, b)) in boxes.into_iter().enumerate() {
        let mut delta = [0., 0.];
        delta[axis] = start + (end - start) * n as f32 / (count - 1) as f32 - b[axis];
        set_bounds(model, group, i, translated(b, delta));
    }
}
#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;

/// Mirror the domain's port constraints before allowing editor output onto disk.
fn validate_mapping(model: &Value) -> Result<(), String> {
    if let Some(ports) = model.get("onboard_ports").and_then(Value::as_array) {
        let mut names = std::collections::HashSet::new();
        for port in ports {
            let name = port["name"].as_str().unwrap_or("");
            if name.trim().is_empty() || !names.insert(name) {
                return Err("Onboard port names must be nonempty and unique".into());
            }
            let speed = port["speed_mbps"]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok());
            if speed
                .and_then(cloud_provider_sim::LinkSpeed::from_mbps)
                .is_none()
            {
                return Err(format!(
                    "Unsupported speed for {name}; use a supported Ethernet rate"
                ));
            }
        }
    }
    for side in faces(model)
        .iter()
        .map(|face| face.side.as_str())
        .chain(["front", "rear", "back"])
    {
        for group in groups(model, side) {
            if let Some(array) = model.pointer(&group.pointer).and_then(Value::as_array) {
                for (index, port) in array.iter().enumerate() {
                    if group.rect_pointer == "/optical_socket/rect"
                        && port.get("optical_socket").is_none_or(Value::is_null)
                    {
                        continue;
                    }
                    if bounds(model, &group, index).is_none() {
                        return Err(format!(
                            "{} connector {} has invalid geometry; undo or reload before saving",
                            group.label,
                            index + 1
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}
