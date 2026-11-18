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

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug, Reflect)]
pub enum Shape {
    Box,
    Ramp,
    Stairs,
    Arch,
    Cylinder,
    Sphere,
    Plane,
    /// An imported glTF model (`Part::model` is its path in the project); `size` is its scale.
    Model,
    /// Where the player character starts. Shown in the editor only.
    PlayerStart,
    /// A button the player presses (E when close). Output: Pressed.
    Interact,
    /// Light in every direction, out to its reach (`Part::light`). Shown in the editor only; never collides.
    PointLight,
    /// Light in a cone along the part's up (+Y): placed on a ceiling it shines down.
    SpotLight,
    /// A camera that stays where it is put, looking along its -Z (`Part::camera`). Editor only; never collides.
    FixedCamera,
    /// A camera that circles its target (`Camera::target`) once a lap, always looking at it.
    OrbitCamera,
    /// A camera that keeps its offset from the player start as the player moves, looking the way it was aimed.
    FollowCamera,
    /// A speaker playing a sound file (`Part::sound`), heard in 3D. Shown in the editor only; never collides.
    /// Inputs: Play, Stop.
    Sound,
}

impl Shape {
    pub const ALL: [Shape; 7] = [Shape::Box, Shape::Ramp, Shape::Stairs, Shape::Arch, Shape::Cylinder, Shape::Sphere, Shape::Plane];
    pub const LIGHTS: [Shape; 2] = [Shape::PointLight, Shape::SpotLight];
    pub fn is_light(self) -> bool { Shape::LIGHTS.contains(&self) }
    pub const CAMERAS: [Shape; 3] = [Shape::FixedCamera, Shape::OrbitCamera, Shape::FollowCamera];
    pub fn is_camera(self) -> bool { Shape::CAMERAS.contains(&self) }
    /// Kinds that are shown in the editor only and never collide: lights, cameras and sounds.
    pub fn is_marker(self) -> bool { self.is_light() || self.is_camera() || self == Shape::Sound }
    /// Every kind with triangles of its own (all but models).
    pub fn built() -> impl Iterator<Item = Shape> { Shape::ALL.into_iter().chain([Shape::PlayerStart, Shape::Interact]).chain(Shape::LIGHTS).chain(Shape::CAMERAS).chain([Shape::Sound]) }
    pub fn label(self) -> &'static str {
        match self {
            Shape::Box => "Box",
            Shape::Ramp => "Ramp",
            Shape::Stairs => "Stairs",
            Shape::Arch => "Arch",
            Shape::Cylinder => "Cylinder",
            Shape::Sphere => "Sphere",
            Shape::Plane => "Plane",
            Shape::Model => "Model",
            Shape::PlayerStart => "Player start",
            Shape::Interact => "Interact",
            Shape::PointLight => "Point light",
            Shape::SpotLight => "Spot light",
            Shape::FixedCamera => "Fixed camera",
            Shape::OrbitCamera => "Orbit camera",
            Shape::FollowCamera => "Follow camera",
            Shape::Sound => "Sound",
        }
    }
    pub fn glyph(self) -> &'static str {
        match self {
            Shape::Box => "▢",
            Shape::Ramp => "◺",
            Shape::Stairs => "▤",
            Shape::Arch => "∩",
            Shape::Cylinder => "◯",
            Shape::Sphere => "●",
            Shape::Plane => "▭",
            Shape::Model => "◆",
            Shape::PlayerStart => "☺",
            Shape::Interact => "☝",
            Shape::PointLight => "☼",
            Shape::SpotLight => "▽",
            Shape::FixedCamera => "▣",
            Shape::OrbitCamera => "↻",
            Shape::FollowCamera => "⇢",
            Shape::Sound => "♪",
        }
    }
    /// One line for the parts pop-up.
    pub fn description(self) -> String {
        let [x, y, z] = self.default_size();
        let what = match self {
            Shape::Box => "a block for walls, floors and cover",
            Shape::Ramp => "a slope rising toward +X",
            Shape::Stairs => "six steps rising toward +X",
            Shape::Arch => "a block with a round-topped opening",
            Shape::Cylinder => "a round column",
            Shape::Sphere => "a ball",
            Shape::Plane => "a thin slab for floors and platforms",
            Shape::Model => return "an imported glTF model".into(),
            Shape::PlayerStart => return "where the player character starts (Shift+` tests from here)".into(),
            Shape::Interact => return "a button the player presses with E; wire Pressed to what it starts".into(),
            Shape::PointLight => return "light in every direction, out to its reach".into(),
            Shape::SpotLight => return "a cone of light, shining away from the surface it is placed on".into(),
            Shape::FixedCamera => return "a view that stays put; Look through to aim it".into(),
            Shape::OrbitCamera => return "a view circling a target you drag, once a lap".into(),
            Shape::FollowCamera => return "a view that keeps its offset from the player as they move".into(),
            Shape::Sound => return "a speaker playing a sound, on start or when wired; heard in 3D".into(),
        };
        format!("{what} · {x} × {y} × {z} m")
    }

    /// Size in metres (x wide, y tall, z deep) when dropped.
    pub fn default_size(self) -> [f32; 3] {
        match self {
            Shape::Box => [2.0, 2.0, 2.0],
            Shape::Ramp => [4.0, 2.0, 3.0],
            Shape::Stairs => [4.0, 2.0, 2.0],
            Shape::Arch => [4.0, 3.0, 1.0],
            Shape::Cylinder => [1.5, 3.0, 1.5],
            Shape::Sphere => [2.0, 2.0, 2.0],
            Shape::Plane => [4.0, 0.1, 4.0],
            Shape::Model => [1.0, 1.0, 1.0],
            Shape::PlayerStart => [0.8, 1.8, 0.8],
            Shape::Interact => [0.5, 1.0, 0.5],
            Shape::PointLight | Shape::SpotLight => [0.3, 0.3, 0.3],
            Shape::FixedCamera | Shape::OrbitCamera | Shape::FollowCamera => [0.4, 0.3, 0.5],
            Shape::Sound => [0.4, 0.4, 0.4],
        }
    }

    /// Events this part sends out along wires.
    pub fn outputs(self) -> &'static [&'static str] {
        match self {
            Shape::Interact => &["Pressed"],
            _ => &[],
        }
    }
}

