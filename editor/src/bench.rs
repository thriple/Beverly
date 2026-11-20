//! The benchmark: `beverly-bench`, a separate small program that measures the graphics card and the processor with
//! the kinds of scenes Beverly's games will ask of them, at 1080p and 4K, and writes a short plain-text log
//! (`beverly-bench-log.txt`, next to the program) that holds no names or paths of the user's. The scenes are in
//! `bench/scenes.rs`, the CPU test in `bench/agents.rs`; the stress level in the editor builds from the same code.
mod agents;
pub mod scenes;

use crate::perf::{Perf, PerfPlugin};
use bevy::diagnostic::SystemInfo;
use bevy::prelude::*;
use bevy::window::{PresentMode, PrimaryWindow};
use scenes::{Bench, Stage};
use std::io::Write;

/// The two sizes every card scene runs at.
const SIZES: [(&str, u32, u32); 2] = [("1080p", 1920, 1080), ("4K", 3840, 2160)];
/// A step is the last of its series when it runs slower than this (frames per second).
const TOO_SLOW: f64 = 20.0;

#[derive(Clone, Copy, Debug)]
enum Kind {
    /// This many tiles of about 100 000 triangles
    Polygons(usize),
    /// Unshadowed point lights
    Lights(usize),
    /// Shadowed point lights
    Shadowed(usize),
    /// Cars with shadowed headlights through a lit town
    Headlights(usize),
    /// Gigabytes of textures held in video memory
    Textures(usize),
    /// Agents steering on the processor
    Agents(usize),
}

struct Step { series: String, label: String, kind: Kind, size: usize }

/// The steps to run; `quick` keeps the two lightest of each series (to check the program runs).
fn plan(quick: bool) -> Vec<Step> {
    let mut v = vec![];
    for n in [1_000, 10_000, 100_000, 1_000_000] {
        v.push(Step { series: "agents".into(), label: format!("{n} agents"), kind: Kind::Agents(n), size: 0 });
    }
    for (s, (name, ..)) in SIZES.iter().enumerate() {
        let mut add = |series: &str, label: String, kind| v.push(Step { series: format!("{series} {name}"), label, kind, size: s });
        for n in [10, 40, 160, 640] { add("polygons", format!("{:.1} million triangles", (n * scenes::TILE_TRIANGLES) as f64 / 1e6), Kind::Polygons(n)); }
        for n in [100, 400, 1600] { add("lights", format!("{n} lights, no shadows"), Kind::Lights(n)); }
        for n in [1, 2, 4, 8, 16, 32] { add("shadowed lights", format!("{n} point lights with shadows"), Kind::Shadowed(n)); }
        for n in [1, 2, 4, 8] { add("headlights", format!("{n} moving headlights with shadows, 300 lights, 600 props"), Kind::Headlights(n)); }
    }
    for gb in [1, 2, 4, 6, 8, 10, 12, 16] { v.push(Step { series: "video memory".into(), label: format!("{gb} GB of textures"), kind: Kind::Textures(gb), size: 0 }); }
    if quick {
        let mut seen: Vec<(String, usize)> = vec![];
        v.retain(|s| match seen.iter_mut().find(|(x, _)| *x == s.series) { Some((_, n)) => { *n += 1; *n < 2 } None => { seen.push((s.series.clone(), 1)); true } });
    }
    v
}

enum Phase { Start, Feed, Warm { frames: u32, secs: f32 }, Measure { times: Vec<f32>, secs: f32 } }

#[derive(Resource)]
struct Run {
    steps: Vec<Step>,
    at: usize,
    phase: Phase,
    /// Series that went too slow: their later steps are left out.
    stopped: Vec<String>,
    /// (series, label, fps) of every step, for the summary
    done: Vec<(String, String, f64)>,
    textures: usize,
    /// Seconds each scene is measured for
    secs: f32,
    log: std::fs::File,
}

