//! Playing a level: the plain walker (a stand-in character until the real characters exist), which moves and slides
//! through the physics world (`physics.rs`) on the fixed step. Used by the editor's test mode (`) and by the game
//! player. `CollisionWorld` keeps the level's solid parts as triangles for a game's own character, until it asks the
//! physics world too.
use crate::level::{Collision, Level, Part, Shape, Tag};
use crate::physics::PartBody;
use crate::script::Script;
use crate::world::{Geometry, PartId, part_affine, part_transform};
use avian3d::prelude::*;
use std::collections::{HashMap, HashSet};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;

const RADIUS: f32 = 0.4;
const EYE: f32 = 1.6;
const GRAVITY: f32 = 22.0;
const JUMP: f32 = 8.0;
const WALK: f32 = 5.0;
const RUN: f32 = 9.0;
const STEP: f32 = (1.0 / crate::physics::HZ) as f32;

/// One solid part's triangles in world space as placed, with the box around them; `off` is how far it has moved
/// since (a moving part), `delta` how far it moved this frame.
struct Piece { id: u64, min: Vec3, max: Vec3, tris: Vec<[Vec3; 3]>, off: Vec3, delta: Vec3 }

/// The solid parts of a level as triangles, for a game's own character (the walker uses the physics world).
#[derive(Default)]
pub struct CollisionWorld { pieces: Vec<Piece> }

