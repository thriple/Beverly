//! Physics while playing, on Avian: the one collision world. Each Solid part becomes a body when play starts: still
//! parts are static (boxes as boxes, everything else exact, triangle for triangle), parts that Move are moving
//! (kinematic) bodies, and Loose parts fall and tumble (convex: a hull, or convex pieces for concave shapes such as
//! stairs, arches and models), with their weight and grip. Resting bodies fall asleep (Avian's own sleeping) until
//! something touches them. A [`Blast`] throws loose parts outward; a [`Pull`] (a winch) drags one toward a point.
//! Physics runs on a fixed 60 Hz step; what is drawn is smoothed between steps.
use crate::level::{Collision, Level, Part, Shape};
use crate::world::{Geometry, PartId, part_transform};
use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::HashMap;

/// Steps a second.
pub const HZ: f64 = 60.0;

/// A part's physics body (a separate entity from what is drawn, which follows it).
#[derive(Component, Clone, Copy)]
pub struct PartBody(pub u64);

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PhysicsPlugins::default())
            .insert_resource(Time::<Fixed>::from_hz(HZ))
            .insert_resource(Gravity(Vec3::NEG_Y * 9.81))
            .init_resource::<Pulling>()
            .add_message::<Blast>()
            .add_message::<Pull>()
            .add_systems(Update, (pause, follow))
            .add_systems(FixedUpdate, shove.run_if(|p: Res<crate::play::Play>| p.on));
    }
}

/// How a part is a body: None for parts that don't collide.
pub fn kind(p: &Part) -> Option<RigidBody> {
    if p.collision != Collision::Solid || p.shape == Shape::PlayerStart || p.shape.is_marker() { return None; }
    Some(match (p.mover.is_some(), p.loose.is_some()) {
        (true, _) => RigidBody::Kinematic,
        (false, true) => RigidBody::Dynamic,
        (false, false) => RigidBody::Static,
    })
}

/// A part's collider in its own space (its size included; its place and turn are the body's).
pub fn collider(p: &Part, geo: &Geometry, body: RigidBody) -> Option<Collider> {
    let size = Vec3::from(p.size);
    if matches!(p.shape, Shape::Box | Shape::Plane) { return Some(Collider::cuboid(size.x, size.y, size.z)); }
    let tris = geo.tris(p)?;
    // shared corners, so the pieces join up
    let mut index: HashMap<[u32; 3], u32> = HashMap::new();
    let mut verts: Vec<Vec3> = vec![];
    let mut ids = |v: Vec3| *index.entry(v.to_array().map(f32::to_bits)).or_insert_with(|| { verts.push(v * size); verts.len() as u32 - 1 });
    let faces: Vec<[u32; 3]> = tris.pos.chunks_exact(3).map(|t| [ids(t[0]), ids(t[1]), ids(t[2])]).collect();
    if faces.is_empty() { return None; }
    Some(match (body, p.shape) {
        (RigidBody::Static, _) => Collider::trimesh(verts, faces),
        (_, Shape::Stairs | Shape::Arch | Shape::Model) => Collider::convex_decomposition(&verts, &faces),
        _ => Collider::convex_hull(&verts)?,
    })
}

/// Make the level's bodies (and the ground at y = 0). Returns them, to be despawned when play stops.
pub fn spawn(commands: &mut Commands, level: &Level, geo: &Geometry) -> Vec<Entity> {
    let mut out = vec![commands.spawn((RigidBody::Static, Collider::half_space(Vec3::Y), Transform::default())).id()];
    for p in &level.parts {
        let Some(body) = kind(p) else { continue };
        let Some(c) = collider(p, geo, body) else { continue };
        let tf = Transform { translation: Vec3::from(p.pos), rotation: Quat::from_array(p.rot), scale: Vec3::ONE };
        let mut e = commands.spawn((PartBody(p.id), body, c, tf));
        if body != RigidBody::Static { e.insert(TransformInterpolation); }
        if let (RigidBody::Dynamic, Some(l)) = (body, p.loose.as_ref()) { e.insert((Mass(l.mass.max(0.01)), Friction::new(l.friction.max(0.0)))); }
        out.push(e.id());
    }
    out
}

/// A blast at `at`: loose parts within `radius` are thrown away from it, by an impulse of `strength` (kg·m/s) at
/// the middle fading to none at the edge (so light things fly further).
#[derive(Message, Clone, Copy, Debug)]
pub struct Blast { pub at: Vec3, pub radius: f32, pub strength: f32 }

/// A winch: loose part `part` is pulled toward `to` with `force` newtons for `secs` seconds (a new pull on the same
/// part replaces the old one; force 0 lets go).
#[derive(Message, Clone, Copy, Debug)]
pub struct Pull { pub part: u64, pub to: Vec3, pub force: f32, pub secs: f32 }

/// The pulls going on: part, where to, force, time left.
#[derive(Resource, Default)]
struct Pulling(Vec<(u64, Vec3, f32, f32)>);

