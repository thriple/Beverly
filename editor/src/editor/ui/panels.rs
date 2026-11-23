//! The dock's panels: the inspector, the level panel, the pen and brush panels, the scene list, the material panel and the question after a change to a part with instances.
use super::*;

/// The inspector (bottom middle): the selected part's settings, or the open material's.
pub(super) fn inspector(ui: &mut Ui, ed: &mut Editor, view: &mut View, exit: &mut MessageWriter<AppExit>) {
    if let Some(path) = ed.material_open.clone().filter(|p| ed.level.materials.contains_key(p)) {
        material_panel(ui, ed, &path);
        return;
    }
    let Some(id) = ed.selected.filter(|id| ed.level.get(*id).is_some()) else {
        small_caps(ui, "INSPECTOR");
        ui.label(egui::RichText::new("Nothing selected: click a part to see its settings").color(MUTED));
        theme::hint(ui, "Parts", &["Tab"]);
        return;
    };
    let mut p = ed.level.get(id).unwrap().clone();
    let before = p.clone();
    let mut delete = false;
    let mut look = false;
    let mut unwire: Option<usize> = None;
    let mut ranges = ed.level.ranges.clone();
    let mut ranges_changed = false;
    { ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
        let (_, icon, colour) = kind(p.shape);
        let file = p.model.as_deref().and_then(|m| m.rsplit('/').next()).map(|f| format!(" · {f}")).unwrap_or_default();
        // instances: the parts with the same contents (shape, colour, textures, Visible, Collision)
        let n = ed.level.instances(id).len();
        let inst = if p.unique { " · Unique".to_string() } else if ed.level.instanced(&p) && n > 0 { format!(" · {} alike", n + 1) } else { String::new() };
        let sub = format!("{} · {}{file}{inst}", kind(p.shape).0.trim_end_matches('s'), p.shape.label());
        let (mut dup, mut del) = (false, false);
        identity(ui, colour, icon, Ok(&mut p.name), &sub, &mut [("copy", "Duplicate", &["Ctrl", "D"], &mut dup), ("trash", "Delete", &["Del"], &mut del)]);
        if dup { ed.duplicate_selected(); }
        if del { delete = true; }
        // the rest is drawn from the part's setting descriptions (level.rs, settings.rs)
        let model = p.shape == Shape::Model;
        let scale = [("size", Setting::number("Scale", "×", 0.01, 0.001, 100000.0).section(level::LOOK).bare())];
        let mut ctx = settings::Ctx { replace: if model { &scale } else { &[] }, ranges: &mut ranges, ranges_changed: false, palette: project_palette(&ed.level) };
        part_sections(ui, ed, &mut p, model, &mut ctx, &mut look);
        texture_group(ui, ed, &mut p);
        sound_group(ui, ed, &mut p);
        wire_group(ui, ed, &p, id, &mut unwire);
        advanced_group(ui, ed, &mut p, &mut ctx);
        ranges_changed = ctx.ranges_changed;
    });
    }
    if let Some(i) = unwire { ed.level.wires.remove(i); }
    if ranges_changed { ed.level.ranges = ranges; }
    if look { let _ = actions::run(ed, Some(view), "view.look_through", exit); }
    if delete {
        ed.delete_selected();
    } else if p != before {
        if p.mover.is_none() {
            // its inputs are gone: so are the wires into them
            ed.level.wires.retain(|w| w.to != id || p.inputs().contains(&w.input.as_str()));
            let outs = p.outputs();
            ed.level.wires.retain(|w| w.from != id || outs.contains(&w.out.as_str()));
        }
        if let Some(m) = ed.level.get_mut(id) { *m = p; }
    }
}

/// The part's settings, a group for each section its setting descriptions name (a light, a camera or a speaker
/// gets its own in place of size and look).
fn part_sections(ui: &mut Ui, ed: &mut Editor, p: &mut level::Part, model: bool, ctx: &mut settings::Ctx, look: &mut bool) {
    for sec in settings::sections(&*p) {
        if sec == level::PART || (sec == level::MOVES && p.shape == Shape::PlayerStart) { continue; }
        // the player start, lights and cameras have no surface to tag
        if sec == level::TAGS { continue; }
        // a light or a camera: its settings in place of size and look (the cone for spots only)
        if sec == level::LOOK && p.shape.is_marker() { continue; }
        if sec == level::CAMERA {
            let orbit = p.shape == Shape::OrbitCamera;
            if let Some(c) = p.camera.as_mut() {
                column(ui, 250.0, "CAMERA", |ui| {
                    settings::section(ui, c, level::CAMERA, ctx);
                    if ui.button("Look through").tip_keys("Fly it to aim it", &["L"]).clicked() { *look = true; }
                });
                if orbit { column(ui, 260.0, "ORBIT", |ui| { settings::section(ui, c, level::ORBIT, ctx); }); }
            }
            continue;
        }
        if sec == level::SPEAKER {
            let files: Vec<String> = ed.level.sounds.keys().cloned().collect();
            if let Some(s) = p.sound.as_mut() {
                column(ui, 260.0, "SOUND", |ui| {
                    let shown = if s.sound.is_empty() { "None yet".to_string() } else { sound_name(&s.sound) };
                    egui::ComboBox::from_id_salt("part sound").selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
                        for f in &files { ui.selectable_value(&mut s.sound, f.clone(), sound_name(f)); }
                    }).response.tip("The sound file it plays (Assets > Sounds)");
                    settings::section(ui, s, level::SPEAKER, ctx);
                    if !s.sound.is_empty() && ui.button("Edit in the Audio tab").tip("Its recordings, volume, pitch and range (every part playing it changes)").clicked() {
                        ed.sound_open = Some(s.sound.clone());
                        ed.tab = audio::AUDIO;
                        ed.dock_view = 0;
                    }
                });
            }
            continue;
        }
        if sec == level::LIGHT {
            let spot = p.shape == Shape::SpotLight;
            if let Some(l) = p.light.as_mut() {
                column(ui, 260.0, "LIGHT", |ui| { settings::section(ui, l, level::LIGHT, ctx); });
                if spot { column(ui, 250.0, "CONE", |ui| { settings::section(ui, l, level::CONE, ctx); }); }
            }
            continue;
        }
        let title = if model && sec == level::LOOK { "SCALE  ·  LOOK" } else { sec };
        column(ui, if sec == level::LOOK { 260.0 } else { 250.0 }, title, |ui| { settings::section(ui, p, sec, ctx); });
    }
    // attached to another part: it goes where that one goes
    column(ui, 250.0, "ATTACHED TO", |ui| {
        let shown = p.parent.and_then(|id| ed.level.get(id)).map_or("Nothing".to_string(), |q| q.name.clone());
        egui::ComboBox::from_id_salt("part parent").selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
            let below = ed.level.descendants(p.id);
            ui.selectable_value(&mut p.parent, None, "Nothing");
            for q in ed.level.parts.iter().filter(|q| q.id != p.id && !below.contains(&q.id)) { ui.selectable_value(&mut p.parent, Some(q.id), &q.name); }
        }).response.tip("The part it is attached to: when that one moves or turns, this one goes with it");
    });
}