impl Run {
    /// One line in the log, kept on disk at once so a crash leaves everything up to it.
    fn say(&mut self, line: impl AsRef<str>) {
        let line = line.as_ref();
        println!("{line}");
        writeln!(self.log, "{line}").ok();
        self.log.flush().ok();
        self.log.sync_data().ok();
    }
}

/// Runs the benchmark. Does not return until it is done.
pub fn run() {
    let dir = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf())).unwrap_or_default();
    let log = std::fs::File::create(dir.join("beverly-bench-log.txt")).or_else(|_| std::fs::File::create("beverly-bench-log.txt")).expect("a log file to write");
    let quick = std::env::args().any(|a| a == "--quick");
    let mut app = App::new();
    let render = crate::gpu::render_plugin(&mut app);
    app.add_plugins(DefaultPlugins.set(render).set(WindowPlugin {
        primary_window: Some(Window { title: "Beverly bench".into(), resolution: (560, 120).into(), present_mode: PresentMode::AutoNoVsync, resizable: false, ..default() }),
        ..default()
    }))
    .add_plugins(PerfPlugin)
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .insert_resource(GlobalAmbientLight { color: Color::srgb(0.85, 0.88, 1.0), brightness: 300.0, ..default() })
    .insert_resource(Run { steps: plan(quick), secs: if quick { 1.0 } else { 4.0 }, at: 0, phase: Phase::Start, stopped: vec![], done: vec![], textures: 0, log })
    .add_systems(Update, (header.run_if(run_once), drive, scenes::drive).chain());
    app.run();
}

fn run_once(mut done: Local<bool>) -> bool { !std::mem::replace(&mut *done, true) }

/// The card and the machine, at the top of the log.
fn header(mut run: ResMut<Run>, caps: Res<crate::gpu::GpuCaps>, info: Option<Res<SystemInfo>>) {
    run.say(format!("Beverly bench, build {}", option_env!("BEVERLY_BUILD").unwrap_or("local")));
    run.say("Each scene runs for a few seconds. If it stops early, send this file as it is.");
    run.say(format!("Graphics card: {} ({}, {})", caps.name, caps.backend, caps.kind));
    run.say(format!("Virtual geometry features: {}", if caps.virtual_geometry() { "yes" } else { "no" }));
    if let Some(i) = info { run.say(format!("Processor: {}, {} cores, {}, {} of memory", i.cpu, i.core_count, i.os, i.memory)); }
    run.say("Video memory size is not asked of the card; the video memory test shows where it runs out.\n");
}

#[allow(clippy::too_many_arguments)]
fn drive(
    mut run: ResMut<Run>,
    time: Res<Time<Real>>,
    perf: Res<Perf>,
    mut stage: Stage,
    old: Query<Entity, With<Bench>>,
    mut exit: MessageWriter<AppExit>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    let (dt, run_secs) = (time.delta_secs(), run.secs);
    match &mut run.phase {
        Phase::Start => {
            let Some(step) = run.steps.get(run.at) else { return finish(&mut run, &mut exit) };
            let (series, label, kind, size) = (step.series.clone(), step.label.clone(), step.kind, SIZES[step.size]);
            if run.stopped.contains(&series) { run.at += 1; return; }
            window.title = format!("Beverly bench: {} {} ({} of {})", series, label, run.at + 1, run.steps.len());
            run.say(format!("starting: {series}, {label}"));
            let px = (size.1, size.2);
            if let Kind::Agents(n) = kind {
                let (one, all, cores) = agents::run(n);
                run.say(format!("  {label}: {one:.2} ms a tick on one core, {all:.2} ms on {cores} cores"));
                run.done.push((series.clone(), label, 1000.0 / all));
                if all > 50.0 { run.stopped.push(series); }
                run.at += 1;
                return;
            }
            // the texture test adds to what is there; every other scene starts clean
            if !matches!(kind, Kind::Textures(_)) || run.textures == 0 { for e in &old { stage.commands.entity(e).despawn(); } }
            match kind {
                Kind::Polygons(n) => scenes::polygons(&mut stage, px, n),
                Kind::Lights(n) => scenes::lights(&mut stage, px, n),
                Kind::Shadowed(n) => scenes::shadowed(&mut stage, px, n),
                Kind::Headlights(n) => scenes::headlights(&mut stage, px, n),
                Kind::Textures(_) => { if run.textures == 0 { scenes::texture_view(&mut stage, px); } }
                Kind::Agents(_) => unreachable!(),
            }
            run.phase = if matches!(kind, Kind::Textures(_)) { Phase::Feed } else { Phase::Warm { frames: 0, secs: 0.0 } };
        }
        // a few textures a frame, so their data never piles up on the way to the card
        Phase::Feed => {
            let Kind::Textures(gb) = run.steps[run.at].kind else { return };
            for _ in 0..2 {
                if run.textures < gb * scenes::PER_GB { scenes::texture(&mut stage, run.textures); run.textures += 1; }
            }
            if run.textures >= gb * scenes::PER_GB { run.phase = Phase::Warm { frames: 0, secs: 0.0 }; }
        }
        Phase::Warm { frames, secs } => {
            *frames += 1;
            *secs += dt;
            // long enough for shaders to build and meshes to reach the card
            if *frames >= 20 && *secs >= 1.0 { run.phase = Phase::Measure { times: vec![], secs: 0.0 }; }
            else if *secs > 30.0 { measured(&mut run, &perf, vec![], 30.0); }
        }
        Phase::Measure { times, secs } => {
            times.push(dt);
            *secs += dt;
            if (*secs >= run_secs && times.len() >= 20) || *secs >= 20.0 {
                let (times, secs) = (std::mem::take(times), *secs);
                measured(&mut run, &perf, times, secs);
            }
        }
    }
}

