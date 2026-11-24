//! Scripted tests: `beverly-player PROJECT --test tests/walk.ron [--record]` plays the project's level on a fixed
//! clock (60 steps a second, however fast the computer draws), presses keys on a timeline, saves screenshots and
//! checks positions, then quits with exit code 0 if every check passed. Works the same with a game's own player
//! (it reads `Play::feet`). Results go to PROJECT/test-output/<test name>/: report.txt and the screenshots.
//!
//! A test file:
//! ```ron
//! (
//!     trace: "tests/walk.trace.csv",   // optional: the feet every step; compared with it once it exists
//!     steps: [
//!         (at: 0.0, press: ["W"]),
//!         (at: 0.5, look: (200, 0)),     // mouse moved this much (pixels) in that step
//!         (at: 1.0, release: ["W"], shot: "walked", expect: [Feet((0, 0, -4.6), 0.3)]),
//!         (at: 2.0, expect: [Part("Lift", (0, 3, 0), 0.1)]),
//!         (at: 2.0, expect: [Path((0, 0, 8), (0, 2, -2), Some(12.5), 0.5)]),  // a walk this long; None: no way
//!         (at: 2.5, expect: [Playing("Bell", true)]),  // a Sound part is playing (skipped with no sound device)
//!         (at: 3.0, expect: [Played("grass 1.ogg", true)]),  // a recording started since the last Played check
//!     ],
//! )
//! ```
//! Keys: letters, digits, Space, Shift, Ctrl, Alt, Tab, Enter, Escape, Up, Down, Left, Right, MouseLeft,
//! MouseRight. The test ends at its last step. `--record` writes the trace again instead of comparing.
use crate::level::Level;
use crate::play::Play;
use crate::world::PartId;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::time::TimeUpdateStrategy;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

const HZ: f32 = 60.0;
/// Frames to wait for the level to start (models loading) before giving up.
const START_LIMIT: u32 = 1200;
/// Frames to wait for screenshots to reach the disk after the last step.
const SAVE_LIMIT: u32 = 300;

#[derive(Deserialize)]
pub struct TestScript {
    /// Optional: a CSV file (relative to the project) of where the feet are each step.
    #[serde(default)]
    pub trace: String,
    /// How far (m) a run may stray from the trace at any step.
    #[serde(default = "within")]
    pub trace_within: f32,
    pub steps: Vec<Step>,
}

fn within() -> f32 { 0.05 }

#[derive(Deserialize)]
pub struct Step {
    pub at: f32,
    #[serde(default)]
    pub press: Vec<String>,
    #[serde(default)]
    pub release: Vec<String>,
    #[serde(default)]
    pub look: (f32, f32),
    #[serde(default)]
    pub shot: String,
    #[serde(default)]
    pub expect: Vec<Expect>,
}