/// Parts in the order of a tree: each followed by the parts attached to it (those that are in the list), with how far in
/// each is. A part whose parent is not in the list starts a branch of its own.
fn nested<'a>(rows: Vec<&'a level::Part>) -> Vec<(&'a level::Part, usize)> {
    let ids: std::collections::HashSet<u64> = rows.iter().map(|p| p.id).collect();
    let mut kids: std::collections::HashMap<u64, Vec<&level::Part>> = std::collections::HashMap::new();
    for p in &rows { if let Some(par) = p.parent.filter(|q| ids.contains(q)) { kids.entry(par).or_default().push(p); } }
    fn walk<'a>(p: &'a level::Part, depth: usize, kids: &std::collections::HashMap<u64, Vec<&'a level::Part>>, out: &mut Vec<(&'a level::Part, usize)>) {
        out.push((p, depth));
        for c in kids.get(&p.id).into_iter().flatten() { walk(c, depth + 1, kids, out); }
    }
    let mut out = vec![];
    for p in rows.iter().filter(|p| p.parent.is_none_or(|q| !ids.contains(&q))) { walk(p, 0, &kids, &mut out); }
    out
}

/// The textures laid on the part: each with its size and slide, and a button to take it off.
fn texture_group(ui: &mut Ui, ed: &Editor, p: &mut level::Part) {
    if !p.surfaces.is_empty() {
        column(ui, 250.0, "TEXTURES", |ui| {
            let names = ed.geo.shapes[&p.shape].face_names();
            let mut remove = None;
            for (i, s) in p.surfaces.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    let face = s.face.map_or("All over".to_string(), |f| names.get(f as usize).cloned().unwrap_or_default());
                    let mat = std::path::Path::new(&s.material).file_stem().map(|m| m.to_string_lossy().into_owned()).unwrap_or_default();
                    ui.label(egui::RichText::new(format!("{face} · {mat}")).color(TEXT));
                    if ui.small_button("✕").tip("Take this texture off").clicked() { remove = Some(i); }
                });
                for (label, v, help) in [("Size", &mut s.tile, "How big one copy of the texture is (across, down)"), ("Slide", &mut s.offset, "How far the texture is slid along the surface")] {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(label).color(MUTED)).tip(help);
                        let lo = if label == "Size" { 0.05 } else { -1000.0 };
                        for x in v.iter_mut() { ui.add(egui::DragValue::new(x).speed(0.01).range(lo..=1000.0).max_decimals(2).suffix(" m")); }
                    });
                }
            }
            if let Some(i) = remove { p.surfaces.remove(i); }
        });
    }
}

/// What the part plays when things happen to it.
fn sound_group(ui: &mut Ui, ed: &Editor, p: &mut level::Part) {
    // what it plays when things happen to it
    let events = p.events();
    if !events.is_empty() {
        let files: Vec<String> = ed.level.sounds.keys().cloned().collect();
        column(ui, 0.0, "SOUNDS", |ui| {
            for e in events {
                let mut chosen = p.sound_on(e).unwrap_or_default().to_string();
                if sound_row(ui, e, &mut chosen, &files, &format!("What it plays when it's {}", event_words(e))) {
                    p.sounds.retain(|h| h.on != e);
                    if !chosen.is_empty() { p.sounds.push(level::Hook { on: e.into(), sound: chosen }); }
                }
            }
        });
    }
}

/// The part's wires: its outputs to wire from, the wires that touch it (with a button to remove each).
fn wire_group(ui: &mut Ui, ed: &mut Editor, p: &level::Part, id: u64, unwire: &mut Option<usize>) {
    let outs = p.outputs();
    let has_wires = ed.level.wires.iter().any(|w| w.from == id || w.to == id);
    if !outs.is_empty() || !p.inputs().is_empty() || has_wires {
        column(ui, 260.0, "WIRES", |ui| {
            let name_of = |pid: u64| ed.level.get(pid).map(|q| q.name.clone()).unwrap_or_default();
            for out in &outs {
                ui.horizontal(|ui| {
                    port(ui, &format!("{out} ▸"), theme::CAT_TRIGGERS);
                    let wiring = ed.wiring == Some((id, out));
                    if ui.small_button(if wiring { "click a part…" } else { "+ wire" }).tip("Then click the part it goes to").clicked() {
                        ed.wiring = if wiring { None } else { Some((id, out)) };
                    }
                });
            }
            for (i, w) in ed.level.wires.iter().enumerate() {
                let text = if w.from == id {
                    format!("{} → {} · {}", w.out, name_of(w.to), w.input)
                } else if w.to == id {
                    format!("{} ← {} · {}", w.input, name_of(w.from), w.out)
                } else {
                    continue;
                };
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(text).color(MUTED));
                    if ui.small_button("✕").tip("Remove this wire").clicked() { *unwire = Some(i); }
                });
            }
            if !p.inputs().is_empty() && !ed.level.wires.iter().any(|w| w.to == id) {
                ui.horizontal(|ui| {
                    for i in p.inputs() { port(ui, &format!("▸ {i}"), theme::CAT_ACTIONS); }
                });
            }
        });
    }
}