/// A part that moves: from where it was placed to `pos + to` over `secs`, eased, when told to.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
pub struct Mover {
    /// The end position, relative to the start (metres).
    #[reflect(@Setting::number("By", "m", 0.05, -100000.0, 100000.0).section(MOVES).help("Where it moves to, from where it was placed"))]
    pub to: [f32; 3],
    #[reflect(@Setting::feel("Speed", &["Slow", "Steady", "Brisk", "Quick"], 20.0, 0.2, "s").log().section(MOVES).help("How long it takes to get there"))]
    pub secs: f32,
}

impl Default for Mover {
    fn default() -> Self { Mover { to: [0.0, 3.0, 0.0], secs: 2.0 } }
}

/// A light part's light: Point light and Spot light (the cone is a spot's only).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Light {
    /// sRGB, 0..1.
    #[reflect(@Setting::colour("Colour").section(LIGHT))]
    pub colour: [f32; 3],
    #[reflect(@Setting::toggle("Shadows").section(LIGHT).help("Off is cheaper: light passes through parts"))]
    pub shadows: bool,
    /// Lumens.
    #[reflect(@Setting::feel("Brightness", &["Dim", "Soft", "Bright", "Blinding"], 1000.0, 10000000.0, "lm").log().section(LIGHT))]
    pub brightness: f32,
    /// How far the light reaches, in metres (the sphere's or cone's handle).
    #[reflect(@Setting::feel("Reach", &["Near", "Room", "Hall", "Far"], 1.0, 100.0, "m").log().section(LIGHT).help("Nothing past this is lit"))]
    pub reach: f32,
    /// The spot's cone, edge to edge, in degrees.
    #[reflect(@Setting::feel("Cone", &["Pin", "Narrow", "Wide", "Flood"], 5.0, 150.0, "°").section(CONE).help("Spot lights only: how wide the cone is"))]
    pub cone: f32,
}

impl Default for Light {
    fn default() -> Self { Light { colour: [1.0, 0.93, 0.82], shadows: true, brightness: 800000.0, reach: 10.0, cone: 50.0 } }
}

/// The part panel's sections.
pub const PART: &str = "PART";
pub const PLACE: &str = "POSITION  ·  ROTATION";
pub const LOOK: &str = "SIZE  ·  LOOK";
pub const MOVES: &str = "MOVES";
pub const LIGHT: &str = "LIGHT";
pub const CONE: &str = "CONE";
pub const TAGS: &str = "TAGS";
pub const CAMERA: &str = "CAMERA";
pub const ORBIT: &str = "ORBIT";

/// A camera part's camera: Fixed, Orbit and Follow cameras (the target and lap are an Orbit camera's only).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Camera {
    /// How much it sees, top to bottom, in degrees.
    #[reflect(@Setting::feel("View", &["Zoomed", "Narrow", "Normal", "Wide", "Fisheye"], 10.0, 120.0, "°").section(CAMERA).help("How much it sees, top to bottom"))]
    pub fov: f32,
    /// The point an Orbit camera circles and looks at.
    #[reflect(@Setting::number("Target", "m", 0.05, -100000.0, 100000.0).section(ORBIT).help("The point it circles and looks at (drag the handle)"))]
    pub target: [f32; 3],
    /// Seconds for one circle.
    #[reflect(@Setting::feel("Lap", &["Quick", "Steady", "Slow", "Crawl"], 3.0, 120.0, "s").log().section(ORBIT).help("How long one circle takes"))]
    pub lap: f32,
}

impl Default for Camera {
    fn default() -> Self { Camera { fov: 45.0, target: [0.0; 3], lap: 20.0 } }
}

