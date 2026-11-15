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
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window { title: name, resolution: (1600, 900).into(), ..default() }),
        ..default()
    }));
    extra(&mut app);
    if let Some(t) = test { crate::scripted::add(&mut app, &dir, &t, &level, record); }
    app.add_plugins(PlayPlugin)
    .insert_resource(ClearColor(Color::srgb(0.045, 0.04, 0.055)))
    .insert_resource(GlobalAmbientLight { color: Color::srgb(0.85, 0.88, 1.0), brightness: 500.0, ..default() })
    .init_resource::<ModelLib>()
    .insert_resource(Game { level, geo: Geometry::default(), started: false })
    .add_systems(Startup, (shapes::setup, setup).chain())
    .add_systems(Update, (models::build_geometry, start, cursor).chain());
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
    commands.spawn((Camera3d::default(), PlayCamera, Msaa::Sample4, Transform::from_xyz(0.0, 3.0, 8.0)));
    let mut mats = world::setup_stage(&mut commands, &mut meshes, &mut assets, true);
    for p in &game.level.parts {
        world::spawn_visual(&mut commands, p, &lib, &mut models, &server, &mut mats, &mut assets, false);
    }
    commands.insert_resource(mats);
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
