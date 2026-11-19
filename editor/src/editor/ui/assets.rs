//! The Assets panel: the folder tree, the search field and the drop hint.
use super::*;

/// The Assets view's drop hint (the whole window takes dropped files); it lights up while files are dragged over.
pub(super) fn drop_hint(ui: &mut Ui, hot: bool) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), Sense::hover());
    let t = theme::spring(ui.ctx(), Id::new("drop-hot"), if hot { 1.0 } else { 0.0 }, theme::SNAPPY).clamp(0.0, 1.0);
    let p = ui.painter();
    if t > 0.0 { p.rect_filled(r, CornerRadius::same(12), look().highlight.gamma_multiply(0.12 * t)); }
    // a dashed rounded outline
    let rr = r.shrink(0.75);
    let rad = 12.0;
    let mut pts = vec![];
    for (c, a0) in [(pos2(rr.right() - rad, rr.top() + rad), -90.0f32), (pos2(rr.right() - rad, rr.bottom() - rad), 0.0), (pos2(rr.left() + rad, rr.bottom() - rad), 90.0), (pos2(rr.left() + rad, rr.top() + rad), 180.0)] {
        for i in 0..=8 {
            let a = (a0 + 90.0 * i as f32 / 8.0).to_radians();
            pts.push(c + vec2(a.cos(), a.sin()) * rad);
        }
    }
    pts.push(pts[0]);
    p.extend(egui::Shape::dashed_line(&pts, Stroke::new(1.5, look().highlight.gamma_multiply(0.6 + 0.4 * t)), 6.0, 4.0));
    p.text(r.center(), Align2::CENTER_CENTER, if hot { "Drop to import" } else { "Drop files anywhere" }, semi(13.0), Color32::from_rgb(0xE6, 0xE6, 0xE8));
    ui.add_space(4.0);
}

