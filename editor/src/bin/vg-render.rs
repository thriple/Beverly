//! Tessera's render check: bakes the test mesh (`tessera::test_mesh`) into a meshlet mesh, draws it with Bevy's
//! virtual geometry and saves a screenshot. `vg-render [OUT.png]` (default `test-output/tessera/test-mesh.png`).
//! Exits with 2 when the card can't run virtual geometry (`GpuCaps::virtual_geometry`), 1 when the screenshot fails.
use beverly::gpu::GpuCaps;
use bevy::pbr::experimental::meshlet::{
    MeshletMesh, MeshletMesh3d, MeshletPlugin, MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR,
};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use std::path::PathBuf;
use std::time::Instant;

/// Frames drawn before the screenshot: the pipelines compile in the background first (on lavapipe 30 frames
/// were still black), and the culling needs last frame's depth.
const SETTLE_FRAMES: u32 = 300;

#[derive(Resource)]
struct Shot {
    path: PathBuf,
    frame: u32,
    saved: Option<bool>,
}

fn main() -> AppExit {
    let path = PathBuf::from(std::env::args().nth(1).unwrap_or("test-output/tessera/test-mesh.png".into()));
    let mut app = App::new();
    let render = beverly::gpu::render_plugin(&mut app);
    let caps = app.world().resource::<GpuCaps>().clone();
    println!("Graphics card: {} ({}, {}), virtual geometry: {}", caps.name, caps.backend, caps.kind, caps.virtual_geometry());
    if !caps.virtual_geometry() {
        println!("missing: {:?}", beverly::gpu::VIRTUAL_GEOMETRY.difference(caps.features));
        return AppExit::from_code(2);
    }

    app.add_plugins((
        DefaultPlugins.set(render).set(WindowPlugin {
            primary_window: Some(Window { title: "vg-render".into(), resolution: (800, 600).into(), ..default() }),
            ..default()
        }),
        MeshletPlugin { cluster_buffer_slots: 1 << 16 },
    ))
    .insert_resource(Shot { path, frame: 0, saved: None })
    .add_systems(Startup, |mut commands: Commands, mut meshlets: ResMut<Assets<MeshletMesh>>, mut materials: ResMut<Assets<StandardMaterial>>| {
        // baked here, not before the app: the bake runs on Bevy's task pools
        let start = Instant::now();
        let mesh = tessera::test_mesh::test_mesh(8, 1);
        let triangles = mesh.indices().map_or(0, |i| i.len() / 3);
        let made = start.elapsed();
        let meshlet = MeshletMesh::from_mesh(&mesh, MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR).expect("bake");
        println!("test mesh: {triangles} triangles, made in {made:.2?}, baked in {:.2?}", start.elapsed() - made);
        commands.spawn((Camera3d::default(), Msaa::Off, Transform::from_xyz(0.0, 0.6, 2.8).looking_at(Vec3::ZERO, Vec3::Y)));
        commands.spawn((DirectionalLight { illuminance: 8000.0, shadow_maps_enabled: true, ..default() }, Transform::from_xyz(1.0, 2.0, 1.5).looking_at(Vec3::ZERO, Vec3::Y)));
        commands.spawn((
            MeshletMesh3d(meshlets.add(meshlet)),
            MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.8, 0.55, 0.35), perceptual_roughness: 0.7, ..default() })),
        ));
    })
    .add_systems(Update, shoot)
    .run()
}

/// Takes the screenshot once the view has settled, then exits once it's saved.
fn shoot(mut shot: ResMut<Shot>, mut commands: Commands, mut exit: MessageWriter<AppExit>) {
    shot.frame += 1;
    if shot.frame == SETTLE_FRAMES {
        let path = shot.path.clone();
        commands.spawn(Screenshot::primary_window()).observe(move |ev: On<ScreenshotCaptured>, mut shot: ResMut<Shot>| {
            let ok = (|| {
                std::fs::create_dir_all(path.parent()?).ok()?;
                ev.image.clone().try_into_dynamic().ok()?.to_rgb8().save(&path).ok()
            })()
            .is_some();
            println!("screenshot {}: {}", path.display(), if ok { "saved" } else { "failed" });
            shot.saved = Some(ok);
        });
    }
    match shot.saved {
        Some(true) => _ = exit.write(AppExit::Success),
        Some(false) => _ = exit.write(AppExit::from_code(1)),
        None if shot.frame > SETTLE_FRAMES + 120 => _ = exit.write(AppExit::from_code(1)),
        None => {}
    }
}
