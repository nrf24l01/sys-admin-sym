use super::*;
use serde_json::json;

#[test]
fn discovers_new_models_without_a_model_list() {
    let data = json!({"models": [
        {"id": "new_model", "display_name": {"en": "New model"},
         "artwork": {"front": {"path": "new.png", "uv": [0,0,1,0.5]},
                     "rear": {"path": "new.png", "uv": [0,0.5,1,1]}},
         "onboard_ports": [{"name": "eth0", "socket": {"rect": [0.1,0.2,0.3,0.4]}}]},
        {"id": "other", "texture": "other.png", "ports": {}}
    ]});
    let entries = entries_in(&data);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].pointer, "/models/0");
    assert_eq!(entries[0].label, "New model");
    let faces = faces(&data["models"][0]);
    assert_eq!(faces[1].uv, [0.0, 0.5, 1.0, 1.0]);
    let groups = groups(&data["models"][0], "rear");
    assert_eq!(groups[0].pointer, "/onboard_ports");
}

#[test]
fn changing_nested_socket_preserves_other_models_and_metadata() {
    let original = json!({"models": [
        {"id": "a", "onboard_ports": [{"name": "eth0", "speed_mbps": 1000,
          "socket": {"rect": [0.1,0.2,0.3,0.4]}, "future": true}]},
        {"id": "b", "future": {"keep": true}}
    ], "version": 42});
    let mut data = original.clone();
    let group = Group {
        label: "onboard_ports".into(),
        pointer: "/onboard_ports".into(),
        rect_pointer: "/socket/rect".into(),
        legacy: false,
    };
    set_bounds(&mut data["models"][0], &group, 0, [0.2, 0.3, 0.4, 0.5]);
    assert_eq!(
        data["models"][0]["onboard_ports"][0]["socket"]["rect"],
        json!([0.2_f32, 0.3_f32, 0.4_f32, 0.5_f32])
    );
    assert_eq!(data["models"][1], original["models"][1]);
    assert_eq!(data["models"][0]["onboard_ports"][0]["future"], true);
    assert_eq!(data["version"], 42);
}

#[test]
fn moving_and_distributing_preserve_box_dimensions_at_edges() {
    assert_eq!(
        translated([0.8, 0.2, 1.0, 0.4], [0.4, -0.5]),
        [0.8, 0.0, 1.0, 0.2]
    );
    let mut model = json!({"ports": {"rj-45": [
        {"left_down": [0.1,0.2], "right_up": [0.2,0.4], "side": "front"},
        {"left_down": [0.2,0.2], "right_up": [0.3,0.4], "side": "front"},
        {"left_down": [0.7,0.2], "right_up": [0.8,0.4], "side": "front"}
    ]}});
    let group = groups(&model, "front").remove(0);
    distribute(&mut model, &group, &[2, 0, 1], false);
    let middle = bounds(&model, &group, 1).unwrap();
    assert!((middle[0] - 0.4).abs() < 0.00001);
    assert!((middle[2] - middle[0] - 0.1).abs() < 0.00001);
}

#[test]
fn resizing_keeps_the_anchor_and_stays_inside_the_texture() {
    assert_eq!(
        resized([0.2, 0.3, 0.5, 0.6], [0.1, -0.1]),
        [0.2, 0.3, 0.6, 0.5]
    );
    assert_eq!(
        resized([0.2, 0.3, 0.5, 0.6], [-1.0, 1.0]),
        [0.2, 0.3, 0.201, 1.0]
    );
}