#[derive(Deserialize)]
pub enum Expect {
    /// The player's feet are within this distance of a point.
    Feet((f32, f32, f32), f32),
    /// The named part is within this distance of a point.
    Part(String, (f32, f32, f32), f32),
    /// The path along the navigation area from one point to another is this long (within a distance), or there is none.
    Path((f32, f32, f32), (f32, f32, f32), Option<f32>, f32),
    /// The named Sound part is playing (true) or not (false). Skipped on a computer with no sound device.
    Playing(String, bool),
    /// This recording (its file name) started (true) or didn't (false) since the step with the last Played check.
    /// Skipped with no sound device.
    Played(String, bool),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Input { Key(KeyCode), Mouse(MouseButton) }

/// A key or mouse button a test can press, by the names the input map uses (`input.rs`); not gamepads.
pub(crate) fn input(name: &str) -> Option<Input> {
    match crate::input::parse(name)? {
        crate::input::Input::Key(k) => Some(Input::Key(k)),
        crate::input::Input::Mouse(b) => Some(Input::Mouse(b)),
        _ => None,
    }
}

/// A test being run.
#[derive(Resource)]
struct Run {
    name: String,
    out: PathBuf,
    steps: Vec<Step>,
    /// Part names to ids, for `Expect::Part`.
    parts: Vec<(String, u64)>,
    trace_path: Option<PathBuf>,
    trace_within: f32,
    golden: Option<Vec<Vec3>>,
    record: bool,
    next: usize,
    /// The level has started and the clock is running.
    begun: bool,
    /// Steps played so far (the clock: step / 60 s).
    step: u32,
    waited: u32,
    held: Vec<Input>,
    trace: Vec<Vec3>,
    shots: Vec<PathBuf>,
    lines: Vec<String>,
    checks: u32,
    failed: u32,
    strayed: Option<(u32, f32)>,
    /// How much of the sounds' log the last Played check had seen.
    heard: usize,
}

/// Read a test file and check its keys and part names against the level.
fn load(dir: &Path, file: &Path, level: &Level, record: bool) -> Result<Run, String> {
    let path = dir.join(file);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let s: TestScript = ron::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut parts = vec![];
    for st in &s.steps {
        for k in st.press.iter().chain(&st.release) {
            input(k).ok_or(format!("at {}s: unknown key \"{k}\"", st.at))?;
        }
        for e in &st.expect {
            if let Expect::Part(n, ..) | Expect::Playing(n, _) = e {
                let p = level.parts.iter().find(|p| &p.name == n).ok_or(format!("at {}s: no part named \"{n}\"", st.at))?;
                parts.push((n.clone(), p.id));
            }
        }
    }
    if s.steps.is_empty() { return Err("the test has no steps".into()); }
    let name = file.file_stem().unwrap_or_default().to_string_lossy().trim_end_matches(".test").to_string();
    let trace_path = (!s.trace.is_empty()).then(|| dir.join(&s.trace));
    let golden = match &trace_path {
        Some(p) if !record && p.is_file() => Some(read_trace(&std::fs::read_to_string(p).map_err(|e| e.to_string())?)?),
        _ => None,
    };
    let mut steps = s.steps;
    steps.sort_by(|a, b| a.at.total_cmp(&b.at));
    Ok(Run {
        out: dir.join("test-output").join(&name), name, steps, parts, trace_path, trace_within: s.trace_within, golden,
        record, next: 0, begun: false, step: 0, waited: 0, held: vec![], trace: vec![], shots: vec![], lines: vec![], checks: 0,
        failed: 0, strayed: None, heard: 0,
    })
}

fn read_trace(text: &str) -> Result<Vec<Vec3>, String> {
    text.lines().skip(1).filter(|l| !l.trim().is_empty()).map(|l| {
        let v: Vec<f32> = l.split(',').skip(1).map(|x| x.trim().parse().map_err(|_| format!("bad trace line: {l}"))).collect::<Result<_, _>>()?;
        if v.len() != 3 { return Err(format!("bad trace line: {l}")); }
        Ok(Vec3::new(v[0], v[1], v[2]))
    }).collect()
}

fn write_trace(t: &[Vec3]) -> String {
    let mut s = String::from("t,x,y,z\n");
    for (i, p) in t.iter().enumerate() {
        s += &format!("{:.4},{:.4},{:.4},{:.4}\n", (i + 1) as f32 / HZ, p.x, p.y, p.z);
    }
    s
}

fn v(p: Vec3) -> String { format!("({:.2}, {:.2}, {:.2})", p.x, p.y, p.z) }

/// Run the test in `file` (relative to the project folder `dir`) against `level`.
pub fn add(app: &mut App, dir: &Path, file: &Path, level: &Level, record: bool) {
    let run = match load(dir, file, level, record) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Couldn't read the test: {e}");
            std::process::exit(2);
        }
    };
    app.insert_resource(run)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / HZ)))
        .add_systems(PreUpdate, drive.after(InputSystems).before(crate::input::read))
        .add_systems(PostUpdate, (record_step, finish).chain());
}