/// Rarely used: Unique, and the surface's tags.
fn advanced_group(ui: &mut Ui, ed: &Editor, p: &mut level::Part, ctx: &mut settings::Ctx) {
    // rarely used: Unique, and its surface's tags (the player start, lights and cameras have no surface to tag)
    let tags = !(p.shape == Shape::PlayerStart || p.shape.is_marker());
    group(ui, "ADVANCED", Some(1 + 2 * tags as usize), |ui| {
        ui.horizontal(|ui| {
            let help = if p.unique {
                "Its contents are its own"
            } else if !ed.level.instanced(&p) {
                "Lights, cameras, moving and wired parts are always individual"
            } else {
                "Parts with the same shape, colour and textures are instances; changing one asks first"
            };
            ui.label(egui::RichText::new("Unique").color(TEXT_2)).tip(help);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                theme::toggle(ui, &mut p.unique).tip("On: changing its look never changes the parts just like it");
            });
        });
        if tags {
            settings::section(ui, p, level::TAGS, ctx);
            ground_row(ui, "part sounds like", &mut p.sounds_like, "What its surface sounds like underfoot: steps play their recordings for it");
        }
    });
}

/// The pen panel's "Paints" row: the material a strip or fill lays, chosen from the project's.
fn material_row(ui: &mut Ui, salt: &str, ed: &Editor, chosen: &mut String) {
    let file = |p: &str| std::path::Path::new(p).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(vec2(76.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_min_width(76.0); ui.label(egui::RichText::new("Paints").color(TEXT_2)); });
        let shown = if chosen.is_empty() { "No material".to_string() } else { file(chosen) };
        egui::ComboBox::from_id_salt(salt).selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
            for m in ed.level.materials.keys() { ui.selectable_value(chosen, m.clone(), file(m)); }
            if ed.level.materials.is_empty() { ui.label(egui::RichText::new("No materials yet: + New material in Assets").color(MUTED)); }
        });
    });
}

/// The bottom-left panel: what the Level tab's topic (its menu) sets, or the level and its topics.
pub(super) fn mode_panel(ui: &mut Ui, ed: &mut Editor) {
    let mut l = ed.level.lighting.clone();
    let mut n = ed.level.navigation.clone();
    let mut build = false;
    let mut ranges = ed.level.ranges.clone();
    let mut ctx = settings::Ctx { replace: &[], ranges: &mut ranges, ranges_changed: false, palette: project_palette(&ed.level) };
    // the level (or its topic) at the top, as a part is when one is selected
    let parts = ed.level.parts.len();
    let (name, icon, colour) = match ed.level_mode {
        LevelMode::Build => (ed.level.name.clone(), "mountain", highlight()),
        LevelMode::Lighting => ("Lighting".into(), "bulb", theme::CAT_LIGHTS),
        LevelMode::Physics => ("Physics".into(), "feel", theme::CAT_FLOW),
        LevelMode::Cameras => ("Cameras".into(), "camera", theme::CAT_CAMERAS),
        LevelMode::Foliage => ("Foliage".into(), "leaf", theme::CAT_FLOW),
    };
    let sub = if ed.level_mode == LevelMode::Build { format!("Level · {parts} part{}", if parts == 1 { "" } else { "s" }) } else { format!("Level · {}", ed.level.name) };
    identity(ui, colour, icon, Err(&name), &sub, &mut []);
    match ed.level_mode {
        LevelMode::Build => {
            ui.label(egui::RichText::new("Click a part in the view to inspect it, or press Tab to place one.").color(TEXT_2));
            ui.add_space(8.0);
            column(ui, 0.0, "SHORTCUTS", |ui| {
                theme::hint(ui, "Parts", &["Tab"]);
                theme::hint(ui, "Fly / edit", &["Caps Lock"]);
                theme::hint(ui, "Test", &["`"]);
                theme::hint(ui, "Save", &["Ctrl", "S"]);
            });
            // the walker's sounds (until characters have their own)
            let files: Vec<String> = ed.level.sounds.keys().cloned().collect();
            let mut w = ed.level.walker.clone();
            column(ui, 0.0, "WALKER SOUNDS", |ui| {
                for e in level::WalkerSounds::EVENTS {
                    let help = match e { "Step" => "Each step (its recordings for the surface underfoot)", "Jump" => "Leaving the ground", _ => "Coming down from a jump or a fall" };
                    sound_row(ui, e, w.get_mut(e).expect("one of its events"), &files, help);
                }
            });
            if w != ed.level.walker { ed.level.walker = w; }
        }
        LevelMode::Lighting => {
            column(ui, 0.0, "LEVEL · LIGHTING", |ui| { settings::section(ui, &mut *l, level::LIGHTING, &mut ctx); });
            // Interior: no sun
            if !l.interior { column(ui, 0.0, level::SUN, |ui| { settings::section(ui, &mut *l, level::SUN, &mut ctx); }); }
        }
        LevelMode::Physics => {
            // where enemies can walk: who for, Build, and how it stands
            column(ui, 0.0, "LEVEL · PHYSICS · NAVIGATION", |ui| {
                settings::section(ui, &mut *n, level::NAVIGATION, &mut ctx);
                ui.add_space(2.0);
                let (status, colour) = match &ed.nav {
                    None => ("Not built yet".to_string(), MUTED),
                    Some(_) if ed.nav_out_of_date() => ("Out of date: the level changed".to_string(), theme::DANGER),
                    Some((m, _)) => {
                        let k = crate::nav::islands(m).len();
                        (format!("{:.0} m² · {k} area{}", crate::nav::area(m), if k == 1 { "" } else { "s" }), theme::OK)
                    }
                };
                ui.label(egui::RichText::new(status).color(colour));
                ui.horizontal(|ui| {
                    if theme::main_button(ui, if ed.nav.is_some() { "Build again" } else { "Build navigation" }).tip("Work out where characters of this size can walk, across floors, stairs and ramps").clicked() { build = true; }
                    if ed.nav.is_some() {
                        theme::toggle(ui, &mut ed.nav_shown).tip("Draw the walkable area over the level");
                        ui.label(egui::RichText::new("Show").color(MUTED));
                    }
                });
                if ed.finder.is_some() {
                    let on = ed.path_try.is_some();
                    if ui.add(egui::Button::selectable(on, "Try a path")).tip("Click a start, then an end, in the view: the way a character would walk").clicked() {
                        ed.path_try = if on { None } else { Some(Default::default()) };
                    }
                    if on { theme::hint(ui, "Stop", &["Esc"]); }
                }
            });
        }
        LevelMode::Foliage => {} // its own panel (`foliage.rs`)
        LevelMode::Cameras => {
            column(ui, 0.0, "LEVEL · CAMERAS", |ui| {
                let cams: Vec<(u64, String)> = ed.level.parts.iter().filter(|p| p.shape.is_camera()).map(|p| (p.id, format!("{}  {}", p.shape.glyph(), p.name))).collect();
                if cams.is_empty() { ui.label(egui::RichText::new("No cameras yet: the Cameras section of the parts pop-up (Tab)").color(MUTED)); }
                for (id, name) in cams {
                    let on = ed.selected == Some(id);
                    if ui.add(egui::Button::selectable(on, egui::RichText::new(name).color(if on { highlight() } else { TEXT })).frame(false)).clicked() { ed.selected = Some(id); ed.material_open = None; ed.sound_open = None; }
                }
                theme::hint(ui, "Look through the selected one", &["L"]);
            });
        }
    }
    if ctx.ranges_changed { ed.level.ranges = ranges; }
    if l != ed.level.lighting { ed.level.lighting = l; }
    if n != ed.level.navigation { ed.level.navigation = n; }
    if build { ed.build_navigation(); }
}

