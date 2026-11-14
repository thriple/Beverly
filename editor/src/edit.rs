//! Editing the level in the 3D view: picking, placing shapes on surfaces, the move / rotate widget, keyboard
//! shortcuts, dropped files, undo, and keeping the 3D entities in step with the level.
use crate::level::{self, Collision, Level, Part};
use crate::shapes::{self, ShapeLib};
use crate::theme::{self, Tone};
use crate::view::{MainCamera, Mode, View};
use crate::{Editor, Pending};
use bevy::prelude::*;
use bevy::window::{FileDragAndDrop, WindowCloseRequested};
use std::collections::HashMap;

pub struct Hit { pub t: f32, pub point: Vec3, pub normal: Vec3, pub part: Option<u64> }

pub fn part_affine(p: &Part) -> bevy::math::Affine3A {
    bevy::math::Affine3A::from_scale_rotation_translation(Vec3::from(p.size), Quat::from_array(p.rot), Vec3::from(p.pos))
}

impl Editor {
    /// The nearest surface along a ray: a part, or the ground (y = 0) seen from above.
    pub fn hit(&self, ray: Ray3d) -> Option<Hit> { self.hit_skip(ray, None) }

    pub fn hit_skip(&self, ray: Ray3d, skip: Option<u64>) -> Option<Hit> {
        let (o, d) = (ray.origin, *ray.direction);
        let mut best: Option<Hit> = None;
        for p in &self.level.parts {
            if Some(p.id) == skip { continue; }
            let m = part_affine(p);
            let inv = m.inverse();
            if let Some((t, n)) = self.tris[&p.shape].raycast(inv.transform_point3(o), inv.transform_vector3(d)) {
                if best.as_ref().is_none_or(|b| t < b.t) {
                    let nw = (Mat3::from(m.matrix3).inverse().transpose() * n).normalize_or_zero();
                    best = Some(Hit { t, point: o + d * t, normal: nw, part: Some(p.id) });
                }
            }
        }
        if d.y < -1e-6 && o.y > 0.0 {
            let t = -o.y / d.y;
            if best.as_ref().is_none_or(|b| t < b.t) {
                best = Some(Hit { t, point: o + d * t, normal: Vec3::Y, part: None });
            }
        }
        best
    }

    pub fn delete_selected(&mut self) {
        if let Some(id) = self.selected.take() {
            let name = self.level.get(id).map(|p| p.name.clone()).unwrap_or_default();
            self.level.parts.retain(|p| p.id != id);
            self.island.show("✕", format!("Deleted {name}"), "Ctrl+Z to undo", Tone::Danger);
        }
    }

