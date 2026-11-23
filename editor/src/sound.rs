//! Sounds while editing and at play time, through kira: a listener on the camera, each playing sound in its own
//! spatial track (so it is heard from where its part is), and which sound files play when. The editor and the game
//! player each call [`Sounds::step`] once a frame with their level.
//!
//! While editing, Sound parts that loop and play On start are heard where they are, so the level sounds like itself;
//! a sound file can be heard once with [`Sounds::preview`]. While playing, On start parts start with the run and wires
//! send Play and Stop.
use crate::level::{Ground, Level, Pick, Plays, Sound, Speaker};
use crate::script::Happened;
use bevy::prelude::*;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::sound::PlaybackState;
use kira::track::{SpatialTrackBuilder, SpatialTrackDistances, SpatialTrackHandle};
use kira::listener::ListenerHandle;
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Semitones, Tween};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

/// How long a stopped sound takes to fade out.
const FADE: Duration = Duration::from_millis(60);

struct Engine {
    manager: AudioManager<DefaultBackend>,
    listener: ListenerHandle,
}

/// One sound playing.
struct Voice {
    /// The Sound part it plays from (None: a preview, heard where the listener is).
    part: Option<u64>,
    /// Its sound file and the settings it started with.
    path: String,
    sound: Sound,
    track: SpatialTrackHandle,
    handle: StaticSoundHandle,
    /// Told to stop (it fades out first).
    stopped: bool,
    /// Started by `cue` or Play: it plays to its end, whatever the speakers are doing.
    cued: bool,
    /// Where it plays from when that isn't a part's place (something that happened somewhere).
    fixed: Option<Vec3>,
}

impl Voice {
    fn stop(&mut self) {
        self.handle.stop(Tween { duration: FADE, ..default() });
        self.stopped = true;
    }
}

#[derive(Resource, Default)]
pub struct Sounds {
    /// None: no sound device (everything else still works, silently).
    engine: Option<Engine>,
    tried: bool,
    /// Decoded recordings by their full path (projects can have files of the same name); None: it couldn't be read.
    clips: HashMap<String, Option<StaticSoundData>>,
    voices: Vec<Voice>,
    /// Each sound file's clip played last (for At random and In turn).
    last: HashMap<String, usize>,
    /// The run the voices belong to (`Play::run` while playing; None while editing).
    run: Option<u32>,
    /// A sound file to hear once, where the listener is (the sound panel's Play), on a surface.
    pub preview: Option<(String, Option<Ground>)>,
    /// Sounds to start now, wherever the run is: (where it plays from, sound file, the surface it is on).
    cues: Vec<(u64, String, Option<Ground>)>,
    /// Each recording's shape for drawing (see [`Sounds::wave`]), by its path in the project.
    waves: HashMap<String, Wave>,
    /// The project the waves are from.
    waves_dir: std::path::PathBuf,
    /// The recordings started lately, oldest first (for scripted tests: what was heard).
    pub log: Vec<String>,
    seed: u64,
}

/// A recording's outline: the loudest it gets, low and high, in each of [`WAVE_BARS`] slices, and its length.
#[derive(Clone)]
pub struct Wave {
    pub bars: Vec<[f32; 2]>,
    pub secs: f32,
}

/// How many slices a recording's outline has.
pub const WAVE_BARS: usize = 160;

impl Sounds {
    /// Whether a sound device was found.
    pub fn on(&self) -> bool { self.engine.is_some() }

    /// Which Sound parts are playing now (a part can play several at once).
    pub fn playing(&self) -> Vec<u64> {
        let mut v: Vec<u64> = self.voices.iter().filter(|v| !v.stopped && v.handle.state() != PlaybackState::Stopped).filter_map(|v| v.part).collect();
        v.dedup();
        v
    }

    /// Start sound file `path` from `id` (a part, or a stand-in such as the Audio tab's speaker) on `ground`, now.
    pub fn cue(&mut self, id: u64, path: String, ground: Option<Ground>) { self.cues.push((id, path, ground)); }

    /// Stop every sound playing from `id`.
    pub fn hush(&mut self, id: u64) {
        for v in self.voices.iter_mut().filter(|v| v.part == Some(id)) { v.stop(); }
    }

    /// A recording's outline (None until [`Sounds::read_waves`] has read it, or if it can't be read).
    pub fn wave(&self, clip: &str) -> Option<&Wave> { self.waves.get(clip) }