/// The impulse a blast gives a body at `pos`.
pub fn blast_impulse(b: &Blast, pos: Vec3) -> Vec3 {
    let d = pos - b.at;
    let k = (1.0 - d.length() / b.radius.max(1e-3)).max(0.0);
    // straight up when it is right on top
    d.try_normalize().unwrap_or(Vec3::Y) * b.strength * k
}

/// Blasts and pulls, each step.
fn shove(mut blasts: MessageReader<Blast>, mut pulls: MessageReader<Pull>, mut pulling: ResMut<Pulling>, time: Res<Time>, mut bodies: Query<(&PartBody, &Position, &RigidBody, Forces)>) {
    for b in blasts.read() {
        for (_, at, kind, mut f) in &mut bodies {
            if kind.is_dynamic() { f.apply_linear_impulse(blast_impulse(b, at.0)); }
        }
    }
    for p in pulls.read() {
        pulling.0.retain(|q| q.0 != p.part);
        if p.force > 0.0 && p.secs > 0.0 { pulling.0.push((p.part, p.to, p.force, p.secs)); }
    }
    let dt = time.delta_secs();
    for (id, to, force, left) in &mut pulling.0 {
        if let Some((_, at, _, mut f)) = bodies.iter_mut().find(|b| b.0.0 == *id && b.2.is_dynamic()) {
            if let Some(dir) = (*to - at.0).try_normalize() { f.apply_force(dir * *force); }
        }
        *left -= dt;
    }
    pulling.0.retain(|q| q.3 > 0.0);
}

/// Physics runs only while playing.
fn pause(play: Res<crate::play::Play>, mut time: ResMut<Time<Physics>>) {
    if play.on == time.is_paused() {
        if play.on { time.unpause() } else { time.pause() }
    }
}

/// What is drawn follows the moving and loose bodies (smoothed between steps).
fn follow(bodies: Query<(&PartBody, &RigidBody, &Transform)>, mut shown: Query<(&PartId, &mut Transform), Without<PartBody>>) {
    let at: HashMap<u64, (Vec3, Quat)> = bodies.iter().filter(|(_, b, _)| !b.is_static()).map(|(id, _, t)| (id.0, (t.translation, t.rotation))).collect();
    if at.is_empty() { return; }
    for (id, mut t) in &mut shown {
        if let Some(&(pos, rot)) = at.get(&id.0) {
            if t.translation != pos || t.rotation != rot {
                t.translation = pos;
                t.rotation = rot;
            }
        }
    }
}

/// Put what is drawn back where the level has it (after play moved it).
pub fn restore(level: &Level, shown: &mut Query<(&PartId, &mut Transform), Without<PartBody>>) {
    for (id, mut t) in shown.iter_mut() {
        if let Some(p) = level.get(id.0).filter(|p| kind(p).is_some_and(|k| !k.is_static())) { *t = part_transform(p); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solid_parts_become_still_moving_or_loose_bodies_with_fitting_colliders() {
        let geo = Geometry::default();
        let part = |shape| Part::placed(shape, None, "P".into(), [0.0, 1.0, 0.0], Quat::IDENTITY.to_array(), [2.0, 1.0, 3.0]);
        assert_eq!(kind(&part(Shape::Box)), Some(RigidBody::Static));
        assert_eq!(kind(&Part { loose: Some(Default::default()), ..part(Shape::Box) }), Some(RigidBody::Dynamic));
        assert_eq!(kind(&Part { mover: Some(Default::default()), loose: Some(Default::default()), ..part(Shape::Box) }), Some(RigidBody::Kinematic));
        assert_eq!(kind(&Part { collision: Collision::WalkThrough, ..part(Shape::Box) }), None);
        assert_eq!(kind(&part(Shape::PointLight)), None);
        // a box is a box of its size; a still ramp is exact, a loose one its hull
        let b = collider(&part(Shape::Box), &geo, RigidBody::Static).unwrap();
        assert_eq!(b.shape_scaled().as_cuboid().map(|c| c.half_extents), Some([1.0, 0.5, 1.5].into()));
        assert!(collider(&part(Shape::Ramp), &geo, RigidBody::Static).unwrap().shape_scaled().as_trimesh().is_some());
        assert!(collider(&part(Shape::Ramp), &geo, RigidBody::Dynamic).unwrap().shape_scaled().as_convex_polyhedron().is_some());
        assert!(collider(&part(Shape::Stairs), &geo, RigidBody::Dynamic).unwrap().shape_scaled().as_compound().is_some());
    }

    #[test]
    fn a_blast_throws_things_away_from_it_harder_when_closer() {
        let b = Blast { at: Vec3::ZERO, radius: 4.0, strength: 100.0 };
        assert_eq!(blast_impulse(&b, Vec3::new(1.0, 0.0, 0.0)), Vec3::new(75.0, 0.0, 0.0));
        assert_eq!(blast_impulse(&b, Vec3::new(0.0, 0.0, -2.0)), Vec3::new(0.0, 0.0, -50.0));
        assert_eq!(blast_impulse(&b, Vec3::new(5.0, 0.0, 0.0)), Vec3::ZERO);
        assert_eq!(blast_impulse(&b, Vec3::ZERO), Vec3::Y * 100.0);
    }
}
