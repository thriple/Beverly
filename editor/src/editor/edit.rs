//! Editing the level in the 3D view: picking, placing shapes on surfaces, the move / rotate widget, keyboard
//! shortcuts, dropped files, undo, and keeping the 3D entities in step with the level.
use crate::level::{self, Level, Part, Shape};
use crate::models::{ModelLib, ProjectRoot};
use crate::play::Play;
use crate::shapes::{self, ShapeLib};
use super::theme::{self, Tone};
use super::view::{MainCamera, Mode, View};
use crate::world::{self, Hit, Mats, part_affine};
use super::{actions, brush, bucket, camera, light, pen, texture, Editor, PathTry, Pending, Saved, TestRequest};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{FileDragAndDrop, WindowCloseRequested};

mod interact;
mod sync;
mod widget;
pub use interact::*;
pub use sync::*;
pub use widget::*;

impl Editor {
    /// The nearest surface along a ray: a part, or the ground (y = 0) seen from above.
    pub fn hit(&self, ray: Ray3d) -> Option<Hit> { self.hit_skip(ray, None) }

    /// What the ray meets first. A zone counts only where what lies behind it is outside it, so a click on the floor
    /// inside a zone picks the floor, and one past it (the sky, a wall beyond) picks the zone.
    pub fn hit_skip(&self, ray: Ray3d, skip: Option<u64>) -> Option<Hit> {
        let solid = world::raycast(&self.level.parts, &self.geo, ray, skip, |p| p.shape != Shape::Zone);
        let Some(zone) = world::raycast(&self.level.parts, &self.geo, ray, skip, |p| p.shape == Shape::Zone) else { return solid };
        let behind_outside = |s: &Hit| zone.part.and_then(|id| self.level.get(id)).is_none_or(|z| !level::in_zone(z, s.point));
        match &solid {
            Some(s) if s.t < zone.t || !behind_outside(s) => solid,
            _ => Some(zone),
        }
    }

    /// Write the project as a Windows game: a zip with the game player (renamed after the project) and the
    /// project's files. The player must sit next to the editor (the release zip has both).
    pub fn export_zip(&mut self) {
        if self.dirty() { self.save(); }
        let exe = self.player_exe();
        let Some(player) = self.player_path() else {
            self.island.show("!", "Couldn't find the game player", format!("{exe} should be next to the editor"), Tone::Danger);
            return;
        };
        let Some(out) = rfd::FileDialog::new().set_title("Export Windows game").set_file_name(format!("{}.zip", self.name)).save_file() else { return };
        match level::export_zip(&self.dir, &self.name, &player, &self.bakes.pack(), &out) {
            Ok(n) => self.island.show("⇪", format!("Exported {}", out.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default()), format!("{n} files · unzip and run {}.exe", self.name), Tone::Accent),
            Err(e) => self.island.show("!", "Couldn't export", e.to_string(), Tone::Danger),
        }
    }