    /// Read the outlines of these recordings (each once).
    pub fn read_waves<'a>(&mut self, dir: &Path, clips: impl Iterator<Item = &'a String>) {
        if self.waves_dir != dir { self.waves.clear(); self.waves_dir = dir.to_path_buf(); }
        for c in clips {
            if self.waves.contains_key(c) { continue; }
            let Some(data) = self.data(dir, c) else { continue };
            self.waves.insert(c.clone(), outline(&data));
        }
    }

    /// A recording, decoded (once).
    fn data(&mut self, dir: &Path, clip: &str) -> Option<StaticSoundData> {
        let file = dir.join(clip);
        self.clips.entry(file.to_string_lossy().into_owned()).or_insert_with(|| match StaticSoundData::from_file(&file) {
            Ok(d) => Some(d),
            Err(e) => { warn!("Couldn't read {clip}: {e}"); None }
        }).clone()
    }

    /// Bring the sounds up to date. `speakers`: the Sound parts (or the Audio tab's stand-ins) and what they play;
    /// `run`: `Some(Play::run)` while playing, None while editing; `inputs`: what the wires sent this frame (part,
    /// input); `ear`: the camera; `at`: where a part is now.
    #[allow(clippy::too_many_arguments)]
    pub fn step(&mut self, level: &Level, speakers: &[(u64, Speaker)], dir: &Path, run: Option<u32>, inputs: Vec<(u64, String)>, happened: Vec<Happened>, ear: &GlobalTransform, at: impl Fn(u64) -> Option<Vec3>) {
        if !self.tried {
            self.tried = true;
            self.seed = 0x2545_F491_4F6C_DD1D;
            match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
                Ok(mut manager) => {
                    let (p, r) = (ear.translation(), ear.rotation());
                    match manager.add_listener(mint(p), quat(r)) {
                        Ok(listener) => self.engine = Some(Engine { manager, listener }),
                        Err(e) => warn!("No sound: {e}"),
                    }
                }
                Err(e) => warn!("No sound: {e}"),
            }
        }
        let Some(engine) = self.engine.as_mut() else { self.preview = None; self.cues.clear(); return };
        engine.listener.set_position(mint(ear.translation()), Tween::default());
        engine.listener.set_orientation(quat(ear.rotation()), Tween::default());

        self.voices.retain(|v| v.handle.state() != PlaybackState::Stopped);
        let speakers = || speakers.iter().map(|(id, s)| (*id, s));
        if run != self.run {
            for v in &mut self.voices { v.stop(); }
            self.voices.clear();
            self.run = run;
            if run.is_some() {
                let starts: Vec<(u64, String)> = speakers().filter(|(_, s)| s.plays == Plays::OnStart).map(|(id, s)| (id, s.sound.clone())).collect();
                for (id, path) in starts { self.start(level, dir, Some(id), &path, at(id), None, false); }
            }
        }
        match run {
            // editing: looping On start sounds play where they are, started again when their settings change
            None => {
                let wanted: Vec<(u64, String)> = speakers()
                    .filter(|(_, s)| s.plays == Plays::OnStart && level.sounds.get(&s.sound).is_some_and(|f| f.looped))
                    .map(|(id, s)| (id, s.sound.clone()))
                    .collect();
                for v in &mut self.voices {
                    let Some(id) = v.part.filter(|_| !v.cued) else { continue };
                    if !wanted.iter().any(|(w, p)| *w == id && *p == v.path && level.sounds.get(p) == Some(&v.sound)) { v.stop(); }
                }
                self.voices.retain(|v| !v.stopped);
                for (id, path) in wanted {
                    if !self.voices.iter().any(|v| v.part == Some(id) && !v.cued) { self.start(level, dir, Some(id), &path, at(id), None, false); }
                }
            }
            // playing: what the wires say, and the sounds of what happens
            Some(_) => {
                for h in happened {
                    let (path, ground) = match h.part {
                        None => (level.walker.get(&h.what).to_string(), h.on.and_then(|p| level.ground_at(p, h.at))),
                        Some(id) => (level.get(id).and_then(|p| p.sound_on(&h.what)).unwrap_or_default().to_string(), None),
                    };
                    if path.is_empty() { continue; }
                    let pos = h.at.or_else(|| h.part.and_then(&at));
                    let n = self.voices.len();
                    self.start(level, dir, h.part, &path, pos, ground, true);
                    // it stays where it happened (a moving part's own sounds follow it)
                    if let (Some(v), Some(p)) = (self.voices.get_mut(n), h.at) { v.fixed = Some(p); }
                }
                for (id, input) in inputs {
                    let Some(path) = speakers().find(|(p, _)| *p == id).map(|(_, s)| s.sound.clone()) else { continue };
                    match input.as_str() {
                        "Play" => self.start(level, dir, Some(id), &path, at(id), None, true),
                        "Stop" => {
                            for v in self.voices.iter_mut().filter(|v| v.part == Some(id)) { v.stop(); }
                        }
                        _ => {}
                    }
                }
            }
        }
        if let Some((path, ground)) = self.preview.take() { self.start(level, dir, None, &path, Some(ear.translation()), ground, true); }
        for (id, path, ground) in std::mem::take(&mut self.cues) { self.start(level, dir, Some(id), &path, at(id), ground, true); }
        // sounds follow their parts (and a preview the listener)
        for v in &mut self.voices {
            let p = match (v.fixed, v.part) { (Some(f), _) => Some(f), (None, Some(id)) => at(id), (None, None) => Some(ear.translation()) };
            if let Some(p) = p { v.track.set_position(mint(p), Tween::default()); }
        }
    }

    /// Start sound file `path` at `pos`, for part `part`, on `ground` (which picks the recordings).
    #[allow(clippy::too_many_arguments)]
    fn start(&mut self, level: &Level, dir: &Path, part: Option<u64>, path: &str, pos: Option<Vec3>, ground: Option<Ground>, cued: bool) {
        let (Some(sound), Some(pos)) = (level.sounds.get(path), pos) else { return };
        let clips = sound.clips_on(ground);
        if clips.is_empty() { return; }
        let key = format!("{path}#{ground:?}");
        let k = pick(sound.pick, clips.len(), self.last.get(&key).copied(), &mut self.seed);
        self.last.insert(key, k);
        let clip = clips[k].clone();
        self.log.push(clip.clone());
        if self.log.len() > 4096 { self.log.drain(..2048); }
        let Some(data) = self.data(dir, &clip) else { return };
        // no more than At once of this sound: the oldest stops
        let most = sound.at_once.round().max(1.0) as usize;
        let mine: Vec<usize> = self.voices.iter().enumerate().filter(|(_, v)| v.path == path && !v.stopped).map(|(i, _)| i).collect();
        if mine.len() >= most {
            for &i in &mine[..=mine.len() - most] { self.voices[i].stop(); }
        }
        let wobble = |seed: &mut u64| rand(seed) * 2.0 - 1.0;
        let volume = sound.volume + sound.vary * 6.0 * wobble(&mut self.seed);
        let pitch = sound.pitch + sound.vary * 2.0 * wobble(&mut self.seed);
        let mut data = data.volume(Decibels(volume)).playback_rate(Semitones(pitch as f64));
        if sound.looped { data = data.loop_region(..); }
        let Some(engine) = self.engine.as_mut() else { return };
        let near = sound.near.max(0.01);
        let builder = SpatialTrackBuilder::new()
            .distances(SpatialTrackDistances { min_distance: near, max_distance: sound.far.max(near + 0.01) })
            // a preview is heard straight on, wherever you look
            .spatialization_strength(if part.is_some() { 0.75 } else { 0.0 })
            .persist_until_sounds_finish(true);
        let Ok(mut track) = engine.manager.add_spatial_sub_track(&engine.listener, mint(pos), builder) else { return };
        match track.play(data) {
            Ok(handle) => self.voices.push(Voice { part, path: path.to_string(), sound: sound.clone(), track, handle, stopped: false, cued, fixed: None }),
            Err(e) => warn!("Couldn't play {clip}: {e}"),
        }
    }
}