#[test]
fn dirty_state_undo_and_save_preserve_entire_json() {
    let path = std::env::temp_dir().join(format!("texture-mapper-{}.json", std::process::id()));
    let original =
        json!({"id": "future", "type": "switch", "texture": "x.png", "ports": {}, "extra": 42});
    std::fs::write(&path, original.to_string()).unwrap();
    let mut doc = Document::load(&path, "").unwrap();
    doc.checkpoint();
    doc.data["extra"] = json!(43);
    assert!(doc.dirty());
    doc.undo();
    assert!(!doc.dirty());
    doc.redo();
    doc.save().unwrap();
    assert!(!doc.dirty());
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(saved["id"], "future");
    assert_eq!(saved["extra"], 43);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn malformed_mapping_is_not_exposed_as_an_editable_box() {
    let model = json!({"ports": {
        "broken": false,
        "rj-45": [{"left_down": [0.8,0.2], "right_up": [0.1,0.4], "side": "front"}]
    }});
    let list = groups(&model, "front");
    assert_eq!(list.len(), 1);
    assert!(visible_indices(&model, &list[0], "front").is_empty());
}

#[test]
fn library_scans_nested_files_and_reports_broken_json() {
    let root = std::env::temp_dir().join(format!("texture-library-{}", std::process::id()));
    std::fs::create_dir_all(root.join("nested")).unwrap();
    std::fs::write(
        root.join("nested/new.json"),
        json!({"models":[{"id":"future", "texture":"new.png", "ports":{}}]}).to_string(),
    )
    .unwrap();
    std::fs::write(root.join("bad.json"), "invalid").unwrap();
    std::fs::write(root.join("parts.json"), "{\"parts\": []}").unwrap();
    let (entries, errors) = discover(&root);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].label, "future");
    assert_eq!(entries[0].pointer, "/models/0");
    assert_eq!(errors.len(), 1);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn save_rejects_invalid_server_ports_without_touching_the_file() {
    let path = std::env::temp_dir().join(format!("mapper-invalid-{}.json", std::process::id()));
    let original = json!({"onboard_ports":[{"name":"eth0","speed_mbps":1000,"socket":{"rect":[0.1,0.2,0.3,0.4]}}]});
    std::fs::write(&path, original.to_string()).unwrap();
    let mut doc = Document::load(&path, "").unwrap();
    doc.data["onboard_ports"][0]["speed_mbps"] = json!(1001);
    assert!(doc.save().is_err());
    let on_disk: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(on_disk, original);
    doc.data["onboard_ports"][0]["speed_mbps"] = json!(1000);
    doc.data["onboard_ports"][0]["name"] = json!("");
    assert!(doc.save().is_err());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn repository_equipment_and_server_models_are_editable_and_valid() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let (entries, errors) = discover(&root);
    assert!(errors.is_empty(), "{errors:?}");
    assert!(!entries.is_empty());
    for entry in entries {
        let doc = Document::load(&entry.path, &entry.pointer).unwrap();
        validate_mapping(doc.model()).unwrap_or_else(|e| panic!("{}: {e}", entry.label));
    }
}

#[test]
fn default_group_has_connectors_on_the_visible_face() {
    let model = json!({"ports":{
        "c14":[{"side":"back","left_down":[0.1,0.2],"right_up":[0.3,0.4]}],
        "rj-45":[{"left_down":[0.1,0.2],"right_up":[0.3,0.4]}]
    }});
    assert_eq!(groups(&model, "front")[0].label, "rj-45");
    assert_eq!(groups(&model, "back")[0].label, "c14");
}

#[test]
fn network_socket_groups_follow_faces_and_keep_combo_rectangles_distinct() {
    let mut model = json!({"ports":[
        {"name":"Gi0","side":"Front","socket":{"rect":[0.1,0.2,0.2,0.4]},"optical_socket":{"rect":[0.3,0.2,0.4,0.4]}},
        {"name":"Gi1","side":"Rear","socket":{"rect":[0.5,0.2,0.6,0.4]},"optical_socket":null}
    ],"power_inlets":[{"side":"Rear","socket":{"rect":[0.7,0.2,0.8,0.4]}}]});
    validate_mapping(&model).unwrap();
    let front = groups(&model, "front");
    let normal = front
        .iter()
        .find(|g| g.pointer == "/ports" && g.rect_pointer == "/socket/rect")
        .expect("network ports are editable");
    let optical = front
        .iter()
        .find(|g| g.rect_pointer == "/optical_socket/rect")
        .expect("combo geometry is editable");
    assert_eq!(visible_indices(&model, normal, "front"), vec![0]);
    assert_eq!(visible_indices(&model, normal, "rear"), vec![1]);
    assert_eq!(visible_indices(&model, optical, "front"), vec![0]);
    set_bounds(&mut model, optical, 0, [0.31, 0.2, 0.41, 0.4]);
    assert_eq!(bounds(&model, normal, 0), Some([0.1, 0.2, 0.2, 0.4]));
    let rear = groups(&model, "rear");
    let power = rear.iter().find(|g| g.pointer == "/power_inlets").unwrap();
    assert_eq!(visible_indices(&model, power, "front"), Vec::<usize>::new());
    assert_eq!(visible_indices(&model, power, "rear"), vec![0]);
}