/// Each step: hold the keys, play the timeline's events that are due, check what's expected.
fn drive(
    mut commands: Commands,
    mut run: ResMut<Run>,
    play: Res<Play>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut motion: ResMut<AccumulatedMouseMotion>,
    parts: Query<(&PartId, &Transform)>,
    sounds: Res<crate::sound::Sounds>,
) {
    if !play.on || run.next >= run.steps.len() { return; }
    let run = &mut *run;
    run.begun = true;
    let t = run.step as f32 / HZ;
    while let Some(st) = run.steps.get(run.next).filter(|s| s.at <= t + 0.5 / HZ) {
        let mut said = vec![];
        for k in &st.release {
            let i = input(k).unwrap();
            run.held.retain(|h| *h != i);
            match i { Input::Key(k) => keys.release(k), Input::Mouse(b) => mouse.release(b) }
            said.push(format!("release {k}"));
        }
        for k in &st.press {
            let i = input(k).unwrap();
            if !run.held.contains(&i) { run.held.push(i); }
            said.push(format!("press {k}"));
        }
        if st.look != (0.0, 0.0) {
            motion.delta += Vec2::new(st.look.0, st.look.1);
            said.push(format!("look {:?}", st.look));
        }
        if !st.shot.is_empty() {
            std::fs::create_dir_all(&run.out).ok();
            let path = run.out.join(format!("{}.png", st.shot));
            std::fs::remove_file(&path).ok();
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path.clone()));
            run.shots.push(path);
            said.push(format!("screenshot {}", st.shot));
        }
        if !said.is_empty() { run.lines.push(format!("{t:6.2}s  {}", said.join(", "))); }
        let heard_now = sounds.log.len();
        let mut checked_played = false;
        for e in &st.expect {
            if let Expect::Played(clip, want) = e {
                if !sounds.on() {
                    run.lines.push(format!("{t:6.2}s  - \"{clip}\" played: skipped (no sound device)"));
                    continue;
                }
                checked_played = true;
                let got = sounds.log[run.heard.min(heard_now)..heard_now].iter().any(|c| c.rsplit('/').next() == Some(clip.as_str()));
                run.checks += 1;
                if got != *want { run.failed += 1; }
                run.lines.push(format!("{t:6.2}s  {} \"{clip}\" {}played: {}", if got == *want { "✓" } else { "✗ FAIL" }, if *want { "" } else { "not " }, if got { "played" } else { "not heard" }));
                continue;
            }
            if let Expect::Path(a, b, want, d) = e {
                let got = play.nav.as_ref().map(|f| f.path(Vec3::from(*a), Vec3::from(*b)).map(|p| crate::nav::length(&p)));
                let ok = match (got, want) { (Some(Ok(l)), Some(w)) => (l - w).abs() <= *d, (Some(Err(_)), None) => true, _ => false };
                run.checks += 1;
                if !ok { run.failed += 1; }
                let said = |l: Option<f32>| l.map_or("no way".to_string(), |l| format!("{l:.2} m"));
                let got = match got { Some(Ok(l)) => said(Some(l)), Some(Err(e)) => e.to_string(), None => "no navigation area".into() };
                run.lines.push(format!("{t:6.2}s  {} path {} → {}: {} ± {d}: {got}", if ok { "✓" } else { "✗ FAIL" }, v(Vec3::from(*a)), v(Vec3::from(*b)), said(*want)));
                continue;
            }
            if let Expect::Playing(n, want) = e {
                let id = run.parts.iter().find(|(m, _)| m == n).map(|x| x.1);
                if !sounds.on() {
                    run.lines.push(format!("{t:6.2}s  - \"{n}\" playing: skipped (no sound device)"));
                    continue;
                }
                let got = id.is_some_and(|id| sounds.playing().contains(&id));
                run.checks += 1;
                if got != *want { run.failed += 1; }
                run.lines.push(format!("{t:6.2}s  {} \"{n}\" {}playing: {}", if got == *want { "✓" } else { "✗ FAIL" }, if *want { "" } else { "not " }, if got { "playing" } else { "silent" }));
                continue;
            }
            let (what, at, want, d) = match e {
                Expect::Feet(p, d) => ("feet".to_string(), Some(play.feet), Vec3::from(*p), *d),
                Expect::Part(n, p, d) => {
                    let id = run.parts.iter().find(|(m, _)| m == n).map(|x| x.1);
                    let at = parts.iter().find(|(pid, _)| Some(pid.0) == id).map(|(_, tr)| tr.translation);
                    (format!("part \"{n}\""), at, Vec3::from(*p), *d)
                }
                Expect::Path(..) | Expect::Playing(..) | Expect::Played(..) => unreachable!("checked above"),
            };
            run.checks += 1;
            let ok = at.is_some_and(|a| a.distance(want) <= d);
            if !ok { run.failed += 1; }
            let got = at.map(v).unwrap_or("not found".into());
            run.lines.push(format!("{t:6.2}s  {} {what} near {} ± {d}: {got}", if ok { "✓" } else { "✗ FAIL" }, v(want)));
        }
        if checked_played { run.heard = heard_now; }
        run.next += 1;
    }
    for h in &run.held {
        match *h { Input::Key(k) => keys.press(k), Input::Mouse(b) => mouse.press(b) }
    }
}

/// After each step: where the feet are (trace row i is the end of step i), against the trace if there is one.
fn record_step(mut run: ResMut<Run>, play: Res<Play>) {
    if !run.begun { return; }
    let i = run.trace.len();
    run.trace.push(play.feet);
    if let Some(g) = run.golden.as_ref().and_then(|g| g.get(i)) {
        let d = g.distance(play.feet);
        if d > run.trace_within && run.strayed.is_none() { run.strayed = Some((i as u32, d)); }
    }
    if run.next < run.steps.len() { run.step += 1; }
}

