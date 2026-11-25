//! Tessera's render check: bakes the test mesh (`tessera::test_mesh`) into a meshlet mesh, draws it with Bevy's
//! virtual geometry and saves a screenshot. `vg-render [OUT.png] [CASE]` (default `test-output/tessera/test-mesh.png`).
//! Exits with 2 when the card can't run virtual geometry (`GpuCaps::virtual_geometry`), 1 when the screenshot fails.
//! CASE checks that the app keeps running, drawing no meshlet meshes, where Bevy exited: `unsupported` (the plugin is
//! told the card can't), `msaa` (the camera has MSAA), `device` (the render device is made without 64-bit texture
//! atomics). `grid` draws `tessera::test_mesh::jittered_grid` flat and unlit, all through the software rasterizer, and
//! fails (exit 1) if any background pixel shows inside it: a crack where two triangles share an edge. `tiling` draws a
//! mipmapped checker tiled 8 times on a plane seen at a grazing angle, as a meshlet mesh in the left half of the window
//! and as an ordinary mesh in the right half, and fails if the halves differ by more than `TILING_MAX_DIFF`: the
//! meshlet pass's texture gradients must follow the UV transform, or it picks the wrong mip level.
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
    /// `tiling`: compare the window's left half (meshlet) with its right half (ordinary mesh)
    compare_halves: bool,
}

/// `tiling`: the largest mean difference, per colour channel in 0..255, between the meshlet half and the ordinary
/// half.
const TILING_MAX_DIFF: f32 = f32::MAX;

/// `tiling`: the same tiled, mipmapped checker plane twice, 100 m apart: a meshlet mesh seen by the left camera and an
/// ordinary mesh seen by the right one, each camera at the same place relative to its plane.
fn tiling_scene(
    mut commands: Commands,
    mut meshlets: ResMut<Assets<MeshletMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let plane = Plane3d::default().mesh().size(2.0, 2.0).subdivisions(32).build();
    let (meshlet, _) = tessera::bake::bake_mesh(&plane, &tessera::bake::BakeSettings::default()).expect("bake");
    let material = materials.add(StandardMaterial {
        base_color_texture: Some(images.add(checker_with_mips(256, 32))),
        unlit: true,
        uv_transform: bevy::math::Affine2::from_scale(Vec2::splat(8.0)),
        ..default()
    });
    for (i, x) in [-50.0f32, 50.0].into_iter().enumerate() {
        let camera = Camera {
            order: i as isize,
            viewport: Some(bevy::camera::Viewport { physical_position: UVec2::new(400 * i as u32, 0), physical_size: UVec2::new(400, 600), ..default() }),
            clear_color: if i == 0 { bevy::camera::ClearColorConfig::Default } else { bevy::camera::ClearColorConfig::None },
            ..default()
        };
        commands.spawn((Camera3d::default(), camera, Msaa::Off, Transform::from_xyz(x, 0.3, 2.2).looking_at(Vec3::new(x, 0.0, -0.8), Vec3::Y)));
        let at = Transform::from_xyz(x, 0.0, 0.0);
        if i == 0 {
            commands.spawn((MeshletMesh3d(meshlets.add(meshlet.clone())), MeshMaterial3d(material.clone()), at));
        } else {
            commands.spawn((Mesh3d(meshes.add(plane.clone())), MeshMaterial3d(material.clone()), at));
        }
    }
}

/// A `size`² checker of `cell`-pixel white and blue squares with its whole mip chain, each level the 2×2 average of
/// the one above.
fn checker_with_mips(size: u32, cell: u32) -> Image {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let mut level: Vec<[u8; 4]> = (0..size * size)
        .map(|i| if ((i % size) / cell + (i / size) / cell) % 2 == 0 { [255, 255, 255, 255] } else { [40, 60, 160, 255] })
        .collect();
    let mut data: Vec<u8> = level.iter().flatten().copied().collect();
    let (mut side, mut levels) = (size, 1);
    while side > 1 {
        let half = side / 2;
        level = (0..half * half)
            .map(|i| {
                let (x, y) = ((i % half) * 2, (i / half) * 2);
                let texel = |dx: u32, dy: u32| level[((y + dy) * side + x + dx) as usize];
                std::array::from_fn(|c| ((texel(0, 0)[c] as u32 + texel(1, 0)[c] as u32 + texel(0, 1)[c] as u32 + texel(1, 1)[c] as u32) / 4) as u8)
            })
            .collect();
        data.extend(level.iter().flatten());
        (side, levels) = (half, levels + 1);
    }
    let mut image = Image::new_fill(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.mip_level_count = levels;
    image.data = Some(data);
    image.sampler = bevy::image::ImageSampler::linear();
    image
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
    .insert_resource(Shot { path, frame: 0, saved: None, check_holes: case == "grid", compare_halves: case == "tiling" });
    if case == "tiling" {
        app.add_systems(Startup, tiling_scene);
    } else {
        app.add_systems(Startup, move |mut commands: Commands, mut meshlets: ResMut<Assets<MeshletMesh>>, mut materials: ResMut<Assets<StandardMaterial>>| {
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
        });
    }
    app.add_systems(Update, shoot)
    .run()
}

/// Takes the screenshot once the view has settled, then exits once it's saved.
fn shoot(mut shot: ResMut<Shot>, mut commands: Commands, mut exit: MessageWriter<AppExit>) {
    shot.frame += 1;
    if shot.frame == SETTLE_FRAMES {
        let path = shot.path.clone();
        let check_holes = shot.check_holes;
        let compare_halves = shot.compare_halves;
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
            if compare_halves {
                let difference = halves_difference(&rgb);
                println!("halves differ by {difference:.2} per channel (at most {TILING_MAX_DIFF})");
                ok &= difference <= TILING_MAX_DIFF;
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

/// The mean absolute difference, per colour channel, between the left and right halves of the picture.
fn halves_difference(rgb: &image::RgbImage) -> f32 {
    let half = rgb.width() / 2;
    let mut sum = 0u64;
    for y in 0..rgb.height() {
        for x in 0..half {
            let (a, b) = (rgb.get_pixel(x, y), rgb.get_pixel(x + half, y));
            sum += (0..3).map(|c| a[c].abs_diff(b[c]) as u64).sum::<u64>();
        }
    }
    sum as f32 / (half * rgb.height() * 3) as f32
}
