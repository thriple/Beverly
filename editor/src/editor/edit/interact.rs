//! Mouse and keyboard work in the 3D view: placing, the widget's drags, selecting, picking, shortcuts.
use super::*;

#[derive(Default)]
pub struct Press { at: Option<Vec2> }

/// Mouse work in the 3D view: placing, the widget, selecting.
#[allow(clippy::too_many_arguments)]
pub fn interact(
    mut ed: ResMut<Editor>,
    view: Res<View>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    cam: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut ghost: Single<(&mut Transform, &mut Visibility, &mut Mesh3d), With<Ghost>>,
    lib: Res<ShapeLib>,
    mut press: Local<Press>,
    mut typed: MessageReader<KeyboardInput>,
) {
    let (cam, gtf) = *cam;
    ed.label = None;
    if ed.testing.is_some() {
        *ghost.1 = Visibility::Hidden;
        return;
    }
    // ---- looking through a camera part: it goes where the view goes, and nothing else is edited
    if ed.looking.is_some() {
        camera::follow(&mut ed, &view);
        *ghost.1 = Visibility::Hidden;
        return;
    }
    let pointer = view.pointer();
    let ray = pointer.and_then(|p| view.ray(cam, gtf, p));
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);

    // ---- Pick (I): the brush's material, or a part to place another of
    if std::mem::take(&mut ed.pick) { pick(&mut ed, ray); }

    // ---- the brush (B)
    if brush::input(&mut ed, &view, ray, &mouse, &keys) {
        *ghost.1 = Visibility::Hidden;
        press.at = None;
        return;
    }

    // ---- the pen (P)
    if pen::input(&mut ed, &view, ray, &mouse, &keys) {
        *ghost.1 = Visibility::Hidden;
        press.at = None;
        return;
    }

    // ---- a texture dragged from the Assets panel
    let cancel = keys.just_pressed(KeyCode::Escape) || mouse.just_pressed(MouseButton::Right);
    if texture::dropping(&mut ed, ray, pointer, mouse.just_released(MouseButton::Left), cancel, shift) {
        *ghost.1 = Visibility::Hidden;
        press.at = None;
        return;
    }

    // ---- placing a shape from the parts pop-up
    if place_step(&mut ed, &mut ghost, &lib, &view, ray, &mouse, &keys, &mut press, shift) { return; }
    *ghost.1 = Visibility::Hidden;

    // ---- wiring: click the part the wire goes to
    if wire_step(&mut ed, pointer, ray, &mouse, &keys, &mut press) { return; }

    // ---- trying a path: click its start, then its end (a third click starts again)
    if path_step(&mut ed, ray, &mouse, &keys, &mut press) { return; }

    // ---- the selected part's texture handles (they win over the widget when the pointer is on one)
    if !mouse.pressed(MouseButton::Left) { ed.drag = None; }
    let snap = ed.snap && !ctrl;
    if ed.drag.is_none() && texture::handles(&mut ed, &view, cam, gtf, ray, pointer, mouse.pressed(MouseButton::Left), mouse.just_pressed(MouseButton::Left), shift, snap) {
        ed.hover_handle = None;
        press.at = None;
        return;
    }
    // ---- the selected light's reach and cone handles
    if ed.drag.is_none() && light::handles(&mut ed, &view, cam, gtf, ray, pointer, mouse.pressed(MouseButton::Left), mouse.just_pressed(MouseButton::Left), snap) {
        ed.hover_handle = None;
        press.at = None;
        return;
    }

    // ---- the selected Orbit camera's target handle
    if ed.drag.is_none() && camera::handles(&mut ed, &view, cam, gtf, ray, pointer, mouse.pressed(MouseButton::Left), mouse.just_pressed(MouseButton::Left)) {
        ed.hover_handle = None;
        press.at = None;
        return;
    }

    // ---- the widget on the selected part
    let sel = ed.selected.and_then(|id| ed.level.get(id)).cloned();
    if drag_step(&mut ed, ray, pointer, ctrl, shift, &mut typed, &sel) { return; }

    // hover a handle, or start a drag
    if hover_step(&mut ed, &view, cam, gtf, ray, pointer, &mouse, shift, &mut typed, &mut press, &sel) { return; }

    // ---- click to select (a press and release without moving much)
    select_step(&mut ed, &view, ray, pointer, &mouse, &mut press);
}

