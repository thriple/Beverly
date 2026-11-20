//! The scenes the benchmark draws, each a function that puts one thing into the world: a field of dense meshes,
//! lights with and without shadows, headlights driving through a lit town, and a pile of big textures. The stress level
//! in the editor builds from the same functions.
use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::ecs::system::SystemParam;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Everything a scene makes carries this, so the next scene can clear the last.
#[derive(Component)]
pub struct Bench;

/// A car's headlight: drives a circle of `radius` metres at `speed` rad/s, starting `phase` round.
#[derive(Component)]
pub struct Car { pub radius: f32, pub speed: f32, pub phase: f32 }

/// What a scene is made with.
#[derive(SystemParam)]
pub struct Stage<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub mats: ResMut<'w, Assets<StandardMaterial>>,
    pub images: ResMut<'w, Assets<Image>>,
}

/// A small, repeatable random number (0 to 1) for placing things the same way every run.
pub fn rand(i: usize, k: usize) -> f32 {
    let mut x = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (k as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    ((x >> 40) as f32) / ((1u64 << 24) as f32)
}

/// Triangles in one tile of the polygon field.
pub const TILE_TRIANGLES: usize = 2 * 223 * 223;

/// A wavy square tile, 2 m across, of `TILE_TRIANGLES` triangles.
fn tile() -> Mesh {
    let n = 223usize;
    let (mut pos, mut nor, mut uv) = (vec![], vec![], vec![]);
    for z in 0..=n {
        for x in 0..=n {
            let (u, v) = (x as f32 / n as f32, z as f32 / n as f32);
            let h = ((u * 40.0).sin() * (v * 31.0).cos()) * 0.04;
            pos.push([u * 2.0 - 1.0, h, v * 2.0 - 1.0]);
            nor.push([0.0, 1.0, 0.0]);
            uv.push([u * 8.0, v * 8.0]);
        }
    }
    let mut idx = vec![];
    for z in 0..n {
        for x in 0..n {
            let a = (z * (n + 1) + x) as u32;
            let b = a + 1;
            let c = a + (n + 1) as u32;
            idx.extend([a, c, b, b, c, c + 1]);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(idx))
}

fn colour(i: usize) -> StandardMaterial {
    StandardMaterial { base_color: Color::hsl(rand(i, 7) * 360.0, 0.5, 0.55), perceptual_roughness: 0.7, ..default() }
}

/// The camera that draws into an image of the given size.
pub fn camera(s: &mut Stage, size: (u32, u32), at: Vec3, look: Vec3) {
    let image = s.images.add(Image::new_target_texture(size.0, size.1, TextureFormat::Rgba8UnormSrgb, None));
    s.commands.spawn((Bench, Camera3d::default(), RenderTarget::Image(image.into()), Msaa::Off, Transform::from_translation(at).looking_at(look, Vec3::Y)));
}

/// `tiles` dense tiles in a block facing the camera: `tiles * TILE_TRIANGLES` triangles, all in view, unshadowed.
pub fn polygons(s: &mut Stage, size: (u32, u32), tiles: usize) {
    let mesh = s.meshes.add(tile());
    let side = (tiles as f32).sqrt().ceil() as usize;
    let mat = s.mats.add(colour(1));
    for i in 0..tiles {
        let (x, z) = ((i % side) as f32, (i / side) as f32);
        s.commands.spawn((Bench, Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), Transform::from_xyz((x - side as f32 / 2.0) * 2.1, 0.0, (z - side as f32 / 2.0) * 2.1).with_rotation(Quat::from_rotation_y(rand(i, 3) * 6.28))));
    }
    s.commands.spawn((Bench, DirectionalLight { shadow_maps_enabled: false, illuminance: 9000.0, ..default() }, Transform::default().looking_to(Vec3::new(-0.4, -1.0, -0.3), Vec3::Y)));
    let d = side as f32 * 2.1;
    camera(s, size, Vec3::new(0.0, d * 0.9, d * 1.1), Vec3::ZERO);
}

/// A square of ground with `props` boxes standing on it (they cast shadows); the camera looks at its middle.
fn town(s: &mut Stage, props: usize, half: f32) {
    let ground = s.meshes.add(Plane3d::default().mesh().size(half * 2.0, half * 2.0));
    let g = s.mats.add(StandardMaterial { base_color: Color::srgb(0.3, 0.32, 0.3), perceptual_roughness: 0.9, ..default() });
    s.commands.spawn((Bench, Mesh3d(ground), MeshMaterial3d(g), Transform::default()));
    let cube = s.meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let mats: Vec<_> = (0..8).map(|i| s.mats.add(colour(i))).collect();
    for i in 0..props {
        let (w, h) = (1.0 + rand(i, 1) * 3.0, 1.0 + rand(i, 2) * 6.0);
        s.commands.spawn((Bench, Mesh3d(cube.clone()), MeshMaterial3d(mats[i % 8].clone()),
            Transform::from_xyz((rand(i, 3) - 0.5) * half * 2.0, h / 2.0, (rand(i, 4) - 0.5) * half * 2.0).with_scale(Vec3::new(w, h, w))));
    }
}

/// `n` point lights over a town of 600 props, none casting shadows.
pub fn lights(s: &mut Stage, size: (u32, u32), n: usize) {
    town(s, 600, 60.0);
    for i in 0..n {
        s.commands.spawn((Bench, PointLight { intensity: 400_000.0, range: 14.0, shadow_maps_enabled: false, color: Color::hsl(rand(i, 5) * 360.0, 0.6, 0.7), ..default() },
            Transform::from_xyz((rand(i, 8) - 0.5) * 110.0, 4.0 + rand(i, 9) * 3.0, (rand(i, 9) - 0.5) * 110.0)));
    }
    camera(s, size, Vec3::new(0.0, 45.0, 75.0), Vec3::new(0.0, 0.0, 0.0));
}

/// `n` point lights that all cast shadows, overlapping in the middle of a town of 600 props.
pub fn shadowed(s: &mut Stage, size: (u32, u32), n: usize) {
    town(s, 600, 40.0);
    for i in 0..n {
        s.commands.spawn((Bench, PointLight { intensity: 1_500_000.0, range: 30.0, shadow_maps_enabled: true, color: Color::hsl(rand(i, 5) * 360.0, 0.4, 0.8), ..default() },
            Transform::from_xyz((rand(i, 8) - 0.5) * 24.0, 5.0 + rand(i, 9) * 3.0, (rand(i, 9) - 0.5) * 24.0)));
    }
    camera(s, size, Vec3::new(0.0, 30.0, 45.0), Vec3::ZERO);
}

/// A lit night town (300 unshadowed lights, 600 props) with `cars` driving round it, each with a shadowed headlight.
pub fn headlights(s: &mut Stage, size: (u32, u32), cars: usize) {
    town(s, 600, 60.0);
    for i in 0..300 {
        s.commands.spawn((Bench, PointLight { intensity: 250_000.0, range: 12.0, shadow_maps_enabled: false, color: Color::srgb(1.0, 0.8, 0.55), ..default() },
            Transform::from_xyz((rand(i, 11) - 0.5) * 110.0, 5.0, (rand(i, 12) - 0.5) * 110.0)));
    }
    let body = s.meshes.add(Cuboid::new(1.8, 1.2, 4.0));
    let paint = s.mats.add(StandardMaterial { base_color: Color::srgb(0.7, 0.1, 0.1), ..default() });
    for i in 0..cars {
        let car = Car { radius: 12.0 + i as f32 * 6.0, speed: 0.5 / (1.0 + i as f32 * 0.2), phase: i as f32 * 1.7 };
        s.commands.spawn((Bench, car, Mesh3d(body.clone()), MeshMaterial3d(paint.clone()), Transform::default())).with_children(|c| {
            c.spawn((SpotLight { intensity: 20_000_000.0, range: 40.0, outer_angle: 0.5, inner_angle: 0.3, shadow_maps_enabled: true, ..default() }, Transform::from_xyz(0.0, 0.2, -2.2).looking_to(Vec3::NEG_Z, Vec3::Y)));
        });
    }
    camera(s, size, Vec3::new(0.0, 40.0, 60.0), Vec3::ZERO);
}

/// Drives the cars round (every frame, the same way each run).
pub fn drive(time: Res<Time<Virtual>>, mut cars: Query<(&Car, &mut Transform)>) {
    for (c, mut t) in &mut cars {
        let a = c.phase + time.elapsed_secs() * c.speed;
        t.translation = Vec3::new(a.cos() * c.radius, 0.7, a.sin() * c.radius);
        t.rotation = Quat::from_rotation_y(std::f32::consts::PI - a);
    }
}

/// How many 2048 x 2048 textures (16 MB each) make a gigabyte.
pub const PER_GB: usize = 64;

/// One 16 MB texture, shown on a quad in a grid so it has to stay in video memory. Number `i`.
pub fn texture(s: &mut Stage, i: usize) {
    let data = vec![(i % 251) as u8; 2048 * 2048 * 4];
    let image = Image::new(Extent3d { width: 2048, height: 2048, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD);
    let handle = s.images.add(image);
    let mat = s.mats.add(StandardMaterial { base_color_texture: Some(handle), unlit: true, ..default() });
    let mesh = s.meshes.add(Plane3d::default().mesh().size(0.9, 0.9));
    let (x, z) = ((i % 32) as f32, (i / 32) as f32);
    s.commands.spawn((Bench, Mesh3d(mesh), MeshMaterial3d(mat), Transform::from_xyz(x - 16.0, 0.0, z - 8.0)));
}

/// The camera over the texture grid.
pub fn texture_view(s: &mut Stage, size: (u32, u32)) { camera(s, size, Vec3::new(0.0, 28.0, 0.1), Vec3::ZERO); }