/// The pen's panel (in Inspect while the pen is out): the curve being edited, and what it paints along itself.
pub(super) fn pen_panel(ui: &mut Ui, ed: &mut Editor) {
    let cur = ed.pen.curve.and_then(|id| ed.level.curve(id)).cloned();
    let sub = match &cur {
        Some(c) => format!("Pen · {} point{}{}", c.points.len(), if c.points.len() == 1 { "" } else { "s" }, if c.closed { " · closed" } else { "" }),
        None => "Click a surface to start a curve".to_string(),
    };
    let mut name = cur.as_ref().map_or_else(|| "Pen".to_string(), |c| c.name.clone());
    let named = cur.is_some();
    identity(ui, theme::VIOLET, "route", if named { Ok(&mut name) } else { Err("Pen") }, &sub, &mut []);
    if let Some(c) = &cur {
        let mut strip = c.strip.clone();
        let mut fill = c.fill.clone();
        let mut path = c.path;
        // the selected point's stretch, when it is in the air: how far it sags
        let air = ed.pen.point.filter(|&i| i < c.segments() && c.stretch(i) == crate::curve::Stretch::Air);
        let mut hang = crate::curve::Hang { sag: air.map_or(0.0, |i| c.points[i].sag) };
        let mut ranges = ed.level.ranges.clone();
        let mut ctx = settings::Ctx { replace: &[], ranges: &mut ranges, ranges_changed: false, palette: vec![] };
        column(ui, 0.0, crate::curve::PATH, |ui| { settings::section(ui, &mut path, crate::curve::PATH, &mut ctx); });
        if air.is_some() { column(ui, 0.0, crate::curve::HANG, |ui| { settings::section(ui, &mut hang, crate::curve::HANG, &mut ctx); }); }
        column(ui, 0.0, crate::curve::STRIP, |ui| {
            let mut on = strip.is_some();
            let mut none = false;
            if settings::toggle_row(ui, "", "Paint a strip", &mut on, "Paints a material along the line as a brush would, kept in step with the curve", false, &mut none) {
                strip = on.then(|| crate::curve::Strip { material: if ed.brush.material.is_empty() { ed.level.materials.keys().next().cloned().unwrap_or_default() } else { ed.brush.material.clone() }, ..Default::default() });
            }
            if let Some(st) = strip.as_mut() {
                material_row(ui, "strip material", ed, &mut st.material);
                ui.add_space(8.0);
                settings::section(ui, st, crate::curve::STRIP, &mut ctx);
            }
        });
        column(ui, 0.0, crate::curve::FILL, |ui| {
            let mut on = fill.is_some();
            let mut none = false;
            if settings::toggle_row(ui, "", "Fill the inside", &mut on, "Paints a material over the ground the curve encloses, with a feathered edge, kept in step with the curve (an open curve is closed by a straight line)", false, &mut none) {
                fill = on.then(|| crate::curve::Fill { material: if ed.brush.material.is_empty() { ed.level.materials.keys().next().cloned().unwrap_or_default() } else { ed.brush.material.clone() }, ..Default::default() });
            }
            if let Some(f) = fill.as_mut() {
                material_row(ui, "fill material", ed, &mut f.material);
                ui.add_space(8.0);
                settings::section(ui, f, crate::curve::FILL, &mut ctx);
            }
        });
        if ctx.ranges_changed { ed.level.ranges = ranges; }
        if let Some(cm) = ed.level.curve_mut(c.id) {
            if strip != c.strip { cm.strip = strip; }
            if fill != c.fill { cm.fill = fill; }
            if path != c.path { cm.path = path; }
            if let Some(p) = air.and_then(|i| cm.points.get_mut(i)).filter(|p| p.sag != hang.sag) { p.sag = hang.sag; }
            if name != c.name && !name.is_empty() { cm.name = name; }
        }
    }
    column(ui, 0.0, "SHORTCUTS", |ui| {
        theme::hint(ui, "Corner point", &["Click"]);
        theme::hint(ui, "Smooth point", &["Drag"]);
        theme::hint(ui, "Break a handle", &["Alt", "Drag"]);
        theme::hint(ui, "Close the curve", &["Click", "First point"]);
        theme::hint(ui, "Finish", &["Enter"]);
        theme::hint(ui, "Delete a point or the curve", &["Del"]);
        theme::hint(ui, "Put it away", &["Esc"]);
    });
    if theme::main_button(ui, "Done").clicked() { ed.pen_on = false; ed.pen.reset(); }
}

/// The brush's panel (in Inspect while the brush is out): what it paints and how.
pub(super) fn brush_panel(ui: &mut Ui, ed: &mut Editor) {
    let name = |p: &str| std::path::Path::new(p).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let sub = if ed.brush.material.is_empty() { "Pick a material to paint with".to_string() } else { format!("Brush · painting {}", name(&ed.brush.material)) };
    identity(ui, theme::VIOLET, "palette", Err("Brush"), &sub, &mut []);
    brush_body(ui, ed);
}