/// Where a camera part is and which way it looks (Bevy's: along -Z), `t` seconds into playing, with the player's
/// feet at `feet` (a Follow camera keeps its offset from `start`, the player start's feet).
pub fn camera_view(p: &Part, t: f32, feet: Vec3, start: Vec3) -> (Vec3, Quat) {
    let pos = Vec3::from(p.pos);
    let rot = Quat::from_array(p.rot);
    let c = p.camera.clone().unwrap_or_default();
    match p.shape {
        Shape::OrbitCamera => {
            let target = Vec3::from(c.target);
            let at = target + Quat::from_rotation_y(std::f32::consts::TAU * t / c.lap.max(0.1)) * (pos - target);
            (at, looking(at, target).unwrap_or(rot))
        }
        Shape::FollowCamera => (pos + feet - start, rot),
        _ => (pos, rot),
    }
}

/// The turn (yaw, then pitch: no roll) that looks from `from` at `to`; None when they are the same point.
pub fn looking(from: Vec3, to: Vec3) -> Option<Quat> {
    let d = (to - from).try_normalize()?;
    Some(Quat::from_euler(EulerRot::YXZ, (-d.x).atan2(-d.z), d.y.clamp(-1.0, 1.0).asin(), 0.0))
}
/// The level panel's sections (shown when nothing is selected).
pub const LIGHTING: &str = "LIGHTING";
pub const SUN: &str = "SUN";

/// The level's light: the sun, or Interior (no sun), and the fill light everywhere.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Lighting {
    #[reflect(@Setting::toggle("Interior").section(LIGHTING).help("No sun: only the fill light and your own lights"))]
    pub interior: bool,
    /// The flat light from every side (sky light outdoors).
    #[reflect(@Setting::feel("Fill", &["Dark", "Dim", "Soft", "Bright"], 10.0, 5000.0, "").log().section(LIGHTING).help("Flat light from every side, so shade is never black"))]
    pub fill: f32,
    /// sRGB, 0..1.
    #[reflect(@Setting::colour("Fill colour").section(LIGHTING))]
    pub fill_colour: [f32; 3],
    /// The compass direction the sunlight comes from, in degrees.
    #[reflect(@Setting::number("Direction", "°", 1.0, 0.0, 360.0).section(SUN).help("Which way the sun is, round the level"))]
    pub direction: f32,
    /// How high the sun is above the horizon, in degrees.
    #[reflect(@Setting::feel("Height", &["Horizon", "Low", "High", "Overhead"], 2.0, 90.0, "°").section(SUN).help("Low: long shadows"))]
    pub height: f32,
    /// sRGB, 0..1.
    #[reflect(@Setting::colour("Colour").section(SUN))]
    pub colour: [f32; 3],
    /// Lux.
    #[reflect(@Setting::feel("Brightness", &["Dusk", "Overcast", "Hazy", "Full sun"], 500.0, 100000.0, "lx").log().section(SUN))]
    pub brightness: f32,
    #[reflect(@Setting::toggle("Shadows").section(SUN))]
    pub shadows: bool,
}

impl Default for Lighting {
    fn default() -> Self {
        Lighting { interior: false, fill: 500.0, fill_colour: [0.85, 0.88, 1.0], direction: 40.0, height: 54.0, colour: [1.0, 1.0, 1.0], brightness: 9000.0, shadows: true }
    }
}

pub const NAVIGATION: &str = "NAVIGATION";

/// Who the level's navigation area is for: the size of the characters that walk it (enemies) and what they climb.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Navigation {
    #[reflect(@Setting::number("Width", "m", 0.05, 0.1, 10.0).section(NAVIGATION).help("How wide the characters are: they keep this far from walls"))]
    pub width: f32,
    #[reflect(@Setting::number("Height", "m", 0.05, 0.2, 20.0).section(NAVIGATION).help("How tall they are: they don't go where it is lower"))]
    pub height: f32,
    #[reflect(@Setting::number("Step", "m", 0.05, 0.0, 5.0).section(NAVIGATION).help("The highest step they walk up"))]
    pub step: f32,
    #[reflect(@Setting::number("Slope", "°", 1.0, 0.0, 89.0).section(NAVIGATION).help("The steepest slope they walk up"))]
    pub slope: f32,
}

impl Default for Navigation {
    fn default() -> Self { Navigation { width: 0.8, height: 1.8, step: 0.45, slope: 45.0 } }
}

impl Navigation {
    fn is_default(&self) -> bool { *self == Navigation::default() }
}

impl Lighting {
    fn is_default(&self) -> bool { *self == Lighting::default() }
    /// Which way the sunlight shines (down from the sun).
    pub fn sun_rotation(&self) -> Quat { Quat::from_euler(EulerRot::YXZ, self.direction.to_radians(), -self.height.to_radians(), 0.0) }
}

impl Mover {
    pub const INPUTS: [&str; 3] = ["Go", "Back", "Toggle"];
    pub const OUTPUTS: [&str; 2] = ["Arrived", "Returned"];
}

/// The sound panel's section.
pub const SOUND: &str = "SOUND";
/// A Sound part's section.
pub const SPEAKER: &str = "PLAYS";