/// Once the last step has played and the screenshots are saved: write the report and quit.
fn finish(mut run: ResMut<Run>, mut exit: MessageWriter<AppExit>, perf: Res<crate::perf::Perf>, shaders: Res<crate::shader::Shaders>) {
    let run = &mut *run;
    if !run.begun {
        run.waited += 1;
        if run.waited < START_LIMIT { return; }
        run.failed += 1;
        run.lines.push("✗ FAIL the level never started".into());
    } else if run.next < run.steps.len() {
        return;
    } else {
        run.waited += 1;
        let saved = run.shots.iter().all(|p| p.is_file());
        if !saved && run.waited < SAVE_LIMIT { return; }
        if !saved {
            run.failed += 1;
            run.lines.push("✗ FAIL a screenshot wasn't saved".into());
        }
    }
    if let Some(p) = &run.trace_path {
        // the last step is checked before it plays, so the trace ends there too
        let n = run.step as usize + 1;
        run.trace.truncate(n);
        match &run.golden {
            Some(g) => {
                let worst = run.trace.iter().zip(g).map(|(a, b)| a.distance(*b)).fold(0.0, f32::max);
                let mut bad = false;
                if let Some((i, d)) = run.strayed {
                    bad = true;
                    run.lines.push(format!("✗ FAIL trace: {d:.3} m off at {:.2}s (allowed {}), worst {worst:.3} m", (i + 1) as f32 / HZ, run.trace_within));
                }
                if g.len() != run.trace.len() {
                    bad = true;
                    run.lines.push(format!("✗ FAIL trace: {} steps, the recording has {} (record it again with --record)", run.trace.len(), g.len()));
                }
                if bad {
                    run.failed += 1;
                } else {
                    run.lines.push(format!("✓ trace: within {} m all {} steps (worst {worst:.4} m)", run.trace_within, g.len()));
                }
                run.checks += 1;
            }
            None => {
                if let Some(d) = p.parent() { std::fs::create_dir_all(d).ok(); }
                match std::fs::write(p, write_trace(&run.trace)) {
                    Ok(()) => run.lines.push(format!("recorded the trace: {} steps{}", run.trace.len(), if run.record { "" } else { " (first run)" })),
                    Err(e) => { run.failed += 1; run.lines.push(format!("✗ FAIL couldn't write the trace: {e}")); }
                }
            }
        }
    }
    // every shader the level uses must have built (a header that doesn't parse, or code the graphics card refused)
    let mut paths: Vec<&String> = shaders.paths().collect();
    paths.sort();
    for path in &paths {
        run.checks += 1;
        match shaders.error(path) {
            Some(e) => { run.failed += 1; run.lines.push(format!("✗ FAIL shader {e}")); }
            None => run.lines.push(format!("✓ shader {path} built")),
        }
    }
    // numbers, not a check: the meter's reading at the end (a game's tests compare these between runs)
    run.lines.push(perf.line());
    let summary = if run.failed == 0 { format!("PASS {}: {} checks", run.name, run.checks) } else { format!("FAIL {}: {} of {} checks failed", run.name, run.failed, run.checks) };
    run.lines.push(summary);
    let report = run.lines.join("\n") + "\n";
    print!("{report}");
    std::fs::create_dir_all(&run.out).ok();
    std::fs::write(run.out.join("report.txt"), &report).ok();
    exit.write(if run.failed == 0 { AppExit::Success } else { AppExit::from_code(1) });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_file_reads_with_whole_numbers_and_unknown_keys_are_caught() {
        let s: TestScript = ron::from_str(r#"(steps: [(at: 0, press: ["W", "Space"]), (at: 1.5, shot: "a", expect: [Feet((0, 0, -5), 0.5), Part("Lift", (1, 2.5, 3), 0.1)])])"#).unwrap();
        assert_eq!(s.steps.len(), 2);
        assert!(matches!(&s.steps[1].expect[1], Expect::Part(n, (1.0, 2.5, 3.0), _) if n == "Lift"));
        assert_eq!(input("w"), Some(Input::Key(KeyCode::KeyW)));
        assert_eq!(input("7"), Some(Input::Key(KeyCode::Digit7)));
        assert_eq!(input("MouseRight"), Some(Input::Mouse(MouseButton::Right)));
        assert_eq!(input("Jump"), None);
    }

    #[test]
    fn a_trace_reads_back_what_was_written() {
        let t = vec![Vec3::new(0.0, 1.0, 2.0), Vec3::new(-0.5, 0.25, 3.125)];
        assert_eq!(read_trace(&write_trace(&t)).unwrap(), t);
    }
}
