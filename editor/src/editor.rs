//! The editor: a simple, no-code-first 3D game editor for Bevy. See README.md and docs/design.html.

mod actions;
mod audio;
mod brush;
mod bucket;
mod pen;
mod preview;
mod thumbs;
mod bev_type;
mod caps;
mod edit;
pub(crate) mod frame;
pub(crate) use frame::paint_icon as frame_icon;
mod tokens;
mod text;
mod vector;
mod camera;
mod light;
mod texture;
pub(crate) mod mcp;
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

/// What the Level tab shows: building (the plain tab), or one topic from its menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LevelMode { Build, Physics, Cameras, Lighting }

/// Trying a path on the navigation area: the start and end picked (clicks in the view) and the way found.
#[derive(Default)]
pub struct PathTry {
    pub from: Option<Vec3>,
    pub to: Option<Vec3>,
    pub found: Option<Result<Vec<Vec3>, &'static str>>,
}

/// The whole editing state. The level is the source of truth; 3D entities are rebuilt from it (edit::sync).
#[derive(Resource)]
pub struct Editor {
    pub level: Level,
    /// The level as last saved, for the unsaved-changes dot and prompt.
    pub saved: Level,
    /// The level as of the last undo step.
    pub committed: Level,
    /// The level's stamps when it was last compared with `committed`: while they are the same nothing needs comparing.
    pub checked: level::Stamps,
    /// The parts' stamp when the children last followed their parents (`edit::follow_parents`).
    pub followed: u64,
    /// Each undo and redo step holds what changed, not a copy of the level (`level::Delta`).
    pub undo: Vec<level::Delta>,
    pub redo: Vec<level::Delta>,
    /// Whether the level differs from the saved one, worked out when its stamps were these (see `dirty`).
    pub dirty_cache: (level::Stamps, bool),
    pub dir: PathBuf,
    pub name: String,
    pub selected: Option<u64>,
    pub placing: Option<Shape>,
    /// Placing: how far the wheel has turned it (radians) and Shift + wheel sized it; and the last thing placed or
    /// picked, which Place (V) places again.
    pub place_turn: f32,
    pub place_scale: f32,
    pub last_placed: Option<(Shape, Option<String>)>,
    /// Pick (I) was pressed: take what is under the pointer (the brush's material, or a part to place).
    pub pick: bool,
    pub snap: bool,
    /// The performance meter's pill is showing (View > Performance meter).
    pub stats: bool,
    /// The bake cache (bake.rs), shared with the app as a resource.
    pub bakes: crate::bake::Bakes,
    /// The shaders in use as shader.rs reads them: their headers and any error in their files (kept up to date by
    /// `shader_sync`, for the material panel).
    pub shader_info: std::collections::BTreeMap<String, (Option<crate::shader::Header>, Option<String>)>,
    pub parts_open: bool,
    pub parts_query: String,
    /// The pop-up opened this frame (so the Tab that opened it doesn't also close it).
    pub parts_fresh: bool,
    /// The keyboard highlight in the pop-up (an index into the matching shapes).
    pub parts_cursor: usize,
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
    /// The sound file shown in the bottom panel instead of the selected part (clicked in Assets or the part's panel).
    pub sound_open: Option<String>,
    /// A sound file to hear once (its panel's Play), on a surface, taken by `edit::hear`.
    pub preview_sound: Option<(String, Option<crate::level::Ground>)>,
    /// The brush (B): out or not, how it paints, and what it is doing.
    pub brush_on: bool,
    pub brush: brush::Brush,
    pub painting: brush::Painting,
    /// The pen (P): out or not, and what it is doing.
    pub pen_on: bool,
    pub pen: pen::Pen,
    /// The bucket fill (G): out or not, how it fills, and what it is doing.
    pub bucket_on: bool,
    pub bucket: bucket::BucketTool,
    pub filling: bucket::Filling,
    /// The Assets panel's search, and its pictures.
    pub assets_query: String,
    pub thumbs: thumbs::Thumbs,
    /// The open tab (an index into `theme::TABS`; see `audio::built`).
    pub tab: usize,
    /// The Audio tab's stage.
    pub stage: audio::Stage,
    pub hover_handle: Option<edit::Grip>,
    /// The selected light's handle being dragged, and the one under the pointer.
    pub light_drag: Option<light::Grip>,
    pub light_hot: Option<light::Grip>,
    /// Looking through a camera part (L).
    pub looking: Option<camera::Looking>,
    pub camera_drag: bool,
    pub camera_hot: bool,
    /// The level's navigation area as last built (Build navigation), with what it was built from; how many builds.
    pub nav: Option<(bevy_rerecast::Navmesh, Vec<(level::Shape, Option<String>, [u32; 10])>)>,
    pub nav_builds: u32,
    /// The navigation area is drawn over the level.
    pub nav_shown: bool,
    /// Finds paths on the area as last built.
    pub finder: Option<crate::nav::Finder>,
    /// Trying a path on the area (Try a path).
    pub path_try: Option<PathTry>,
    /// The Level tab's topic (the chips in the top notch): the dock's settings and list follow it.
    pub level_mode: LevelMode,
    /// The dock on the view's right: open (or folded to its rail), and what it shows (Inspect, the list, Assets).
    pub dock_open: bool,
    pub dock_view: usize,
    /// The window is maximised (its own title bar's button, or a double-click on the bar).
    pub maximized: bool,
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
                ed.level = lvl;
                ed.sync_committed();
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
            checked: level.stamps(),
            followed: level.parts.stamp(),
            dirty_cache: (level.stamps(), false),
            level,
            undo: vec![],
            redo: vec![],
            dir,
            name,
            selected: None,
            placing: None,
            snap: true,
            stats: false,
            bakes: crate::bake::Bakes::new(crate::bake::Bakes::default_root(), None),
            shader_info: default(),
            parts_open: false,
            parts_query: String::new(),
            parts_fresh: false,
            parts_cursor: 0,
            assets: vec![],
            hovering_files: false,
            pending: None,
            island: default(),
            geo: default(),
            placing_model: None,
            place_turn: 0.0,
            place_scale: 1.0,
            last_placed: None,
            pick: false,
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
            sound_open: None,
            preview_sound: None,
            brush_on: false,
            brush: brush::Brush::default(),
            painting: brush::Painting::default(),
            pen_on: false,
            pen: pen::Pen::default(),
            bucket_on: false,
            bucket: bucket::BucketTool::default(),
            filling: bucket::Filling::default(),
            assets_query: String::new(),
            thumbs: thumbs::Thumbs::default(),
            tab: 0,
            stage: audio::Stage::default(),
            hover_handle: None,
            light_drag: None,
            light_hot: None,
            looking: None,
            camera_drag: false,
            camera_hot: false,
            nav: None,
            nav_builds: 0,
            nav_shown: true,
            finder: None,
            path_try: None,
            level_mode: LevelMode::Build,
            dock_open: true,
            dock_view: 0,
            maximized: false,
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