/// Placing a shape from the parts pop-up: the ghost follows the pointer and a click puts it down. True while placing.
fn place_step(ed: &mut Editor, ghost: &mut (Mut<Transform>, Mut<Visibility>, Mut<Mesh3d>), lib: &ShapeLib, view: &View, ray: Option<Ray3d>, mouse: &ButtonInput<MouseButton>, keys: &ButtonInput<KeyCode>, press: &mut Press, shift: bool) -> bool {
    let Some(shape) = ed.placing else { return false };
    let (ref mut gt, ref mut vis, ref mut mesh) = *ghost;
    // a model's box comes from its file once loaded; until then a 1 m stand-in
    // (for a Sound part, the file is its sound's)
    let sound = ed.placing_model.clone().filter(|_| shape == Shape::Sound);
    let model = ed.placing_model.clone().filter(|_| shape == Shape::Model);
    let (lo, hi) = match &model {
        Some(m) => ed.geo.models.get(m).map(|t| t.bounds()).unwrap_or((vec3(-0.5, 0.0, -0.5), vec3(0.5, 1.0, 0.5))),
        None => (Vec3::splat(-0.5), Vec3::splat(0.5)),
    };
    if shape != Shape::Model { mesh.0 = lib.mesh(shape); }
    // the wheel turns it about the surface's normal (15 degrees a notch), Shift + wheel sizes it
    if view.wheel != 0.0 {
        if shift { ed.place_scale = (ed.place_scale * 1.1f32.powf(view.wheel)).clamp(0.05, 50.0); } else { ed.place_turn += view.wheel * 15f32.to_radians(); }
    }
    let (turn, scale) = if shape.is_camera() { (0.0, 1.0) } else { (ed.place_turn, ed.place_scale) };
    match ray.and_then(|r| ed.hit(r)) {
        Some(h) => {
            let size = Vec3::from(shape.default_size()) * scale;
            let n = if shift || shape == Shape::PlayerStart || shape.is_camera() { Vec3::Y } else { h.normal };
            let rot = Quat::from_rotation_arc(Vec3::Y, n) * Quat::from_rotation_y(turn);
            let mut p = h.point;
            if ed.snap && n.y > 0.999 {
                p.x = snapf(p.x, 0.25);
                p.z = snapf(p.z, 0.25);
            }
            // the bottom of the part sits on the surface
            let pos = p - n * lo.y * size.y;
            **gt = match shape {
                // a camera stands at eye height, looking the way the view does
                s if s.is_camera() => Transform { translation: p + Vec3::Y * camera::EYE, rotation: Quat::from_rotation_y(view.yaw), scale: size },
                Shape::Model => Transform { translation: pos + rot * ((lo + hi) * 0.5 * size), rotation: rot, scale: (hi - lo).max(Vec3::splat(0.05)) * size },
                _ => Transform { translation: pos, rotation: rot, scale: size },
            };
            **vis = Visibility::Visible;
            if mouse.just_pressed(MouseButton::Left) {
                let base = match model.as_ref().or(sound.as_ref()) {
                    Some(m) => std::path::Path::new(m).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| shape.label().into()),
                    None => shape.label().to_string(),
                };
                let name = ed.level.fresh_name(&base);
                let mut part = Part::placed(shape, model.clone(), name.clone(), pos.to_array(), rot.to_array(), size.to_array());
                if shape.is_camera() { camera::place(&mut part, p, view.yaw); }
                if let (Some(sp), Some(path)) = (part.sound.as_mut(), sound.clone()) { sp.sound = path; }
                ed.selected = Some(ed.level.add(part));
                ed.last_placed = Some((shape, model.clone().or(sound.clone())));
                ed.placing = None;
                ed.island.show(shape.glyph(), format!("Placed {name}"), "Drag the arrows to move it · Tab for more", Tone::Plain);
                press.at = None;
            }
        }
        None => **vis = Visibility::Hidden,
    }
    if mouse.just_pressed(MouseButton::Right) || keys.just_pressed(KeyCode::Escape) {
        ed.placing = None;
        (ed.place_turn, ed.place_scale) = (0.0, 1.0);
    }
    if ed.placing.is_none() { *ghost.1 = Visibility::Hidden; }
    true
}

