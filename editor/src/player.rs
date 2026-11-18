//! The game player: runs a Beverly project as a game. An exported game is this program, renamed after the
//! project, with the project's files beside it. Usage: `beverly-player [PROJECT FOLDER] [--test FILE [--record]]`
//! (default folder: the folder the program is in, then the current folder; `--test`: see `scripted`).
use crate::level::{self, Shape};
use crate::models::{self, ModelLib, ProjectRoot};
use crate::play::{Play, PlayCamera, PlayPlugin};
use crate::shapes::{self, ShapeLib};
use crate::world::{self, Geometry};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use std::path::PathBuf;

#[derive(Resource)]
struct Game { level: level::Level, geo: Geometry, started: bool }

/// The command line: the project folder, a test to run, and whether to record its trace again.
fn args() -> (Option<PathBuf>, Option<PathBuf>, bool) {
    let (mut dir, mut test, mut record) = (None, None, false);
    let mut a = std::env::args().skip(1);
    while let Some(x) = a.next() {
        match x.as_str() {
            "--test" => test = a.next().map(PathBuf::from),
            "--record" => record = true,
            _ => dir = Some(PathBuf::from(x)),
        }
    }
    (dir, test, record)
}

fn project_dir(arg: Option<PathBuf>) -> PathBuf {
    if let Some(a) = arg { return level::project_dir_of(&a); }
    let beside = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf()));
    match beside {
        Some(d) if level::project_file(&d).is_some() => d,
        _ => std::env::current_dir().unwrap_or_default(),
    }
}

/// Run the game. A game with its own code builds its own player with this; `extra` adds its plugins.
pub fn run(extra: fn(&mut App)) {
    let (dir, test, record) = args();
    let dir = project_dir(dir);
    let (name, level) = match level::load(&dir) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Couldn't load the game in {}: {e}", dir.display());
            std::process::exit(1);
        }
    };
    let mut app = App::new();
    models::register_source(&mut app, &ProjectRoot::new(dir.clone()));
    let render = crate::gpu::render_plugin(&mut app);
    app.add_plugins(DefaultPlugins.set(render).set(WindowPlugin {
        primary_window: Some(Window { title: name, resolution: (1600, 900).into(), ..default() }),
        ..default()
    }));
    extra(&mut app);
    if let Some(t) = test { crate::scripted::add(&mut app, &dir, &t, &level, record); }
    app.add_plugins((PlayPlugin, crate::paint::PaintPlugin))
    .insert_resource(ClearColor(Color::srgb(0.045, 0.04, 0.055)))
    .insert_resource(GlobalAmbientLight { color: Color::srgb(0.85, 0.88, 1.0), brightness: 500.0, ..default() })
    .init_resource::<ModelLib>()
    .init_resource::<crate::sound::Sounds>()
    .insert_resource(Game { level, geo: Geometry::default(), started: false })
    .add_systems(Startup, (shapes::setup, setup).chain())
    .add_systems(Update, (models::build_geometry, stage, start, cursor, hear, paint).chain());
    // a scripted test that failed quits with code 1
    if let AppExit::Error(code) = app.run() { std::process::exit(code.get() as i32); }
}

#[allow(clippy::too_many_arguments)]
fn setup(
    mut commands: Commands,
    game: Res<Game>,
    lib: Res<ShapeLib>,
    mut models: ResMut<ModelLib>,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut assets: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((Camera3d::default(), PlayCamera, Msaa::Off, bevy::anti_alias::smaa::Smaa::default(), Transform::from_xyz(0.0, 3.0, 8.0)));
    let mut mats = world::setup_stage(&mut commands, &mut meshes, &mut assets, true);
    let mut looks = world::Looks { lib: &lib, models: &mut models, server: &server, mats: &mut mats, assets: &mut assets, meshes: &mut meshes, materials: &game.level.materials, shadows: true };
    for p in &game.level.parts {
        world::spawn_visual(&mut commands, p, &mut looks, false);
    }
    commands.insert_resource(mats);
}

/// The sun and fill light as the level says (once, when the stage's sun exists).
fn stage(game: Res<Game>, mut done: Local<bool>, mut sun: Query<(&mut DirectionalLight, &mut Transform), With<world::Sun>>, mut fill: ResMut<GlobalAmbientLight>) {
    if *done || sun.is_empty() { return; }
    world::light_stage(&game.level.lighting, true, &mut sun, &mut fill);
    *done = true;
}

/// Start playing once every model has loaded (so the walker collides with them too).
fn start(mut commands: Commands, mut game: ResMut<Game>, lib: Res<ModelLib>, mut play: ResMut<Play>, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    if game.started || lib.pending() { return; }
    let game = &mut *game;
    game.geo.sync_models(&lib);
    game.started = true;
    let (feet, yaw) = match game.level.parts.iter().find(|p| p.shape == Shape::PlayerStart) {
        Some(p) => (Vec3::from(p.pos) - Vec3::Y * p.size[1] * 0.5, Quat::from_array(p.rot).to_euler(EulerRot::YXZ).0),
        None => (Vec3::ZERO, 0.0),
    };
    play.start(&mut commands, &mut meshes, &mut mats, &game.level, &game.geo, feet, yaw);
}

/// The brush strokes drawn on the parts.
fn paint(game: Res<Game>, mut paint: crate::paint::PaintSync) { paint.run(&game.level); }

/// The sounds, heard from the camera.
fn hear(mut sounds: ResMut<crate::sound::Sounds>, game: Res<Game>, mut play: ResMut<Play>, root: Res<ProjectRoot>, ear: Single<&GlobalTransform, With<PlayCamera>>) {
    let run = play.on.then_some(play.run);
    let inputs = std::mem::take(&mut play.script.inputs);
    let happened = std::mem::take(&mut play.script.happened);
    let script = &play.script;
    sounds.step(&game.level, &crate::sound::speakers(&game.level), &root.get(), run, inputs, happened, &ear, |id| game.level.get(id).map(|p| Vec3::from(p.pos) + script.offset_of(id).unwrap_or_default()));
}

/// The mouse is captured while playing; Esc lets it go, a click takes it back.
fn cursor(mut opts: Single<&mut CursorOptions, With<PrimaryWindow>>, keys: Res<ButtonInput<KeyCode>>, mouse: Res<ButtonInput<MouseButton>>) {
    if keys.just_pressed(KeyCode::Escape) {
        opts.grab_mode = CursorGrabMode::None;
        opts.visible = true;
    }
    if mouse.just_pressed(MouseButton::Left) {
        opts.grab_mode = CursorGrabMode::Locked;
        opts.visible = false;
    }
}