impl CollisionWorld {
    pub fn build(level: &Level, geo: &Geometry) -> Self {
        let solid = |p: &Part| p.collision == Collision::Solid && p.shape != Shape::PlayerStart && !p.shape.is_light() && !p.shape.is_camera();
        let pieces = level
            .parts
            .iter()
            .filter(|p| solid(p))
            .filter_map(|p| {
                let t = geo.tris(p)?;
                let m = part_affine(p);
                let tris: Vec<[Vec3; 3]> = t.pos.chunks_exact(3).map(|c| [m.transform_point3(c[0]), m.transform_point3(c[1]), m.transform_point3(c[2])]).collect();
                let (min, max) = tris.iter().flatten().fold((Vec3::MAX, Vec3::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
                (!tris.is_empty()).then_some(Piece { id: p.id, min, max, tris, off: Vec3::ZERO, delta: Vec3::ZERO })
            })
            .collect();
        Self { pieces }
    }

    /// Each solid part's triangles, in world space, where it was placed.
    pub fn pieces(&self) -> impl Iterator<Item = &[[Vec3; 3]]> { self.pieces.iter().map(|p| p.tris.as_slice()) }

    /// How far each piece (same order as [`Self::pieces`]) has moved from where it was placed.
    pub fn offsets(&self) -> impl Iterator<Item = Vec3> + '_ { self.pieces.iter().map(|p| p.off) }

    /// The piece of part `id`, if it is solid.
    pub fn piece_of(&self, id: u64) -> Option<usize> { self.pieces.iter().position(|p| p.id == id) }

    /// Move piece `i` to `off` from where it was placed.
    pub fn set_offset(&mut self, i: usize, off: Vec3) {
        let p = &mut self.pieces[i];
        p.delta += off - p.off;
        p.off = off;
    }

    /// How far piece `i` moved this frame.
    pub fn delta(&self, i: usize) -> Vec3 { self.pieces[i].delta }

    pub(crate) fn clear_deltas(&mut self) { for p in &mut self.pieces { p.delta = Vec3::ZERO; } }

    /// Nearest hit along a ray within `max` (the ground plane included).
    pub fn ray(&self, o: Vec3, d: Vec3, max: f32) -> Option<f32> {
        let mut best = if d.y < -1e-6 && o.y > 0.0 { Some(-o.y / d.y).filter(|t| *t <= max) } else { None };
        for p in &self.pieces {
            let o = o - p.off;
            let (lo, hi) = (o.min(o + d * max), o.max(o + d * max));
            if !(p.min.cmple(hi).all() && p.max.cmpge(lo).all()) { continue; }
            for [a, b, c] in &p.tris {
                if let Some(t) = ray_tri(o, d, *a, *b, *c) {
                    if t <= max && best.is_none_or(|bt| t < bt) { best = Some(t); }
                }
            }
        }
        best
    }
}

fn ray_tri(o: Vec3, d: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let (e1, e2) = (b - a, c - a);
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-9 { return None; }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) { return None; }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 { return None; }
    let t = e2.dot(q) * inv;
    (t > 1e-4).then_some(t)
}

/// The walker: feet position, velocity, facing, and the camera's yaw / pitch. It moves on the fixed step; `prev` is
/// where it was the step before, to smooth what is drawn between steps.
#[derive(Default, Clone)]
pub struct Walker { pub pos: Vec3, pub vel: Vec3, pub facing: f32, pub yaw: f32, pub pitch: f32, pub ground: bool, prev: Vec3, on: Option<u64> }

impl Walker {
    /// The part it stands on.
    pub fn standing_on(&self) -> Option<u64> { self.on }
}

/// Test mode / game state.
#[derive(Resource, Default)]
pub struct Play {
    pub on: bool,
    pub walker: Walker,
    /// The level's solid parts as triangles, for a game's own character (until it asks the physics world).
    pub world: CollisionWorld,
    body: Option<Entity>,
    /// The physics bodies made for this run (see `physics.rs`).
    bodies: Vec<Entity>,
    /// Where the moving and loose parts are drawn when not playing.
    homes: HashMap<u64, Transform>,
    /// Slippery parts (surface tag).
    slippery: HashSet<u64>,
    /// Jump and E pressed since the last step (keys are read every frame, the walker moves on the fixed step).
    jump: bool,
    pub(crate) press: bool,
    /// A game's own player character plays instead of the walker: its plugin sets this to its controls hint,
    /// then runs while `on`, starting afresh whenever `run` changes, from `start`, and moves the `PlayCamera`.
    pub custom: Option<&'static str>,
    /// Counts the starts.
    pub run: u32,
    /// This run's start: feet, facing (yaw).
    pub start: (Vec3, f32),
    /// Where the player's feet are (a custom character sets this each frame; buttons are pressed from here).
    pub feet: Vec3,
    /// Buttons, wires and moving parts. A custom character rides moving parts by following
    /// [`CollisionWorld::offsets`].
    pub script: Script,
}

#[derive(Component)]
pub struct PlayCamera;

#[derive(Component)]
struct Body;

pub struct PlayPlugin;

impl Plugin for PlayPlugin {
    fn build(&self, app: &mut App) {
        let playing = |p: Res<Play>| p.on;
        let walking = |p: Res<Play>| p.on && p.custom.is_none();
        app.init_resource::<Play>()
            .add_plugins(crate::physics::PhysicsPlugin)
            .add_systems(Update, (keys.run_if(playing), follow.run_if(walking)))
            .add_systems(FixedUpdate, (crate::script::run.run_if(playing), walk.run_if(walking)).chain());
    }
}

impl Play {
    /// Start playing `level` with the walker standing at `feet`, facing `yaw`.
    #[allow(clippy::too_many_arguments)]
    pub fn start(&mut self, commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, level: &Level, geo: &Geometry, feet: Vec3, yaw: f32) {
        self.stop(commands);
        self.world = CollisionWorld::build(level, geo);
        self.script = Script::new(level, &self.world);
        self.bodies = crate::physics::spawn(commands, level, geo);
        self.homes = level.parts.iter().filter(|p| crate::physics::kind(p).is_some_and(|k| !k.is_static())).map(|p| (p.id, part_transform(p))).collect();
        self.slippery = level.parts.iter().filter(|p| p.tags.contains(&Tag::Slippery)).map(|p| p.id).collect();
        (self.jump, self.press) = (false, false);
        self.feet = feet;
        self.run += 1;
        self.start = (feet, yaw);
        self.on = true;
        if self.custom.is_some() { return; }
        let at = feet + Vec3::Y * 0.05;
        self.walker = Walker { pos: at, prev: at, facing: yaw, yaw, pitch: -0.25, ..default() };
        let body_mat = mats.add(StandardMaterial { base_color: Color::srgb(0.85, 0.82, 0.78), perceptual_roughness: 0.6, ..default() });
        let visor = mats.add(StandardMaterial { base_color: Color::srgb(0.96, 0.86, 0.29), emissive: LinearRgba::rgb(0.6, 0.5, 0.1), ..default() });
        let body = commands
            .spawn((Body, Transform::from_translation(feet), Visibility::Visible))
            .with_children(|b| {
                b.spawn((Mesh3d(meshes.add(Capsule3d::new(RADIUS, 1.0))), MeshMaterial3d(body_mat), Transform::from_xyz(0.0, 0.9, 0.0)));
                b.spawn((Mesh3d(meshes.add(Cuboid::new(0.5, 0.14, 0.12))), MeshMaterial3d(visor), Transform::from_xyz(0.0, 1.45, -0.36)));
            })
            .id();
        self.body = Some(body);
    }