/// Wiring: click the part the wire goes to. True while a wire is being made or chosen.
fn wire_step(ed: &mut Editor, pointer: Option<Vec2>, ray: Option<Ray3d>, mouse: &ButtonInput<MouseButton>, keys: &ButtonInput<KeyCode>, press: &mut Press) -> bool {
    if let Some((from, out)) = ed.wiring {
        if mouse.just_pressed(MouseButton::Right) || keys.just_pressed(KeyCode::Escape) {
            ed.wiring = None;
        } else if mouse.just_pressed(MouseButton::Left) {
            if let (Some(ptr), Some(to)) = (pointer, ray.and_then(|r| ed.hit(r)).and_then(|h| h.part)) {
                let p = ed.level.get(to).unwrap();
                if to == from {
                } else if p.inputs().is_empty() {
                    let name = p.name.clone();
                    ed.island.show("!", format!("{name} can't take wires yet"), "Turn on Moves to give it Go, Back and Toggle", Tone::Danger);
                } else {
                    ed.wire_pick = Some((from, out, to, ptr));
                    ed.wiring = None;
                }
            }
        }
        press.at = None;
        return true;
    }
    if ed.wire_pick.is_some() {
        press.at = None;
        return true;
    }
    false
}

/// Trying a path: click its start, then its end (a third click starts again). True while it is on.
fn path_step(ed: &mut Editor, ray: Option<Ray3d>, mouse: &ButtonInput<MouseButton>, keys: &ButtonInput<KeyCode>, press: &mut Press) -> bool {
    if ed.path_try.is_some() {
        if mouse.just_pressed(MouseButton::Right) || keys.just_pressed(KeyCode::Escape) {
            ed.path_try = None;
        } else if mouse.just_pressed(MouseButton::Left) {
            if let Some(at) = ray.and_then(|r| ed.hit(r)).map(|h| h.point) {
                let t = ed.path_try.as_mut().expect("checked above");
                if t.from.is_none() || t.to.is_some() { *t = PathTry { from: Some(at), ..default() }; } else { t.to = Some(at); }
                ed.find_path();
                match ed.path_try.as_ref().and_then(|t| t.found.clone()) {
                    Some(Ok(p)) => ed.island.show("⬡", format!("A {:.1} m walk", crate::nav::length(&p)), "Click to try another · Esc stops", Tone::Accent),
                    Some(Err(e)) => ed.island.show("!", "No path", e, Tone::Danger),
                    None => {}
                }
            }
        }
        press.at = None;
        return true;
    }
    false
}

