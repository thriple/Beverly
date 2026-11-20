//! Tessera's instance benchmark: `vg-bench COUNT` places COUNT copies of a small baked test mesh on a grid, with a
//! camera and a shadowed sun (so five views: the camera and four cascades), runs headless and prints the CPU time the
//! meshlet systems take per frame (`MeshletCpuTimes`): with nothing moving, with 1% of the copies moving, and with
//! 1% replaced each frame (despawned, and spawned anew with another material).
use bevy::camera::RenderTarget;
use bevy::pbr::experimental::meshlet::{
    MeshletCpuTimes, MeshletMesh, MeshletMesh3d, MeshletPlugin, MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR,
};
use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::render::render_resource::TextureFormat;
use bevy::render::RenderApp;
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
use std::time::{Duration, Instant};

const WARMUP: u32 = 20;
const MEASURED: u32 = 60;

/// The copies that move in the second half (every 100th).
#[derive(Component)]
struct Mover(Vec3);

/// What the copies do: 0 nothing, 1 the movers move, 2 the movers are replaced each frame.
#[derive(Resource)]
struct Phase(u8);

/// The mesh and the two materials, for respawning.
#[derive(Resource)]
struct Parts(Handle<MeshletMesh>, [Handle<StandardMaterial>; 2]);

fn main() -> AppExit {
    let count: u32 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(10_000);
    let mut app = App::new();
    let render = beverly::gpu::render_plugin(&mut app);
    let caps = app.world().resource::<beverly::gpu::GpuCaps>().clone();
    if !caps.virtual_geometry() {
        println!("{}: no virtual geometry", caps.name);
        return AppExit::from_code(2);
    }
    app.add_plugins((
        DefaultPlugins
            .set(render)
            .set(WindowPlugin { primary_window: None, exit_condition: ExitCondition::DontExit, ..default() })
            .disable::<WinitPlugin>()
            // the render world's resources are read straight after each update
            .disable::<PipelinedRenderingPlugin>(),
        MeshletPlugin { cluster_buffer_slots: 1 << 22, supported: caps.virtual_geometry() },
    ))
    .insert_resource(Phase(0))
    .add_systems(Startup, move |mut commands: Commands, mut meshlets: ResMut<Assets<MeshletMesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>| {
        let mesh = tessera::test_mesh::test_mesh(3, 1);
        let mesh = meshlets.add(MeshletMesh::from_mesh(&mesh, MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR).expect("bake"));
        let material = materials.add(StandardMaterial::default());
        commands.insert_resource(Parts(mesh.clone(), [material.clone(), materials.add(StandardMaterial { base_color: Color::srgb(0.2, 0.4, 0.9), ..default() })]));
        let target = images.add(Image::new_target_texture(256, 256, TextureFormat::Rgba8UnormSrgb, None));
        let side = (count as f32).sqrt().ceil() as u32;
        let half = side as f32 * 1.5;
        commands.spawn((Camera3d::default(), RenderTarget::Image(target.into()), Msaa::Off, Transform::from_xyz(-half, 20.0, -half).looking_at(Vec3::ZERO, Vec3::Y)));
        commands.spawn((DirectionalLight { shadow_maps_enabled: true, ..default() }, Transform::from_xyz(1.0, 2.0, 1.5).looking_at(Vec3::ZERO, Vec3::Y)));
        for i in 0..count {
            let at = Vec3::new((i % side) as f32 * 3.0 - half, 0.0, (i / side) as f32 * 3.0 - half);
            let mut e = commands.spawn((MeshletMesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::from_translation(at)));
            if i % 100 == 0 {
                e.insert(Mover(at));
            }
        }
    })
    .add_systems(Update, |phase: Res<Phase>, time: Res<Time>, parts: Res<Parts>, mut flip: Local<usize>, mut commands: Commands, mut movers: Query<(Entity, &Mover, &mut Transform)>| {
        *flip += 1;
        for (e, m, mut t) in &mut movers {
            match phase.0 {
                1 => t.translation = m.0 + Vec3::Y * time.elapsed_secs().sin(),
                2 => {
                    commands.entity(e).despawn();
                    commands.spawn((MeshletMesh3d(parts.0.clone()), MeshMaterial3d(parts.1[*flip % 2].clone()), Transform::from_translation(m.0), Mover(m.0)));
                }
                _ => {}
            }
        }
    });
    app.finish();
    app.cleanup();

    println!("{} copies, {} ({}); ms per frame, average of {MEASURED} frames:", count, caps.name, caps.backend);
    println!("{:<11} {:>8} {:>8} {:>10} {:>11} {:>8} {:>8}", "", "extract", "queue", "resources", "bind groups", "total", "frame");
    for (label, phase) in [("static", 0), ("1% moving", 1), ("1% replaced", 2)] {
        app.world_mut().resource_mut::<Phase>().0 = phase;
        for _ in 0..WARMUP {
            app.update();
        }
        let mut sum = MeshletCpuTimes::default();
        let mut frame = Duration::ZERO;
        for _ in 0..MEASURED {
            let start = Instant::now();
            app.update();
            frame += start.elapsed();
            let t = app.sub_app(RenderApp).world().resource::<MeshletCpuTimes>();
            sum.extract += t.extract;
            sum.queue += t.queue;
            sum.prepare_resources += t.prepare_resources;
            sum.prepare_bind_groups += t.prepare_bind_groups;
        }
        let ms = |d: Duration| d.as_secs_f64() * 1000.0 / MEASURED as f64;
        let total = sum.extract + sum.queue + sum.prepare_resources + sum.prepare_bind_groups;
        println!(
            "{label:<11} {:>8.3} {:>8.3} {:>10.3} {:>11.3} {:>8.3} {:>8.1}",
            ms(sum.extract), ms(sum.queue), ms(sum.prepare_resources), ms(sum.prepare_bind_groups), ms(total), ms(frame)
        );
    }
    AppExit::Success
}