    pub fn duplicate_selected(&mut self) {
        let Some(p) = self.selected.and_then(|id| self.level.get(id)).cloned() else { return };
        let mut c = p.clone();
        c.pos[0] += p.size[0] + 0.5;
        c.name = self.level.fresh_name(p.shape.label());
        let name = c.name.clone();
        self.selected = Some(self.level.add(c));
        self.island.show("❐", format!("Duplicated as {name}"), "Ctrl+D", Tone::Plain);
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.level, prev.clone()));
            self.committed = prev;
            self.fix_selection();
            self.island.show("↶", "Undo", format!("{} more", self.undo.len()), Tone::Plain);
        } else {
            self.island.show("↶", "Nothing to undo", "", Tone::Plain);
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.level, next.clone()));
            self.committed = next;
            self.fix_selection();
            self.island.show("↷", "Redo", format!("{} more", self.redo.len()), Tone::Plain);
        }
    }

    fn fix_selection(&mut self) {
        if self.selected.is_some_and(|id| self.level.get(id).is_none()) { self.selected = None; }
    }

    pub fn save(&mut self) {
        match level::save(&self.dir, &self.name, &self.level) {
            Ok(()) => {
                self.saved = self.level.clone();
                self.island.show("✓", "Saved", self.dir.display().to_string(), Tone::Accent);
            }
            Err(e) => self.island.show("!", "Couldn't save", e, Tone::Danger),
        }
    }

    pub fn save_as(&mut self) {
        let Some(dir) = rfd::FileDialog::new().set_title("Save project as (pick or make an empty folder)").pick_folder() else { return };
        if let Err(e) = copy_assets(&self.dir, &dir) {
            self.island.show("!", "Couldn't copy assets", e.to_string(), Tone::Danger);
            return;
        }
        self.dir = dir;
        self.name = self.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.save();
        self.refresh_assets();
    }

    /// Run a New / Open / Quit now (the unsaved-changes question has been answered).
    pub fn run(&mut self, what: Pending, exit: &mut MessageWriter<AppExit>) {
        match what {
            Pending::Quit => {
                exit.write(AppExit::Success);
            }
            Pending::New => match level::new_project_dir() {
                Ok(dir) => {
                    let lvl = Level { name: "main".into(), ..default() };
                    self.load_into(dir, lvl);
                    self.island.show("+", format!("New project: {}", self.name), self.dir.display().to_string(), Tone::Accent);
                }
                Err(e) => self.island.show("!", "Couldn't make a project folder", e.to_string(), Tone::Danger),
            },
            Pending::Open => {
                let start = level::projects_root();
                let Some(dir) = rfd::FileDialog::new().set_title("Open a project folder").set_directory(start).pick_folder() else { return };
                match level::load(&dir) {
                    Ok((_name, lvl)) => {
                        self.load_into(dir, lvl);
                        self.island.show("▸", format!("Opened {}", self.name), format!("{} parts", self.level.parts.len()), Tone::Accent);
                    }
                    Err(e) => self.island.show("!", "Couldn't open that folder", e, Tone::Danger),
                }
            }
        }
    }

    /// Ask first if there is unsaved work.
    pub fn request(&mut self, what: Pending, exit: &mut MessageWriter<AppExit>) {
        if self.dirty() { self.pending = Some(what) } else { self.run(what, exit) }
    }

    fn load_into(&mut self, dir: std::path::PathBuf, lvl: Level) {
        self.name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.dir = dir;
        self.saved = lvl.clone();
        self.committed = lvl.clone();
        self.level = lvl;
        self.undo.clear();
        self.redo.clear();
        self.selected = None;
        self.placing = None;
        self.refresh_assets();
    }
}