/// A widget drag in progress: its handle moves the selected part, a typed number sets it exactly. True while dragging.
fn drag_step(ed: &mut Editor, ray: Option<Ray3d>, pointer: Option<Vec2>, ctrl: bool, shift: bool, typed: &mut MessageReader<KeyboardInput>, sel: &Option<Part>) -> bool {
    if let (Some(drag_h), Some(p)) = (ed.drag.as_ref().map(|d| d.handle), sel.as_ref()) {
        let bx = ed.geo.local_box(p);
        let d = ed.drag.as_mut().unwrap();
        if d.done { return true; }
        // a number typed during the drag: Enter sets it exactly, Esc puts the part back
        let (mut enter, mut esc) = (false, false);
        for k in typed.read().filter(|k| k.state.is_pressed()) {
            match &k.logical_key {
                Key::Character(c) if c.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-') => d.typed.push_str(c),
                Key::Backspace => { d.typed.pop(); }
                Key::Enter => enter = true,
                Key::Escape => esc = true,
                _ => {}
            }
        }
        let (start_pos, start_rot, s0, orig) = (d.start_pos, d.start_rot, d.s0, d.orig.clone());
        let typed_val = d.typed.parse::<f32>().ok();
        let typing = !d.typed.is_empty();
        let typed_text = d.typed.clone();
        if esc {
            d.done = true;
            if let Some(m) = ed.level.get_mut(p.id) { *m = orig; }
            ed.island.show("✕", "Cancelled", "", Tone::Plain);
            return true;
        }
        let snap = ed.snap && !ctrl;
        let at = pointer.unwrap_or_default() + vec2(18.0, 14.0);
        let keys_hint = if typing { format!("typed {typed_text} · Enter sets it · Esc cancels") } else if snap { "type a number · Ctrl: free".into() } else { "type a number".into() };
        let world_handle = |g: Grip, part: &Part| scale_handles(part, bx).into_iter().find(|(h, _)| *h == g).map(|(_, w)| w).unwrap_or(start_pos);
        // what the drag (or the typed number) asks for
        let mut next: Option<Part> = None;
        let mut text = String::new();
        let mut color = theme::highlight();
        match drag_h {
            Grip::Axis(i) => {
                let delta = match (enter, typed_val) {
                    (true, Some(v)) => Some(v),
                    _ => ray.and_then(|r| axis_param(start_pos, AXES[i], r)).map(|s| if snap { snapf(s - s0, 0.25) } else { s - s0 }),
                };
                if let Some(delta) = delta {
                    let mut m = p.clone();
                    m.pos = (start_pos + AXES[i] * delta).to_array();
                    next = Some(m);
                    text = format!("Move along {} · {:+.2} m", axis_name(i), delta);
                    color = axis_color(i);
                }
            }
            Grip::Ring => {
                let turn = match (enter, typed_val) {
                    (true, Some(v)) => Some(v),
                    _ => ray.and_then(|r| ring_angle(start_pos, r)).map(|a| {
                        let mut delta = (a - s0 + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                        if snap { delta = snapf(delta, 15f32.to_radians()); }
                        -delta.to_degrees()
                    }),
                };
                if let Some(deg) = turn {
                    let mut m = p.clone();
                    m.rot = (Quat::from_rotation_y(deg.to_radians()) * start_rot).to_array();
                    next = Some(m);
                    text = format!("Rotate · {deg:+.0}°");
                }
            }
            Grip::Face(i, pos) => {
                let extent0 = orig.size[i] * (bx.1[i] - bx.0[i]);
                let dir = Quat::from_array(orig.rot) * (AXES[i] * if pos { 1.0 } else { -1.0 });
                let h0 = world_handle(drag_h, &orig);
                let extent = match (enter, typed_val) {
                    (true, Some(v)) => Some(v),
                    _ => ray.and_then(|r| axis_param(h0, dir, r)).map(|s| {
                        let e = extent0 + (s - s0) * if shift { 2.0 } else { 1.0 };
                        if snap { snapf(e, 0.25) } else { e }
                    }),
                };
                if let Some(e) = extent {
                    let m = scaled(&orig, bx, drag_h, e, shift);
                    text = format!("Stretch {} · {:.2} m", axis_name(i), m.size[i] * (bx.1[i] - bx.0[i]));
                    color = axis_color(i);
                    next = Some(m);
                }
            }
            Grip::Corner(k) => {
                let h0 = world_handle(drag_h, &orig);
                let a = part_affine(&orig);
                let pin = if shift { a.transform_point3((bx.0 + bx.1) * 0.5) } else { a.transform_point3(corner_local(7 - k, bx.0, bx.1)) };
                let l0 = h0.distance(pin).max(1e-4);
                let dir = (h0 - pin) / l0;
                let factor = match (enter, typed_val) {
                    (true, Some(v)) => Some(v),
                    _ => ray.and_then(|r| axis_param(h0, dir, r)).map(|s| {
                        let f = (l0 + s - s0) / l0;
                        if snap { snapf(f, 0.05) } else { f }
                    }),
                };
                if let Some(f) = factor {
                    next = Some(scaled(&orig, bx, drag_h, f, shift));
                    text = format!("Scale · ×{:.2}{}", f.max(0.01), if shift { " from the middle" } else { "" });
                }
            }
        }
        if let Some(m) = next {
            if let Some(slot) = ed.level.get_mut(p.id) { *slot = m; }
        }
        if enter {
            if let Some(d) = ed.drag.as_mut() { d.done = true; }
            if typed_val.is_none() { ed.island.show("!", "Not a number", typed_text, Tone::Danger); }
            return true;
        }
        let shift_hint = if matches!(drag_h, Grip::Corner(_) | Grip::Face(..)) && !shift { " · Shift: from the middle" } else { "" };
        ed.label = Some((at, format!("{text}  {keys_hint}{shift_hint}"), color));
        return true;
    }
    false
}

/// The widget's handle under the pointer (its label), and the press that starts a drag. True when a drag starts.
fn hover_step(ed: &mut Editor, view: &View, cam: &Camera, gtf: &GlobalTransform, ray: Option<Ray3d>, pointer: Option<Vec2>, mouse: &ButtonInput<MouseButton>, shift: bool, typed: &mut MessageReader<KeyboardInput>, press: &mut Press, sel: &Option<Part>) -> bool {
    // hover a handle?
    ed.hover_handle = None;
    if let (Some(p), Some(ptr)) = (sel.as_ref(), pointer) {
        let c = Vec3::from(p.pos);
        let len = widget_len(&view, c);
        let sc = |w: Vec3| view.to_screen(cam, gtf, w);
        let mut best: Option<(f32, Grip)> = None;
        if let Some(c2) = sc(c) {
            for (i, a) in AXES.iter().enumerate() {
                if let Some(e2) = sc(c + *a * len) {
                    let dd = seg_dist(ptr, c2, e2);
                    if dd < 10.0 && best.is_none_or(|(b, _)| dd < b) { best = Some((dd, Grip::Axis(i))); }
                }
            }
            let ring: Vec<Vec2> = (0..=48)
                .filter_map(|k| {
                    let a = std::f32::consts::TAU * k as f32 / 48.0;
                    sc(c + vec3(a.cos(), 0.0, a.sin()) * len * 0.75)
                })
                .collect();
            for w in ring.windows(2) {
                let dd = seg_dist(ptr, w[0], w[1]);
                if dd < 8.0 && best.is_none_or(|(b, _)| dd < b) { best = Some((dd, Grip::Ring)); }
            }
        }
        // the scale cubes win over the arrows and ring when the pointer is right on one
        for (g, w) in scale_handles(p, ed.geo.local_box(p)) {
            if let Some(s) = sc(w) {
                let dd = s.distance(ptr) - 3.0;
                if dd < 9.0 && best.is_none_or(|(b, _)| dd < b) { best = Some((dd, g)); }
            }
        }
        ed.hover_handle = best.map(|(_, h)| h);
        if let Some(h) = ed.hover_handle {
            let at = ptr + vec2(18.0, 14.0);
            ed.label = Some(match h {
                Grip::Axis(i) => (at, format!("Move along {}", axis_name(i)), axis_color(i)),
                Grip::Ring => (at, "Rotate".into(), theme::highlight()),
                Grip::Corner(_) => (at, "Scale (Shift: from the middle)".into(), theme::highlight()),
                Grip::Face(i, _) => (at, format!("Stretch along {} (Shift: both sides)", axis_name(i)), axis_color(i)),
            });
            if mouse.just_pressed(MouseButton::Left) {
                if let Some(ray) = ray {
                    let bx = ed.geo.local_box(p);
                    let s0 = match h {
                        Grip::Axis(i) => axis_param(c, AXES[i], ray),
                        Grip::Ring => ring_angle(c, ray),
                        Grip::Face(i, pos) => {
                            let h0 = scale_handles(p, bx).into_iter().find(|(g, _)| *g == h).map(|(_, w)| w).unwrap_or(c);
                            axis_param(h0, Quat::from_array(p.rot) * (AXES[i] * if pos { 1.0 } else { -1.0 }), ray)
                        }
                        Grip::Corner(k) => {
                            let a = part_affine(p);
                            let h0 = a.transform_point3(corner_local(k, bx.0, bx.1));
                            let pin = if shift { a.transform_point3((bx.0 + bx.1) * 0.5) } else { a.transform_point3(corner_local(7 - k, bx.0, bx.1)) };
                            axis_param(h0, (h0 - pin).normalize_or_zero(), ray)
                        }
                    };
                    if let Some(s0) = s0 {
                        typed.clear();
                        ed.drag = Some(Drag { handle: h, start_pos: c, start_rot: Quat::from_array(p.rot), s0, orig: p.clone(), typed: String::new(), done: false });
                        press.at = None;
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// A click without a drag selects the part under the pointer.
fn select_step(ed: &mut Editor, view: &View, ray: Option<Ray3d>, pointer: Option<Vec2>, mouse: &ButtonInput<MouseButton>, press: &mut Press) {
    // ---- click to select (a press and release without moving much)
    if mouse.just_pressed(MouseButton::Left) && pointer.is_some() {
        press.at = pointer;
    }
    if mouse.just_released(MouseButton::Left) {
        if let (Some(a), Some(b)) = (press.at.take(), pointer) {
            if a.distance(b) < 5.0 || view.mode == Mode::Builder {
                let hit = ray.and_then(|r| ed.hit(r));
                ed.selected = hit.as_ref().and_then(|h| h.part);
                ed.material_open = None;
                ed.sound_open = None;
                ed.tex_face = hit.and_then(|h| h.face);
            }
        }
    }
}

/// Keyboard shortcuts for the 3D view (typing in a text field turns most of them off).
pub fn shortcuts(mut ed: ResMut<Editor>, keys: Res<ButtonInput<KeyCode>>, mut view: ResMut<View>, mut exit: MessageWriter<AppExit>) {
    // during a widget drag the keyboard types its number (see `interact`)
    if ed.pending.is_some() || ed.testing.is_some() || ed.drag.is_some() { return; }
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let typing = view.keyboard_in_ui;
    let jp = |k| keys.just_pressed(k);
    // keys to named actions (actions.rs), which the menus and the MCP server run too
    let mut act = |id: &str| { let _ = actions::run(&mut ed, Some(&mut view), id, &mut exit); };
    if ctrl {
        if jp(KeyCode::KeyS) { act(if shift { "file.save_as" } else { "file.save" }); }
        if jp(KeyCode::KeyO) { act("file.open"); }
        if jp(KeyCode::KeyN) { act("file.new"); }
        if jp(KeyCode::KeyQ) { act("file.quit"); }
        if typing { return; }
        if jp(KeyCode::KeyZ) { act(if shift { "edit.redo" } else { "edit.undo" }); }
        if jp(KeyCode::KeyY) { act("edit.redo"); }
        if jp(KeyCode::KeyD) { act("edit.duplicate"); }
        if jp(KeyCode::Digit1) { act("tab.level"); }
        if jp(KeyCode::Digit5) { act("tab.audio"); }
        return;
    }
    if typing { return; }
    let parts_open = ed.parts_open;
    let looking = ed.looking.is_some();
    let mut act = |id: &str| { let _ = actions::run(&mut ed, Some(&mut view), id, &mut exit); };
    if jp(KeyCode::Tab) && !parts_open { act("view.parts"); }
    if (jp(KeyCode::Escape) && looking) || jp(KeyCode::KeyL) { act("view.look_through"); }
    if jp(KeyCode::Escape) && !parts_open && !looking { act("edit.deselect"); }
    if jp(KeyCode::Delete) || jp(KeyCode::Backspace) { act("edit.delete"); }
    if jp(KeyCode::KeyF) { act("view.frame"); }
    if jp(KeyCode::Enter) || jp(KeyCode::NumpadEnter) { act("edit.change_all"); }
    if jp(KeyCode::KeyU) { act("edit.make_unique"); }
    if jp(KeyCode::KeyB) { act("tool.brush"); }
    if jp(KeyCode::KeyV) { act("tool.place"); }
    if jp(KeyCode::KeyI) { act("tool.pick"); }
    if jp(KeyCode::KeyP) { act("tool.pen"); act("audio.play"); }
}

/// Pick (I): with the brush out, the material showing most under the pointer (a painted one, else the surface's own)
/// becomes the brush's; otherwise the part under the pointer becomes what Place puts down (its shape, model or sound).
pub(super) fn pick(ed: &mut Editor, ray: Option<Ray3d>) {
    let Some((h, p)) = ray.and_then(|r| ed.hit(r)).and_then(|h| { let p = ed.level.get(h.part?)?.clone(); Some((h, p)) }) else {
        ed.island.show("!", "Nothing to pick there", "Point at a part and press I", Tone::Plain);
        return;
    };
    let name = |m: &str| std::path::Path::new(m).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    if ed.brush_on {
        let local = p.to_local(h.point);
        let pc = crate::paint::pieces(&p);
        let painted = p.painted().iter().enumerate()
            .map(|(i, m)| (m.to_string(), crate::paint::coverage(&pc, pc.len(), i as i32, local, h.normal.y, h.point.y)))
            .filter(|(_, c)| *c > 0.3).max_by(|a, b| a.1.total_cmp(&b.1)).map(|(m, _)| m);
        let own = p.surfaces.iter().find(|s| s.face.is_some() && s.face == h.face).or_else(|| p.surfaces.iter().find(|s| s.face.is_none())).map(|s| s.material.clone());
        match painted.or(own).filter(|m| ed.level.materials.contains_key(m)) {
            Some(m) => {
                ed.island.show("◍", format!("Picked {}", name(&m)), "The brush paints with it", Tone::Accent);
                ed.brush.material = m;
            }
            None => ed.island.show("!", "No material there", format!("{} has no material where you pointed", p.name), Tone::Plain),
        }
        return;
    }
    if p.shape == Shape::PlayerStart {
        ed.island.show("!", "There is one player start", "Move it instead", Tone::Plain);
        return;
    }
    let model = match p.shape { Shape::Model => p.model.clone(), Shape::Sound => p.sound.as_ref().map(|s| s.sound.clone()), _ => None };
    ed.placing = Some(p.shape);
    ed.placing_model = model.clone();
    ed.last_placed = Some((p.shape, model));
    ed.island.show(p.shape.glyph(), format!("Picked {}", p.name), "Click a surface to place another · V places it again", Tone::Accent);
}
