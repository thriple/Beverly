//! Beverly: a simple, no-code-first 3D game editor for Bevy. See README.md and docs/design.html.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod caps;
mod edit;
mod level;
mod shapes;
mod theme;
mod ui;
mod view;

use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use level::{Level, Shape};
use std::collections::HashMap;
use std::path::PathBuf;

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
    pub assets_open: bool,
    pub assets: Vec<(&'static str, Vec<String>)>,
    pub hovering_files: bool,
    pub pending: Option<Pending>,
    pub island: theme::Island,
    pub tris: HashMap<Shape, shapes::Tris>,
    pub drag: Option<edit::Drag>,
    pub hover_handle: Option<edit::Grip>,
    /// A label to float near the cursor this frame (move widget): window position, text, colour.
    pub label: Option<(Vec2, String, bevy_egui::egui::Color32)>,
}

impl Editor {
    fn new() -> Self { Self::at(level::new_project_dir().unwrap_or_else(|_| PathBuf::from("Untitled"))) }

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
            assets_open: true,
            assets: vec![],
            hovering_files: false,
            pending: None,
            island: default(),
            tris: Shape::ALL.iter().map(|&s| (s, shapes::build(s))).collect(),
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
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Beverly".into(), resolution: (1600, 960).into(), ..default() }),
            close_when_requested: false,
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .insert_resource(ClearColor(Color::srgb(0.045, 0.04, 0.055)))
        .insert_resource(GlobalAmbientLight { color: Color::srgb(0.85, 0.88, 1.0), brightness: 500.0, ..default() })
        .init_resource::<caps::CapsLock>()
        .insert_resource(Editor::new())
        .init_gizmo_group::<edit::WidgetGizmos>()
        .init_gizmo_group::<edit::OutlineGizmos>()
        .add_systems(Startup, (shapes::setup, view::setup, edit::setup, caps::startup))
        .add_systems(
            Update,
            (view::input, edit::interact, edit::shortcuts, edit::files, edit::sync, edit::draw, view::grid, view::apply_viewport, edit::commit, edit::close_requested, view::frame_log)
                .chain(),
        )
        .add_systems(EguiPrimaryContextPass, ui::ui)
        .run();
}