    pub fn stop(&mut self, commands: &mut Commands) {
        if let Some(b) = self.body.take() { commands.entity(b).despawn(); }
        for e in self.bodies.drain(..) { commands.entity(e).try_despawn(); }
        // what moved goes back where the level has it
        let homes = std::mem::take(&mut self.homes);
        if !homes.is_empty() {
            commands.queue(move |w: &mut World| {
                let mut shown = w.query_filtered::<(&PartId, &mut Transform), Without<crate::physics::PartBody>>();
                for (id, mut t) in shown.iter_mut(w) {
                    if let Some(h) = homes.get(&id.0) { *t = *h; }
                }
            });
        }
        self.script.reset();
        self.on = false;
    }
}

/// Keys that act once (jump, E) are caught every frame and used on the next step.
fn keys(mut play: ResMut<Play>, keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::Space) { play.jump = true; }
    if keys.just_pressed(KeyCode::KeyE) { play.press = true; }
}

/// One fixed step of the walker: steer, fall, then move and slide through the physics world.
fn walk(mut play: ResMut<Play>, keys: Res<ButtonInput<KeyCode>>, motion: Res<AccumulatedMouseMotion>, slide: MoveAndSlide, bodies: Query<(&PartBody, Option<&LinearVelocity>)>) {
    let play = &mut *play;
    let w = &mut play.walker;
    let d = motion.delta;
    w.yaw -= d.x * 0.0025;
    w.pitch = (w.pitch - d.y * 0.0025).clamp(-1.2, 0.9);
    let rot = Quat::from_rotation_y(w.yaw);
    let (f, r) = (rot * Vec3::NEG_Z, rot * Vec3::X);
    let mut dir = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) { dir += f; }
    if keys.pressed(KeyCode::KeyS) { dir -= f; }
    if keys.pressed(KeyCode::KeyD) { dir += r; }
    if keys.pressed(KeyCode::KeyA) { dir -= r; }
    let dir = dir.normalize_or_zero();
    let speed = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) { RUN } else { WALK };
    if dir != Vec3::ZERO { w.facing = (-dir.x).atan2(-dir.z); }
    w.prev = w.pos;
    // ride whatever we stand on
    let ground = w.on.and_then(|id| bodies.iter().find(|(b, _)| b.0 == id)).and_then(|(_, v)| v).map_or(Vec3::ZERO, |v| v.0);
    w.pos += ground * STEP;
    // steer: quick on the ground, a little in the air and on slippery ground
    let target = dir * speed;
    let slippery = w.on.is_some_and(|id| play.slippery.contains(&id));
    let k = if slippery { 1.0 } else if w.ground { 14.0 } else { 3.0 };
    let h = Vec3::new(w.vel.x, 0.0, w.vel.z).lerp(target, (k * STEP).min(1.0));
    w.vel.x = h.x;
    w.vel.z = h.z;
    if std::mem::take(&mut play.jump) && w.ground { w.vel.y = JUMP; }
    w.vel.y -= GRAVITY * STEP;
    // move and slide; then what is underfoot
    let shape = Collider::capsule(RADIUS, 1.0);
    let centre = Vec3::Y * 0.9;
    let none = SpatialQueryFilter::default();
    let out = slide.move_and_slide(&shape, w.pos + centre, Quat::IDENTITY, w.vel, std::time::Duration::from_secs_f32(STEP), &MoveAndSlideConfig::default(), &none, |_| MoveAndSlideHitResponse::Accept);
    w.pos = out.position - centre;
    w.vel = out.projected_velocity;
    let under = slide.spatial_query.cast_shape(&shape, w.pos + centre, Quat::IDENTITY, Dir3::NEG_Y, &ShapeCastConfig::from_max_distance(0.06), &none);
    let floor = under.filter(|h| h.normal1.y > 0.6);
    w.ground = floor.is_some();
    w.on = floor.and_then(|h| bodies.get(h.entity).ok()).map(|(b, _)| b.0);
    if w.ground && w.vel.y < 0.0 { w.vel.y = 0.0; }
    play.feet = w.pos;
}

fn follow(play: Res<Play>, fixed: Res<Time<Fixed>>, ray: SpatialQuery, mut body: Query<&mut Transform, (With<Body>, Without<PlayCamera>)>, mut cam: Query<&mut Transform, With<PlayCamera>>) {
    let w = &play.walker;
    // drawn between the last two steps
    let at = w.prev.lerp(w.pos, fixed.overstep_fraction());
    if let Ok(mut t) = body.single_mut() {
        t.translation = at;
        t.rotation = Quat::from_rotation_y(w.facing);
    }
    let rot = Quat::from_euler(EulerRot::YXZ, w.yaw, w.pitch, 0.0);
    let target = at + Vec3::Y * EYE;
    let back = rot * Vec3::Z;
    let dist = Dir3::new(back).ok().and_then(|d| ray.cast_ray(target, d, 5.0, true, &SpatialQueryFilter::default())).map(|h| (h.distance - 0.3).max(0.6)).unwrap_or(5.0);
    for mut t in &mut cam {
        t.translation = target + back * dist;
        t.rotation = rot;
    }
}
