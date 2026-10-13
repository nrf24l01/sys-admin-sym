use super::*;

fn execute(editor: &mut Editor, pending: Pending, exit: &mut MessageWriter<AppExit>) {
    match pending {
        Pending::Open(entry) => editor.open(&entry),
        Pending::Reload => {
            if let Some(doc) = &editor.doc {
                let entry = Entry {
                    path: doc.path.clone(),
                    pointer: doc.pointer.clone(),
                    label: "configuration".into(),
                };
                let output = doc.output.clone();
                editor.open(&entry);
                if let Some(doc) = &mut editor.doc {
                    doc.output = output;
                }
            }
        }
        Pending::Exit => {
            exit.write(AppExit::Success);
        }
    }
}
pub(super) fn ui(
    mut contexts: EguiContexts,
    assets: Res<AssetServer>,
    images: Res<Assets<Image>>,
    mut editor: ResMut<Editor>,
    mut close: MessageReader<WindowCloseRequested>,
    mut exit: MessageWriter<AppExit>,
) -> Result {
    let s = &mut *editor;
    let face = s.face();
    if let Some(face) = &face {
        let path = face
            .path
            .strip_prefix("assets/")
            .unwrap_or(&face.path)
            .to_owned();
        let handle = s
            .images
            .entry(path.clone())
            .or_insert_with(|| assets.load(path.clone()))
            .clone();
        s.textures
            .entry(path)
            .or_insert_with(|| contexts.add_image(EguiTextureHandle::Strong(handle)));
    }
    let ctx = contexts.ctx_mut()?;
    if let (Some(pointer), Some(canvas), Some(viewport)) = (
        ctx.input(|i| i.pointer.hover_pos()),
        s.canvas_rect,
        s.canvas_viewport,
    ) {
        let wheel_y = ctx.input(|i| i.smooth_scroll_delta.y);
        if canvas.contains(pointer) && wheel_y.abs() > 0.01 && s.canvas_content_size.x > 0. {
            let old_zoom = s.zoom;
            let new_zoom = (old_zoom * (wheel_y * 0.002).exp()).clamp(0.25, 8.0);
            if (new_zoom - old_zoom).abs() > f32::EPSILON {
                let cursor_in_viewport = pointer - viewport.min;
                let image_point = (pointer - canvas.min) / s.canvas_content_size;
                let new_size = s.canvas_content_size * (new_zoom / old_zoom);
                s.pending_scroll_offset = Some(zoom_scroll_offset(
                    cursor_in_viewport,
                    image_point,
                    new_size,
                ));
                s.zoom = new_zoom;
            }
            // The same wheel input must not also pan the canvas.
            ctx.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
        }
    }
    let mut request = if close.read().next().is_some() {
        Some(Pending::Exit)
    } else {
        None
    };
    if s.pending.is_none() {
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            s.save();
        }
        if let Some(doc) = &mut s.doc
            && !ctx.egui_wants_keyboard_input()
        {
            if ctx
                .input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z) && !i.modifiers.shift)
            {
                doc.undo();
                s.selected.clear();
            }
            if ctx.input(|i| {
                i.modifiers.command
                    && (i.key_pressed(egui::Key::Y)
                        || (i.modifiers.shift && i.key_pressed(egui::Key::Z)))
            }) {
                doc.redo();
                s.selected.clear();
            }
        }
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            s.delete();
        }
        let arrow = (!ctx.egui_wants_keyboard_input())
            .then(|| {
                ctx.input(|i| {
                    [
                        (egui::Key::ArrowLeft, [-1., 0.]),
                        (egui::Key::ArrowRight, [1., 0.]),
                        (egui::Key::ArrowUp, [0., -1.]),
                        (egui::Key::ArrowDown, [0., 1.]),
                    ]
                    .into_iter()
                    .find(|(key, _)| i.key_pressed(*key))
                    .map(|(_, delta)| (delta, i.modifiers.shift))
                })
            })
            .flatten();
        if let (Some((direction, resize)), Some(group)) = (arrow, s.active_group())
            && !s.selected.is_empty()
        {
            let width = s.canvas_content_size.x.max(400.);
            let height = s.canvas_content_size.y.max(400.);
            let delta = [direction[0] * 4. / width, direction[1] * 4. / height];
            let ids = s.selected.clone();
            let doc = s.doc.as_mut().unwrap();
            doc.checkpoint();
            for id in ids {
                if let Some(b) = bounds(doc.model(), &group, id) {
                    let next = if resize {
                        resized(b, delta)
                    } else {
                        translated(b, delta)
                    };
                    set_bounds(doc.model_mut(), &group, id, next);
                }
            }
        }
    }
    let mut root = egui::Ui::new(
        ctx.clone(),
        "texture_editor".into(),
        egui::UiBuilder::new().max_rect(ctx.viewport_rect()),
    );
    egui::Panel::top("toolbar").show(&mut root, |u| {
        u.horizontal(|u| {
            u.heading("Texture Editor");
            u.separator();
            let loaded = s.doc.is_some();
            if u.add_enabled(loaded, egui::Button::new("Save  Ctrl+S"))
                .clicked()
            {
                s.save();
            }
            if u.add_enabled(loaded, egui::Button::new("Reload")).clicked() {
                request = Some(Pending::Reload);
            }
            if let Some(doc) = &mut s.doc {
                if u.add_enabled(!doc.undo.is_empty(), egui::Button::new("Undo"))
                    .clicked()
                {
                    doc.undo();
                    s.selected.clear();
                }
                if u.add_enabled(!doc.redo.is_empty(), egui::Button::new("Redo"))
                    .clicked()
                {
                    doc.redo();
                    s.selected.clear();
                }
                u.separator();
                u.label(if doc.dirty() {
                    "● Unsaved changes"
                } else {
                    "Saved"
                });
            }
        });
    });
    egui::Panel::bottom("status").show(&mut root, |u| {
        u.label(&s.status);
    });
    egui::Panel::left("library")
        .default_size(290.)
        .resizable(true)
        .show(&mut root, |u| {
            u.heading("Texture library");
            u.add(egui::TextEdit::singleline(&mut s.filter).hint_text("Search model or file…"));
            if u.button("Refresh library").clicked() {
                (s.entries, s.warnings) = discover(&s.root);
            }
            u.small(format!("{} editable entries", s.entries.len()));
            if !s.warnings.is_empty() {
                u.collapsing("Discovery warnings", |u| {
                    for warning in &s.warnings {
                        u.small(warning);
                    }
                });
            }
            u.separator();
            let filter = s.filter.to_lowercase();
            egui::ScrollArea::vertical().show(u, |u| {
                for entry in &s.entries {
                    let file = entry
                        .path
                        .strip_prefix(&s.root)
                        .unwrap_or(&entry.path)
                        .display()
                        .to_string();
                    if !format!("{} {file}", entry.label)
                        .to_lowercase()
                        .contains(&filter)
                    {
                        continue;
                    }
                    let selected = s
                        .doc
                        .as_ref()
                        .is_some_and(|d| d.path == entry.path && d.pointer == entry.pointer);
                    if u.selectable_label(selected, &entry.label)
                        .on_hover_text(format!("{file}\n{}", entry.pointer))
                        .clicked()
                        && !selected
                    {
                        request = Some(Pending::Open(entry.clone()));
                    }
                    u.small(file);
                    u.add_space(5.);
                }
            });
        });
    egui::Panel::right("inspector").default_size(265.).show(&mut root, |u| {
        u.heading("Mapping inspector");
        if s.doc.is_none() {u.label("Open a texture to edit its connector mappings.");return;}
        let doc = s.doc.as_ref().unwrap();
        u.small(doc.path.strip_prefix(&s.root).unwrap_or(&doc.path).display().to_string()).on_hover_text(doc.path.display().to_string());
        let faces = faces(doc.model());
        u.horizontal(|u|for face in &faces {
            if u.selectable_label(s.side == face.side,&face.side).clicked() {
                s.side = face.side.clone(); s.selected.clear();
                s.group = groups(s.doc.as_ref().unwrap().model(),&s.side).first().map(|g|g.pointer.clone()).unwrap_or_default();
            }
        });
        let groups = groups(s.doc.as_ref().unwrap().model(),&s.side);
        if groups.is_empty() {u.label("This texture has no editable connector groups.");return;}
        u.label("Port type");
        egui::ScrollArea::vertical().id_salt("mapping_groups").max_height(145.).show(u, |u| {
            for group in &groups {
                if u.selectable_label(s.group == group.pointer, &group.label).clicked() {
                    s.group.clone_from(&group.pointer);
                    s.selected.clear();
                }
            }
        });
        let Some(group) = s.active_group() else {return;};
        let hardware = group.pointer.starts_with("/chassis/") || group.pointer == "/power_inlets";
        u.horizontal(|u| {
            if u.add_enabled(!hardware,egui::Button::new("Add connector")).clicked(){s.add();}
            if u.add_enabled(!hardware && !s.selected.is_empty(),egui::Button::new("Delete")).clicked(){s.delete();}
        });
        if hardware {u.small("Hardware slot counts come from model specifications. Edit their positions here.");}
        u.checkbox(&mut s.vertical,"Distribute vertically");
        if u.add_enabled(s.selected.len() >= 3,egui::Button::new("Distribute selection")).clicked() {
            let doc = s.doc.as_mut().unwrap();doc.checkpoint();distribute(doc.model_mut(),&group,&s.selected,s.vertical);
        }
        u.separator();
        let indices = visible_indices(s.doc.as_ref().unwrap().model(),&group,&s.side);
        u.label(format!("{} connectors · {} selected",indices.len(),s.selected.len()));
        egui::ScrollArea::vertical().id_salt("connectors").max_height(190.).show(u,|u|for i in indices {
            let port = &s.doc.as_ref().unwrap().model().pointer(&group.pointer).unwrap()[i];
            let name = port["name"].as_str().map(str::to_owned).unwrap_or_else(||format!("Connector {}",i+1));
            if u.selectable_label(s.selected.contains(&i),name).clicked(){select(&mut s.selected,i,u.input(|i|i.modifiers.shift));}
        });
        if let [index] = s.selected.as_slice() {
            let index = *index;
            if let Some(mut b) = bounds(s.doc.as_ref().unwrap().model(),&group,index) {
                u.separator();u.label("Normalized coordinates");
                let mut changed = false;
                egui::Grid::new("coordinates").show(u,|u|for (axis,label) in ["Left","Top","Right","Bottom"].iter().enumerate() {
                    u.label(*label);changed |= u.add(egui::DragValue::new(&mut b[axis]).speed(0.001).range(0.0..=1.0)).changed();u.end_row();
                });
                if changed {b[2]=b[2].max(b[0]+0.001).min(1.);b[3]=b[3].max(b[1]+0.001).min(1.);b[0]=b[0].min(b[2]-0.001);b[1]=b[1].min(b[3]-0.001);
                    let doc = s.doc.as_mut().unwrap();doc.checkpoint();set_bounds(doc.model_mut(),&group,index,b);}
                let port = &s.doc.as_ref().unwrap().model().pointer(&group.pointer).unwrap()[index];
                let mut name = port["name"].as_str().unwrap_or("").to_owned();
                if port.get("name").is_some() {u.label("Name");if u.text_edit_singleline(&mut name).changed(){let doc=s.doc.as_mut().unwrap();doc.checkpoint();doc.model_mut().pointer_mut(&group.pointer).unwrap()[index]["name"]=json!(name);}}
                if group.legacy {
                    let mut speed = s.doc.as_ref().unwrap().model().pointer(&group.pointer).unwrap()[index]["speed"].as_str().unwrap_or("").to_owned();
                    u.label("Speed (optional)");if u.text_edit_singleline(&mut speed).changed(){let doc=s.doc.as_mut().unwrap();doc.checkpoint();let port=&mut doc.model_mut().pointer_mut(&group.pointer).unwrap()[index];if speed.is_empty(){port.as_object_mut().unwrap().remove("speed");}else{port["speed"]=json!(speed);}}
                } else if group.pointer == "/onboard_ports" {
                    let mut speed=s.doc.as_ref().unwrap().model().pointer(&group.pointer).unwrap()[index]["speed_mbps"].as_u64().unwrap_or(1000);
                    u.label("Speed (Mbps)");if u.add(egui::DragValue::new(&mut speed).range(1..=400000)).changed(){let doc=s.doc.as_mut().unwrap();doc.checkpoint();doc.model_mut().pointer_mut(&group.pointer).unwrap()[index]["speed_mbps"]=json!(speed);}
                }
            }
        }
        u.separator();u.small("Add connector creates a hitbox. Drag boxes or use arrows to move; Shift+arrows resize. Shift-click selects several.");
    });
    egui::CentralPanel::default().show(&mut root, |u| {
        if s.doc.is_none() {
            u.heading("Choose a texture from the library");
            u.label("Open → edit connector mappings → save → switch to another texture.");
            return;
        }
        let current_face = s.face();
        let Some(face) = current_face else {
            u.label("No texture configured for this view.");
            return;
        };
        u.horizontal(|u| {
            u.label(&face.path);
            if u.button("Fit").clicked() {
                s.zoom = 1.;
                s.pending_scroll_offset = Some(egui::Vec2::ZERO);
            }
            u.add(egui::Slider::new(&mut s.zoom, 0.25..=8.0).text("Zoom"));
        });
        let path = face.path.strip_prefix("assets/").unwrap_or(&face.path);
        let Some(handle) = s.images.get(path) else {
            u.label("Loading texture…");
            ctx.request_repaint();
            return;
        };
        if matches!(assets.load_state(handle.id()), LoadState::Failed(_)) {
            u.colored_label(
                egui::Color32::LIGHT_RED,
                "Texture failed to load. Check its path, or choose another entry.",
            );
            return;
        }
        let Some(image) = images.get(handle) else {
            u.label("Loading texture…");
            ctx.request_repaint();
            return;
        };
        let Some(&texture) = s.textures.get(path) else {
            return;
        };
        let [l, t, r, b] = face.uv;
        let aspect = image.width() as f32 * (r - l) / (image.height() as f32 * (b - t));
        if !aspect.is_finite() || aspect <= 0. {
            u.label("Invalid face crop in asset JSON.");
            return;
        }
        let available = u.available_size().max(egui::vec2(1., 1.));
        let width = available.x.min(available.y * aspect) * s.zoom;
        let mut canvas_scroll = egui::ScrollArea::both().id_salt("texture_canvas");
        if let Some(offset) = s.pending_scroll_offset.take() {
            canvas_scroll = canvas_scroll.scroll_offset(offset);
        }
        let output = canvas_scroll.show(u, |u| {
            let (canvas, _) =
                u.allocate_exact_size(egui::vec2(width, width / aspect), egui::Sense::hover());
            s.canvas_rect = Some(canvas);
            s.canvas_content_size = canvas.size();
            u.painter()
                .rect_filled(canvas, 0., egui::Color32::from_gray(35));
            u.painter().image(
                texture,
                canvas,
                egui::Rect::from_min_max(egui::pos2(l, t), egui::pos2(r, b)),
                egui::Color32::WHITE,
            );
            let Some(group) = s.active_group() else {
                return;
            };
            let indices = visible_indices(s.doc.as_ref().unwrap().model(), &group, &s.side);
            for i in indices {
                let coords = bounds(s.doc.as_ref().unwrap().model(), &group, i).unwrap();
                let rect = egui::Rect::from_min_max(
                    egui::pos2(
                        canvas.left() + canvas.width() * coords[0],
                        canvas.top() + canvas.height() * coords[1],
                    ),
                    egui::pos2(
                        canvas.left() + canvas.width() * coords[2],
                        canvas.top() + canvas.height() * coords[3],
                    ),
                );
                let selected = s.selected.contains(&i);
                let color = if selected {
                    egui::Color32::YELLOW
                } else {
                    egui::Color32::LIGHT_BLUE
                };
                u.painter().rect_stroke(
                    rect,
                    0.,
                    egui::Stroke::new(if selected { 2. } else { 1. }, color),
                    egui::StrokeKind::Inside,
                );
                u.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    (i + 1).to_string(),
                    egui::FontId::monospace(12.),
                    color,
                );
                let hit = u.interact(
                    rect,
                    u.id().with(("port", &group.pointer, i)),
                    egui::Sense::click_and_drag(),
                );
                let mut resizing = false;
                if selected {
                    for (corner, point) in [(0, rect.left_top()), (1, rect.right_bottom())] {
                        u.painter().circle_filled(point, 5., color);
                        let hit = u.interact(
                            egui::Rect::from_center_size(point, egui::vec2(14., 14.)),
                            u.id().with(("resize", &group.pointer, i, corner)),
                            egui::Sense::drag(),
                        );
                        if hit.drag_started() {
                            s.doc.as_mut().unwrap().checkpoint();
                        }
                        if hit.dragged()
                            && let Some(pointer) = hit.interact_pointer_pos()
                        {
                            resizing = true;
                            let mut next = coords;
                            let x = ((pointer.x - canvas.left()) / canvas.width()).clamp(0., 1.);
                            let y = ((pointer.y - canvas.top()) / canvas.height()).clamp(0., 1.);
                            if corner == 0 {
                                next[0] = x.min(coords[2] - 0.001).max(0.);
                                next[1] = y.min(coords[3] - 0.001).max(0.);
                            } else {
                                next[2] = x.max(coords[0] + 0.001).min(1.);
                                next[3] = y.max(coords[1] + 0.001).min(1.);
                            }
                            set_bounds(s.doc.as_mut().unwrap().model_mut(), &group, i, next);
                        }
                    }
                }
                if hit.clicked() {
                    select(&mut s.selected, i, u.input(|i| i.modifiers.shift));
                }
                if !resizing && hit.drag_started() {
                    s.doc.as_mut().unwrap().checkpoint();
                    if !s.selected.contains(&i) {
                        s.selected = vec![i];
                    }
                }
                if !resizing && hit.dragged() {
                    let delta = hit.drag_delta();
                    let mut shift = [delta.x / canvas.width(), delta.y / canvas.height()];
                    // Apply one clamped delta to the whole selection so relative positions survive.
                    for &id in &s.selected {
                        if let Some(b) = bounds(s.doc.as_ref().unwrap().model(), &group, id) {
                            shift[0] = shift[0].clamp(-b[0], 1. - b[2]);
                            shift[1] = shift[1].clamp(-b[1], 1. - b[3]);
                        }
                    }
                    for &id in &s.selected {
                        if let Some(b) = bounds(s.doc.as_ref().unwrap().model(), &group, id) {
                            set_bounds(
                                s.doc.as_mut().unwrap().model_mut(),
                                &group,
                                id,
                                translated(b, shift),
                            );
                        }
                    }
                }
            }
        });
        s.canvas_viewport = Some(output.inner_rect);
    });
    if let Some(request) = request {
        if s.doc.as_ref().is_some_and(Document::dirty) {
            if s.pending.is_none() {
                s.pending = Some(request);
            }
        } else {
            execute(s, request, &mut exit);
        }
    }
    if s.pending.is_some() {
        let mut choice = None;
        egui::Modal::new("unsaved_changes".into()).show(ctx, |u| {
            u.heading("Save changes before continuing?");
            u.label("The current texture has unsaved mapping changes.");
            u.horizontal(|u| {
                if u.button("Save and continue").clicked() {
                    choice = Some(0);
                }
                if u.button("Discard changes").clicked() {
                    choice = Some(1);
                }
                if u.button("Cancel").clicked() {
                    choice = Some(2);
                }
            });
            if s.status.starts_with("Save failed:") {
                u.colored_label(egui::Color32::LIGHT_RED, &s.status);
            }
        });
        match choice {
            Some(0) if s.save() => {
                let pending = s.pending.take().unwrap();
                execute(s, pending, &mut exit);
            }
            Some(1) => {
                let pending = s.pending.take().unwrap();
                execute(s, pending, &mut exit);
            }
            Some(2) => {
                s.pending = None;
            }
            _ => {}
        }
    }
    Ok(())
}
fn select(selected: &mut Vec<usize>, index: usize, shift: bool) {
    if !shift {
        *selected = vec![index];
    } else if let Some(position) = selected.iter().position(|&i| i == index) {
        selected.remove(position);
    } else {
        selected.push(index);
    }
}

fn zoom_scroll_offset(
    cursor_in_viewport: egui::Vec2,
    image_point: egui::Vec2,
    new_size: egui::Vec2,
) -> egui::Vec2 {
    egui::vec2(
        (image_point.x * new_size.x - cursor_in_viewport.x).max(0.),
        (image_point.y * new_size.y - cursor_in_viewport.y).max(0.),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_anchored_zoom_keeps_the_texture_point_under_the_cursor() {
        let cursor = egui::vec2(150., 80.);
        let image_point = egui::vec2(0.5, 0.25);
        let new_size = egui::vec2(600., 400.);
        let offset = zoom_scroll_offset(cursor, image_point, new_size);
        assert_eq!(offset, egui::vec2(150., 20.));
        assert_eq!(offset + cursor, egui::vec2(300., 100.));
    }
}
