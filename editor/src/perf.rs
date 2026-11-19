//! The performance meter's numbers: `Perf`, refreshed every frame from Bevy's diagnostics. The editor's stats pill,
//! the MCP server's `get_project` and the scripted tests' reports all read it.
use bevy::diagnostic::{DiagnosticPath, DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::diagnostic::RenderDiagnosticsPlugin;

pub struct PerfPlugin;

impl Plugin for PerfPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Perf>()
            .add_plugins((FrameTimeDiagnosticsPlugin::default(), EntityCountDiagnosticsPlugin::default(), SystemInformationDiagnosticsPlugin, RenderDiagnosticsPlugin))
            .add_systems(Last, measure);
    }
}

#[derive(Resource, Default, Clone, Debug)]
pub struct Perf {
    pub fps: f64,
    /// Time between frames (ms)
    pub frame_ms: f64,
    /// The card's time on the last frame's passes (ms); None when the card or driver can't time them
    pub gpu_ms: Option<f64>,
    /// Triangles the card drew in the last frame, shadow passes included; None without pipeline statistics
    pub triangles: Option<f64>,
    /// Meshes in view (each is a draw before Bevy batches them)
    pub meshes: usize,
    pub entities: usize,
    pub lights: usize,
    /// This program's memory (GB)
    pub ram_gb: Option<f64>,
}

/// How a number reads: fine, worth a look, too much. The limits are a 60 fps game on a mid-range card.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grade { Green, Amber, Red }

impl Grade {
    fn of(v: f64, amber: f64, red: f64) -> Grade { if v >= red { Grade::Red } else if v >= amber { Grade::Amber } else { Grade::Green } }
    pub fn name(self) -> &'static str { match self { Grade::Green => "green", Grade::Amber => "amber", Grade::Red => "red" } }
}

impl Perf {
    /// For the MCP server's get_project: each number with its grade (green, amber or red); a number the card can't
    /// give is null.
    pub fn json(&self) -> serde_json::Value {
        let g = |g: Option<Grade>| g.map(Grade::name);
        serde_json::json!({
            "fps": self.fps.round(), "frame_ms": self.frame_ms, "frame": self.frame_grade().name(),
            "card_ms": self.gpu_ms, "card": g(self.gpu_grade()),
            "triangles": self.triangles, "triangles_grade": g(self.triangles_grade()),
            "meshes": self.meshes, "meshes_grade": self.meshes_grade().name(),
            "lights": self.lights, "lights_grade": self.lights_grade().name(),
            "entities": self.entities,
            "memory_gb": self.ram_gb, "memory": g(self.ram_grade()),
        })
    }

    /// One line for a scripted test's report.
    pub fn line(&self) -> String {
        let opt = |v: Option<f64>, unit: &str| v.map_or("n/a".to_string(), |v| format!("{v:.1}{unit}"));
        format!("perf: {:.0} fps ({:.1} ms), card {}, triangles {}, {} meshes, {} lights, {} entities, memory {}",
            self.fps, self.frame_ms, opt(self.gpu_ms, " ms"), opt(self.triangles, ""), self.meshes, self.lights, self.entities, opt(self.ram_gb, " GB"))
    }

    pub fn frame_grade(&self) -> Grade { Grade::of(self.frame_ms, 18.0, 33.0) }
    pub fn gpu_grade(&self) -> Option<Grade> { self.gpu_ms.map(|v| Grade::of(v, 12.0, 20.0)) }
    pub fn triangles_grade(&self) -> Option<Grade> { self.triangles.map(|v| Grade::of(v, 2e6, 6e6)) }
    pub fn meshes_grade(&self) -> Grade { Grade::of(self.meshes as f64, 1500.0, 4000.0) }
    pub fn lights_grade(&self) -> Grade { Grade::of(self.lights as f64, 64.0, 200.0) }
    pub fn ram_grade(&self) -> Option<Grade> { self.ram_gb.map(|v| Grade::of(v, 4.0, 8.0)) }
}

fn measure(
    store: Res<DiagnosticsStore>,
    mut perf: ResMut<Perf>,
    meshes: Query<&ViewVisibility, With<Mesh3d>>,
    lights: Query<(), Or<(With<PointLight>, With<SpotLight>, With<DirectionalLight>)>>,
) {
    let get = |path: &DiagnosticPath| store.get(path).and_then(|d| d.smoothed());
    perf.fps = get(&FrameTimeDiagnosticsPlugin::FPS).unwrap_or(0.0);
    perf.frame_ms = get(&FrameTimeDiagnosticsPlugin::FRAME_TIME).unwrap_or(0.0);
    perf.entities = get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT).unwrap_or(0.0) as usize;
    perf.ram_gb = get(&SystemInformationDiagnosticsPlugin::PROCESS_MEM_USAGE);
    perf.meshes = meshes.iter().filter(|v| v.get()).count();
    perf.lights = lights.iter().count();
    // the render passes are spans named render/PASS/FIELD; the card's time and triangles are their sums (a card that
    // can't count triangles, such as a software renderer, says 0)
    let sum = |field: &str| {
        let mut found = None;
        for d in store.iter() {
            let mut parts = d.path().components();
            if parts.next() == Some("render") && d.path().components().last() == Some(field) {
                if let Some(v) = d.smoothed() { *found.get_or_insert(0.0) += v; }
            }
        }
        found
    };
    perf.gpu_ms = sum("elapsed_gpu");
    perf.triangles = sum("clipper_primitives_out").filter(|t| *t > 0.0);
}
