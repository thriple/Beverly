//! Tessera's render check: bakes the test mesh (`tessera::test_mesh`) into a meshlet mesh, draws it with Bevy's
//! virtual geometry and saves a screenshot. `vg-render [OUT.png] [CASE]` (default `test-output/tessera/test-mesh.png`).
//! Exits with 2 when the card can't run virtual geometry (`GpuCaps::virtual_geometry`), 1 when the screenshot fails.
//! CASE checks that the app keeps running, drawing no meshlet meshes, where Bevy exited: `unsupported` (the plugin is
//! told the card can't), `msaa` (the camera has MSAA), `device` (the render device is made without 64-bit texture
//! atomics). `grid` draws `tessera::test_mesh::jittered_grid` flat and unlit, all through the software rasterizer, and
//! fails (exit 1) if any background pixel shows inside it: a crack where two triangles share an edge.
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
    /// `grid`: count background pixels inside the drawn square
    check_holes: bool,
}

fn main() -> AppExit {
    let path = PathBuf::from(std::env::args().nth(1).unwrap_or("test-output/tessera/test-mesh.png".into()));
    let case = std::env::args().nth(2).unwrap_or_default();
    let mut app = App::new();
    let mut render = beverly::gpu::render_plugin(&mut app);
    if case == "device" {
        if let bevy::render::settings::RenderCreation::Automatic(settings) = &mut render.render_creation {
            settings.disabled_features = Some(bevy::render::settings::WgpuFeatures::TEXTURE_INT64_ATOMIC);
        }
    }
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
        MeshletPlugin { cluster_buffer_slots: 1 << 16, supported: caps.virtual_geometry() && case != "unsupported" },
    ))
    .insert_resource(Shot { path, frame: 0, saved: None, check_holes: case == "grid" })
    .add_systems(Startup, move |mut commands: Commands, mut meshlets: ResMut<Assets<MeshletMesh>>, mut materials: ResMut<Assets<StandardMaterial>>| {
        // baked here, not before the app: the bake runs on Bevy's task pools
        let start = Instant::now();
        let grid = case == "grid";
        let mesh = if grid { tessera::test_mesh::jittered_grid(160, 1) } else { tessera::test_mesh::test_mesh(8, 1) };
        let triangles = mesh.indices().map_or(0, |i| i.len() / 3);
        let made = start.elapsed();
        let meshlet = MeshletMesh::from_mesh(&mesh, MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR).expect("bake");
        println!("test mesh: {triangles} triangles, made in {made:.2?}, baked in {:.2?}", start.elapsed() - made);
        let msaa = if case == "msaa" { Msaa::Sample4 } else { Msaa::Off };
        if grid {
            commands.spawn((Camera3d::default(), Msaa::Off, Transform::from_xyz(0.0, 0.0, 3.2).looking_at(Vec3::ZERO, Vec3::Y)));
            commands.spawn((
                MeshletMesh3d(meshlets.add(meshlet)),
                MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, ..default() })),
            ));
            return;
        }
        commands.spawn((Camera3d::default(), msaa, Transform::from_xyz(0.0, 0.6, 2.8).looking_at(Vec3::ZERO, Vec3::Y)));
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
        let check_holes = shot.check_holes;
        commands.spawn(Screenshot::primary_window()).observe(move |ev: On<ScreenshotCaptured>, mut shot: ResMut<Shot>| {
            let Some(rgb) = ev.image.clone().try_into_dynamic().ok().map(|i| i.to_rgb8()) else {
                println!("screenshot {}: failed", path.display());
                shot.saved = Some(false);
                return;
            };
            let mut ok = std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new("."))).is_ok() && rgb.save(&path).is_ok();
            println!("screenshot {}: {}", path.display(), if ok { "saved" } else { "failed" });
            if check_holes {
                let holes = holes_inside(&rgb);
                println!("grid holes: {holes:?}");
                ok &= holes == Some(0);
            }
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

/// Background-coloured pixels inside the bounding box of everything else, 2 pixels in from its border; None when
/// nothing was drawn.
fn holes_inside(rgb: &image::RgbImage) -> Option<usize> {
    let background = *rgb.get_pixel(0, 0);
    let drawn: Vec<(u32, u32)> = rgb.enumerate_pixels().filter(|(_, _, p)| **p != background).map(|(x, y, _)| (x, y)).collect();
    let (x0, x1) = (drawn.iter().map(|d| d.0).min()? + 2, drawn.iter().map(|d| d.0).max()?.checked_sub(2)?);
    let (y0, y1) = (drawn.iter().map(|d| d.1).min()? + 2, drawn.iter().map(|d| d.1).max()?.checked_sub(2)?);
    Some((y0..=y1).flat_map(|y| (x0..=x1).map(move |x| (x, y))).filter(|&(x, y)| *rgb.get_pixel(x, y) == background).count())
}
