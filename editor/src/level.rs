//! What gets saved: a project is a folder of plain text files.
//!
//! ```text
//! My Game/
//!   project.bev          name, levels
//!   levels/main.bev      the parts placed in a level
//!   assets/textures/ ... files dropped onto the Assets panel are copied here
//!   assets/materials/    a texture and its settings, shared by every surface that uses it
//!   assets/models/ assets/sounds/ assets/other/
//! ```
//!
//! `.bev` files are RON text (characters/*.bev will join them). Each starts with `format`, the [`FORMAT`] that wrote
//! it, so a newer Beverly can upgrade older files as it opens them and an older one refuses newer files instead of
//! misreading them. Projects from before `.bev` (project.ron, levels/*.ron, format 0) still open; saving one writes
//! .bev files in place of its .ron ones.
use crate::settings::{Label, Ranges, Setting};
use bevy::math::{EulerRot, Quat, Vec3};
use bevy::reflect::{std_traits::ReflectDefault, Reflect};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

mod delta;
mod hierarchy;
mod material;
mod paint;
mod project;
mod shape;
mod sound;
mod track;
mod world;
pub use delta::Delta;
pub use hierarchy::{beyond, frame, within};
pub use material::*;
pub use paint::*;
pub use project::*;
pub use shape::*;
pub use sound::*;
pub use track::{Stamps, Tracked};
pub use world::*;



fn is_default<T: Default + PartialEq>(v: &T) -> bool { *v == T::default() }



/// A wire: when part `from` sends `out`, part `to` gets `input`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Wire {
    pub from: u64,
    pub out: String,
    pub to: u64,
    pub input: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Reflect)]
pub enum Collision {
    Solid,
    #[reflect(@Label("Walk-through"))]
    WalkThrough,
    Trigger,
}

impl Collision {
    pub const ALL: [Collision; 3] = [Collision::Solid, Collision::WalkThrough, Collision::Trigger];
    pub fn label(self) -> &'static str {
        match self {
            Collision::Solid => "Solid",
            Collision::WalkThrough => "Walk-through",
            Collision::Trigger => "Trigger",
        }
    }
}

/// What a part's surface is, for the characters and systems that care: climbing, digging, swimming, grinding,
/// perching birds and sliding.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Reflect)]
pub enum Tag {
    Climbable,
    Diggable,
    Water,
    #[reflect(@Label("Grind rail"))]
    GrindRail,
    #[reflect(@Label("Perch spot"))]
    PerchSpot,
    Slippery,
    /// Pen paths lie on it, whatever its slope or height.
    Ground,
    /// Pen paths pass over it as if it weren't there (unless a stretch says Over everything).
    #[reflect(@Label("Paths ignore"))]
    PathsIgnore,
}

impl Tag {
    pub const ALL: [Tag; 8] = [Tag::Climbable, Tag::Diggable, Tag::Water, Tag::GrindRail, Tag::PerchSpot, Tag::Slippery, Tag::Ground, Tag::PathsIgnore];
}

/// A loose part: it falls and tumbles when played (a crate, a barrel), is pushed by characters and blasts.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Loose {
    /// Kilograms.
    #[reflect(@Setting::feel("Weight", &["Feather", "Light", "Medium", "Heavy", "Massive"], 1.0, 2000.0, "kg").log().section(LOOK).help("Light things are pushed and thrown easily"))]
    pub mass: f32,
    /// How much it grips what it lies on (0: slides like ice).
    #[reflect(@Setting::feel("Grip", &["Icy", "Loose", "Firm", "Glued"], 0.0, 1.5, "").section(LOOK).help("How much it grips what it lies on"))]
    pub friction: f32,
}

impl Default for Loose {
    fn default() -> Self { Loose { mass: 20.0, friction: 0.6 } }
}