fn copy_assets(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    level::make_folders(to)?;
    for f in level::ASSET_FOLDERS {
        let src = from.join("assets").join(f);
        let Ok(rd) = std::fs::read_dir(&src) else { continue };
        for e in rd.flatten() {
            if e.path().is_file() { std::fs::copy(e.path(), to.join("assets").join(f).join(e.file_name()))?; }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- widget

#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct WidgetGizmos;

/// The selection outline: thin, drawn over everything.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct OutlineGizmos;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grip { Axis(usize), Ring }

pub struct Drag { pub handle: Grip, start_pos: Vec3, start_rot: Quat, s0: f32 }

const AXES: [Vec3; 3] = [Vec3::X, Vec3::Y, Vec3::Z];
fn axis_color(i: usize) -> bevy_egui::egui::Color32 { [theme::AXIS_X, theme::AXIS_Y, theme::AXIS_Z][i] }
fn axis_name(i: usize) -> &'static str { ["X", "Y", "Z"][i] }
fn c32(c: bevy_egui::egui::Color32) -> Color { Color::srgba_u8(c.r(), c.g(), c.b(), c.a()) }

fn widget_len(view: &View, c: Vec3) -> f32 { view.pos.distance(c) * 0.16 }

/// Closest point on the line c + s·a to a ray, as s (None when the ray runs along the axis).
fn axis_param(c: Vec3, a: Vec3, ray: Ray3d) -> Option<f32> {
    let d = *ray.direction;
    let w = c - ray.origin;
    let b = a.dot(d);
    let den = 1.0 - b * b;
    if den < 1e-3 { return None; }
    Some((b * d.dot(w) - a.dot(w)) / den)
}

fn ring_angle(c: Vec3, ray: Ray3d) -> Option<f32> {
    let d = *ray.direction;
    if d.y.abs() < 1e-4 { return None; }
    let t = (c.y - ray.origin.y) / d.y;
    if t <= 0.0 { return None; }
    let q = ray.origin + d * t - c;
    Some(q.z.atan2(q.x))
}

fn seg_dist(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn snapf(v: f32, step: f32) -> f32 { (v / step).round() * step }

// ---------------------------------------------------------------- systems

#[derive(Component)]
pub struct Ghost;

#[derive(Resource)]
pub struct Mats { cache: HashMap<[u8; 3], Handle<StandardMaterial>>, collider: Handle<StandardMaterial> }

pub fn setup(mut commands: Commands, mut mats: ResMut<Assets<StandardMaterial>>, mut meshes: ResMut<Assets<Mesh>>, mut configs: ResMut<GizmoConfigStore>) {
    let see_through = |c: Color| StandardMaterial { base_color: c, alpha_mode: AlphaMode::Blend, unlit: true, cull_mode: None, double_sided: true, ..default() };
    let collider = mats.add(see_through(Color::srgba(0.96, 0.86, 0.29, 0.10)));
    let ghost = mats.add(see_through(Color::srgba(0.96, 0.86, 0.29, 0.28)));
    commands.insert_resource(Mats { cache: HashMap::new(), collider });
    commands.spawn((Ghost, Mesh3d(meshes.add(Cuboid::default())), MeshMaterial3d(ghost), Transform::default(), Visibility::Hidden));
    // the ground: a dark plane at y = 0 (the grid is drawn over it)
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(4000.0, 4000.0))),
        MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::srgb(0.075, 0.068, 0.085), perceptual_roughness: 1.0, ..default() })),
    ));
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: std::env::var_os("BEVERLY_LITE").is_none(), ..default() },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.7, -0.95, 0.0)),
    ));
    let (cfg, _) = configs.config_mut::<WidgetGizmos>();
    cfg.line.width = 3.5;
    cfg.depth_bias = -1.0;
    let (cfg, _) = configs.config_mut::<OutlineGizmos>();
    cfg.line.width = 1.5;
    cfg.depth_bias = -1.0;
    let (cfg, _) = configs.config_mut::<DefaultGizmoConfigGroup>();
    cfg.line.width = 1.0;
}

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
) {
    let (cam, gtf) = *cam;
    ed.label = None;
    let pointer = view.pointer();
    let ray = pointer.and_then(|p| view.ray(cam, gtf, p));
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);

    // ---- placing a shape from the parts pop-up
    if let Some(shape) = ed.placing {
        let (ref mut gt, ref mut vis, ref mut mesh) = *ghost;
        mesh.0 = lib.mesh(shape);
        match ray.and_then(|r| ed.hit(r)) {
            Some(h) => {
                let size = Vec3::from(shape.default_size());
                let n = if shift { Vec3::Y } else { h.normal };
                let rot = Quat::from_rotation_arc(Vec3::Y, n);
                let mut p = h.point;
                if ed.snap && n.y > 0.999 {
                    p.x = snapf(p.x, 0.25);
                    p.z = snapf(p.z, 0.25);
                }
                let pos = p + n * size.y * 0.5;
                **gt = Transform { translation: pos, rotation: rot, scale: size };
                **vis = Visibility::Visible;
                if mouse.just_pressed(MouseButton::Left) {
                    let name = ed.level.fresh_name(shape.label());
                    let part = Part {
                        id: 0,
                        name: name.clone(),
                        shape,
                        pos: pos.to_array(),
                        rot: rot.to_array(),
                        size: size.to_array(),
                        color: [0.62, 0.64, 0.70],
                        visible: true,
                        collision: Collision::Solid,
                    };
                    ed.selected = Some(ed.level.add(part));
                    ed.placing = None;
                    ed.island.show(shape.glyph(), format!("Placed {name}"), "Drag the arrows to move it · Tab for more", Tone::Plain);
                    press.at = None;
                }
            }
            None => **vis = Visibility::Hidden,
        }
        if mouse.just_pressed(MouseButton::Right) || keys.just_pressed(KeyCode::Escape) {
            ed.placing = None;
        }
        if ed.placing.is_none() { *ghost.1 = Visibility::Hidden; }
        return;
    }
    *ghost.1 = Visibility::Hidden;

    // ---- the widget on the selected part
    if !mouse.pressed(MouseButton::Left) { ed.drag = None; }
    let sel = ed.selected.and_then(|id| ed.level.get(id)).cloned();
    if let (Some(drag_h), Some(p), Some(ray)) = (ed.drag.as_ref().map(|d| d.handle), sel.as_ref(), ray) {
        if !mouse.pressed(MouseButton::Left) {
            ed.drag = None;
        } else {
            let d = ed.drag.as_ref().unwrap();
            let (start_pos, start_rot, s0) = (d.start_pos, d.start_rot, d.s0);
            let snap = ed.snap && !ctrl;
            let at = pointer.unwrap_or_default() + vec2(18.0, 14.0);
            let keys_hint = if snap { "Ctrl: free" } else { "" };
            match drag_h {
                Grip::Axis(i) => {
                    if let Some(s) = axis_param(start_pos, AXES[i], ray) {
                        let mut delta = s - s0;
                        if snap { delta = snapf(delta, 0.25); }
                        let pos = start_pos + AXES[i] * delta;
                        if let Some(m) = ed.level.get_mut(p.id) { m.pos = pos.to_array(); }
                        ed.label = Some((at, format!("Move along {} · {:+.2} m  {keys_hint}", axis_name(i), delta), axis_color(i)));
                    }
                }
                Grip::Ring => {
                    if let Some(a) = ring_angle(start_pos, ray) {
                        let mut delta = a - s0;
                        delta = (delta + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                        if snap { delta = snapf(delta, 15f32.to_radians()); }
                        let rot = Quat::from_rotation_y(-delta) * start_rot;
                        if let Some(m) = ed.level.get_mut(p.id) { m.rot = rot.to_array(); }
                        ed.label = Some((at, format!("Rotate · {:+.0}°  {keys_hint}", -delta.to_degrees()), theme::ACCENT));
                    }
                }
            }
            return;
        }
    }

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
        ed.hover_handle = best.map(|(_, h)| h);
        if let Some(h) = ed.hover_handle {
            let at = ptr + vec2(18.0, 14.0);
            ed.label = Some(match h {
                Grip::Axis(i) => (at, format!("Move along {}", axis_name(i)), axis_color(i)),
                Grip::Ring => (at, "Rotate".into(), theme::ACCENT),
            });
            if mouse.just_pressed(MouseButton::Left) {
                if let Some(ray) = ray {
                    let s0 = match h {
                        Grip::Axis(i) => axis_param(c, AXES[i], ray),
                        Grip::Ring => ring_angle(c, ray),
                    };
                    if let Some(s0) = s0 {
                        ed.drag = Some(Drag { handle: h, start_pos: c, start_rot: Quat::from_array(p.rot), s0 });
                        press.at = None;
                        return;
                    }
                }
            }
        }
    }

    // ---- click to select (a press and release without moving much)
    if mouse.just_pressed(MouseButton::Left) && pointer.is_some() {
        press.at = pointer;
    }
    if mouse.just_released(MouseButton::Left) {
        if let (Some(a), Some(b)) = (press.at.take(), pointer) {
            if a.distance(b) < 5.0 || view.mode == Mode::Builder {
                ed.selected = ray.and_then(|r| ed.hit(r)).and_then(|h| h.part);
            }
        }
    }
}

