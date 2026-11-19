//! Sounds: sound files, how they pick clips, the walker's sounds, speakers, and the level's sound lists.
use super::*;

/// The sound panel's section.
pub const SOUND: &str = "SOUND";
/// A sound's reach (how near is full volume, how far it carries) and its rarely used settings.
pub const REACH: &str = "REACH";
pub const SOUND_MORE: &str = "SOUND  ·  MORE";
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
    #[reflect(@Setting::choice("Pick").icon("shuffle").section(SOUND).help("Which recording plays each time (at random never plays the same one twice running)"))]
    pub pick: Pick,
    /// Decibels.
    #[reflect(@Setting::feel("Volume", &["Whisper", "Quiet", "Normal", "Loud"], -30.0, 9.0, "dB").icon("volume").section(SOUND))]
    pub volume: f32,
    /// Semitones.
    #[reflect(@Setting::feel("Pitch", &["Low", "Normal", "High"], -12.0, 12.0, "semitones").icon("pitch").section(SOUND))]
    pub pitch: f32,
    /// 0..1: how much volume (up to ±6 dB) and pitch (up to ±2 semitones) change each time it plays.
    #[reflect(@Setting::feel("Vary", &["Same", "A little", "A lot"], 0.0, 1.0, "").icon("vary").section(SOUND).help("How much volume and pitch change each time it plays, so repeats don't sound the same"))]
    pub vary: f32,
    /// Metres: full volume this close.
    #[reflect(@Setting::feel("Near", &["Close", "Arm's length", "Room"], 0.5, 20.0, "m").log().icon("near").section(REACH).help("Full volume this close"))]
    pub near: f32,
    /// Metres: silent past this.
    #[reflect(@Setting::feel("Range", &["Room", "Yard", "Field", "Valley"], 5.0, 500.0, "m").log().icon("range").section(REACH).help("Heard out to here, fading with distance"))]
    pub far: f32,
    #[reflect(@Setting::toggle("Loop").icon("loop").section(SOUND).help("Plays over and over until stopped"))]
    pub looped: bool,
    /// How many may play at once (the oldest stops); a count.
    #[reflect(@Setting::feel("At once", &["One", "A few", "Many"], 1.0, 16.0, "").log().section(SOUND_MORE).help("How many can play at the same time; the oldest stops for a new one"))]
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
    /// Whether anything in the level plays this sound file: a Sound part, a part's sounds, or the walker's.
    pub fn plays(&self, path: &str) -> bool {
        self.sound_users(path) > 0 || self.parts.iter().any(|p| p.sounds.iter().any(|h| h.sound == path))
            || WalkerSounds::EVENTS.iter().any(|e| self.walker.get(e) == path)
    }
}