/// One basic shape placed in a level. With `visible` off it is only a collider.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
pub struct Part {
    pub id: u64,
    #[reflect(@Setting::text("Name").section(PART))]
    pub name: String,
    pub shape: Shape,
    #[reflect(@Setting::number("Position", "m", 0.05, -100000.0, 100000.0).section(PLACE).bare())]
    pub pos: [f32; 3],
    /// Rotation as a quaternion (x, y, z, w).
    #[reflect(@Setting::turn("Turn").section(PLACE))]
    pub rot: [f32; 4],
    #[reflect(@Setting::number("Size", "m", 0.02, 0.02, 100000.0).section(LOOK).bare())]
    pub size: [f32; 3],
    /// sRGB, 0..1.
    #[reflect(@Setting::colour("Colour").section(LOOK))]
    pub color: [f32; 3],
    #[reflect(@Setting::toggle("Visible").section(LOOK).help("Off: an invisible collider (shown hatched in the editor only)"))]
    pub visible: bool,
    #[reflect(@Setting::choice("Collision").section(LOOK).bare())]
    pub collision: Collision,
    /// None: it stays where it is put (things stand on it and bump into it). Some: loose (Solid parts; a part that
    /// Moves is a moving body whatever this says).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[reflect(@Setting::optional("Physics", "Still", "Loose").section(LOOK).help("Loose: falls and tumbles when played (Solid parts)"))]
    pub loose: Option<Loose>,
    /// For `Shape::Model`: the model file, relative to the project folder (e.g. "assets/models/tower.glb").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Textures laid on the part (basic shapes): all over, or face by face.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surfaces: Vec<Surface>,
    /// Off (the default): parts with the same contents (what is inside the box: see [`Part::same_contents`]) are
    /// instances of each other, and changing one asks whether to change them all. On: its contents are its own.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[reflect(@Setting::toggle("Unique").section(PART).help("On: changing its look never changes the parts just like it"))]
    pub unique: bool,
    /// What its surface is (see [`Tag`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[reflect(@Setting::tags("Tags").section(TAGS).bare().help("What its surface is, for the characters and systems that care"))]
    pub tags: Vec<Tag>,
    /// A light part's light (`Shape::PointLight`, `Shape::SpotLight`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[reflect(@Setting::optional("Light", "Off", "On").section(LIGHT))]
    pub light: Option<Light>,
    /// A camera part's camera (`Shape::FixedCamera`, `OrbitCamera`, `FollowCamera`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[reflect(@Setting::optional("Camera", "Off", "On").section(CAMERA))]
    pub camera: Option<Camera>,
    /// A Sound part's sound (`Shape::Sound`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[reflect(@Setting::optional("Sound", "Off", "On").section(SPEAKER))]
    pub sound: Option<Speaker>,
    /// Sounds it plays when things happen to it (Pressed, Arrived, Hit…).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sounds: Vec<Hook>,
    /// Brush strokes painted on it, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub strokes: Vec<Stroke>,
    /// What its surface sounds like underfoot (None: nothing special; a material can say instead).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sounds_like: Option<Ground>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[reflect(@Setting::optional("Moves", "Stays put", "Moves when told to").section(MOVES).help("Moves to an end position and back when wired to"))]
    pub mover: Option<Mover>,
    /// The part this one is attached to (see `hierarchy.rs`): when that one moves or turns, this one goes with it. The
    /// position and turn here are still the world's; the file keeps them as offsets from the parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u64>,
}

impl Part {
    /// A part as it is first placed: the default colour (white for a model, which tints it), and Walk-through for a player start.
    pub fn placed(shape: Shape, model: Option<String>, name: String, pos: [f32; 3], rot: [f32; 4], size: [f32; 3]) -> Part {
        Part {
            id: 0, name, shape, pos, rot, size,
            color: match shape { Shape::Interact => [0.94, 0.55, 0.26], Shape::Model => [1.0; 3], _ => [0.62, 0.64, 0.70] },
            visible: true,
            collision: if shape == Shape::PlayerStart || shape.is_marker() { Collision::WalkThrough } else { Collision::Solid },
            model, mover: None, surfaces: vec![], unique: false, tags: vec![], loose: None,
            light: shape.is_light().then(Light::default),
            camera: shape.is_camera().then(Camera::default),
            sound: (shape == Shape::Sound).then(Speaker::default),
            sounds: vec![], sounds_like: None, strokes: vec![], parent: None,
        }
    }