/// Keyboard shortcuts for the 3D view (typing in a text field turns most of them off).
pub fn shortcuts(mut ed: ResMut<Editor>, keys: Res<ButtonInput<KeyCode>>, mut view: ResMut<View>, mut exit: MessageWriter<AppExit>) {
    if ed.pending.is_some() { return; }
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let typing = view.keyboard_in_ui;
    let jp = |k| keys.just_pressed(k);
    if ctrl {
        if jp(KeyCode::KeyS) {
            if shift { ed.save_as() } else { ed.save() }
        }
        if jp(KeyCode::KeyO) { ed.request(Pending::Open, &mut exit); }
        if jp(KeyCode::KeyN) { ed.request(Pending::New, &mut exit); }
        if jp(KeyCode::KeyQ) { ed.request(Pending::Quit, &mut exit); }
        if typing { return; }
        if jp(KeyCode::KeyZ) {
            if shift { ed.redo() } else { ed.undo() }
        }
        if jp(KeyCode::KeyY) { ed.redo(); }
        if jp(KeyCode::KeyD) { ed.duplicate_selected(); }
        return;
    }
    if typing { return; }
    if jp(KeyCode::Tab) && !ed.parts_open {
        ed.parts_open = true;
        ed.parts_fresh = true;
        ed.parts_query.clear();
        ed.placing = None;
    }
    if jp(KeyCode::Escape) && !ed.parts_open {
        if ed.placing.is_some() { ed.placing = None } else { ed.selected = None }
    }
    if jp(KeyCode::Delete) || jp(KeyCode::Backspace) { ed.delete_selected(); }
    if jp(KeyCode::KeyF) {
        if let Some(p) = ed.selected.and_then(|id| ed.level.get(id)) {
            let c = Vec3::from(p.pos);
            let r = Vec3::from(p.size).length();
            let f = view.forward();
            view.pos = c - f * (r * 1.6 + 2.0);
        }
    }
}