/// A step's numbers into the log, and on to the next.
fn measured(run: &mut Run, perf: &Perf, mut times: Vec<f32>, secs: f32) {
    let step = &run.steps[run.at];
    let (series, label, size) = (step.series.clone(), step.label.clone(), SIZES[step.size].0);
    let fps = if secs > 0.0 { times.len() as f64 / secs as f64 } else { 0.0 };
    times.sort_by(|a, b| a.total_cmp(b));
    let worst = times.get((times.len() as f32 * 0.99) as usize).or(times.last()).copied().unwrap_or(0.0) * 1000.0;
    let opt = |v: Option<f64>, unit: &str| v.map_or("n/a".to_string(), |v| format!("{v:.1}{unit}"));
    run.say(format!("  {label} at {size}: {fps:.0} fps ({:.1} ms a frame, worst 1% {worst:.1} ms), card {}, triangles drawn {}, memory {}",
        if fps > 0.0 { 1000.0 / fps } else { 0.0 }, opt(perf.gpu_ms, " ms"), perf.triangles.map_or("n/a".to_string(), |t| if t >= 1e5 { format!("{:.1} million", t / 1e6) } else { format!("{:.0} thousand", t / 1e3) }), opt(perf.ram_gb, " GB")));
    run.done.push((series.clone(), label, fps));
    if fps < TOO_SLOW { run.say(format!("  ({series} stopped: too slow to go on)")); run.stopped.push(series); }
    run.at += 1;
    run.phase = Phase::Start;
}

/// The summary: for each series the heaviest step that kept 60 fps and the heaviest that kept 30.
fn finish(run: &mut Run, exit: &mut MessageWriter<AppExit>) {
    run.say("\nSummary (the heaviest step that held 60 fps, and 30 fps):");
    let mut series: Vec<String> = vec![];
    for (s, ..) in &run.done { if !series.contains(s) { series.push(s.clone()); } }
    for s in series {
        let rows: Vec<_> = run.done.iter().filter(|(x, ..)| *x == s).collect();
        let last = |at: f64| rows.iter().filter(|(.., f)| *f >= at).last().map_or("none".to_string(), |(_, l, _)| l.clone());
        run.say(format!("  {s}: 60 fps: {}; 30 fps: {}", last(60.0), last(30.0)));
    }
    run.say("\nDone. Send this file back.");
    exit.write(AppExit::Success);
}
