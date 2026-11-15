//! The editor: a simple, no-code-first 3D game editor for Bevy. See README.md and docs/design.html.

mod actions;
mod bev_type;
mod caps;
mod edit;
mod light;
mod texture;
mod mcp;
pub(crate) mod theme;
mod ui;
mod view;

use crate::{level, models, play, shapes, update, world};

use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use level::{Level, Shape};
use std::path::{Path, PathBuf};

/// The editor view to return to after test mode.
#[derive(Clone, Copy)]
pub struct Saved { pub pos: Vec3, pub yaw: f32, pub pitch: f32, pub mode: view::Mode }

/// Test mode asked for by the MCP server (the ` key does the same itself).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TestRequest { Start(Vec3), Stop }

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
    pub test_request: Option<TestRequest>,
    /// Another project was opened: the model cache and the asset folder must follow.
    pub project_changed: bool,
    pub drag: Option<edit::Drag>,
    /// A texture being dragged from the Assets panel, the part and face it would land on (`None` face: all of it),
    /// the face of the selected part whose texture handles show, and those handles' drag and hover.
    pub texturing: Option<String>,
    pub tex_hover: Option<(u64, Option<u8>)>,
    pub tex_face: Option<u8>,
    pub tex_drag: Option<texture::Drag>,
    pub tex_hot: Option<texture::Grip>,
    /// The material shown in the bottom panel instead of the selected part (clicked in Assets).
    pub material_open: Option<String>,
    pub hover_handle: Option<edit::Grip>,
    /// The selected light's handle being dragged, and the one under the pointer.
    pub light_drag: Option<light::Grip>,
    pub light_hot: Option<light::Grip>,
    /// A label to float near the cursor this frame (move widget): window position, text, colour.
    pub label: Option<(Vec2, String, bevy_egui::egui::Color32)>,
    /// The game player's file name (without .exe), next to the editor; exports copy it.
    pub player: &'static str,
    /// Wiring: the part and output a wire is being drawn from (the next click picks the part it goes to).
    pub wiring: Option<(u64, &'static str)>,
    /// The input menu after that click: from, output, to, and where on screen.
    pub wire_pick: Option<(u64, &'static str, u64, Vec2)>,
    /// Help > About Beverly / Check for updates.
    pub about_open: bool,
    pub updates_open: bool,
    pub features_open: bool,
    pub updater: update::Updater,
    /// Help > MCP Server: the window, and the server (off unless the user turned it on).
    pub mcp_open: bool,
    pub mcp: mcp::Mcp,
    /// "Change all 12, or make this one Unique?": the part whose contents were just changed, and its instances
    /// that kept theirs (see `Level::split_from`).
    pub ask: Option<(u64, Vec<u64>)>,
}

impl Editor {
    /// `beverly [PROJECT]` opens that project (its folder or a .bev file in it); without one, the project in a folder beside the editor if
    /// there is exactly one (a game's own editor ships with its project); otherwise a new one is made.
    fn new() -> Self {
        let beside = || {
            let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
            let mut found = std::fs::read_dir(exe_dir).ok()?.flatten().map(|e| e.path()).filter(|d| level::project_file(d).is_some());
            let one = found.next()?;
            found.next().is_none().then_some(one)
        };
        if let Some(dir) = std::env::args().nth(1).map(|a| level::project_dir_of(Path::new(&a))).or_else(beside) {
            if let Ok((name, lvl)) = level::load(&dir) {
                let mut ed = Self::at(dir);
                ed.name = name;
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
            test_request: None,
            project_changed: false,
            drag: None,
            texturing: None,
            tex_hover: None,
            tex_face: None,
            tex_drag: None,
            tex_hot: None,
            material_open: None,
            hover_handle: None,
            light_drag: None,
            light_hot: None,
            label: None,
            player: "beverly-player",
            wiring: None,
            wire_pick: None,
            about_open: false,
            updates_open: false,
            features_open: false,
            updater: default(),
            mcp_open: false,
            mcp: mcp::Mcp::off(),
            ask: None,
        };
        ed.refresh_assets();
        ed
    }

    pub fn dirty(&self) -> bool { self.level != self.saved }

    /// A window over the editor (the unsaved-changes question, Help's windows): the 3D view ignores the keys.
    pub fn dialog_open(&self) -> bool { self.pending.is_some() || self.about_open || self.updates_open || self.features_open || self.mcp_open }

    pub fn refresh_assets(&mut self) {
        self.assets = level::ASSET_FOLDERS.iter().map(|f| (*f, level::list_assets(&self.dir, f))).collect();
    }
}

/// Run the editor. A game with its own code builds its own copy of the editor with this: `title` for the window,
/// `player` the file name of its game player (without .exe), and `extra` adds its plugins (e.g. its player
/// character, see [`play::Play::custom`]).
pub fn run(title: &str, player: &'static str, extra: fn(&mut App)) {
    update::clean_up();
    // Beverly itself opens .bev files; a game's own editor leaves that to it
    if title == "Beverly" { bev_type::register(); }
    let mut ed = Editor::new();
    ed.player = player;
    ed.mcp = mcp::Mcp::start();
    let root = models::ProjectRoot::new(ed.dir.clone());
    let mut app = App::new();
    models::register_source(&mut app, &root);
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: title.into(), resolution: (1600, 960).into(), ..default() }),
            close_when_requested: false,
            ..default()
        }));
    extra(&mut app);
    app.add_plugins((EguiPlugin::default(), play::PlayPlugin))
        .init_resource::<models::ModelLib>()
        .insert_resource(ClearColor(Color::srgb(0.045, 0.04, 0.055)))
        .insert_resource(GlobalAmbientLight { color: Color::srgb(0.85, 0.88, 1.0), brightness: 500.0, ..default() })
        .init_resource::<caps::CapsLock>()
        .insert_resource(ed)
        .init_gizmo_group::<edit::WidgetGizmos>()
        .init_gizmo_group::<edit::OutlineGizmos>()
        .add_systems(Startup, (shapes::setup, view::setup, edit::setup, caps::startup))
        .init_resource::<mcp::host::Injected>()
        .add_systems(PreUpdate, mcp::host::inject.after(bevy::input::InputSystems))
        .add_systems(Update, (window_icon, mcp::host::serve))
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

/// Beverly's icon on the editor window (the title bar and taskbar), once the window exists.
fn window_icon(mut done: Local<bool>, window: Query<Entity, With<bevy::window::PrimaryWindow>>, _main: bevy::ecs::system::NonSendMarker) {
    if *done { return; }
    let Ok(entity) = window.single() else { return };
    bevy::winit::WINIT_WINDOWS.with_borrow(|w| {
        let Some(win) = w.get_window(entity) else { return };
        *done = true;
        let Ok(img) = image::load_from_memory(include_bytes!("../assets/icon.png")) else { return };
        let img = img.into_rgba8();
        let (wd, ht) = img.dimensions();
        if let Ok(icon) = winit::window::Icon::from_rgba(img.into_raw(), wd, ht) { win.set_window_icon(Some(icon)); }
    });
}