/// The brush's settings, rules and shortcuts (the Foliage topic shows them under its Mixes).
pub(super) fn brush_body(ui: &mut Ui, ed: &mut Editor) {
    let name = |p: &str| std::path::Path::new(p).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut b = ed.brush.clone();
    let mut ranges = ed.level.ranges.clone();
    let mut ctx = settings::Ctx { replace: &[], ranges: &mut ranges, ranges_changed: false, palette: vec![] };
    column(ui, 0.0, "BRUSH", |ui| {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(vec2(76.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_min_width(76.0); ui.label(egui::RichText::new("Paints").color(TEXT_2)); });
            let shown = if b.material.is_empty() { "No material".to_string() } else { name(&b.material) };
            egui::ComboBox::from_id_salt("brush material").selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
                for m in ed.level.materials.keys() { ui.selectable_value(&mut b.material, m.clone(), name(m)); }
                if ed.level.materials.is_empty() { ui.label(egui::RichText::new("No materials yet: + New material in Assets").color(MUTED)); }
            });
        });
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(vec2(76.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_min_width(76.0); ui.label(egui::RichText::new("Preset").color(TEXT_2)).tip("Four brushes to switch between (1 to 4); each remembers how you left it"); });
            let mut k = b.preset;
            if theme::choices(ui, "brush-preset", &crate::editor::brush::PRESETS, &mut k) { b.pick(k); }
        });
        // a Mix's ground, unless this stroke says otherwise
        if ed.level.materials.get(&b.material).is_some_and(|m| m.ground.is_some() || !m.grass.is_empty()) {
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(vec2(76.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_min_width(76.0); ui.label(egui::RichText::new("Ground").color(TEXT_2)).tip("What it lays under the plants: the Mix's own, none, or another material for these strokes"); });
                let own = ed.level.materials.get(&b.material).and_then(|m| m.ground.as_deref()).map_or("None".to_string(), name);
                let shown = match b.ground.as_deref() { None => format!("{own} (the Mix's)"), Some("") => "None".to_string(), Some(g) => name(g) };
                egui::ComboBox::from_id_salt("brush ground").selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
                    ui.selectable_value(&mut b.ground, None, format!("{own} (the Mix's)"));
                    ui.selectable_value(&mut b.ground, Some(String::new()), "None");
                    for m in ed.level.materials.keys().filter(|m| **m != b.material) { ui.selectable_value(&mut b.ground, Some(m.clone()), name(m)); }
                });
            });
        }
        ui.add_space(8.0);
        settings::section(ui, &mut b, crate::editor::brush::BRUSH, &mut ctx);
    });
    column(ui, 0.0, level::RULES, |ui| {
        settings::section(ui, &mut b.rules, level::RULES, &mut ctx);
        ui.add_space(8.0);
        // only between two heights in the world
        let mut band = b.rules.band.is_some();
        let mut none = false;
        if settings::toggle_row(ui, "", "Only between heights", &mut band, "Paints only between two heights in the world, wherever the part goes", false, &mut none) {
            b.rules.band = band.then(|| { let y = ed.painting.over.map_or(0.0, |o| o.1.y); [y - 1.0, y + 1.0] });
        }
        if let Some([lo, hi]) = b.rules.band.as_mut() {
            settings::row(ui, "", "From", "The lowest it paints (metres)", false, &mut none, |ui| settings::number(ui, lo, 0.05, -1e6, 1e6, " m", None, ui.available_width()));
            settings::row(ui, "", "To", "The highest it paints (metres)", false, &mut none, |ui| settings::number(ui, hi, 0.05, -1e6, 1e6, " m", None, ui.available_width()));
        }
        // only on parts with a surface tag
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(vec2(76.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_min_width(76.0); ui.label(egui::RichText::new("Only on").color(TEXT_2)).tip("Paints only parts with this surface tag (their Tags, in Advanced)"); });
            let label = |t: Option<level::Tag>| t.map_or("Any surface".to_string(), |t| format!("{t:?}").chars().enumerate().flat_map(|(i, c)| if i > 0 && c.is_uppercase() { vec![' ', c.to_ascii_lowercase()] } else { vec![c] }).collect());
            egui::ComboBox::from_id_salt("brush only on").selected_text(label(b.rules.on)).width(ui.available_width()).show_ui(ui, |ui| {
                ui.selectable_value(&mut b.rules.on, None, label(None));
                for t in level::Tag::ALL { ui.selectable_value(&mut b.rules.on, Some(t), label(Some(t))); }
            });
        });
    });
    column(ui, 0.0, "SHORTCUTS", |ui| {
        theme::hint(ui, "Presets S to XL", &["1", "2", "3", "4"]);
        theme::hint(ui, "Smaller / larger", &["[", "]"]);
        theme::hint(ui, "Softer / harder", &["Shift", "[", "]"]);
        theme::hint(ui, "Size it in the view", &["Alt", "Right-drag"]);
        theme::hint(ui, "Erase", &["Alt"]);
        theme::hint(ui, "Put it away", &["Esc"]);
    });
    if theme::main_button(ui, "Done").clicked() { ed.brush_on = false; }
    if ctx.ranges_changed { ed.level.ranges = ranges; }
    if b != ed.brush { b.keep(); ed.brush = b; }
}