    pub fn outputs(&self) -> Vec<&'static str> {
        let mut v = self.shape.outputs().to_vec();
        if self.mover.is_some() { v.extend(Mover::OUTPUTS); }
        v
    }
    /// What can happen to it while playing, each of which can play a sound: what it sends along wires, Starts
    /// (beginning to move) and Hit (a loose part knocking into something).
    pub fn events(&self) -> Vec<&'static str> {
        let mut v = self.outputs();
        if self.mover.is_some() { v.insert(0, "Starts"); }
        if self.loose.is_some() { v.push("Hit"); }
        v
    }
    /// The sound it plays when `e` happens.
    pub fn sound_on(&self, e: &str) -> Option<&str> { self.sounds.iter().find(|h| h.on == e).map(|h| h.sound.as_str()).filter(|s| !s.is_empty()) }

    pub fn inputs(&self) -> &'static [&'static str] {
        if self.mover.is_some() { &Mover::INPUTS } else if self.shape == Shape::Sound { &Speaker::INPUTS } else { &[] }
    }

    /// Whether two parts hold the same contents: what is inside the box (the shape or model, its colour, its
    /// textures and how they lie, Visible, Collision, tags, Physics, strokes painted by hand). The box itself (name, position,
    /// turn, size) is each part's own, as an instance's placement is in SketchUp, and so are the strokes a pen curve or
    /// a bucket fill made on it (they come from where it stands).
    pub fn same_contents(&self, o: &Part) -> bool {
        self.shape == o.shape && self.model == o.model && self.color == o.color && self.surfaces == o.surfaces && self.visible == o.visible && self.collision == o.collision
            && self.tags == o.tags && self.loose == o.loose && self.sounds_like == o.sounds_like && self.own_strokes().eq(o.own_strokes())
    }
    /// The strokes painted on it by hand (not made by a pen curve or a bucket fill).
    pub fn own_strokes(&self) -> impl Iterator<Item = &Stroke> { self.strokes.iter().filter(|s| s.curve.is_none() && s.bucket.is_none()) }
    /// Take `from`'s contents, keeping this part's box.
    pub fn set_contents(&mut self, from: &Part) {
        self.shape = from.shape;
        self.model = from.model.clone();
        self.color = from.color;
        self.surfaces = from.surfaces.clone();
        self.visible = from.visible;
        self.collision = from.collision;
        self.tags = from.tags.clone();
        self.loose = from.loose.clone();
        self.sounds_like = from.sounds_like;
        // its own made strokes stay; the ones painted by hand are the other's
        let made: Vec<Stroke> = self.strokes.iter().filter(|s| s.curve.is_some() || s.bucket.is_some()).cloned().collect();
        self.strokes = from.own_strokes().cloned().chain(made).collect();
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct Level {
    /// The file format that wrote this level (see [`FORMAT`]); 0 for files from before it existed.
    #[serde(default)]
    pub format: u32,
    pub name: String,
    pub parts: Tracked<Vec<Part>>,
    pub next_id: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wires: Tracked<Vec<Wire>>,
    /// Pen curves (see `curve.rs`); their ids come from the same count as the parts'.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub curves: Tracked<Vec<crate::curve::Curve>>,
    /// Bucket fills (see `bucket.rs`); their ids come from the same count as the parts'.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buckets: Tracked<Vec<Bucket>>,
    /// Slider range ends changed under More in the panels (see `settings`).
    #[serde(default, skip_serializing_if = "Ranges::is_empty")]
    pub ranges: Tracked<Ranges>,
    #[serde(default, skip_serializing_if = "Lighting::is_default")]
    pub lighting: Tracked<Lighting>,
    #[serde(default, skip_serializing_if = "Navigation::is_default")]
    pub navigation: Tracked<Navigation>,
    #[serde(default, skip_serializing_if = "WalkerSounds::is_default")]
    pub walker: Tracked<WalkerSounds>,
    /// The project's materials, by path ("assets/materials/brick.bev"). Saved as their own files, not in the level;
    /// kept here so undo and the unsaved-changes dot cover them.
    #[serde(skip)]
    pub materials: Tracked<std::collections::BTreeMap<String, Material>>,
    /// The project's sound files, by path ("assets/sounds/birds.bev"), kept and saved as the materials are.
    #[serde(skip)]
    pub sounds: Tracked<std::collections::BTreeMap<String, Sound>>,
}

impl Level {
    /// What changed where: see [`Stamps`].
    pub fn stamps(&self) -> Stamps {
        [self.parts.stamp(), self.wires.stamp(), self.curves.stamp(), self.buckets.stamp(), self.ranges.stamp(), self.lighting.stamp(), self.navigation.stamp(), self.walker.stamp(), self.materials.stamp(), self.sounds.stamp()]
    }

    pub fn get(&self, id: u64) -> Option<&Part> { self.parts.iter().find(|p| p.id == id) }
    pub fn get_mut(&mut self, id: u64) -> Option<&mut Part> { self.parts.iter_mut().find(|p| p.id == id) }

    /// A new name like "Box 3": the lowest number not used by another part of that shape.
    pub fn fresh_name(&self, base: &str) -> String {
        (1..).map(|n| format!("{base} {n}")).find(|n| !self.parts.iter().any(|p| &p.name == n)).unwrap()
    }

    /// Whether a part is instanced: only a mesh plus a material (a basic shape or a model), not Unique, with no
    /// behaviour (Moves, wires), which keeps a part individual.
    pub fn instanced(&self, p: &Part) -> bool {
        (Shape::ALL.contains(&p.shape) || p.shape == Shape::Model) && !p.unique && p.mover.is_none()
            && !self.wires.iter().any(|w| w.from == p.id || w.to == p.id)
    }
    /// The other parts that are instances of the same contents as part `id`.
    pub fn instances(&self, id: u64) -> Vec<u64> {
        let Some(p) = self.get(id).filter(|p| self.instanced(p)) else { return vec![] };
        self.parts.iter().filter(|q| q.id != id && self.instanced(q) && q.same_contents(p)).map(|q| q.id).collect()
    }
    /// The part whose contents changed since `before` while its instances there kept theirs: (it, them). Changing
    /// them all, or making it Unique, settles it.
    pub fn split_from(&self, before: &Level) -> Option<(u64, Vec<u64>)> {
        self.parts.iter().find_map(|p| {
            let old = before.get(p.id).filter(|o| !o.same_contents(p) && self.instanced(p))?;
            let them: Vec<u64> = before.instances(p.id).into_iter().filter(|o| self.get(*o).is_some_and(|q| q.same_contents(old) && self.instanced(q))).collect();
            (!them.is_empty()).then_some((p.id, them))
        })
    }
    /// Give `them` part `id`'s contents.
    pub fn change_all(&mut self, id: u64, them: &[u64]) {
        let Some(p) = self.get(id).cloned() else { return };
        for q in self.parts.iter_mut().filter(|q| them.contains(&q.id)) { q.set_contents(&p); }
    }

    pub fn add(&mut self, mut part: Part) -> u64 {
        self.next_id += 1;
        part.id = self.next_id;
        self.parts.push(part);
        self.next_id
    }
    /// A new curve through these points (named like "Curve 2"); its id.
    pub fn add_curve(&mut self, points: Vec<crate::curve::PenPoint>, closed: bool) -> u64 {
        self.next_id += 1;
        let name = (1..).map(|n| format!("Curve {n}")).find(|n| !self.curves.iter().any(|c| &c.name == n)).expect("a free name");
        self.curves.push(crate::curve::Curve { id: self.next_id, name, points, closed, strip: None, fill: None, path: Default::default() });
        self.next_id
    }
    pub fn curve(&self, id: u64) -> Option<&crate::curve::Curve> { self.curves.iter().find(|c| c.id == id) }
    pub fn curve_mut(&mut self, id: u64) -> Option<&mut crate::curve::Curve> { self.curves.iter_mut().find(|c| c.id == id) }
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exports_a_zip_with_the_player_and_the_project() {
        let base = std::env::temp_dir().join(format!("beverly-export-{}", std::process::id()));
        let dir = base.join("proj");
        let lvl = Level { name: "main".into(), ..Default::default() };
        save(&dir, "My Game", &lvl).unwrap();
        std::fs::write(dir.join("assets/textures/a.png"), b"png").unwrap();
        let player = base.join("player.bin");
        std::fs::write(&player, b"exe").unwrap();
        let out = base.join("out.zip");
        let n = export_zip(&dir, "My Game", &player, &[("bakes/k/a".into(), b"baked".to_vec())], &out).unwrap();
        assert_eq!(n, 5); // player, project.bev, levels/main.bev, the texture, the bake
        let mut z = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
        let ext = if cfg!(windows) { ".exe" } else { "" };
        assert!(z.by_name(&format!("My Game/My Game{ext}")).is_ok());
        assert!(z.by_name("My Game/levels/main.bev").is_ok());
        assert!(z.by_name("My Game/assets/textures/a.png").is_ok());
        assert!(z.by_name("My Game/bakes/k/a").is_ok());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn saves_bev_files_with_their_format_and_still_opens_ron_projects() {
        let dir = std::env::temp_dir().join(format!("beverly-bev-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // a project as older Beverlys saved it
        make_folders(&dir).unwrap();
        std::fs::write(dir.join("project.ron"), r#"(name: "Old", levels: ["main"])"#).unwrap();
        std::fs::write(dir.join("levels/main.ron"), r#"(name: "main", parts: [], next_id: 0)"#).unwrap();
        assert!(is_old(&dir));
        assert_eq!(project_dir_of(&dir.join("levels/main.ron")), dir, "a file inside opens its project");
        let (name, lvl) = load(&dir).unwrap();
        assert_eq!((name.as_str(), lvl.format), ("Old", FORMAT));
        // saving upgrades it: .bev files that start with their format, and no .ron left behind
        save(&dir, &name, &lvl).unwrap();
        assert!(!is_old(&dir) && !dir.join("project.ron").exists() && !dir.join("levels/main.ron").exists());
        let text = std::fs::read_to_string(dir.join("levels/main.bev")).unwrap();
        assert!(text.trim_start_matches('(').trim_start().starts_with(&format!("format: {FORMAT},")), "{text}");
        assert_eq!(load(&dir).unwrap(), (name, lvl));
        // a newer Beverly's files are refused, not misread
        std::fs::write(dir.join("project.bev"), format!(r#"(format: {}, name: "New", levels: ["main"])"#, FORMAT + 1)).unwrap();
        assert!(load(&dir).unwrap_err().contains("newer Beverly"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parts_with_parents_are_saved_as_offsets_in_format_2_and_load_back_to_the_world() {
        let dir = std::env::temp_dir().join(format!("beverly-parent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut lvl = Level { name: "main".into(), ..Default::default() };
        let a = lvl.add(Part::placed(Shape::Box, None, "Ship".into(), [100.0, 5.0, 40.0], Quat::from_rotation_y(0.5).to_array(), [4.0; 3]));
        let b = lvl.add(Part::placed(Shape::Box, None, "Door".into(), [102.0, 5.0, 40.0], [0.0, 0.0, 0.0, 1.0], [1.0; 3]));
        save(&dir, "T", &lvl).unwrap();
        assert!(std::fs::read_to_string(dir.join("levels/main.bev")).unwrap().contains("format: 1,"), "no parents: still format 1");
        lvl.set_parent(b, Some(a)).unwrap();
        save(&dir, "T", &lvl).unwrap();
        let text = std::fs::read_to_string(dir.join("levels/main.bev")).unwrap();
        assert!(text.contains("format: 2,") && text.contains(&format!("parent: Some({a})")), "{text}");
        // the file holds the door's offset from the ship, not its place in the world
        assert!(!text.contains("102.0"), "{text}");
        let (_, back) = load(&dir).unwrap();
        for id in [a, b] { assert!(Vec3::from(back.get(id).unwrap().pos).distance(Vec3::from(lvl.get(id).unwrap().pos)) < 1e-3); }
        assert_eq!(back.get(b).unwrap().parent, Some(a));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn surfaces_cover_faces_and_share_one_material_per_texture() {
        let dir = std::env::temp_dir().join(format!("beverly-mat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut lvl = Level { name: "main".into(), ..Default::default() };
        let brick = lvl.material_for_texture("assets/textures/brick.png");
        assert_eq!(brick, "assets/materials/brick.bev");
        assert_eq!(lvl.material_for_texture("assets/textures/brick.png"), brick, "the same texture reuses its material");
        assert_eq!(lvl.material_for_texture("assets/textures/other/brick.png"), "assets/materials/brick 2.bev");
        lvl.materials.get_mut(&brick).unwrap().glow = 2.0;
        let mut p = Part::placed(Shape::Box, None, "Box 1".into(), [0.0; 3], [0.0, 0.0, 0.0, 1.0], [2.0; 3]);
        let s = |face, m: &str| Surface { face, material: m.into(), tile: [1.0, 1.0], offset: [0.0, 0.0] };
        p.lay(s(None, "all"));
        p.lay(s(Some(2), "top"));
        assert_eq!((p.surface(2).unwrap().material.as_str(), p.surface(0).unwrap().material.as_str()), ("top", "all"));
        p.lay(s(Some(2), "top again"));
        assert_eq!(p.surfaces.len(), 2, "a face holds one surface");
        p.lay(s(None, &brick));
        assert_eq!(p.surfaces.len(), 1, "all over replaces the faces' own");
        lvl.add(p);
        assert_eq!(lvl.material_users(&brick), (1, 1));
        // materials are saved as their own files and come back with the level
        save(&dir, "Mats", &lvl).unwrap();
        assert!(dir.join("assets/materials/brick 2.bev").is_file());
        let (_, back) = load(&dir).unwrap();
        assert_eq!(back.materials[&brick].glow, 2.0);
        assert_eq!(back.materials, lvl.materials);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sound_parts_play_sound_files_saved_beside_the_level() {
        let dir = std::env::temp_dir().join(format!("beverly-snd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut lvl = Level { name: "main".into(), ..Default::default() };
        let birds = lvl.sound_for_clip("assets/sounds/birds.ogg");
        assert_eq!(birds, "assets/sounds/birds.bev");
        assert_eq!(lvl.sound_for_clip("assets/sounds/birds.ogg"), birds, "the same recording reuses its sound");
        lvl.sounds.get_mut(&birds).unwrap().clips.push("assets/sounds/birds 2.ogg".into());
        assert_eq!(lvl.sound_for_clip("assets/sounds/birds.ogg"), "assets/sounds/birds 2.bev", "a sound with more recordings isn't just that one");
        lvl.sounds.get_mut(&birds).unwrap().looped = true;
        let mut p = Part::placed(Shape::Sound, None, "Birds".into(), [0.0; 3], [0.0, 0.0, 0.0, 1.0], Shape::Sound.default_size());
        assert_eq!((p.collision, p.inputs()), (Collision::WalkThrough, &Speaker::INPUTS[..]));
        p.sound.as_mut().unwrap().sound = birds.clone();
        lvl.add(p);
        assert_eq!(lvl.sound_users(&birds), 1);
        save(&dir, "Sounds", &lvl).unwrap();
        assert!(dir.join("assets/sounds/birds 2.bev").is_file());
        let (_, back) = load(&dir).unwrap();
        assert_eq!(back.sounds, lvl.sounds);
        assert_eq!(back.parts[0].sound.as_ref().unwrap().sound, birds);
        assert!(!is_recording("birds.bev") && is_recording("Birds.OGG"));
        // on a surface with a set of its own it plays that set; elsewhere its usual recordings
        let mut steps = Sound { clips: vec!["step.ogg".into()], ..Default::default() };
        steps.sets.push(SoundSet { ground: Ground::Grass, clips: vec!["grass 1.ogg".into(), "grass 2.ogg".into()] });
        steps.sets.push(SoundSet { ground: Ground::Metal, clips: vec![] });
        assert_eq!(steps.clips_on(Some(Ground::Grass)), ["grass 1.ogg", "grass 2.ogg"]);
        assert_eq!(steps.clips_on(Some(Ground::Metal)), ["step.ogg"], "an empty set plays the usual ones");
        assert_eq!(steps.clips_on(None), ["step.ogg"]);
        assert_eq!(steps.all_clips().count(), 3);
        let text = ron::to_string(&steps).unwrap();
        assert_eq!(ron::from_str::<Sound>(&text).unwrap(), steps);
        // underfoot: the part's own sound, or the material on its top face over it
        let mut lvl = Level::default();
        let mut floor = Part::placed(Shape::Box, None, "Floor".into(), [0.0; 3], [0.0, 0.0, 0.0, 1.0], [4.0, 1.0, 4.0]);
        floor.sounds_like = Some(Ground::Wood);
        let id = lvl.add(floor);
        assert_eq!(lvl.ground_under(id), Some(Ground::Wood));
        lvl.materials.insert("assets/materials/turf.bev".into(), Material { sounds_like: Some(Ground::Grass), ..Default::default() });
        lvl.get_mut(id).unwrap().lay(Surface { face: None, material: "assets/materials/turf.bev".into(), tile: [1.0, 1.0], offset: [0.0, 0.0] });
        assert_eq!(lvl.ground_under(id), Some(Ground::Grass));
        // what can happen to a part: its outputs, Starts when it moves, Hit when it's loose
        let mut b = Part::placed(Shape::Interact, None, "B".into(), [0.0; 3], [0.0, 0.0, 0.0, 1.0], [0.5; 3]);
        assert_eq!(b.events(), ["Pressed"]);
        b.mover = Some(Mover::default());
        b.loose = Some(Loose::default());
        assert_eq!(b.events(), ["Starts", "Pressed", "Arrived", "Returned", "Hit"]);
        b.sounds.push(Hook { on: "Hit".into(), sound: "assets/sounds/knock.bev".into() });
        assert_eq!((b.sound_on("Hit"), b.sound_on("Pressed")), (Some("assets/sounds/knock.bev"), None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_orbit_camera_circles_its_target_and_a_follow_camera_keeps_its_offset() {
        let cam = |shape| Part { camera: Some(Camera { target: [0.0, 0.0, 0.0], lap: 8.0, ..Default::default() }), ..Part::placed(shape, None, "Cam".into(), [0.0, 2.0, 4.0], [0.0, 0.0, 0.0, 1.0], [0.4; 3]) };
        let (feet, start) = (Vec3::new(3.0, 0.0, -1.0), Vec3::ZERO);
        let orbit = cam(Shape::OrbitCamera);
        let (at, rot) = camera_view(&orbit, 2.0, feet, start);
        assert!(at.distance(Vec3::new(4.0, 2.0, 0.0)) < 1e-4, "a quarter lap round: {at}");
        assert!((rot * Vec3::NEG_Z).distance(-at.normalize()) < 1e-4, "looking at the target");
        assert!(camera_view(&orbit, 8.0, feet, start).0.distance(Vec3::from(orbit.pos)) < 1e-4, "a lap brings it back");
        let follow = cam(Shape::FollowCamera);
        assert_eq!(camera_view(&follow, 5.0, feet, start), (Vec3::new(3.0, 2.0, 3.0), Quat::IDENTITY));
        let fixed = cam(Shape::FixedCamera);
        assert_eq!(camera_view(&fixed, 5.0, feet, start).0, Vec3::new(0.0, 2.0, 4.0));
        assert!(looking(Vec3::ZERO, Vec3::ZERO).is_none());
        assert!((looking(Vec3::ZERO, Vec3::new(1.0, 1.0, 0.0)).unwrap() * Vec3::NEG_Z).distance(Vec3::new(1.0, 1.0, 0.0).normalize()) < 1e-5);
    }

    #[test]
    fn instances_share_their_contents_and_unique_or_behaviour_keeps_a_part_alone() {
        let mut lvl = Level { name: "main".into(), ..Default::default() };
        let boxed = |x: f32| Part::placed(Shape::Box, None, format!("Box {x}"), [x, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0], [2.0; 3]);
        let a = lvl.add(boxed(0.0));
        let b = lvl.add(Part { size: [1.0, 3.0, 1.0], rot: [0.0, 0.38268343, 0.0, 0.9238795], ..boxed(3.0) });
        let c = lvl.add(boxed(6.0));
        let ball = lvl.add(Part::placed(Shape::Sphere, None, "Ball".into(), [9.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0], [2.0; 3]));
        // the box (place, turn, size) is each part's own; the contents are shared
        assert_eq!(lvl.instances(a), [b, c]);
        assert!(lvl.instances(ball).is_empty());
        // texturing one splits it from the others until they all change or it is made Unique
        let before = lvl.clone();
        lvl.get_mut(a).unwrap().lay(Surface { face: None, material: "assets/materials/brick.bev".into(), tile: [1.0, 1.0], offset: [0.0, 0.0] });
        assert_eq!(lvl.split_from(&before), Some((a, vec![b, c])));
        let mut all = lvl.clone();
        all.change_all(a, &[b, c]);
        assert_eq!(all.split_from(&before), None);
        assert_eq!(all.instances(a), [b, c]);
        assert_eq!(all.get(b).unwrap().size, [1.0, 3.0, 1.0], "their boxes stay theirs");
        // a different colour is different contents (the colour is a plain shape's material)
        assert!(!boxed(0.0).same_contents(&Part { color: [1.0, 0.0, 0.0], ..boxed(0.0) }));
        // so are different surface tags
        assert!(!boxed(0.0).same_contents(&Part { tags: vec![Tag::Slippery], ..boxed(0.0) }));
        lvl.get_mut(a).unwrap().unique = true;
        assert_eq!(lvl.split_from(&before), None);
        assert_eq!(lvl.instances(b), [c]);
        // moving or wired parts are individual
        let mut lvl = before.clone();
        lvl.get_mut(c).unwrap().mover = Some(Mover::default());
        assert_eq!(lvl.instances(a), [b]);
        lvl.wires.push(Wire { from: b, out: "x".into(), to: ball, input: "y".into() });
        assert!(lvl.instances(a).is_empty());
        // Unique is saved only when on
        let text = ron::to_string(&Part { unique: true, ..boxed(0.0) }).unwrap();
        assert!(text.contains("unique:true") && !ron::to_string(&boxed(0.0)).unwrap().contains("unique"), "{text}");
    }

    #[test]
    fn light_parts_carry_a_light_are_individual_and_never_collide() {
        let mut lvl = Level { name: "main".into(), ..Default::default() };
        let mut spot = Part::placed(Shape::SpotLight, None, "Spot 1".into(), [0.0, 3.0, 0.0], [1.0, 0.0, 0.0, 0.0], Shape::SpotLight.default_size());
        assert_eq!(spot.light, Some(Light::default()));
        spot.collision = Collision::Solid;
        let a = lvl.add(spot.clone());
        lvl.add(spot);
        assert!(lvl.instances(a).is_empty(), "lights are always individual");
        assert!(lvl.parts.iter().all(|p| crate::physics::kind(p).is_none()), "not even set Solid");
        let text = ron::to_string(&lvl.parts[0]).unwrap();
        assert_eq!(ron::from_str::<Part>(&text).unwrap(), lvl.parts[0]);
        assert!(!ron::to_string(&Part::placed(Shape::Box, None, "Box".into(), [0.0; 3], [0.0, 0.0, 0.0, 1.0], [1.0; 3])).unwrap().contains("light"));
    }
}