/// A level's Sound parts and what they play.
pub fn speakers(level: &Level) -> Vec<(u64, Speaker)> {
    level.parts.iter().filter(|p| p.shape == crate::level::Shape::Sound).filter_map(|p| Some((p.id, p.sound.clone()?))).collect()
}

/// The loudest a recording gets, low and high, in each slice of it.
fn outline(d: &StaticSoundData) -> Wave {
    let n = d.frames.len().max(1);
    let bars = (0..WAVE_BARS).map(|b| {
        let (lo, hi) = (b * n / WAVE_BARS, ((b + 1) * n / WAVE_BARS).max(b * n / WAVE_BARS + 1).min(n));
        d.frames[lo.min(d.frames.len())..hi.min(d.frames.len())].iter().fold([0.0f32, 0.0], |[a, z], f| {
            let m = (f.left + f.right) * 0.5;
            [a.min(m), z.max(m)]
        })
    }).collect();
    Wave { bars, secs: d.frames.len() as f32 / d.sample_rate.max(1) as f32 }
}

/// Which clip plays next: in turn after `last`, or at random but never `last` again.
fn pick(how: Pick, n: usize, last: Option<usize>, seed: &mut u64) -> usize {
    match (how, last) {
        (_, None) if how == Pick::InTurn => 0,
        (Pick::InTurn, Some(l)) => (l + 1) % n,
        _ if n == 1 => 0,
        (Pick::Random, Some(l)) => (l + 1 + (rand(seed) * (n - 1) as f32) as usize % (n - 1)) % n,
        _ => (rand(seed) * n as f32) as usize % n,
    }
}

/// 0..1 (xorshift).
fn rand(seed: &mut u64) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    (*seed >> 40) as f32 / (1u64 << 24) as f32
}

fn mint(v: Vec3) -> ::mint::Vector3<f32> { ::mint::Vector3 { x: v.x, y: v.y, z: v.z } }
fn quat(q: Quat) -> ::mint::Quaternion<f32> { ::mint::Quaternion { v: mint(Vec3::new(q.x, q.y, q.z)), s: q.w } }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_turn_goes_round_and_random_never_repeats() {
        let mut seed = 99;
        let mut last = None;
        let turns: Vec<usize> = (0..5).map(|_| { let k = pick(Pick::InTurn, 3, last, &mut seed); last = Some(k); k }).collect();
        assert_eq!(turns, [0, 1, 2, 0, 1]);
        let mut last = None;
        let mut seen = [0; 4];
        for _ in 0..400 {
            let k = pick(Pick::Random, 4, last, &mut seed);
            assert!(k < 4 && Some(k) != last);
            seen[k] += 1;
            last = Some(k);
        }
        assert!(seen.iter().all(|&n| n > 60), "{seen:?}");
        assert_eq!(pick(Pick::Random, 1, Some(0), &mut seed), 0);
    }
}