/// The bucket fill's panel (in Inspect while it is out): what it lays (the brush's material, strength and Lays), what
/// it fills, and the fills in the level.
pub(super) fn bucket_panel(ui: &mut Ui, ed: &mut Editor) {
    let name = |p: &str| std::path::Path::new(p).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let sub = match &ed.filling.over {
        Some((_, reach)) => format!("Bucket · a click fills {} part{}", reach.len(), if reach.len() == 1 { "" } else { "s" }),
        None if ed.brush.material.is_empty() => "Pick a material to fill with".to_string(),
        None => format!("Bucket · filling with {}", name(&ed.brush.material)),
    };
    identity(ui, theme::VIOLET, "palette", Err("Bucket fill"), &sub, &mut []);
    let mut b = ed.brush.clone();
    let mut t = ed.bucket.clone();
    let mut ranges = ed.level.ranges.clone();
    let mut ctx = settings::Ctx { replace: &[], ranges: &mut ranges, ranges_changed: false, palette: vec![] };
    let tag_label = |t: Option<level::Tag>| t.map_or("The clicked part's".to_string(), |t| format!("{t:?}").chars().enumerate().flat_map(|(i, c)| if i > 0 && c.is_uppercase() { vec![' ', c.to_ascii_lowercase()] } else { vec![c] }).collect());
    column(ui, 0.0, crate::editor::bucket::BUCKET, |ui| {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(vec2(76.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_min_width(76.0); ui.label(egui::RichText::new("Fills with").color(TEXT_2)).tip("The material it lays (the brush's)"); });
            let shown = if b.material.is_empty() { "No material".to_string() } else { name(&b.material) };
            egui::ComboBox::from_id_salt("bucket material").selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
                for m in ed.level.materials.keys() { ui.selectable_value(&mut b.material, m.clone(), name(m)); }
                if ed.level.materials.is_empty() { ui.label(egui::RichText::new("No materials yet: + New material in Assets").color(MUTED)); }
            });
        });
        ui.add_space(8.0);
        settings::section(ui, &mut t, crate::editor::bucket::BUCKET, &mut ctx);
        if t.fills == level::Fills::Colour { settings::section(ui, &mut t, crate::editor::bucket::TOLERANCE, &mut ctx); }
        if t.fills == level::Fills::Tag {
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(vec2(76.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_min_width(76.0); ui.label(egui::RichText::new("Tag").color(TEXT_2)).tip("The surface tag it fills: every part with it"); });
                egui::ComboBox::from_id_salt("bucket tag").selected_text(tag_label(t.tag)).width(ui.available_width()).show_ui(ui, |ui| {
                    ui.selectable_value(&mut t.tag, None, tag_label(None));
                    for g in level::Tag::ALL { ui.selectable_value(&mut t.tag, Some(g), tag_label(Some(g))); }
                });
            });
        }
        ui.add_space(8.0);
        let mut layer = crate::editor::bucket::Layer { strength: b.strength, lays: b.lays };
        if settings::section(ui, &mut layer, crate::editor::bucket::LAYER, &mut ctx) { (b.strength, b.lays) = (layer.strength, layer.lays); }
    });
    // the fills in the level, newest last, each with its trash
    if !ed.level.buckets.is_empty() {
        let mut gone = None;
        column(ui, 0.0, "IN THE LEVEL", |ui| {
            for f in ed.level.buckets.iter() {
                let on = ed.level.get(f.part).map_or("a part that is gone".to_string(), |p| p.name.clone());
                let what = if f.material.is_empty() { "Erased".to_string() } else { name(&f.material) };
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{what} · {} from {on}", f.fills.word())).color(TEXT_2));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if super::audio_ui::trash(ui, "Take this fill out") { gone = Some(f.id); }
                    });
                });
            }
        });
        if let Some(id) = gone { ed.level.buckets.retain(|f| f.id != id); }
    }
    column(ui, 0.0, "SHORTCUTS", |ui| {
        theme::hint(ui, "Fill what matches", &["Click"]);
        theme::hint(ui, "Erase what matches", &["Alt", "Click"]);
        theme::hint(ui, "Put it away", &["Esc"]);
    });
    if theme::main_button(ui, "Done").clicked() { ed.bucket_on = false; }
    if ctx.ranges_changed { ed.level.ranges = ranges; }
    if t != ed.bucket { ed.bucket = t; }
    if b != ed.brush { b.keep(); ed.brush = b; }
}

/// A row choosing a sound file (or none) for `label`; true when it changed.
pub(super) fn sound_row(ui: &mut Ui, label: &str, chosen: &mut String, files: &[String], help: &str) -> bool {
    let before = chosen.clone();
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(vec2(76.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_min_width(76.0); ui.label(egui::RichText::new(label).color(TEXT_2)).tip(help); });
        let shown = if chosen.is_empty() { "None".to_string() } else { sound_name(chosen) };
        egui::ComboBox::from_id_salt(("sound row", label, ui.id())).selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
            ui.selectable_value(chosen, String::new(), "None");
            for f in files { ui.selectable_value(chosen, f.clone(), sound_name(f)); }
            if files.is_empty() { ui.label(egui::RichText::new("No sounds yet (Audio tab)").color(MUTED)); }
        });
    });
    *chosen != before
}

/// A row choosing what a surface sounds like underfoot.
pub(super) fn ground_row(ui: &mut Ui, id: &str, g: &mut Option<level::Ground>, help: &str) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(vec2(76.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_min_width(76.0); ui.label(egui::RichText::new("Sounds like").color(TEXT_2)).tip(help); });
        egui::ComboBox::from_id_salt(id).selected_text(g.map_or("Nothing special", |g| g.label())).width(ui.available_width()).show_ui(ui, |ui| {
            ui.selectable_value(g, None, "Nothing special");
            for x in level::Ground::ALL { ui.selectable_value(g, Some(x), x.label()); }
        });
    });
}

/// An event in words: "pressed", "hit", "starts moving".
pub(super) fn event_words(e: &str) -> String {
    match e { "Starts" => "starting to move".into(), "Hit" => "knocked into something".into(), _ => e.to_lowercase() }
}