/// Files dragged in from Explorer are copied into the project's assets.
pub fn files(mut ed: ResMut<Editor>, mut drops: MessageReader<FileDragAndDrop>) {
    let mut imported: Vec<&'static str> = vec![];
    for e in drops.read() {
        match e {
            FileDragAndDrop::HoveredFile { .. } => ed.hovering_files = true,
            FileDragAndDrop::HoveredFileCanceled { .. } => ed.hovering_files = false,
            FileDragAndDrop::DroppedFile { path_buf, .. } => {
                ed.hovering_files = false;
                let dir = ed.dir.clone();
                match level::import(&dir, path_buf) {
                    Ok(f) => imported.push(f),
                    Err(err) => ed.island.show("!", "Couldn't import a file", err.to_string(), Tone::Danger),
                }
            }
        }
    }
    if !imported.is_empty() {
        ed.refresh_assets();
        let n = imported.len();
        let folder = imported[0];
        ed.island.show("⇣", format!("Imported {n} file{}", if n == 1 { "" } else { "s" }), format!("into {folder}"), Tone::Accent);
    }
}

#[derive(Component)]
pub struct PartView { id: u64, applied: Part }

/// Make the 3D entities match the level (spawn new parts, update changed ones, remove deleted ones).
pub fn sync(
    mut commands: Commands,
    ed: Res<Editor>,
    lib: Res<ShapeLib>,
    mut mats: ResMut<Mats>,
    mut assets: ResMut<Assets<StandardMaterial>>,
    mut views: Query<(Entity, &mut PartView, &mut Transform, &mut Mesh3d, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    if !ed.is_changed() { return; }
    let mut material = |p: &Part| -> Handle<StandardMaterial> {
        if !p.visible { return mats.collider.clone(); }
        let key = p.color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
        mats.cache
            .entry(key)
            .or_insert_with(|| assets.add(StandardMaterial { base_color: Color::srgb_u8(key[0], key[1], key[2]), perceptual_roughness: 0.75, ..default() }))
            .clone()
    };
    let tf = |p: &Part| Transform { translation: Vec3::from(p.pos), rotation: Quat::from_array(p.rot), scale: Vec3::from(p.size) };
    let mut seen = std::collections::HashSet::new();
    for (e, mut v, mut t, mut mesh, mut mat) in &mut views {
        match ed.level.get(v.id) {
            None => commands.entity(e).despawn(),
            Some(p) => {
                seen.insert(p.id);
                if v.applied != *p {
                    *t = tf(p);
                    if v.applied.shape != p.shape { mesh.0 = lib.mesh(p.shape); }
                    mat.0 = material(p);
                    v.applied = p.clone();
                }
            }
        }
    }
    for p in ed.level.parts.iter().filter(|p| !seen.contains(&p.id)) {
        commands.spawn((PartView { id: p.id, applied: p.clone() }, tf(p), Mesh3d(lib.mesh(p.shape)), MeshMaterial3d(material(p))));
    }
}

/// Outlines (selection, hidden colliders) and the widget.
pub fn draw(ed: Res<Editor>, view: Res<View>, mut g: Gizmos, mut w: Gizmos<WidgetGizmos>, mut o: Gizmos<OutlineGizmos>) {
    let outline = |g: &mut Gizmos<_>, p: &Part, color: Color| {
        let m = part_affine(p);
        for (a, b) in shapes::BOX_EDGES {
            g.line(m.transform_point3(shapes::box_corner(a)), m.transform_point3(shapes::box_corner(b)), color);
        }
    };
    let _ = &mut g;
    for p in ed.level.parts.iter().filter(|p| !p.visible && Some(p.id) != ed.selected) {
        let m = part_affine(p);
        for (a, b) in shapes::BOX_EDGES {
            g.line(m.transform_point3(shapes::box_corner(a)), m.transform_point3(shapes::box_corner(b)), Color::srgba(0.96, 0.86, 0.29, 0.45));
        }
    }
    let Some(p) = ed.selected.and_then(|id| ed.level.get(id)) else { return };
    outline(&mut o, p, Color::srgb(0.96, 0.86, 0.29));
    if ed.placing.is_some() { return; }
    let c = Vec3::from(p.pos);
    let len = widget_len(&view, c);
    let active = ed.drag.as_ref().map(|d| d.handle).or(ed.hover_handle);
    for (i, a) in AXES.iter().enumerate() {
        let hot = active == Some(Grip::Axis(i));
        let col = if hot { Color::WHITE } else { c32(axis_color(i)) };
        w.arrow(c, c + *a * len, col);
    }
    let hot = active == Some(Grip::Ring);
    let col = if hot { Color::WHITE } else { Color::srgba(0.96, 0.86, 0.29, 0.85) };
    w.circle(Isometry3d::new(c, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), len * 0.75, col).resolution(64);
}

/// One undo step per finished change: when the level differs from the last step and no drag is going on.
pub fn commit(mut ed: ResMut<Editor>, mouse: Res<ButtonInput<MouseButton>>) {
    if ed.level == ed.committed || ed.drag.is_some() || mouse.pressed(MouseButton::Left) { return; }
    let now = ed.level.clone();
    let prev = std::mem::replace(&mut ed.committed, now);
    ed.undo.push(prev);
    if ed.undo.len() > 300 { ed.undo.remove(0); }
    ed.redo.clear();
}

pub fn close_requested(mut ed: ResMut<Editor>, mut closes: MessageReader<WindowCloseRequested>, mut exit: MessageWriter<AppExit>) {
    if closes.read().count() > 0 { ed.request(Pending::Quit, &mut exit); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clicks_hit_parts() {
        let mut ed = Editor::at(std::env::temp_dir());
        let p = Part {
            id: 0, name: "Sphere 1".into(), shape: crate::level::Shape::Sphere, pos: [2.25, 1.0, 3.25], rot: Quat::IDENTITY.to_array(),
            size: [2.0; 3], color: [0.5; 3], visible: true, collision: Collision::Solid,
        };
        let id = ed.level.add(p);
        let o = vec3(14.0, 10.0, 14.0);
        let ray = Ray3d::new(o, Dir3::new(vec3(2.25, 1.0, 3.25) - o).unwrap());
        let h = ed.hit(ray).expect("hit");
        assert_eq!(h.part, Some(id));
        assert!((h.point - vec3(2.25, 1.0, 3.25)).length() < 1.05);
    }
}