    /// Whether the level differs from the saved one. Looks at the stamps first; the level is compared with the saved one
    /// only when it was borrowed for change since (and `refresh_dirty` keeps that answer for the frames after).
    pub fn dirty(&self) -> bool {
        if self.level.name != self.saved.name { return true; }
        let now = self.level.stamps();
        if now == self.saved.stamps() { return false; }
        if now == self.dirty_cache.0 { return self.dirty_cache.1; }
        self.level != self.saved
    }

    /// Make the level the last undo step's (a new project, an opened one, or a change that joins the step before).
    pub fn sync_committed(&mut self) {
        self.committed = self.level.clone();
        self.checked = self.level.stamps();
    }

    /// Keep the answer of `dirty` for the next frames.
    pub fn refresh_dirty(&mut self) {
        let now = self.level.stamps();
        if now != self.dirty_cache.0 { self.dirty_cache = (now, now != self.saved.stamps() && self.level != self.saved); }
    }

    /// A window over the editor (the unsaved-changes question, Help's windows): the 3D view ignores the keys.
    pub fn dialog_open(&self) -> bool { self.pending.is_some() || self.about_open || self.updates_open || self.features_open || self.mcp_open }

    /// Work out the level's navigation area (Build navigation).
    pub fn build_navigation(&mut self) {
        match crate::nav::build(&self.level, &self.geo) {
            Ok(m) => {
                let (islands, area) = (crate::nav::islands(&m).len(), crate::nav::area(&m));
                self.finder = crate::nav::Finder::new(&m, &self.level.navigation).ok();
                self.nav = Some((m, crate::nav::key(&self.level)));
                self.nav_builds += 1;
                self.nav_shown = true;
                self.find_path();
                self.island.show("⬡", format!("Navigation built: {area:.0} m²"), format!("{islands} separate area{}", if islands == 1 { "" } else { "s" }), theme::Tone::Accent);
            }
            Err(e) => self.island.show("!", "Couldn't build the navigation", e, theme::Tone::Danger),
        }
    }
    /// The way between the tried path's start and end, once both are picked.
    pub fn find_path(&mut self) {
        let Some(t) = self.path_try.as_mut() else { return };
        t.found = match (t.from, t.to, &self.finder) {
            (Some(a), Some(b), Some(f)) => Some(f.path(a, b)),
            _ => None,
        };
    }
    /// The navigation area was built from a level that has changed since (parts moved, settings changed).
    pub fn nav_out_of_date(&self) -> bool { self.nav.as_ref().is_some_and(|(_, k)| *k != crate::nav::key(&self.level)) }