/// The dock's list: the level's parts (those of the Level topic), found by name, in sections by kind, each with its
/// icon in its category colour; hidden parts and too many lights say so.
pub(super) fn scene_panel(ui: &mut Ui, ed: &mut Editor) {
    let shown: fn(&level::Part) -> bool = match ed.level_mode {
        LevelMode::Build => |_| true,
        LevelMode::Lighting => |p| p.shape.is_light(),
        LevelMode::Cameras => |p| p.shape.is_camera(),
        LevelMode::Physics => |p| crate::physics::kind(p).is_some(),
        LevelMode::Foliage => |p| crate::paint::paintable(p),
    };
    // the search field
    let qid = Id::new("scene-find");
    let mut q: String = ui.data(|d| d.get_temp(qid)).unwrap_or_default();
    theme::text_input(ui, &mut q, "Find in scene", Some("search"), ui.available_width());
    ui.data_mut(|d| d.insert_temp(qid, q.clone()));
    let q = q.to_lowercase();
    if ed.level.parts.is_empty() { ui.label(egui::RichText::new("Nothing placed yet. Press Tab.").color(MUTED).size(12.5)); }
    let over: Vec<u64> = { let w = crate::editor::light::budget(&ed.level); if w.len() > crate::editor::light::BUDGET { w } else { vec![] } };
    let mut pick = None;
    let mut flip = None;
    for sec in ["Shapes", "Models", "Triggers", "Game", "Lights", "Cameras"] {
        let rows: Vec<&level::Part> = ed.level.parts.iter().filter(|p| shown(p) && kind(p.shape).0 == sec && (q.is_empty() || p.name.to_lowercase().contains(&q))).collect();
        if rows.is_empty() { continue; }
        ui.add_space(4.0);
        ui.label(egui::RichText::new(sec).size(11.5).color(MUTED));
        for (p, depth) in nested(rows) {
            let (_, icon, colour) = kind(p.shape);
            let indent = depth as f32 * 14.0;
            // a branch of its own, though attached to a part of another kind: say which
            let label = match p.parent.filter(|_| depth == 0).and_then(|id| ed.level.get(id)) { Some(par) => format!("{} · on {}", p.name, par.name), None => p.name.clone() };
            let on = ed.selected == Some(p.id);
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
            let painter = ui.painter();
            if on { painter.rect_filled(r, CornerRadius::same(7), theme::SELECTED_ROW); } else if resp.hovered() { painter.rect_filled(r, CornerRadius::same(7), theme::FIELD); }
            frame::paint_icon(painter, icon, pos2(r.left() + indent + 10.0 + 7.0, r.center().y), 14.0, if p.visible { colour } else { colour.gamma_multiply(0.45) });
            painter.text(pos2(r.left() + indent + 34.0, r.center().y), Align2::LEFT_CENTER, &label, body(12.5), if on { Color32::WHITE } else if p.visible { Color32::from_rgb(0xDC, 0xDC, 0xE0) } else { MUTED });
            // on the right: a warning, or Hidden; the eye shows or hides it (hidden: a collider only)
            let eye = ERect::from_center_size(pos2(r.right() - 16.0, r.center().y), vec2(24.0, 24.0));
            let er = ui.interact(eye, Id::new(("eye", p.id)), Sense::click()).tip(if p.visible { "Hide it (it stays a collider)" } else { "Show it" });
            if resp.hovered() || er.hovered() || !p.visible {
                frame::paint_icon(ui.painter(), "eye", eye.center(), 14.0, if er.hovered() { TEXT } else { MUTED });
            }
            let note = if over.contains(&p.id) { Some(("Too many lights", theme::DANGER_TEXT)) } else if !p.visible { Some(("Hidden", MUTED)) } else { None };
            if let Some((n, c)) = note { ui.painter().text(pos2(eye.left() - 6.0, r.center().y), Align2::RIGHT_CENTER, n, body(11.5), c); }
            if er.clicked() { flip = Some(p.id); } else if resp.clicked() { pick = Some(p.id); }
        }
    }
    if let Some(id) = pick { ed.selected = Some(id); ed.material_open = None; ed.sound_open = None; }
    if let Some(id) = flip {
        if let Some(p) = ed.level.get_mut(id) { p.visible = !p.visible; }
    }
}

/// "Change all 12, or make this one Unique?" after a change to a part's contents that its instances didn't get.
pub(super) fn ask_card(ctx: &egui::Context, ed: &mut Editor, view: &View) {
    let Some((id, them)) = ed.ask.clone() else { return };
    let Some(name) = ed.level.get(id).map(|p| p.name.clone()) else { return };
    let key = Id::new("ask");
    // fade and scale in (SNAPPY), above the hints at the bottom of the view
    let t = theme::spring_from(ctx, key, 0.0, 1.0, theme::SNAPPY);
    let at = pos2(view.rect.center().x, view.rect.max.y - 58.0);
    let (mut all, mut unique) = (false, false);
    let resp = egui::Area::new(key).order(egui::Order::Foreground).pivot(Align2::CENTER_BOTTOM).fixed_pos(at).show(ctx, |ui| {
        ui.multiply_opacity(t.clamp(0.0, 1.0));
        egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, white(24))).corner_radius(R_CARD).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
            ui.label(egui::RichText::new(format!("{name} has {} like it", them.len())).font(semi(14.0)));
            ui.label(egui::RichText::new("Its instances share what is inside the box: shape, colour, textures, Visible, Collision").color(MUTED).size(12.0));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let button = |ui: &mut Ui, label: String, key: &str| {
                    let r = ui.add(egui::Button::new(egui::RichText::new(label).font(semi(13.0))).min_size(vec2(0.0, 28.0)));
                    theme::kbd(ui, key);
                    r.clicked()
                };
                all = button(ui, format!("Change all {}", them.len() + 1), "Enter");
                ui.add_space(10.0);
                unique = button(ui, "Make this one Unique".into(), "U");
                ui.add_space(10.0);
                theme::hint(ui, "Undo", &["Ctrl", "Z"]);
            });
        });
    });
    let s = 0.94 + 0.06 * t;
    let c = resp.response.rect.center_bottom();
    ctx.set_transform_layer(resp.response.layer_id, TSTransform::new(c.to_vec2() * (1.0 - s), s));
    if all { ed.settle(true); }
    if unique { ed.settle(false); }
}

