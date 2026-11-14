//! Playing a level: the plain walker (a stand-in character until the real characters exist) and its collision
//! with the level's solid parts. Used by the editor's test mode (`) and by the game player.
use crate::level::{Collision, Level, Part, Shape};
use crate::script::Script;
use crate::world::{Geometry, part_affine};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;

const RADIUS: f32 = 0.4;
const SPHERES: [f32; 2] = [0.4, 1.4]; // centre heights above the feet
const EYE: f32 = 1.6;
const GRAVITY: f32 = 22.0;
const JUMP: f32 = 8.0;
const WALK: f32 = 5.0;
const RUN: f32 = 9.0;
const STEP: f32 = 1.0 / 120.0;

/// One solid part's triangles in world space as placed, with the box around them; `off` is how far it has moved
/// since (a moving part), `delta` how far it moved this frame.
struct Piece { id: u64, min: Vec3, max: Vec3, tris: Vec<[Vec3; 3]>, off: Vec3, delta: Vec3 }

/// The solid parts of a level, ready for collision.
#[derive(Default)]
pub struct CollisionWorld { pieces: Vec<Piece> }

impl CollisionWorld {
    pub fn build(level: &Level, geo: &Geometry) -> Self {
        let solid = |p: &Part| p.collision == Collision::Solid && p.shape != Shape::PlayerStart;
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

    /// Push a sphere out of everything it overlaps. Returns the push and the most upward contact normal, with
    /// the piece it belongs to.
    fn push_sphere(&self, c: Vec3, r: f32) -> (Vec3, Option<(Vec3, usize)>) {
        let mut push = Vec3::ZERO;
        let mut up: Option<(Vec3, usize)> = None;
        for (i, p) in self.pieces.iter().enumerate() {
            let c = c - p.off;
            let (lo, hi) = (c - Vec3::splat(r), c + Vec3::splat(r));
            if !(p.min.cmple(hi).all() && p.max.cmpge(lo).all()) { continue; }
            for [a, b, t3] in &p.tris {
                let cc = c + push;
                let q = closest_on_tri(cc, *a, *b, *t3);
                let d = cc - q;
                let dist = d.length();
                if dist < r && dist > 1e-6 {
                    let n = d / dist;
                    push += n * (r - dist);
                    if up.is_none_or(|u| n.y > u.0.y) { up = Some((n, i)); }
                }
            }
        }
        (push, up)
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

/// Closest point on triangle abc to p (Ericson, Real-Time Collision Detection 5.1.5).
fn closest_on_tri(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 { return a; }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 { return b; }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 { return a + ab * (d1 / (d1 - d3)); }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 { return c; }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 { return a + ac * (d2 / (d2 - d6)); }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 { return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6))); }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

/// The walker: feet position, velocity, facing, and the camera's yaw / pitch.
#[derive(Default, Clone)]
pub struct Walker { pub pos: Vec3, pub vel: Vec3, pub facing: f32, pub yaw: f32, pub pitch: f32, pub ground: bool, acc: f32, on: Option<usize> }

/// Test mode / game state.
#[derive(Resource, Default)]
pub struct Play {
    pub on: bool,
    pub walker: Walker,
    pub world: CollisionWorld,
    body: Option<Entity>,
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
        app.init_resource::<Play>()
            .add_systems(Update, (crate::script::run.run_if(|p: Res<Play>| p.on), (walk, follow).chain().run_if(|p: Res<Play>| p.on && p.custom.is_none())).chain());
    }
}

impl Play {
    /// Start playing `level` with the walker standing at `feet`, facing `yaw`.
    #[allow(clippy::too_many_arguments)]
    pub fn start(&mut self, commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, level: &Level, geo: &Geometry, feet: Vec3, yaw: f32) {
        self.stop(commands);
        self.world = CollisionWorld::build(level, geo);
        self.script = Script::new(level, &self.world);
        self.feet = feet;
        self.run += 1;
        self.start = (feet, yaw);
        self.on = true;
        if self.custom.is_some() { return; }
        self.walker = Walker { pos: feet + Vec3::Y * 0.05, facing: yaw, yaw, pitch: -0.25, ..default() };
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
        self.script.reset(commands);
        self.on = false;
    }
}

fn walk(mut play: ResMut<Play>, keys: Res<ButtonInput<KeyCode>>, motion: Res<AccumulatedMouseMotion>, time: Res<Time>) {
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
    let jump = keys.just_pressed(KeyCode::Space);
    // ride whatever we stand on
    if let Some(i) = w.on { w.pos += play.world.delta(i); }
    w.acc += time.delta_secs().min(0.1);
    let mut first = true;
    while w.acc >= STEP {
        w.acc -= STEP;
        // steer: quick on the ground, a little in the air
        let target = dir * speed;
        let k = if w.ground { 14.0 } else { 3.0 };
        let h = Vec3::new(w.vel.x, 0.0, w.vel.z).lerp(target, (k * STEP).min(1.0));
        w.vel.x = h.x;
        w.vel.z = h.z;
        if jump && first && w.ground { w.vel.y = JUMP; }
        first = false;
        w.vel.y -= GRAVITY * STEP;
        w.pos += w.vel * STEP;
        // collide
        w.ground = false;
        w.on = None;
        for _ in 0..3 {
            for hgt in SPHERES {
                let (push, up) = play.world.push_sphere(w.pos + Vec3::Y * hgt, RADIUS);
                if push == Vec3::ZERO { continue; }
                w.pos += push;
                if let Some((n, i)) = up {
                    if n.y > 0.6 {
                        w.ground = true;
                        w.on = Some(i);
                    }
                    let into = w.vel.dot(n);
                    if into < 0.0 { w.vel -= n * into; }
                }
            }
        }
        if w.pos.y <= 0.0 {
            w.pos.y = 0.0;
            w.vel.y = w.vel.y.max(0.0);
            w.ground = true;
        }
        if w.ground && w.vel.y < 0.0 { w.vel.y = 0.0; }
    }
    play.feet = w.pos;
}

fn follow(play: Res<Play>, mut body: Query<&mut Transform, (With<Body>, Without<PlayCamera>)>, mut cam: Query<&mut Transform, With<PlayCamera>>) {
    let w = &play.walker;
    if let Ok(mut t) = body.single_mut() {
        t.translation = w.pos;
        t.rotation = Quat::from_rotation_y(w.facing);
    }
    let rot = Quat::from_euler(EulerRot::YXZ, w.yaw, w.pitch, 0.0);
    let target = w.pos + Vec3::Y * EYE;
    let back = rot * Vec3::Z;
    let dist = play.world.ray(target, back, 5.0).map(|t| (t - 0.3).max(0.6)).unwrap_or(5.0);
    for mut t in &mut cam {
        t.translation = target + back * dist;
        t.rotation = rot;
    }
}