/// How a sound file picks the clip that plays.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default, Reflect)]
pub enum Pick {
    /// Any clip but the one before.
    #[default]
    #[reflect(@Label("At random"))]
    Random,
    #[reflect(@Label("In turn"))]
    InTurn,
}

/// What a surface is made of, for sounds that change with it (footsteps on grass, metal, water).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Reflect)]
pub enum Ground { Stone, Wood, Metal, Grass, Dirt, Sand, Gravel, Water, Snow, Carpet }

impl Ground {
    pub const ALL: [Ground; 10] = [Ground::Stone, Ground::Wood, Ground::Metal, Ground::Grass, Ground::Dirt, Ground::Sand, Ground::Gravel, Ground::Water, Ground::Snow, Ground::Carpet];
    pub fn label(self) -> &'static str {
        match self {
            Ground::Stone => "Stone", Ground::Wood => "Wood", Ground::Metal => "Metal", Ground::Grass => "Grass", Ground::Dirt => "Dirt",
            Ground::Sand => "Sand", Ground::Gravel => "Gravel", Ground::Water => "Water", Ground::Snow => "Snow", Ground::Carpet => "Carpet",
        }
    }
}

/// The recordings a sound plays on one kind of surface instead of its own.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
pub struct SoundSet {
    pub ground: Ground,
    pub clips: Vec<String>,
}

/// A sound file (assets/sounds/NAME.bev): which recordings it plays and how, wherever it is used.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Sound {
    pub format: u32,
    /// The recordings (the user's own), e.g. "assets/sounds/step 1.ogg"; one plays each time.
    pub clips: Vec<String>,
    /// Recordings for particular surfaces (on others, `clips`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sets: Vec<SoundSet>,
    #[reflect(@Setting::choice("Pick").section(SOUND).help("Which recording plays each time (at random never plays the same one twice running)"))]
    pub pick: Pick,
    /// Decibels.
    #[reflect(@Setting::feel("Volume", &["Whisper", "Quiet", "Normal", "Loud"], -30.0, 9.0, "dB").section(SOUND))]
    pub volume: f32,
    /// Semitones.
    #[reflect(@Setting::feel("Pitch", &["Low", "Normal", "High"], -12.0, 12.0, "semitones").section(SOUND))]
    pub pitch: f32,
    /// 0..1: how much volume (up to ±6 dB) and pitch (up to ±2 semitones) change each time it plays.
    #[reflect(@Setting::feel("Vary", &["Same", "A little", "A lot"], 0.0, 1.0, "").section(SOUND).help("How much volume and pitch change each time it plays, so repeats don't sound the same"))]
    pub vary: f32,
    /// Metres: full volume this close.
    #[reflect(@Setting::feel("Near", &["Close", "Arm's length", "Room"], 0.5, 20.0, "m").log().section(SOUND).help("Full volume this close"))]
    pub near: f32,
    /// Metres: silent past this.
    #[reflect(@Setting::feel("Range", &["Room", "Yard", "Field", "Valley"], 5.0, 500.0, "m").log().section(SOUND).help("Heard out to here, fading with distance"))]
    pub far: f32,
    #[reflect(@Setting::toggle("Loop").section(SOUND).help("Plays over and over until stopped"))]
    pub looped: bool,
    /// How many may play at once (the oldest stops); a count.
    #[reflect(@Setting::feel("At once", &["One", "A few", "Many"], 1.0, 16.0, "").log().section(SOUND).help("How many can play at the same time; the oldest stops for a new one"))]
    pub at_once: f32,
}

impl Default for Sound {
    fn default() -> Self { Sound { format: 0, clips: vec![], sets: vec![], pick: Pick::Random, volume: 0.0, pitch: 0.0, vary: 0.0, near: 2.0, far: 40.0, looped: false, at_once: 4.0 } }
}

impl Sound {
    /// The recordings it plays on `ground` (its own set there, else its usual ones).
    pub fn clips_on(&self, ground: Option<Ground>) -> &[String] {
        ground.and_then(|g| self.sets.iter().find(|s| s.ground == g && !s.clips.is_empty())).map_or(&self.clips[..], |s| &s.clips[..])
    }
    /// Every recording it can play.
    pub fn all_clips(&self) -> impl Iterator<Item = &String> { self.clips.iter().chain(self.sets.iter().flat_map(|s| &s.clips)) }
}

/// A sound a part plays when something happens to it (one of [`Part::events`]).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
pub struct Hook {
    pub on: String,
    pub sound: String,
}

/// The walker's sounds (until characters come with their own): sound files, empty for none.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default, Reflect)]
#[serde(default)]
pub struct WalkerSounds {
    pub steps: String,
    pub jump: String,
    pub land: String,
}