/// The project's assets by folder (a row each, with how many it holds; click to open): models to place, materials
/// to change or drag, textures to drag onto faces.
pub(super) fn asset_tree(ui: &mut Ui, ed: &mut Editor) {
    let mut assets = ed.assets.clone();
    // materials as the editor holds them (new and changed ones are saved with the project)
    if let Some(m) = assets.iter_mut().find(|(f, _)| *f == "materials") {
        m.1 = ed.level.materials.keys().map(|k| k.trim_start_matches("assets/materials/").to_string()).collect();
    }
    // sounds likewise, then the recordings on disk
    if let Some(m) = assets.iter_mut().find(|(f, _)| *f == "sounds") {
        let recordings: Vec<String> = m.1.iter().filter(|f| level::is_recording(f)).cloned().collect();
        m.1 = ed.level.sounds.keys().map(|k| k.trim_start_matches("assets/sounds/").to_string()).chain(recordings).collect();
    }
    // the search: every word must be in a file's name (later: its tags and pips); folders with no match drop out and the
    // others open
    search_field(ui, &mut ed.assets_query, "Search assets");
    let words: Vec<String> = ed.assets_query.split_whitespace().map(str::to_lowercase).collect();
    let searching = !words.is_empty();
    for (_, files) in assets.iter_mut() {
        files.retain(|f| { let n = f.to_lowercase(); words.iter().all(|w| n.contains(w.as_str())) });
    }
    if searching && assets.iter().all(|(_, f)| f.is_empty()) {
        ui.add_space(14.0);
        ui.vertical_centered(|ui| ui.label(egui::RichText::new(format!("No assets match “{}”", ed.assets_query.trim())).color(MUTED)));
    }
    let row = |ui: &mut Ui, indent: f32, sense: Sense| -> (ERect, egui::Response) {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), sense);
        if resp.hovered() { ui.painter().rect_filled(r, CornerRadius::same(7), theme::FIELD); }
        (r.with_min_x(r.left() + indent), resp)
    };
    for (folder, files) in &assets {
        if searching && files.is_empty() { continue; }
        let id = Id::new(("asset-folder", *folder));
        let open: bool = searching || ui.data(|d| d.get_temp(id)).unwrap_or(*folder == "textures");
        let (r, resp) = row(ui, 10.0, Sense::click());
        frame::paint_icon(ui.painter(), if open { "down" } else { "right" }, pos2(r.left() + 5.0, r.center().y), 12.0, if open { TEXT } else { MUTED });
        ui.painter().text(pos2(r.left() + 18.0, r.center().y), Align2::LEFT_CENTER, format!("{}{}", folder[..1].to_uppercase(), &folder[1..]), body(12.5), if open { Color32::WHITE } else { Color32::from_rgb(0xDC, 0xDC, 0xE0) });
        ui.painter().text(pos2(r.right() - 10.0, r.center().y), Align2::RIGHT_CENTER, files.len().to_string(), body(12.5), MUTED);
        if resp.clicked() && !searching { ui.data_mut(|d| d.insert_temp(id, !open)); }
        if !open { continue; }
        if *folder == "materials" && !searching {
            let (r, resp) = row(ui, 30.0, Sense::click());
            frame::paint_icon(ui.painter(), "plus", pos2(r.left() + 7.0, r.center().y), 14.0, theme::accent());
            ui.painter().text(pos2(r.left() + 22.0, r.center().y), Align2::LEFT_CENTER, "New material", body(12.5), theme::accent());
            if resp.tip("A plain material: give it a texture, tint, shine or glow").clicked() {
                ed.material_open = Some(ed.level.new_material("Material", level::Material::default()));
            }
        }
        if *folder == "sounds" && !searching {
            let (r, resp) = row(ui, 30.0, Sense::click());
            frame::paint_icon(ui.painter(), "plus", pos2(r.left() + 7.0, r.center().y), 14.0, theme::accent());
            ui.painter().text(pos2(r.left() + 22.0, r.center().y), Align2::LEFT_CENTER, "New sound", body(12.5), theme::accent());
            if resp.tip("A sound file: add recordings, then set its volume, pitch and range (in the Audio tab)").clicked() {
                ed.sound_open = Some(ed.level.new_sound("Sound", level::Sound::default()));
                ed.tab = audio::AUDIO;
                ed.dock_view = 0;
            }
        }
        for f in files {
            let path = format!("assets/{folder}/{f}");
            if *folder == "sounds" {
                // sound files: click to change, drag (or Place) to put one in the level; recordings: click or drag to place
                let file = f.ends_with(".bev");
                let (r, resp) = row(ui, 30.0, Sense::click_and_drag());
                let open = ed.sound_open.as_deref() == Some(path.as_str());
                frame::paint_icon(ui.painter(), if file { "wave" } else { "play" }, pos2(r.left() + 7.0, r.center().y), 14.0, if file { theme::CAT_ACTIONS } else { MUTED });
                ui.painter().text(pos2(r.left() + 22.0, r.center().y), Align2::LEFT_CENTER, f.trim_end_matches(".bev"), body(12.5), if open { theme::CAT_ACTIONS } else { Color32::from_rgb(0xDC, 0xDC, 0xE0) });
                let place = |ed: &mut Editor, sound: String| {
                    ed.island.show("♪", format!("Placing {}", sound_name(&sound)), "Click a surface · Esc cancels", Tone::Accent);
                    ed.placing = Some(Shape::Sound);
                    ed.placing_model = Some(sound);
                };
                if ed.tab == audio::AUDIO {
                    // the Audio tab: open it (a recording gets a sound of its own)
                    let resp = resp.tip(if file { "Click to open it" } else { "A recording: click to open a sound playing it" });
                    if resp.clicked() { ed.sound_open = Some(if file { path } else { ed.level.sound_for_clip(&path) }); ed.dock_view = 0; }
                } else if file {
                    let resp = resp.on_hover_cursor(egui::CursorIcon::Grab).tip("Double-click to open it in the Audio tab · drag into the view to place a Sound part");
                    if resp.double_clicked() { ed.sound_open = Some(path); ed.tab = audio::AUDIO; ed.dock_view = 0; } else if resp.drag_started() { place(ed, path); }
                } else {
                    let resp = resp.on_hover_cursor(egui::CursorIcon::Grab).tip("A recording: click, then click a surface, to place a Sound part playing it");
                    if resp.clicked() || resp.drag_started() { let s = ed.level.sound_for_clip(&path); place(ed, s); }
                }
                continue;
            }
            let model = crate::models::is_model(f);
            let texture = crate::editor::texture::is_texture(f);
            let material = *folder == "materials";
            let sense = if model { Sense::click() } else if material || texture { Sense::click_and_drag() } else { Sense::hover() };
            let (r, resp) = row(ui, 30.0, sense);
            let shader = *folder == "shaders";
            let (icon, colour) = if model { ("cube", theme::CAT_SHAPES) } else if material { ("palette", theme::VIOLET) } else if texture { ("grid", theme::VIOLET) } else if shader { ("spark", theme::VIOLET) } else { ("folder", MUTED) };
            let open = material && ed.material_open.as_deref() == Some(path.as_str());
            // a picture on the left: a texture's own, a material's texture (else its tint as a swatch)
            let pic = if texture || model { Some(Some(path.clone())) } else if material { ed.level.materials.get(&path).map(|m| m.texture.clone()) } else { None };
            let mut text_x = 22.0;
            if let Some(tex) = pic {
                let box_ = ERect::from_min_size(pos2(r.left() + 1.0, r.center().y - 12.5), vec2(25.0, 25.0));
                match tex.and_then(|t| ed.thumbs.get(ui.ctx(), &ed.dir, &t, &ed.bakes)) {
                    Some(id) => { ui.painter().image(id, box_, ERect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE); }
                    // a model's picture is still being taken: its icon meanwhile
                    None if model => frame::paint_icon(ui.painter(), "cube", box_.center(), 14.0, theme::CAT_SHAPES),
                    None => {
                        let tint = ed.level.materials.get(&path).map_or([0.4; 3], |m| m.tint);
                        ui.painter().rect_filled(box_, CornerRadius::same(5), Color32::from_rgb((tint[0] * 255.0) as u8, (tint[1] * 255.0) as u8, (tint[2] * 255.0) as u8));
                    }
                }
                if !model { ui.painter().rect_stroke(box_, CornerRadius::same(5), Stroke::new(1.0, white(30)), StrokeKind::Inside); }
                text_x = 35.0;
            } else {
                frame::paint_icon(ui.painter(), icon, pos2(r.left() + 7.0, r.center().y), 14.0, colour);
            }
            ui.painter().text(pos2(r.left() + text_x, r.center().y), Align2::LEFT_CENTER, f.trim_end_matches(".bev"), body(12.5), if open { theme::VIOLET } else { Color32::from_rgb(0xDC, 0xDC, 0xE0) });
            if model {
                // models: click to place
                if resp.tip("Click, then click a surface to place it").clicked() {
                    ed.placing = Some(Shape::Model);
                    ed.placing_model = Some(path);
                    ed.island.show("◆", format!("Placing {f}"), "Click a surface · Shift keeps it upright · Esc cancels", Tone::Accent);
                }
            } else if material {
                // materials: click to change, drag onto a face
                let resp = resp.on_hover_cursor(egui::CursorIcon::Grab).tip(if ed.brush_on { "Click to paint with it" } else { "Click to change it (every surface using it changes) · drag onto a face" });
                if resp.clicked() && ed.brush_on { ed.brush.material = path; ed.dock_view = 0; }
                else if resp.clicked() { ed.material_open = Some(path); } else if resp.drag_started() { ed.texturing = Some(path); }
            } else if shader {
                resp.tip("A shader: choose it on a material");
            } else if texture {
                // textures: drag onto a face (or click, then click a face)
                let resp = resp.on_hover_cursor(egui::CursorIcon::Grab).tip("Drag onto a face of a part (Shift: the whole part)");
                if resp.drag_started() || resp.clicked() { ed.texturing = Some(path); }
            }
        }
    }
}

/// A search field (the dock's): a drawn magnifier and a line of text.
pub(super) fn search_field(ui: &mut Ui, text: &mut String, hint: &str) {
    let focused = ui.memory(|m| m.focused()).is_some_and(|f| f == ui.next_auto_id());
    egui::Frame::new()
        .fill(white(10))
        .stroke(Stroke::new(1.0, if focused { Color32::from_rgba_unmultiplied(0xf5, 0xdc, 0x4a, 150) } else { LINE }))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (ir, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                let c = ir.center() - vec2(1.0, 1.0);
                ui.painter().circle_stroke(c, 4.5, Stroke::new(1.4, MUTED));
                ui.painter().line_segment([c + vec2(3.3, 3.3), c + vec2(6.5, 6.5)], Stroke::new(1.6, MUTED));
                ui.add(egui::TextEdit::singleline(text).hint_text(hint).frame(egui::Frame::NONE).font(body(13.0)).desired_width(ui.available_width()));
            });
        });
    ui.add_space(4.0);
}
