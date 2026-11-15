//! Physics while playing, on Avian: the one collision world. Each Solid part becomes a body when play starts: still
//! parts are static (boxes as boxes, everything else exact, triangle for triangle), parts that Move are moving
//! (kinematic) bodies, and Loose parts fall and tumble (convex: a hull, or convex pieces for concave shapes such as
//! stairs, arches and models). Physics runs on a fixed 60 Hz step; what is drawn is smoothed between steps.
use crate::level::{Body as Kind, Collision, Level, Part, Shape};
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
            .add_systems(Update, (pause, follow));
    }
}

/// How a part is a body: None for parts that don't collide.
pub fn kind(p: &Part) -> Option<RigidBody> {
    if p.collision != Collision::Solid || p.shape == Shape::PlayerStart || p.shape.is_light() || p.shape.is_camera() { return None; }
    Some(match (p.mover.is_some(), p.body) {
        (true, _) => RigidBody::Kinematic,
        (false, Kind::Loose) => RigidBody::Dynamic,
        (false, Kind::Still) => RigidBody::Static,
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
        out.push(e.id());
    }
    out
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
        assert_eq!(kind(&Part { body: Kind::Loose, ..part(Shape::Box) }), Some(RigidBody::Dynamic));
        assert_eq!(kind(&Part { mover: Some(Default::default()), body: Kind::Loose, ..part(Shape::Box) }), Some(RigidBody::Kinematic));
        assert_eq!(kind(&Part { collision: Collision::WalkThrough, ..part(Shape::Box) }), None);
        assert_eq!(kind(&part(Shape::PointLight)), None);
        // a box is a box of its size; a still ramp is exact, a loose one its hull
        let b = collider(&part(Shape::Box), &geo, RigidBody::Static).unwrap();
        assert_eq!(b.shape_scaled().as_cuboid().map(|c| c.half_extents), Some([1.0, 0.5, 1.5].into()));
        assert!(collider(&part(Shape::Ramp), &geo, RigidBody::Static).unwrap().shape_scaled().as_trimesh().is_some());
        assert!(collider(&part(Shape::Ramp), &geo, RigidBody::Dynamic).unwrap().shape_scaled().as_convex_polyhedron().is_some());
        assert!(collider(&part(Shape::Stairs), &geo, RigidBody::Dynamic).unwrap().shape_scaled().as_compound().is_some());
    }
}