/// A port chip: an output ("Pressed ▸") or an input ("▸ Go") in its category colour.
/// A material's settings: every surface using it changes with it.
pub(super) fn material_panel(ui: &mut Ui, ed: &mut Editor, path: &str) {
    let mut m = ed.level.materials[path].clone();
    let before = m.clone();
    let mut ranges = ed.level.ranges.clone();
    let ranges_changed;
    let (surfaces, parts) = ed.level.material_users(path);
    let name = path.trim_start_matches("assets/materials/").trim_end_matches(".bev").to_string();
    let textures: Vec<String> = ed.assets.iter().find(|(f, _)| *f == "textures").map(|(_, t)| t.iter().filter(|t| crate::editor::texture::is_texture(t)).cloned().collect()).unwrap_or_default();
    let (mut copy, mut close) = (false, false);
    let n = |n: usize, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
    identity(ui, theme::VIOLET, "palette", Err(&name), &format!("Material · on {} of {}", n(surfaces, "surface"), n(parts, "part")), &mut []);
    ui.horizontal(|ui| {
        if ui.button("Copy").tip("A new material like this one, to change on its own").clicked() { copy = true; }
        if ui.button("Close").tip_keys("Back to the selected part", &["Esc"]).clicked() { close = true; }
    });
    ui.add_space(8.0);
    column(ui, 0.0, "TEXTURE", |ui| {
        let shown = m.texture.as_deref().and_then(|t| t.rsplit('/').next()).unwrap_or("None (plain)").to_string();
        egui::ComboBox::from_id_salt("material texture").selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
            ui.selectable_value(&mut m.texture, None, "None (plain)");
            for t in &textures { ui.selectable_value(&mut m.texture, Some(format!("assets/textures/{t}")), t); }
        });
        ui.label(egui::RichText::new("Drag the material from Assets onto faces").color(MUTED).size(11.0));
    });
    let mut ctx = settings::Ctx { replace: &[], ranges: &mut ranges, ranges_changed: false, palette: project_palette(&ed.level) };
    column(ui, 0.0, "SHADER", |ui| {
        let files = crate::shader::list(&ed.dir);
        let shown = m.shader.as_deref().and_then(|s| s.rsplit('/').next()).unwrap_or("None (standard)").to_string();
        egui::ComboBox::from_id_salt("material shader").selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
            if ui.selectable_value(&mut m.shader, None, "None (standard)").clicked() { m.shader_values.clear(); }
            for f in &files { if ui.selectable_value(&mut m.shader, Some(f.clone()), f.trim_start_matches("assets/shaders/")).clicked() { m.shader_values.clear(); } }
        });
        if let Some(path) = m.shader.clone() {
            match ed.shader_info.get(&path) {
                Some((Some(h), _)) => { settings::shader_settings(ui, h, &mut m.shader_values, &ctx.palette); }
                Some((None, None)) | None => { ui.label(egui::RichText::new("Reading the shader…").color(MUTED).size(11.0)); }
                Some((None, Some(_))) => {}
            }
            if let Some((_, Some(e))) = ed.shader_info.get(&path) { ui.label(egui::RichText::new(e).color(theme::DANGER_TEXT).size(11.0)); }
        } else {
            ui.label(egui::RichText::new("A shader draws the material its own way: write one in assets/shaders").color(MUTED).size(11.0));
        }
    });
    column(ui, 0.0, "LOOK", |ui| {
        settings::section(ui, &mut m, level::MATERIAL, &mut ctx);
        ground_row(ui, "material sounds like", &mut m.sounds_like, "What it sounds like underfoot, wherever it's laid (else what the part says)");
    });
    column(ui, 0.0, "GRASS", |ui| {
        // the species it grows, each with its share, then how thick
        let names = crate::grass::species_names(&ed.level);
        let mut gone = None;
        for (i, g) in m.grass.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(("grass species", i)).selected_text(g.species.replace('_', " ")).width(ui.available_width() * 0.5).show_ui(ui, |ui| {
                    for n in &names { ui.selectable_value(&mut g.species, n.clone(), n.replace('_', " ")); }
                });
                let w = ui.available_width() - 30.0;
                settings::number(ui, &mut g.share, 0.01, 0.0, 1.0, "", None, w).on_hover_text("Its share of the blades");
                if super::audio_ui::trash(ui, "Grow none of this") { gone = Some(i); }
            });
        }
        if let Some(i) = gone { m.grass.remove(i); }
        if m.grass.len() < 4 && ui.button("+ Add grass").tip("Grass grows wherever this material is laid or painted").clicked() {
            let next = names.iter().find(|n| !m.grass.iter().any(|g| &g.species == *n)).cloned().unwrap_or_default();
            m.grass.push(level::Grows { species: next, share: if m.grass.is_empty() { 1.0 } else { 0.2 } });
        }
        if m.grass.is_empty() {
            ui.label(egui::RichText::new("Grows nothing. Add grass and it grows wherever the material is laid or painted").color(MUTED).size(11.0));
        } else {
            settings::section(ui, &mut m, level::GRASS, &mut ctx);
        }
    });
    column(ui, 0.0, "GROUND", |ui| {
        // what it lays under its plants, then how far and how softly
        let name = |p: &str| p.trim_start_matches("assets/materials/").trim_end_matches(".bev").to_string();
        let shown = m.ground.as_deref().map_or("None (its own colour)".to_string(), name);
        egui::ComboBox::from_id_salt("material ground").selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
            ui.selectable_value(&mut m.ground, None, "None (its own colour)");
            for k in ed.level.materials.keys().filter(|k| k.as_str() != path) { ui.selectable_value(&mut m.ground, Some(k.clone()), name(k)); }
        });
        if m.ground.is_some() {
            settings::section(ui, &mut m, level::GROUND, &mut ctx);
        } else {
            ui.label(egui::RichText::new("Pick a material and wherever this is painted it lays that ground under its plants, wider and softer than the plants").color(MUTED).size(11.0));
        }
    });
    ranges_changed = ctx.ranges_changed;
    if m != before { ed.level.materials.insert(path.to_string(), m.clone()); }
    if ranges_changed { ed.level.ranges = ranges; }
    if copy {
        let new = ed.level.new_material(&format!("{name} copy"), m);
        ed.island.show("◈", "Copied the material", new.trim_start_matches("assets/materials/").to_string(), theme::Tone::Accent);
        ed.material_open = Some(new);
    }
    if close || ui.input(|i| i.key_pressed(egui::Key::Escape)) { ed.material_open = None; }
}

pub(super) fn port(ui: &mut Ui, text: &str, color: Color32) {
    egui::Frame::new().fill(color.gamma_multiply(0.22)).stroke(Stroke::new(1.0, color.gamma_multiply(0.7))).corner_radius(CornerRadius { nw: 8, ne: 8, se: 8, sw: 3 }).inner_margin(egui::Margin::symmetric(7, 2)).show(ui, |ui| {
        ui.label(egui::RichText::new(text).font(semi(12.0)).color(TEXT));
    });
}