impl WalkerSounds {
    /// What the walker does that can make a sound.
    pub const EVENTS: [&str; 3] = ["Step", "Jump", "Land"];
    pub fn is_default(&self) -> bool { *self == Self::default() }
    pub fn get(&self, e: &str) -> &str {
        match e { "Step" => &self.steps, "Jump" => &self.jump, "Land" => &self.land, _ => "" }
    }
    pub fn get_mut(&mut self, e: &str) -> Option<&mut String> {
        match e { "Step" => Some(&mut self.steps), "Jump" => Some(&mut self.jump), "Land" => Some(&mut self.land), _ => None }
    }
}

/// When a Sound part plays.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default, Reflect)]
pub enum Plays {
    /// When play starts (a looping sound also plays while editing).
    #[default]
    #[reflect(@Label("On start"))]
    OnStart,
    /// Only when a wire tells it to (Play, Stop).
    #[reflect(@Label("When wired"))]
    WhenWired,
}

/// A Sound part's sound: the file it plays and when.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Speaker {
    /// The sound file, e.g. "assets/sounds/birds.bev" (empty: none yet).
    pub sound: String,
    #[reflect(@Setting::choice("Plays").section(SPEAKER).help("On start: as play begins (a loop also plays while editing) · When wired: Play and Stop come along wires"))]
    pub plays: Plays,
}

impl Speaker {
    pub const INPUTS: [&str; 2] = ["Play", "Stop"];
}

pub fn load_sound(dir: &Path, path: &str) -> Result<Sound, String> {
    let text = std::fs::read_to_string(dir.join(path)).map_err(|e| format!("{path}: {e}"))?;
    let m: Sound = ron::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    if m.format > FORMAT { return Err(format!("{path} was saved by a newer Beverly")); }
    Ok(m)
}

pub fn save_sound(dir: &Path, path: &str, m: &Sound) -> Result<(), String> {
    let text = ron::ser::to_string_pretty(&Sound { format: FORMAT, ..m.clone() }, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir.join("assets/sounds")).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(path), text).map_err(|e| e.to_string())
}

/// Every sound file in the project, by path.
pub fn load_sounds(dir: &Path) -> std::collections::BTreeMap<String, Sound> {
    list_assets(dir, "sounds").into_iter().filter(|f| f.ends_with(".bev")).filter_map(|f| {
        let path = format!("assets/sounds/{f}");
        load_sound(dir, &path).ok().map(|m| (path, m))
    }).collect()
}

/// Whether a file is a recording Beverly plays.
pub fn is_recording(f: &str) -> bool {
    let ext = Path::new(f).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "wav" | "ogg" | "mp3" | "flac")
}

impl Level {
    /// The sound for a recording: the one already made for just it, or a new "assets/sounds/<recording name>.bev".
    pub fn sound_for_clip(&mut self, clip: &str) -> String {
        let stem = Path::new(clip).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "sound".into());
        for n in 1.. {
            let path = if n == 1 { format!("assets/sounds/{stem}.bev") } else { format!("assets/sounds/{stem} {n}.bev") };
            match self.sounds.get(&path) {
                Some(m) if m.clips == [clip] => return path,
                Some(_) => continue,
                None => {
                    self.sounds.insert(path.clone(), Sound { clips: vec![clip.into()], ..Default::default() });
                    return path;
                }
            }
        }
        unreachable!()
    }
    /// A new sound, "assets/sounds/<base> N.bev".
    pub fn new_sound(&mut self, base: &str, m: Sound) -> String {
        let path = (1..).map(|n| format!("assets/sounds/{base} {n}.bev")).find(|p| !self.sounds.contains_key(p)).unwrap();
        self.sounds.insert(path.clone(), m);
        path
    }
    /// What the top of part `id` sounds like underfoot: the material on its top face, else the part's own.
    pub fn ground_under(&self, id: u64) -> Option<Ground> {
        let p = self.get(id)?;
        let rot = Quat::from_array(p.rot);
        let faces = crate::shapes::build(p.shape).faces;
        let top = faces.iter().enumerate().filter_map(|(i, l)| match l { crate::shapes::Lay::Flat(n) => Some((i, (rot * *n).y)), _ => None })
            .max_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i as u8);
        let from_material = p.surfaces.iter().find(|s| s.face.is_some() && s.face == top).or_else(|| p.surfaces.iter().find(|s| s.face.is_none()))
            .and_then(|s| self.materials.get(&s.material)).and_then(|m| m.sounds_like);
        from_material.or(p.sounds_like)
    }
    /// How many Sound parts play a sound file.
    pub fn sound_users(&self, path: &str) -> usize { self.parts.iter().filter(|p| p.sound.as_ref().is_some_and(|s| s.sound == path)).count() }
}

/// A material laid on a part, all over or on one face, through its file in assets/materials. Where it sits is the
/// surface's own (two parts can share a material and lay it differently).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
pub struct Surface {
    /// The face (see `shapes::Tris::faces`), or `None` for all of the part. A face's own surface wins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<u8>,
    /// The material file, e.g. "assets/materials/brick.bev".
    pub material: String,
    /// How big one copy of the texture is on the surface, in metres (across, down).
    pub tile: [f32; 2],
    /// How far the texture is slid along the surface, in metres (across, down).
    #[serde(default)]
    pub offset: [f32; 2],
}

