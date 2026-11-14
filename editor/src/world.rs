//! What the editor and the game player share about a level in 3D: where a part is, how it looks, ray tests
//! against it, and the stage (ground, sun).
use crate::level::{Part, Shape};
use crate::models::ModelLib;
use crate::shapes::{self, ShapeLib, Tris};
use bevy::math::Affine3A;
use bevy::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

pub struct Hit { pub t: f32, pub point: Vec3, pub normal: Vec3, pub part: Option<u64> }

/// Unit box (shapes) or model space (models) to world.
pub fn part_affine(p: &Part) -> Affine3A { Affine3A::from_scale_rotation_translation(Vec3::from(p.size), Quat::from_array(p.rot), Vec3::from(p.pos)) }

/// Which part an entity shows.
#[derive(Component, Clone, Copy)]
pub struct PartId(pub u64);

pub fn part_transform(p: &Part) -> Transform { Transform { translation: Vec3::from(p.pos), rotation: Quat::from_array(p.rot), scale: Vec3::from(p.size) } }

/// Every part's triangles: the basic shapes' unit-box ones, and each loaded model's.
pub struct Geometry { pub shapes: HashMap<Shape, Tris>, pub models: HashMap<String, Arc<Tris>> }

impl Default for Geometry {
    fn default() -> Self {
        let shapes = Shape::ALL.iter().chain([&Shape::PlayerStart, &Shape::Interact]).map(|&s| (s, shapes::build(s))).collect();
        Self { shapes, models: HashMap::new() }
    }
}

impl Geometry {
    pub fn tris(&self, p: &Part) -> Option<&Tris> {
        match p.shape {
            Shape::Model => p.model.as_ref().and_then(|m| self.models.get(m)).map(|a| a.as_ref()),
            s => self.shapes.get(&s),
        }
    }
    /// Copy newly loaded model triangles in.
    pub fn sync_models(&mut self, lib: &ModelLib) {
        for (k, g) in lib.all_geo() {
            if !self.models.contains_key(k) { self.models.insert(k.clone(), g.clone()); }
        }
    }
    /// The box around a part in its own space (unit box for shapes, the model's bounds for models).
    pub fn local_box(&self, p: &Part) -> (Vec3, Vec3) {
        match p.shape {
            Shape::Model => self.tris(p).map(|t| t.bounds()).unwrap_or((Vec3::splat(-0.5), Vec3::splat(0.5))),
            _ => (Vec3::splat(-0.5), Vec3::splat(0.5)),
        }
    }
}

/// The nearest surface along a ray: a part (those `keep` accepts, except `skip`), or the ground (y = 0) seen from above.
pub fn raycast(parts: &[Part], geo: &Geometry, ray: Ray3d, skip: Option<u64>, keep: impl Fn(&Part) -> bool) -> Option<Hit> {
    let (o, d) = (ray.origin, *ray.direction);
    let mut best: Option<Hit> = None;
    for p in parts {
        if Some(p.id) == skip || !keep(p) { continue; }
        let Some(tris) = geo.tris(p) else { continue };
        let m = part_affine(p);
        let inv = m.inverse();
        if let Some((t, n)) = tris.raycast(inv.transform_point3(o), inv.transform_vector3(d)) {
            if best.as_ref().is_none_or(|b| t < b.t) {
                let nw = (Mat3::from(m.matrix3).inverse().transpose() * n).normalize_or_zero();
                best = Some(Hit { t, point: o + d * t, normal: nw, part: Some(p.id) });
            }
        }
    }
    if d.y < -1e-6 && o.y > 0.0 {
        let t = -o.y / d.y;
        if best.as_ref().is_none_or(|b| t < b.t) {
            best = Some(Hit { t, point: o + d * t, normal: Vec3::Y, part: None });
        }
    }
    best
}

/// Shared materials: one per colour, plus the see-through ones for hidden colliders and ghosts.
#[derive(Resource)]
pub struct Mats { cache: HashMap<[u8; 3], Handle<StandardMaterial>>, pub collider: Handle<StandardMaterial>, pub ghost: Handle<StandardMaterial> }

impl Mats {
    pub fn color(&mut self, assets: &mut Assets<StandardMaterial>, c: [f32; 3]) -> Handle<StandardMaterial> {
        let key = c.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
        self.cache
            .entry(key)
            .or_insert_with(|| assets.add(StandardMaterial { base_color: Color::srgb_u8(key[0], key[1], key[2]), perceptual_roughness: 0.75, ..default() }))
            .clone()
    }
}

/// The stage: a dark ground plane at y = 0 and the sun (`shadows` off for slow software renderers). Returns the
/// shared materials for the caller to keep as a resource.
pub fn setup_stage(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, shadows: bool) -> Mats {
    let see_through = |c: Color| StandardMaterial { base_color: c, alpha_mode: AlphaMode::Blend, unlit: true, cull_mode: None, double_sided: true, ..default() };
    let collider = mats.add(see_through(Color::srgba(0.96, 0.86, 0.29, 0.10)));
    let ghost = mats.add(see_through(Color::srgba(0.96, 0.86, 0.29, 0.28)));
    let shared = Mats { cache: HashMap::new(), collider, ghost };
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(4000.0, 4000.0))),
        MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::srgb(0.075, 0.068, 0.085), perceptual_roughness: 1.0, ..default() })),
    ));
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: shadows, ..default() },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.7, -0.95, 0.0)),
    ));
    shared
}

/// Spawn how a part looks. `editor`: hidden colliders and the player start show as see-through stand-ins;
/// in the game they are not drawn.
pub fn spawn_visual(commands: &mut Commands, p: &Part, lib: &ShapeLib, models: &mut ModelLib, server: &AssetServer, mats: &mut Mats, assets: &mut Assets<StandardMaterial>, editor: bool) -> Option<Entity> {
    let tf = (part_transform(p), PartId(p.id));
    match p.shape {
        Shape::PlayerStart => editor.then(|| commands.spawn((tf, Mesh3d(lib.mesh(Shape::PlayerStart)), MeshMaterial3d(mats.ghost.clone()))).id()),
        Shape::Model => {
            let path = p.model.as_ref()?;
            let scene = models.request(server, path);
            if !p.visible && !editor { return None; }
            Some(commands.spawn((tf, WorldAssetRoot(scene))).id())
        }
        s => {
            if !p.visible && !editor { return None; }
            let mat = if p.visible { mats.color(assets, p.color) } else { mats.collider.clone() };
            Some(commands.spawn((tf, Mesh3d(lib.mesh(s)), MeshMaterial3d(mat))).id())
        }
    }
}
