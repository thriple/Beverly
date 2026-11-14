//! Editing the level in the 3D view: picking, placing shapes on surfaces, the move / rotate widget, keyboard
//! shortcuts, dropped files, undo, and keeping the 3D entities in step with the level.
use crate::level::{self, Collision, Level, Part, Shape};
use crate::models::{ModelLib, ProjectRoot};
use crate::play::Play;
use crate::shapes::{self, ShapeLib};
use super::theme::{self, Tone};
use super::view::{MainCamera, Mode, View};
use crate::world::{self, Hit, Mats, part_affine};
use super::{Editor, Pending, Saved};
use bevy::prelude::*;
use bevy::window::{FileDragAndDrop, WindowCloseRequested};

impl Editor {
    /// The nearest surface along a ray: a part, or the ground (y = 0) seen from above.
    pub fn hit(&self, ray: Ray3d) -> Option<Hit> { self.hit_skip(ray, None) }

    pub fn hit_skip(&self, ray: Ray3d, skip: Option<u64>) -> Option<Hit> { world::raycast(&self.level.parts, &self.geo, ray, skip, |_| true) }

    /// Write the project as a Windows game: a zip with the game player (renamed after the project) and the
    /// project's files. The player must sit next to the editor (the release zip has both).
    pub fn export_zip(&mut self) {
        if self.dirty() { self.save(); }
        let exe = if cfg!(windows) { format!("{}.exe", self.player) } else { self.player.to_string() };
        let player = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(&exe)));
        let Some(player) = player.filter(|p| p.is_file()) else {
            self.island.show("!", "Couldn't find the game player", format!("{exe} should be next to the editor"), Tone::Danger);
            return;
        };
        let Some(out) = rfd::FileDialog::new().set_title("Export Windows game").set_file_name(format!("{}.zip", self.name)).save_file() else { return };
        match level::export_zip(&self.dir, &self.name, &player, &out) {
            Ok(n) => self.island.show("⇪", format!("Exported {}", out.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default()), format!("{n} files · unzip and run {}.exe", self.name), Tone::Accent),
            Err(e) => self.island.show("!", "Couldn't export", e.to_string(), Tone::Danger),
        }
    }

    pub fn delete_selected(&mut self) {
        if let Some(id) = self.selected.take() {
            let name = self.level.get(id).map(|p| p.name.clone()).unwrap_or_default();
            self.level.parts.retain(|p| p.id != id);
            self.level.wires.retain(|w| w.from != id && w.to != id);
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
                    Ok((name, lvl)) => {
                        self.load_into(dir, lvl);
                        self.name = name;
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
        self.project_changed = true;
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

pub fn setup(mut commands: Commands, mut mats: ResMut<Assets<StandardMaterial>>, mut meshes: ResMut<Assets<Mesh>>, mut configs: ResMut<GizmoConfigStore>) {
    let shared = world::setup_stage(&mut commands, &mut meshes, &mut mats, std::env::var_os("BEVERLY_LITE").is_none());
    commands.insert_resource(shared);
    let ghost = mats.add(StandardMaterial { base_color: Color::srgba(0.96, 0.86, 0.29, 0.28), alpha_mode: AlphaMode::Blend, unlit: true, cull_mode: None, ..default() });
    commands.spawn((Ghost, Mesh3d(meshes.add(Cuboid::default())), MeshMaterial3d(ghost), Transform::default(), Visibility::Hidden));
    let (cfg, _) = configs.config_mut::<WidgetGizmos>();
    cfg.line.width = 3.5;
    cfg.depth_bias = -1.0;
    let (cfg, _) = configs.config_mut::<OutlineGizmos>();
    cfg.line.width = 1.5;
    cfg.depth_bias = -1.0;
    let (cfg, _) = configs.config_mut::<DefaultGizmoConfigGroup>();
    cfg.line.width = 1.0;
}

/// Another project was opened: point the asset folder at it and forget the old models.
pub fn project_follow(mut ed: ResMut<Editor>, root: Res<ProjectRoot>, mut lib: ResMut<ModelLib>) {
    if !ed.project_changed { return; }
    ed.project_changed = false;
    root.set(ed.dir.clone());
    lib.clear();
    ed.geo.models.clear();
}

/// Newly loaded models' triangles into the editor's geometry (for clicking and placing on them).
pub fn models_in(mut ed: ResMut<Editor>, mut lib: ResMut<ModelLib>, server: Res<AssetServer>, mut seen: Local<u64>) {
    // a model being placed starts loading right away, so its real box is known while placing
    if let Some(m) = ed.placing_model.as_ref().filter(|_| ed.placing == Some(Shape::Model)) {
        if lib.geo(m).is_none() { lib.request(&server, m); }
    }
    if lib.generation == *seen { return; }
    *seen = lib.generation;
    ed.bypass_change_detection().geo.sync_models(&lib);
    ed.set_changed();
}

/// Test mode: ` drops the walker at the reticle (Shift+`: at the player start) and ` again returns to editing.
#[allow(clippy::too_many_arguments)]
pub fn test_mode(
    mut commands: Commands,
    mut ed: ResMut<Editor>,
    mut view: ResMut<View>,
    mut play: ResMut<Play>,
    keys: Res<ButtonInput<KeyCode>>,
    cam: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    if !keys.just_pressed(KeyCode::Backquote) || ed.pending.is_some() || ed.parts_open || (view.keyboard_in_ui && ed.testing.is_none()) { return; }
    if let Some(s) = ed.testing.take() {
        play.stop(&mut commands);
        view.pos = s.pos;
        view.yaw = s.yaw;
        view.pitch = s.pitch;
        view.mode = s.mode;
        ed.island.show("✎", "Back to editing", "` tests again", Tone::Plain);
        return;
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let (cam, gtf) = *cam;
    let start = ed.level.parts.iter().find(|p| p.shape == Shape::PlayerStart).map(|p| Vec3::from(p.pos) - Vec3::Y * p.size[1] * 0.5);
    let feet = if shift {
        match start {
            Some(f) => f,
            None => {
                ed.island.show("!", "No Player start yet", "Place one from the Game section (Tab)", Tone::Danger);
                return;
            }
        }
    } else {
        match view.ray(cam, gtf, view.rect.center()).and_then(|r| ed.hit(r)) {
            Some(h) => h.point,
            None => view.pos.with_y(0.0),
        }
    };
    ed.testing = Some(Saved { pos: view.pos, yaw: view.yaw, pitch: view.pitch, mode: view.mode });
    ed.placing = None;
    ed.drag = None;
    play.start(&mut commands, &mut meshes, &mut mats, &ed.level, &ed.geo, feet, view.yaw);
    let controls = play.custom.unwrap_or("WASD walk · Shift run · Space jump · E press");
    ed.island.show("▶", "Testing", format!("{controls} · ` back to editing"), Tone::Accent);
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
    if ed.testing.is_some() {
        *ghost.1 = Visibility::Hidden;
        return;
    }
    let pointer = view.pointer();
    let ray = pointer.and_then(|p| view.ray(cam, gtf, p));
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);

    // ---- placing a shape from the parts pop-up
    if let Some(shape) = ed.placing {
        let (ref mut gt, ref mut vis, ref mut mesh) = *ghost;
        // a model's box comes from its file once loaded; until then a 1 m stand-in
        let model = ed.placing_model.clone();
        let (lo, hi) = match &model {
            Some(m) => ed.geo.models.get(m).map(|t| t.bounds()).unwrap_or((vec3(-0.5, 0.0, -0.5), vec3(0.5, 1.0, 0.5))),
            None => (Vec3::splat(-0.5), Vec3::splat(0.5)),
        };
        if shape != Shape::Model { mesh.0 = lib.mesh(shape); }
        match ray.and_then(|r| ed.hit(r)) {
            Some(h) => {
                let size = Vec3::from(shape.default_size());
                let n = if shift || shape == Shape::PlayerStart { Vec3::Y } else { h.normal };
                let rot = Quat::from_rotation_arc(Vec3::Y, n);
                let mut p = h.point;
                if ed.snap && n.y > 0.999 {
                    p.x = snapf(p.x, 0.25);
                    p.z = snapf(p.z, 0.25);
                }
                // the bottom of the part sits on the surface
                let pos = p - n * lo.y * size.y;
                **gt = match shape {
                    Shape::Model => Transform { translation: pos + rot * ((lo + hi) * 0.5 * size), rotation: rot, scale: (hi - lo).max(Vec3::splat(0.05)) * size },
                    _ => Transform { translation: pos, rotation: rot, scale: size },
                };
                **vis = Visibility::Visible;
                if mouse.just_pressed(MouseButton::Left) {
                    let base = match &model {
                        Some(m) => std::path::Path::new(m).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Model".into()),
                        None => shape.label().to_string(),
                    };
                    let name = ed.level.fresh_name(&base);
                    let part = Part {
                        id: 0,
                        name: name.clone(),
                        shape,
                        pos: pos.to_array(),
                        rot: rot.to_array(),
                        size: size.to_array(),
                        color: if shape == Shape::Interact { [0.94, 0.55, 0.26] } else { [0.62, 0.64, 0.70] },
                        visible: true,
                        collision: if shape == Shape::PlayerStart { Collision::WalkThrough } else { Collision::Solid },
                        model: model.clone(),
                        mover: None,
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

    // ---- wiring: click the part the wire goes to
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
        return;
    }
    if ed.wire_pick.is_some() {
        press.at = None;
        return;
    }

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
    if ed.pending.is_some() || ed.testing.is_some() { return; }
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

/// Make the 3D entities match the level (spawn new parts, update moved ones, rebuild changed ones, remove deleted ones).
#[allow(clippy::too_many_arguments)]
pub fn sync(
    mut commands: Commands,
    ed: Res<Editor>,
    lib: Res<ShapeLib>,
    mut models: ResMut<ModelLib>,
    server: Res<AssetServer>,
    mut mats: ResMut<Mats>,
    mut assets: ResMut<Assets<StandardMaterial>>,
    mut views: Query<(Entity, &mut PartView, &mut Transform)>,
) {
    if !ed.is_changed() { return; }
    let same_look = |a: &Part, b: &Part| a.shape == b.shape && a.model == b.model && a.visible == b.visible && a.color == b.color;
    let mut seen = std::collections::HashSet::new();
    for (e, mut v, mut t) in &mut views {
        match ed.level.get(v.id) {
            Some(p) if same_look(&v.applied, p) => {
                seen.insert(p.id);
                if v.applied != *p {
                    *t = world::part_transform(p);
                    v.applied = p.clone();
                }
            }
            _ => commands.entity(e).despawn(),
        }
    }
    for p in ed.level.parts.iter().filter(|p| !seen.contains(&p.id)) {
        if let Some(e) = world::spawn_visual(&mut commands, p, &lib, &mut models, &server, &mut mats, &mut assets, true) {
            commands.entity(e).insert(PartView { id: p.id, applied: p.clone() });
        }
    }
}

/// Outlines (selection, hidden colliders) and the widget.
pub fn draw(ed: Res<Editor>, view: Res<View>, mut g: Gizmos, mut w: Gizmos<WidgetGizmos>, mut o: Gizmos<OutlineGizmos>) {
    if ed.testing.is_some() { return; }
    // the part's box: unit box for shapes, the model's bounds for models
    let corner = |p: &Part, i: usize| {
        let (lo, hi) = ed.geo.local_box(p);
        part_affine(p).transform_point3(lo + (hi - lo) * (shapes::box_corner(i) + Vec3::splat(0.5)))
    };
    let outline = |g: &mut Gizmos<_>, p: &Part, color: Color| {
        for (a, b) in shapes::BOX_EDGES { g.line(corner(p, a), corner(p, b), color); }
    };
    for p in ed.level.parts.iter().filter(|p| !p.visible && Some(p.id) != ed.selected) {
        for (a, b) in shapes::BOX_EDGES { g.line(corner(p, a), corner(p, b), Color::srgba(0.96, 0.86, 0.29, 0.45)); }
    }
    // moving parts: where they go (an outline at the end, and the path there)
    for p in &ed.level.parts {
        let Some(m) = &p.mover else { continue };
        let a = if Some(p.id) == ed.selected { 0.9 } else { 0.35 };
        let col = Color::srgba(0.36, 0.61, 0.94, a);
        let mut end = p.clone();
        end.pos = (Vec3::from(p.pos) + Vec3::from(m.to)).to_array();
        for (i, j) in shapes::BOX_EDGES { g.line(corner(&end, i), corner(&end, j), col); }
        g.arrow(Vec3::from(p.pos), Vec3::from(end.pos), col);
    }
    // wires: arcs from the part that sends to the part that receives
    for w in &ed.level.wires {
        let (Some(a), Some(b)) = (ed.level.get(w.from), ed.level.get(w.to)) else { continue };
        let (a, b) = (Vec3::from(a.pos), Vec3::from(b.pos));
        let lift = Vec3::Y * (a.distance(b) * 0.3 + 0.5);
        let hot = ed.selected == Some(w.from) || ed.selected == Some(w.to);
        let col = Color::srgba(0.94, 0.55, 0.26, if hot { 1.0 } else { 0.55 });
        let pts: Vec<Vec3> = (0..=24).map(|k| {
            let t = k as f32 / 24.0;
            a.lerp(b, t) + lift * 4.0 * t * (1.0 - t)
        }).collect();
        g.linestrip(pts.iter().copied(), col);
        let tip = pts[23];
        g.arrow(tip, b, col);
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
            size: [2.0; 3], color: [0.5; 3], visible: true, collision: Collision::Solid, model: None, mover: None,
        };
        let id = ed.level.add(p);
        let o = vec3(14.0, 10.0, 14.0);
        let ray = Ray3d::new(o, Dir3::new(vec3(2.25, 1.0, 3.25) - o).unwrap());
        let h = ed.hit(ray).expect("hit");
        assert_eq!(h.part, Some(id));
        assert!((h.point - vec3(2.25, 1.0, 3.25)).length() < 1.05);
    }
}