impl Part {
    /// The surface that covers `face`: its own, else the all-over one.
    pub fn surface(&self, face: u8) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.face == Some(face)).or_else(|| self.surfaces.iter().find(|s| s.face.is_none()))
    }
    /// Lay `s` on the part: on its face (or all over), replacing what was there. All over clears the faces' own.
    pub fn lay(&mut self, s: Surface) {
        if s.face.is_none() { self.surfaces.clear(); }
        self.surfaces.retain(|o| o.face != s.face);
        self.surfaces.push(s);
    }
}

/// The material panel's section.
pub const MATERIAL: &str = "MATERIAL";

/// A material file (assets/materials/NAME.bev): what every surface using it looks like.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Material {
    pub format: u32,
    /// The texture, e.g. "assets/textures/brick.png" (none: plain, in the tint).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub texture: Option<String>,
    /// sRGB, 0..1; multiplies the texture.
    #[reflect(@Setting::colour("Tint").alpha("opacity").section(MATERIAL).help("Colours the texture (white leaves it as it is)"))]
    pub tint: [f32; 3],
    #[reflect(@Setting::feel("Opacity", &["Clear", "Faint", "Frosted", "Solid"], 0.0, 1.0, "").section(MATERIAL).help("See-through below Solid, like glass"))]
    pub opacity: f32,
    #[reflect(@Setting::feel("Metal", &["Not metal", "Worn", "Metal"], 0.0, 1.0, "").section(MATERIAL))]
    pub metal: f32,
    #[reflect(@Setting::feel("Roughness", &["Glossy", "Satin", "Matte"], 0.0, 1.0, "").section(MATERIAL))]
    pub roughness: f32,
    #[reflect(@Setting::feel("Glow", &["None", "Faint", "Glowing", "Bright"], 0.0, 10.0, "").section(MATERIAL).help("Gives off its own light, for screens, signs and light strips"))]
    pub glow: f32,
    /// What it sounds like underfoot (None: what the part says).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sounds_like: Option<Ground>,
}

impl Default for Material {
    fn default() -> Self { Material { format: 0, texture: None, tint: [1.0; 3], opacity: 1.0, metal: 0.0, roughness: 0.75, glow: 0.0, sounds_like: None } }
}

pub fn load_material(dir: &Path, path: &str) -> Result<Material, String> {
    let text = std::fs::read_to_string(dir.join(path)).map_err(|e| format!("{path}: {e}"))?;
    let m: Material = ron::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    if m.format > FORMAT { return Err(format!("{path} was saved by a newer Beverly")); }
    Ok(m)
}

pub fn save_material(dir: &Path, path: &str, m: &Material) -> Result<(), String> {
    let text = ron::ser::to_string_pretty(&Material { format: FORMAT, ..m.clone() }, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir.join("assets/materials")).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(path), text).map_err(|e| e.to_string())
}

/// Every material file in the project, by path.
pub fn load_materials(dir: &Path) -> std::collections::BTreeMap<String, Material> {
    list_assets(dir, "materials").into_iter().filter(|f| f.ends_with(".bev")).filter_map(|f| {
        let path = format!("assets/materials/{f}");
        load_material(dir, &path).ok().map(|m| (path, m))
    }).collect()
}

impl Level {
    /// The material for a texture: the one already made for it, or a new "assets/materials/<texture name>.bev".
    pub fn material_for_texture(&mut self, texture: &str) -> String {
        let stem = Path::new(texture).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "material".into());
        for n in 1.. {
            let path = if n == 1 { format!("assets/materials/{stem}.bev") } else { format!("assets/materials/{stem} {n}.bev") };
            match self.materials.get(&path) {
                Some(m) if m.texture.as_deref() == Some(texture) => return path,
                Some(_) => continue,
                None => {
                    self.materials.insert(path.clone(), Material { texture: Some(texture.into()), ..Default::default() });
                    return path;
                }
            }
        }
        unreachable!()
    }
    /// A new plain material, "assets/materials/<base> N.bev".
    pub fn new_material(&mut self, base: &str, m: Material) -> String {
        let path = (1..).map(|n| format!("assets/materials/{base} {n}.bev")).find(|p| !self.materials.contains_key(p)).unwrap();
        self.materials.insert(path.clone(), m);
        path
    }
    /// How many surfaces use a material, on how many parts.
    pub fn material_users(&self, path: &str) -> (usize, usize) {
        let per = self.parts.iter().map(|p| p.surfaces.iter().filter(|s| s.material == path).count());
        per.clone().fold((0, 0), |(s, p), n| (s + n, p + (n > 0) as usize))
    }
}

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
    /// What its surface sounds like underfoot (None: nothing special; a material can say instead).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sounds_like: Option<Ground>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[reflect(@Setting::optional("Moves", "Stays put", "Moves when told to").section(MOVES).help("Moves to an end position and back when wired to"))]
    pub mover: Option<Mover>,
}