    pub fn refresh_assets(&mut self) {
        self.assets = level::ASSET_FOLDERS.iter().map(|f| (*f, level::list_assets(&self.dir, f))).collect();
        self.thumbs.clear();
    }
}

/// Keeps `ed.shader_info` current, asks for the open material's shader (so its settings show) and says when a shader
/// file stops parsing. Writes to the editor only when something changed.
fn shader_sync(mut ed: ResMut<Editor>, mut shaders: ResMut<crate::shader::Shaders>) {
    if let Some(path) = ed.material_open.as_ref().and_then(|m| ed.level.materials.get(m)).and_then(|m| m.shader.clone()) { shaders.request(&path); }
    let info: std::collections::BTreeMap<_, _> = shaders.paths().map(|p| (p.clone(), (shaders.header(p).cloned(), shaders.error(p)))).collect();
    if info == ed.shader_info { return; }
    for (p, (_, err)) in &info {
        if let (Some(e), true) = (err, ed.shader_info.get(p).is_none_or(|old| old.1.as_ref() != err.as_ref())) {
            ed.island.show("!", format!("Shader {} has a problem", p.trim_start_matches("assets/shaders/")), e.clone(), theme::Tone::Danger);
        }
    }
    ed.shader_info = info;
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
    let render = crate::gpu::render_plugin(&mut app);
    app.add_plugins(DefaultPlugins.set(render).set(WindowPlugin {
            primary_window: Some(Window { title: title.into(), resolution: (1600, 960).into(), decorations: false, ..default() }),
            close_when_requested: false,
            ..default()
        }));
    extra(&mut app);
    app.add_plugins((crate::bake::BakePlugin(ed.bakes.clone()), EguiPlugin::default(), vector::VectorPlugin, text::TextPlugin, play::PlayPlugin, crate::paint::PaintPlugin, crate::grass::GrassPlugin, crate::perf::PerfPlugin, crate::shader::ShaderPlugin, view::grid_plugin))
        .init_resource::<models::ModelLib>()
        .insert_resource(preview::Previews::new(ed.thumbs.asked.clone()))
        .insert_resource(ClearColor(Color::srgb(0.045, 0.04, 0.055)))
        .insert_resource(GlobalAmbientLight { color: Color::srgb(0.85, 0.88, 1.0), brightness: 500.0, ..default() })
        .init_resource::<caps::CapsLock>()
        .init_resource::<crate::sound::Sounds>()
        .insert_resource(ed)
        .init_gizmo_group::<edit::WidgetGizmos>()
        .init_gizmo_group::<edit::OutlineGizmos>()
        .add_systems(Startup, (shapes::setup, view::setup, view::spawn_grid, edit::setup, caps::startup))
        .init_resource::<mcp::host::Injected>()
        .add_systems(PreUpdate, mcp::host::inject.after(bevy::input::InputSystems))
        .add_systems(Update, (window_icon, mcp::host::serve, models::fill_models, shader_sync, preview::run))
        .add_systems(
            Update,
            (
                edit::project_follow,
                models::build_geometry,
                edit::models_in,
                view::input,
                camera::lens,
                edit::test_mode.run_if(audio::in_level),
                edit::interact.run_if(audio::in_level),
                edit::shortcuts,
                edit::files,
                (edit::follow_parents, edit::sync, edit::paint, edit::grass).chain(),
                (audio::follow_tab, audio::stage).chain(),
                edit::hear,
                edit::stage,
                edit::nav_overlay.run_if(audio::in_level),
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