    /// The game player's file name, and its path next to the editor (if it is there).
    pub fn player_exe(&self) -> String { if cfg!(windows) { format!("{}.exe", self.player) } else { self.player.to_string() } }
    pub fn player_path(&self) -> Option<std::path::PathBuf> {
        std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(self.player_exe()))).filter(|p| p.is_file())
    }

    pub fn delete_selected(&mut self) {
        if let Some(id) = self.selected.take() {
            let name = self.level.get(id).map(|p| p.name.clone()).unwrap_or_default();
            self.level.remove_part(id);
            self.island.show("✕", format!("Deleted {name}"), "Ctrl+Z to undo", Tone::Danger);
        }
    }

    pub fn duplicate_selected(&mut self) {
        let Some(p) = self.selected.and_then(|id| self.level.get(id)).cloned() else { return };
        // with everything attached to it
        let Some(copy) = self.level.copy_tree(p.id, [p.size[0] + 0.5, 0.0, 0.0]) else { return };
        self.selected = Some(copy);
        let name = self.level.get(copy).map(|c| c.name.clone()).unwrap_or_default();
        self.island.show("❐", format!("Duplicated as {name}"), "Ctrl+D", Tone::Plain);
    }

    pub fn undo(&mut self) {
        self.ask = None;
        if let Some(step) = self.undo.pop() {
            step.apply(&mut self.level, false);
            step.apply(&mut self.committed, false);
            self.checked = self.level.stamps();
            self.redo.push(step);
            self.fix_selection();
            self.island.show("↶", "Undo", format!("{} more", self.undo.len()), Tone::Plain);
        } else {
            self.island.show("↶", "Nothing to undo", "", Tone::Plain);
        }
    }

    pub fn redo(&mut self) {
        self.ask = None;
        if let Some(step) = self.redo.pop() {
            step.apply(&mut self.level, true);
            step.apply(&mut self.committed, true);
            self.checked = self.level.stamps();
            self.undo.push(step);
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
                        let note = if level::is_old(&self.dir) { " · from an older Beverly: saving updates its files to .bev" } else { "" };
                        self.island.show("▸", format!("Opened {}", self.name), format!("{} parts{note}", self.level.parts.len()), Tone::Accent);
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

    fn load_into(&mut self, dir: std::path::PathBuf, mut lvl: Level) {
        super::scatter::grow(&mut lvl, &default());
        self.name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.dir = dir;
        self.saved = lvl.clone();
        self.level = lvl;
        self.sync_committed();
        self.undo.clear();
        self.redo.clear();
        self.ask = None;
        self.nav = None;
        self.finder = None;
        self.path_try = None;
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


// ---------------------------------------------------------------- systems

#[derive(Component)]
pub struct Ghost;

pub fn setup(mut commands: Commands, mut mats: ResMut<Assets<StandardMaterial>>, mut meshes: ResMut<Assets<Mesh>>, mut configs: ResMut<GizmoConfigStore>) {
    let shared = world::setup_stage(&mut commands, &mut meshes, &mut mats, std::env::var_os("BEVERLY_LITE").is_none());
    commands.insert_resource(shared);
    let ghost = mats.add(StandardMaterial { base_color: Color::srgba(0.96, 0.86, 0.29, 0.28), alpha_mode: AlphaMode::Blend, unlit: true, cull_mode: None, ..default() });
    commands.spawn((Ghost, Mesh3d(meshes.add(Cuboid::default())), MeshMaterial3d(ghost), Transform::default(), Visibility::Hidden));
    // the editor's lines are on a layer of their own, which only the editor's view shows (not screenshots from a point)
    let (cfg, _) = configs.config_mut::<WidgetGizmos>();
    cfg.render_layers = world::EDITOR_ONLY;
    cfg.line.width = 3.5;
    cfg.depth_bias = -1.0;
    let (cfg, _) = configs.config_mut::<OutlineGizmos>();
    cfg.render_layers = world::EDITOR_ONLY;
    cfg.line.width = 1.5;
    cfg.depth_bias = -1.0;
    let (cfg, _) = configs.config_mut::<DefaultGizmoConfigGroup>();
    cfg.render_layers = world::EDITOR_ONLY;
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
    let asked = ed.test_request.take();
    let key = keys.just_pressed(KeyCode::Backquote) && ed.pending.is_none() && !ed.parts_open && !(view.keyboard_in_ui && ed.testing.is_none());
    if !key && asked.is_none() { return; }
    if asked == Some(TestRequest::Stop) && ed.testing.is_none() { return; }
    if matches!(asked, Some(TestRequest::Start(_))) && ed.testing.is_some() { return; }
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
    let feet = if let Some(TestRequest::Start(f)) = asked { f } else if shift {
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


/// Files dragged in from Explorer are copied into the project's assets; a single recording is then placed as a Sound part
/// (in the Audio tab: opened as a sound).
pub fn files(mut ed: ResMut<Editor>, mut drops: MessageReader<FileDragAndDrop>) {
    let mut imported: Vec<&'static str> = vec![];
    let mut recordings: Vec<String> = vec![];
    for e in drops.read() {
        match e {
            FileDragAndDrop::HoveredFile { .. } => ed.hovering_files = true,
            FileDragAndDrop::HoveredFileCanceled { .. } => ed.hovering_files = false,
            FileDragAndDrop::DroppedFile { path_buf, .. } => {
                ed.hovering_files = false;
                let dir = ed.dir.clone();
                match level::import(&dir, path_buf) {
                    Ok(f) => {
                        imported.push(f);
                        let name = path_buf.file_name().unwrap_or_default().to_string_lossy();
                        if level::is_recording(&name) { recordings.push(format!("assets/sounds/{name}")); }
                    }
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
    // one recording dropped: place a Sound part playing it (click a surface)
    if let [r] = recordings.as_slice() {
        let sound = ed.level.sound_for_clip(r);
        // in the Audio tab it opens there instead
        if ed.tab == super::audio::AUDIO {
            ed.sound_open = Some(sound);
            return;
        }
        ed.island.show("♪", format!("Placing {}", std::path::Path::new(&sound).file_stem().unwrap_or_default().to_string_lossy()), "Click a surface · Esc cancels", Tone::Accent);
        ed.placing = Some(Shape::Sound);
        ed.placing_model = Some(sound);
    }
}



impl Editor {
    /// Answer the question: give the instances the part's new contents (`all`), or make the part Unique.
    pub fn settle(&mut self, all: bool) {
        let Some((id, them)) = self.ask.take() else { return };
        let Some(name) = self.level.get(id).map(|p| p.name.clone()) else { return };
        if all {
            self.level.change_all(id, &them);
            self.island.show("◇", format!("Changed all {}", them.len() + 1), format!("{name} and its instances"), Tone::Accent);
        } else {
            self.level.get_mut(id).expect("found above").unique = true;
            self.island.show("◆", format!("{name} is Unique"), "Its changes stay on it", Tone::Accent);
        }
        // the answer to a question joins the change's step; Make Unique on its own is a step of its own
        if !them.is_empty() { self.amend(); }
    }
}

/// Parts attached to a part that moved or turned go with it (each frame the parts may have changed; the level as of
/// the last undo step says where they were, so the change is one undo step and a drag can be followed live).
pub fn follow_parents(mut ed: ResMut<Editor>) {
    if ed.level.parts.stamp() != ed.followed { ed.follow(); }
}

/// One undo step per finished change: when the level differs from the last step and no drag is going on.
pub fn commit(mut ed: ResMut<Editor>, mouse: Res<ButtonInput<MouseButton>>) {
    // (looking through a camera is one step, made when it ends)
    if ed.drag.is_some() || ed.looking.is_some() || mouse.pressed(MouseButton::Left) { return; }
    ed.refresh_dirty();
    pen::strips(&mut ed);
    super::scatter::scatters(&mut ed);
    ed.commit();
}

impl Editor {
    /// Make the change since the last undo step a step of its own (if there is one). A change to a part's
    /// contents that its instances didn't get asks "Change all, or make this one Unique?" (`ask`).
    pub fn commit(&mut self) {
        // what was moved takes what is attached to it along, in the same step, and the bucket fills follow the change
        if self.level.parts.stamp() != self.followed { self.follow(); }
        bucket::fills(self);
        // nothing was borrowed for change since the last look: nothing to compare
        let stamps = self.level.stamps();
        if stamps == self.checked && self.level.name == self.committed.name { return; }
        self.checked = stamps;
        let step = level::Delta::between(&self.committed, &self.level);
        if step.is_empty() { return; }
        if let Some(split) = self.level.split_from(&self.committed) { self.ask = Some(split); }
        step.apply(&mut self.committed, true);
        debug_assert!(self.committed == self.level, "an undo step took the last state to a different level");
        self.undo.push(step);
        if self.undo.len() > 300 { self.undo.remove(0); }
        self.redo.clear();
    }

    /// Parts attached to a part that moved or turned since the last step go with it.
    fn follow(&mut self) {
        let Editor { level, committed, .. } = self;
        level.follow(committed);
        self.followed = self.level.parts.stamp();
    }

    /// Make the changes made since the last step part of it (the answer to the instances question).
    fn amend(&mut self) {
        let Some(last) = self.undo.pop() else { return self.sync_committed() };
        let mut before = self.committed.clone();
        last.apply(&mut before, false);
        self.undo.push(level::Delta::between(&before, &self.level));
        self.sync_committed();
    }
}

pub fn close_requested(mut ed: ResMut<Editor>, mut closes: MessageReader<WindowCloseRequested>, mut exit: MessageWriter<AppExit>) {
    if closes.read().count() > 0 { ed.request(Pending::Quit, &mut exit); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::Collision;
    #[test]
    fn clicks_hit_parts() {
        let mut ed = Editor::at(std::env::temp_dir());
        let p = Part {
            id: 0, name: "Sphere 1".into(), shape: crate::level::Shape::Sphere, pos: [2.25, 1.0, 3.25], rot: Quat::IDENTITY.to_array(),
            size: [2.0; 3], color: [0.5; 3], visible: true, collision: Collision::Solid, model: None, mover: None, surfaces: vec![], unique: false, tags: vec![], loose: None, light: None, camera: None, sound: None, zone: None, sounds: vec![], sounds_like: None, strokes: vec![], parent: None, scatter: None,
        };
        let id = ed.level.add(p);
        let o = vec3(14.0, 10.0, 14.0);
        let ray = Ray3d::new(o, Dir3::new(vec3(2.25, 1.0, 3.25) - o).unwrap());
        let h = ed.hit(ray).expect("hit");
        assert_eq!(h.part, Some(id));
        assert!((h.point - vec3(2.25, 1.0, 3.25)).length() < 1.05);
    }

    #[test]
    fn scaling_keeps_the_opposite_corner_or_face_or_the_middle_put() {
        let p = Part {
            id: 1, name: "Box 1".into(), shape: Shape::Box, pos: [1.0, 2.0, 3.0], rot: Quat::from_rotation_y(0.7).to_array(),
            size: [2.0, 1.0, 4.0], color: [0.5; 3], visible: true, collision: Collision::Solid, model: None, mover: None, surfaces: vec![], unique: false, tags: vec![], loose: None, light: None, camera: None, sound: None, zone: None, sounds: vec![], sounds_like: None, strokes: vec![], parent: None, scatter: None,
        };
        let bx = (Vec3::splat(-0.5), Vec3::splat(0.5));
        let at = |q: &Part, g: Grip| scale_handles(q, bx).into_iter().find(|(h, _)| *h == g).unwrap().1;
        // a corner doubles the part; the opposite corner stays
        let q = scaled(&p, bx, Grip::Corner(5), 2.0, false);
        assert_eq!(q.size, [4.0, 2.0, 8.0]);
        assert!(at(&q, Grip::Corner(2)).distance(at(&p, Grip::Corner(2))) < 1e-4);
        // a face stretches its axis to the length asked; the opposite face stays
        let q = scaled(&p, bx, Grip::Face(2, true), 6.0, false);
        assert_eq!(q.size, [2.0, 1.0, 6.0]);
        assert!(at(&q, Grip::Face(2, false)).distance(at(&p, Grip::Face(2, false))) < 1e-4);
        // from the middle: the middle stays
        let q = scaled(&p, bx, Grip::Face(0, false), 5.0, true);
        assert!(Vec3::from(q.pos).distance(Vec3::from(p.pos)) < 1e-5);
        // never down to nothing
        assert!(scaled(&p, bx, Grip::Face(1, true), -3.0, false).size[1] >= 0.02);
    }

    #[test]
    fn changing_an_instance_asks_and_change_all_or_unique_is_one_undo_step() {
        let mut ed = Editor::at(std::env::temp_dir());
        let boxed = |x: f32| Part::placed(Shape::Box, None, format!("Box {x}"), [x, 1.0, 0.0], Quat::IDENTITY.to_array(), [2.0; 3]);
        let (a, b, c) = (ed.level.add(boxed(0.0)), ed.level.add(boxed(3.0)), ed.level.add(boxed(6.0)));
        ed.commit();
        let steps = ed.undo.len();
        ed.lay_texture(a, Some(2), "assets/textures/brick.png").unwrap();
        ed.commit();
        assert_eq!(ed.ask, Some((a, vec![b, c])), "it asks");
        ed.settle(true);
        assert!(ed.ask.is_none());
        assert!([b, c].iter().all(|id| ed.level.get(*id).unwrap().surfaces == ed.level.get(a).unwrap().surfaces), "all changed");
        ed.commit();
        assert_eq!(ed.undo.len(), steps + 1, "one step");
        ed.undo();
        assert!(ed.level.parts.iter().all(|p| p.surfaces.is_empty()), "one undo takes it all back");
        // Make Unique: only this one changes, and later changes to it never ask
        ed.lay_texture(b, None, "assets/textures/brick.png").unwrap();
        ed.commit();
        ed.settle(false);
        assert!(ed.level.get(b).unwrap().unique && ed.level.get(a).unwrap().surfaces.is_empty());
        ed.level.get_mut(b).unwrap().visible = false;
        ed.commit();
        assert!(ed.ask.is_none());
        // the box (size, place, turn) is each part's own: no question
        ed.level.get_mut(a).unwrap().size = [1.0; 3];
        ed.level.get_mut(a).unwrap().pos = [0.0, 5.0, 0.0];
        ed.commit();
        assert!(ed.ask.is_none());
        // the colour is inside: it asks
        ed.level.get_mut(a).unwrap().color = [1.0, 0.0, 0.0];
        ed.commit();
        assert_eq!(ed.ask, Some((a, vec![c])));
    }
}