impl Part {
    /// A part as it is first placed: the default colour, and Walk-through for a player start.
    pub fn placed(shape: Shape, model: Option<String>, name: String, pos: [f32; 3], rot: [f32; 4], size: [f32; 3]) -> Part {
        Part {
            id: 0, name, shape, pos, rot, size,
            color: if shape == Shape::Interact { [0.94, 0.55, 0.26] } else { [0.62, 0.64, 0.70] },
            visible: true,
            collision: if shape == Shape::PlayerStart || shape.is_marker() { Collision::WalkThrough } else { Collision::Solid },
            model, mover: None, surfaces: vec![], unique: false, tags: vec![], loose: None,
            light: shape.is_light().then(Light::default),
            camera: shape.is_camera().then(Camera::default),
            sound: (shape == Shape::Sound).then(Speaker::default),
            sounds: vec![], sounds_like: None,
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
    /// textures and how they lie, Visible, Collision, tags, Physics). The box itself (name, position, turn, size) is each part's
    /// own, as an instance's placement is in SketchUp.
    pub fn same_contents(&self, o: &Part) -> bool {
        self.shape == o.shape && self.model == o.model && self.color == o.color && self.surfaces == o.surfaces && self.visible == o.visible && self.collision == o.collision
            && self.tags == o.tags && self.loose == o.loose && self.sounds_like == o.sounds_like
    }
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
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct Level {
    /// The file format that wrote this level (see [`FORMAT`]); 0 for files from before it existed.
    #[serde(default)]
    pub format: u32,
    pub name: String,
    pub parts: Vec<Part>,
    pub next_id: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wires: Vec<Wire>,
    /// Slider range ends changed under More in the panels (see `settings`).
    #[serde(default, skip_serializing_if = "Ranges::is_empty")]
    pub ranges: Ranges,
    #[serde(default, skip_serializing_if = "Lighting::is_default")]
    pub lighting: Lighting,
    #[serde(default, skip_serializing_if = "Navigation::is_default")]
    pub navigation: Navigation,
    #[serde(default, skip_serializing_if = "WalkerSounds::is_default")]
    pub walker: WalkerSounds,
    /// The project's materials, by path ("assets/materials/brick.bev"). Saved as their own files, not in the level;
    /// kept here so undo and the unsaved-changes dot cover them.
    #[serde(skip)]
    pub materials: std::collections::BTreeMap<String, Material>,
    /// The project's sound files, by path ("assets/sounds/birds.bev"), kept and saved as the materials are.
    #[serde(skip)]
    pub sounds: std::collections::BTreeMap<String, Sound>,
}

impl Level {
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
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjectFile {
    #[serde(default)]
    pub format: u32,
    pub name: String,
    pub levels: Vec<String>,
}

pub const ASSET_FOLDERS: [&str; 5] = ["textures", "materials", "models", "sounds", "other"];

/// Which assets folder a dropped file goes into, by its extension.
pub fn folder_for(path: &Path) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "tga" | "bmp" | "ktx2" | "dds" | "webp" | "hdr" | "exr" => "textures",
        "glb" | "gltf" | "obj" | "fbx" | "dae" => "models",
        "wav" | "ogg" | "mp3" | "flac" => "sounds",
        "bev" => "materials",
        _ => "other",
    }
}

/// The default place for new projects: Documents/Beverly (or the home folder's Beverly).
pub fn projects_root() -> PathBuf {
    let home = std::env::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let docs = home.join("Documents");
    if docs.is_dir() { docs.join("Beverly") } else { home.join("Beverly") }
}

/// Create a fresh project folder "Untitled N" under [`projects_root`].
pub fn new_project_dir() -> std::io::Result<PathBuf> {
    let root = projects_root();
    std::fs::create_dir_all(&root)?;
    let dir = (1..).map(|n| root.join(format!("Untitled {n}"))).find(|d| !d.exists()).unwrap();
    make_folders(&dir)?;
    Ok(dir)
}

pub fn make_folders(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir.join("levels"))?;
    for f in ASSET_FOLDERS {
        std::fs::create_dir_all(dir.join("assets").join(f))?;
    }
    Ok(())
}

/// The file format Beverly writes. Raise it when a saved field is renamed or changes meaning, and teach [`load`] to
/// upgrade files from before (a new field with a default needs neither).
pub const FORMAT: u32 = 1;

/// The project file in `dir`: project.bev, or project.ron in projects from before .bev files.
pub fn project_file(dir: &Path) -> Option<PathBuf> {
    ["bev", "ron"].map(|e| dir.join(format!("project.{e}"))).into_iter().find(|p| p.is_file())
}

/// The project a path given to Beverly belongs to: a project folder, or any file or folder inside one (project.bev,
/// levels/main.bev), so a .bev file opened from Explorer opens its project.
pub fn project_dir_of(path: &Path) -> PathBuf {
    path.ancestors().find(|d| project_file(d).is_some()).unwrap_or(path).to_path_buf()
}

/// A project saved by a Beverly from before .bev files; saving it upgrades it.
pub fn is_old(dir: &Path) -> bool { project_file(dir).is_some_and(|p| p.extension().is_some_and(|e| e == "ron")) }

pub fn save(dir: &Path, name: &str, level: &Level) -> Result<(), String> {
    make_folders(dir).map_err(|e| e.to_string())?;
    let cfg = ron::ser::PrettyConfig::default();
    let pf = ProjectFile { format: FORMAT, name: name.to_owned(), levels: vec!["main".into()] };
    let p = ron::ser::to_string_pretty(&pf, cfg.clone()).map_err(|e| e.to_string())?;
    let l = ron::ser::to_string_pretty(&Level { format: FORMAT, ..level.clone() }, cfg).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("project.bev"), p).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("levels").join("main.bev"), l).map_err(|e| e.to_string())?;
    for (path, m) in &level.materials {
        if load_material(dir, path).ok().as_ref() != Some(&Material { format: FORMAT, ..m.clone() }) { save_material(dir, path, m)?; }
    }
    for (path, m) in &level.sounds {
        if load_sound(dir, path).ok().as_ref() != Some(&Sound { format: FORMAT, ..m.clone() }) { save_sound(dir, path, m)?; }
    }
    // the .ron files these replace, in a project from before .bev files
    for old in [dir.join("project.ron"), dir.join("levels").join("main.ron")] {
        if old.is_file() { std::fs::remove_file(old).map_err(|e| e.to_string())?; }
    }
    Ok(())
}

