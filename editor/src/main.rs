//! Beverly: a simple, no-code-first 3D game editor for Bevy. See README.md and docs/design.html.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod caps;
mod edit;
mod theme;
mod ui;
mod view;

use beverly::{level, models, play, shapes, world};

use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use level::{Level, Shape};
use std::path::PathBuf;

/// The editor view to return to after test mode.
#[derive(Clone, Copy)]
pub struct Saved { pub pos: Vec3, pub yaw: f32, pub pitch: f32, pub mode: view::Mode }

/// Something that would lose unsaved work: asked about first.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pending { Quit, New, Open }

/// The whole editing state. The level is the source of truth; 3D entities are rebuilt from it (edit::sync).
#[derive(Resource)]
pub struct Editor {
    pub level: Level,
    /// The level as last saved, for the unsaved-changes dot and prompt.
    pub saved: Level,
    /// The level as of the last undo step.
    pub committed: Level,
    pub undo: Vec<Level>,
    pub redo: Vec<Level>,
    pub dir: PathBuf,
    pub name: String,
    pub selected: Option<u64>,
    pub placing: Option<Shape>,
    pub snap: bool,
    pub parts_open: bool,
    pub parts_query: String,
    /// The pop-up opened this frame (so the Tab that opened it doesn't also close it).
    pub parts_fresh: bool,
    /// The keyboard highlight in the pop-up (an index into the matching shapes).
    pub parts_cursor: usize,
    pub assets_open: bool,
    pub assets: Vec<(&'static str, Vec<String>)>,
    pub hovering_files: bool,
    pub pending: Option<Pending>,
    pub island: theme::Island,
    /// Every part's triangles (shapes, and models once loaded).
    pub geo: world::Geometry,
    /// The model file being placed, when `placing` is `Shape::Model`.
    pub placing_model: Option<String>,
    /// Test mode (`): where the editor camera was, to go back to.
    pub testing: Option<Saved>,
    /// Another project was opened: the model cache and the asset folder must follow.
    pub project_changed: bool,
    pub drag: Option<edit::Drag>,
    pub hover_handle: Option<edit::Grip>,
    /// A label to float near the cursor this frame (move widget): window position, text, colour.
    pub label: Option<(Vec2, String, bevy_egui::egui::Color32)>,
}

impl Editor {
    /// `beverly [PROJECT FOLDER]` opens that project; otherwise a new one is made.
    fn new() -> Self {
        if let Some(dir) = std::env::args().nth(1).map(PathBuf::from) {
            if let Ok((_, lvl)) = level::load(&dir) {
                let mut ed = Self::at(dir);
                ed.saved = lvl.clone();
                ed.committed = lvl.clone();
                ed.level = lvl;
                return ed;
            }
        }
        Self::at(level::new_project_dir().unwrap_or_else(|_| PathBuf::from("Untitled")))
    }

    /// An empty level in the project folder `dir`.
    pub fn at(dir: PathBuf) -> Self {
        let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Untitled".into());
        let level = Level { name: "main".into(), ..default() };
        let mut ed = Self {
            saved: level.clone(),
            committed: level.clone(),
            level,
            undo: vec![],
            redo: vec![],
            dir,
            name,
            selected: None,
            placing: None,
            snap: true,
            parts_open: false,
            parts_query: String::new(),
            parts_fresh: false,
            parts_cursor: 0,
            assets_open: true,
            assets: vec![],
            hovering_files: false,
            pending: None,
            island: default(),
            geo: default(),
            placing_model: None,
            testing: None,
            project_changed: false,
            drag: None,
            hover_handle: None,
            label: None,
        };
        ed.refresh_assets();
        ed
    }

    pub fn dirty(&self) -> bool { self.level != self.saved }

    pub fn refresh_assets(&mut self) {
        self.assets = level::ASSET_FOLDERS.iter().map(|f| (*f, level::list_assets(&self.dir, f))).collect();
    }
}

fn main() {
    let ed = Editor::new();
    let root = models::ProjectRoot::new(ed.dir.clone());
    let mut app = App::new();
    models::register_source(&mut app, &root);
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Beverly".into(), resolution: (1600, 960).into(), ..default() }),
            close_when_requested: false,
            ..default()
        }))
        .add_plugins((EguiPlugin::default(), play::PlayPlugin))
        .init_resource::<models::ModelLib>()
        .insert_resource(ClearColor(Color::srgb(0.045, 0.04, 0.055)))
        .insert_resource(GlobalAmbientLight { color: Color::srgb(0.85, 0.88, 1.0), brightness: 500.0, ..default() })
        .init_resource::<caps::CapsLock>()
        .insert_resource(ed)
        .init_gizmo_group::<edit::WidgetGizmos>()
        .init_gizmo_group::<edit::OutlineGizmos>()
        .add_systems(Startup, (shapes::setup, view::setup, edit::setup, caps::startup))
        .add_systems(
            Update,
            (
                edit::project_follow,
                models::build_geometry,
                edit::models_in,
                view::input,
                edit::test_mode,
                edit::interact,
                edit::shortcuts,
                edit::files,
                edit::sync,
                edit::draw,
                view::grid,
                view::apply_viewport,
                edit::commit,
                edit::close_requested,
                view::frame_log,
            )
                .chain(),
        )
        .add_systems(EguiPrimaryContextPass, ui::ui)
        .run();
}