pub fn load(dir: &Path) -> Result<(String, Level), String> {
    let file = project_file(dir).ok_or("This folder has no project.bev")?;
    let ext = file.extension().unwrap_or_default().to_string_lossy().into_owned();
    let p = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
    let pf: ProjectFile = ron::from_str(&p).map_err(|e| format!("project.{ext}: {e}"))?;
    let first = pf.levels.first().cloned().unwrap_or_else(|| "main".into());
    let l = std::fs::read_to_string(dir.join("levels").join(format!("{first}.{ext}"))).map_err(|e| e.to_string())?;
    let mut level: Level = ron::from_str(&l).map_err(|e| format!("levels/{first}.{ext}: {e}"))?;
    if let Some(n) = [pf.format, level.format].into_iter().find(|&n| n > FORMAT) {
        return Err(format!("It was saved by a newer Beverly (file format {n}; this one reads up to {FORMAT}): update Beverly to open it"));
    }
    // format 0 (.ron) to 1 (.bev) changed nothing inside the files
    level.format = FORMAT;
    level.materials = load_materials(dir).into_iter().map(|(k, m)| (k, Material { format: 0, ..m })).collect();
    level.sounds = load_sounds(dir).into_iter().map(|(k, m)| (k, Sound { format: 0, ..m })).collect();
    Ok((pf.name, level))
}

/// Copy a dropped file into the project's assets; returns the folder it went into.
pub fn import(dir: &Path, src: &Path) -> std::io::Result<&'static str> {
    let folder = folder_for(src);
    let dest_dir = dir.join("assets").join(folder);
    std::fs::create_dir_all(&dest_dir)?;
    let name = src.file_name().unwrap_or_default();
    std::fs::copy(src, dest_dir.join(name))?;
    Ok(folder)
}

pub fn list_assets(dir: &Path, folder: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir.join("assets").join(folder))
        .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Write a playable Windows game: `<name>/<name>.exe` (a copy of the game player) and the project's files beside
/// it, zipped. Returns how many files went in.
pub fn export_zip(dir: &Path, name: &str, player: &Path, out: &Path) -> std::io::Result<usize> {
    use std::io::Write;
    let mut z = zip::ZipWriter::new(std::fs::File::create(out)?);
    let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let ext = if cfg!(windows) { ".exe" } else { "" };
    z.start_file(format!("{name}/{name}{ext}"), opt.unix_permissions(0o755)).map_err(std::io::Error::other)?;
    z.write_all(&std::fs::read(player)?)?;
    let mut n = 1;
    let mut stack = vec![dir.join("project.bev"), dir.join("project.ron"), dir.join("levels"), dir.join("assets")];
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            for e in std::fs::read_dir(&p)?.flatten() { stack.push(e.path()); }
        } else if p.is_file() {
            let rel = p.strip_prefix(dir).unwrap_or(&p).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
            z.start_file(format!("{name}/{rel}"), opt).map_err(std::io::Error::other)?;
            z.write_all(&std::fs::read(&p)?)?;
            n += 1;
        }
    }
    z.finish().map_err(std::io::Error::other)?;
    Ok(n)
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
        let n = export_zip(&dir, "My Game", &player, &out).unwrap();
        assert_eq!(n, 4); // player, project.bev, levels/main.bev, the texture
        let mut z = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
        let ext = if cfg!(windows) { ".exe" } else { "" };
        assert!(z.by_name(&format!("My Game/My Game{ext}")).is_ok());
        assert!(z.by_name("My Game/levels/main.bev").is_ok());
        assert!(z.by_name("My Game/assets/textures/a.png").is_ok());
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
