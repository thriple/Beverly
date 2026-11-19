//! What the editor and the game player share about a level in 3D: where a part is, how it looks, ray tests
//! against it, and the stage (ground, sun).
use crate::level::{self, Part, Shape, Surface};
use crate::models::ModelLib;
use crate::shapes::{self, ShapeLib, Tris};
use bevy::math::Affine3A;
use bevy::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

/// `face`: which face of a basic shape was hit (see `shapes::Tris::faces`).
pub struct Hit { pub t: f32, pub point: Vec3, pub normal: Vec3, pub part: Option<u64>, pub face: Option<u8> }

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
        let shapes = Shape::built().map(|s| (s, shapes::build(s))).collect();
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
        if let Some((t, n, face)) = tris.raycast(inv.transform_point3(o), inv.transform_vector3(d)) {
            if best.as_ref().is_none_or(|b| t < b.t) {
                let nw = (Mat3::from(m.matrix3).inverse().transpose() * n).normalize_or_zero();
                best = Some(Hit { t, point: o + d * t, normal: nw, part: Some(p.id), face });
            }
        }
    }
    if d.y < -1e-6 && o.y > 0.0 {
        let t = -o.y / d.y;
        if best.as_ref().is_none_or(|b| t < b.t) {
            best = Some(Hit { t, point: o + d * t, normal: Vec3::Y, part: None, face: None });
        }
    }
    best
}

/// Shared materials: one per colour, one per material file and way it is laid, plus the see-through ones for
/// hidden colliders and ghosts. Also the meshes of textured shapes, one per shape, size and faces, so instances
/// share them (Bevy draws parts sharing a mesh and a material in one batch).
#[derive(Resource)]
pub struct Mats {
    cache: HashMap<[u8; 3], Handle<StandardMaterial>>,
    pub collider: Handle<StandardMaterial>,
    pub ghost: Handle<StandardMaterial>,
    laid: HashMap<(String, [u32; 4]), Handle<StandardMaterial>>,
    images: HashMap<String, Handle<Image>>,
    /// Weak, so a mesh goes once no part uses it (a textured part being resized makes one per size).
    faces: HashMap<(Shape, [u32; 3], Vec<u8>), AssetId<Mesh>>,
}

impl Mats {
    pub fn color(&mut self, assets: &mut Assets<StandardMaterial>, c: [f32; 3]) -> Handle<StandardMaterial> {
        let key = c.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
        self.cache
            .entry(key)
            .or_insert_with(|| assets.add(StandardMaterial { base_color: Color::srgb_u8(key[0], key[1], key[2]), perceptual_roughness: 0.75, ..default() }))
            .clone()
    }

    /// The faces `faces` of a basic shape of this size, laid out for textures in metres.
    pub fn faces_mesh(&mut self, meshes: &mut Assets<Mesh>, shape: Shape, size: [f32; 3], faces: &[u8]) -> Handle<Mesh> {
        let key = (shape, size.map(f32::to_bits), faces.to_vec());
        if let Some(h) = self.faces.get(&key).and_then(|id| meshes.get_strong_handle(*id)) { return h; }
        let h = meshes.add(shapes::build(shape).faces_mesh(Vec3::from(size), |f| faces.contains(&f)));
        self.faces.insert(key, h.id());
        h
    }

    /// A texture, loaded once, repeating.
    fn image(&mut self, server: &AssetServer, t: &str) -> Handle<Image> {
        self.images.entry(t.to_string()).or_insert_with(|| {
            use bevy::image::{ImageAddressMode as A, ImageFilterMode as F, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
            server.load_builder().with_settings(|l: &mut ImageLoaderSettings| {
                l.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                    address_mode_u: A::Repeat, address_mode_v: A::Repeat, mag_filter: F::Linear, min_filter: F::Linear, ..default()
                });
            }).load(format!("project://{t}"))
        }).clone()
    }

    /// How a material looks painted on with the brush: see-through where the strokes don't cover it, drawn over
    /// the part (one copy of the texture every `tile` metres).
    pub fn painted(&mut self, server: &AssetServer, m: &level::Material, tile: f32) -> StandardMaterial {
        let texture = m.texture.as_ref().map(|t| self.image(server, t));
        let tint = Color::srgb(m.tint[0], m.tint[1], m.tint[2]);
        StandardMaterial {
            base_color: tint.with_alpha(m.opacity.clamp(0.0, 1.0)),
            base_color_texture: texture.clone(),
            emissive: LinearRgba { alpha: 1.0, ..tint.to_linear() * m.glow },
            emissive_texture: if m.glow > 0.0 { texture } else { None },
            uv_transform: bevy::math::Affine2::from_scale(Vec2::splat(1.0 / tile.max(0.01))),
            metallic: m.metal.clamp(0.0, 1.0),
            perceptual_roughness: m.roughness.clamp(0.0, 1.0),
            alpha_mode: AlphaMode::Blend,
            depth_bias: 1.0,
            ..default()
        }
    }

    /// How a surface looks: its material, laid at the surface's size and offset.
    pub fn surface(&mut self, assets: &mut Assets<StandardMaterial>, server: &AssetServer, m: &level::Material, s: &Surface) -> Handle<StandardMaterial> {
        let key = (format!("{m:?}"), [s.tile[0], s.tile[1], s.offset[0], s.offset[1]].map(f32::to_bits));
        if let Some(h) = self.laid.get(&key) { return h.clone(); }
        let texture = m.texture.as_ref().map(|t| self.image(server, t));
        let tile = Vec2::from(s.tile).max(Vec2::splat(0.01));
        let tint = Color::srgb(m.tint[0], m.tint[1], m.tint[2]);
        let see_through = m.opacity < 0.999;
        let h = assets.add(StandardMaterial {
            base_color: tint.with_alpha(m.opacity.clamp(0.0, 1.0)),
            base_color_texture: texture.clone(),
            // Glow 1: its own colour at full brightness, whatever the light (alpha stays 1)
            emissive: LinearRgba { alpha: 1.0, ..tint.to_linear() * m.glow },
            emissive_texture: if m.glow > 0.0 { texture } else { None },
            uv_transform: bevy::math::Affine2::from_scale_angle_translation(1.0 / tile, 0.0, -Vec2::from(s.offset) / tile),
            metallic: m.metal.clamp(0.0, 1.0),
            perceptual_roughness: m.roughness.clamp(0.0, 1.0),
            alpha_mode: if see_through { AlphaMode::Blend } else { AlphaMode::Opaque },
            ..default()
        });
        self.laid.insert(key, h.clone());
        h
    }
}

/// The stage: a dark ground plane at y = 0 and the sun (`shadows` off for slow software renderers). Returns the
/// shared materials for the caller to keep as a resource.
pub fn setup_stage(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, shadows: bool) -> Mats {
    let see_through = |c: Color| StandardMaterial { base_color: c, alpha_mode: AlphaMode::Blend, unlit: true, cull_mode: None, double_sided: true, ..default() };
    let collider = mats.add(see_through(Color::srgba(0.96, 0.86, 0.29, 0.10)));
    let ghost = mats.add(see_through(Color::srgba(0.96, 0.86, 0.29, 0.28)));
    let shared = Mats { cache: HashMap::new(), collider, ghost, laid: HashMap::new(), images: HashMap::new(), faces: HashMap::new() };
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(4000.0, 4000.0))),
        MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::srgb(0.075, 0.068, 0.085), perceptual_roughness: 1.0, ..default() })),
    ));
    commands.spawn((
        Sun,
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: shadows, ..default() },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.7, -0.95, 0.0)),
    ));
    shared
}

/// The layer of what only the editor's view shows (its lines, handles and stand-ins).
pub const EDITOR_ONLY: bevy::camera::visibility::RenderLayers = bevy::camera::visibility::RenderLayers::layer(1);

/// The stage's sun.
#[derive(Component)]
pub struct Sun;

/// Light the stage as the level says: its sun (dark for Interior) and the fill light. `shadows`: false turns the
/// sun's shadows off whatever the level says (slow software renderers).
pub fn light_stage(l: &level::Lighting, shadows: bool, sun: &mut Query<(&mut DirectionalLight, &mut Transform), With<Sun>>, fill: &mut GlobalAmbientLight) {
    let rgb = |c: [f32; 3]| Color::srgb(c[0], c[1], c[2]);
    for (mut light, mut tf) in sun.iter_mut() {
        light.color = rgb(l.colour);
        light.illuminance = if l.interior { 0.0 } else { l.brightness };
        light.shadow_maps_enabled = l.shadows && shadows && !l.interior;
        tf.rotation = l.sun_rotation();
    }
    fill.color = rgb(l.fill_colour);
    fill.brightness = l.fill;
}

/// What spawning a part's look needs. `materials`: the project's, by path.
pub struct Looks<'a> {
    pub lib: &'a ShapeLib,
    pub models: &'a mut ModelLib,
    pub server: &'a AssetServer,
    pub mats: &'a mut Mats,
    pub assets: &'a mut Assets<StandardMaterial>,
    pub meshes: &'a mut Assets<Mesh>,
    pub materials: &'a std::collections::BTreeMap<String, level::Material>,
    /// Lights may cast shadows (off for slow software renderers).
    pub shadows: bool,
}

/// Spawn how a part looks. `editor`: hidden colliders and the player start show as see-through stand-ins, lights as
/// bulbs, cameras as small bodies and sounds as speakers, all on the editor's own layer (only its view shows them, not pictures through
/// a camera); in the game they are not drawn. A textured shape is an entity with a child per material it shows.
pub fn spawn_visual(commands: &mut Commands, p: &Part, k: &mut Looks, editor: bool) -> Option<Entity> {
    let Looks { lib, models, server, mats, assets, meshes, materials, shadows } = k;
    let tf = (part_transform(p), PartId(p.id));
    match p.shape {
        s if s.is_light() => {
            let l = p.light.clone().unwrap_or_default();
            let colour = Color::srgb(l.colour[0], l.colour[1], l.colour[2]);
            // the editor shows a bulb (clicked to select it) in the light's colour; the game only the light
            let part = match editor {
                true => commands.spawn((tf, Mesh3d(lib.mesh(s)), MeshMaterial3d(assets.add(StandardMaterial { base_color: colour, unlit: true, ..default() })), EDITOR_ONLY)).id(),
                false => commands.spawn((tf, Visibility::default())).id(),
            };
            let shadows_enabled = l.shadows && *shadows;
            // (Bevy's spot shines along -Z; the part's along its up)
            match s {
                Shape::PointLight => commands.spawn((PointLight { color: colour, intensity: l.brightness, range: l.reach, shadow_maps_enabled: shadows_enabled, ..default() }, ChildOf(part))),
                _ => commands.spawn((
                    SpotLight { color: colour, intensity: l.brightness, range: l.reach, outer_angle: (l.cone * 0.5).to_radians(), inner_angle: (l.cone * 0.4).to_radians(), shadow_maps_enabled: shadows_enabled, ..default() },
                    Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    ChildOf(part),
                )),
            };
            Some(part)
        }
        // a camera part: a small body in the editor (clicked to select it); nothing in the game
        s if s.is_camera() => editor.then(|| commands.spawn((tf, Mesh3d(lib.mesh(s)), MeshMaterial3d(assets.add(StandardMaterial { base_color: Color::srgb(0.37, 0.78, 0.85), unlit: true, ..default() })), EDITOR_ONLY)).id()),
        Shape::PlayerStart => editor.then(|| commands.spawn((tf, Mesh3d(lib.mesh(Shape::PlayerStart)), MeshMaterial3d(mats.ghost.clone()), EDITOR_ONLY)).id()),
        // a Sound part: a speaker in the Actions colour in the editor (clicked to select it); heard, not seen, in the game
        Shape::Sound => editor.then(|| commands.spawn((tf, Mesh3d(lib.mesh(Shape::Sound)), MeshMaterial3d(assets.add(StandardMaterial { base_color: Color::srgb_u8(0x3D, 0x7B, 0xFF), unlit: true, ..default() })), EDITOR_ONLY)).id()),
        Shape::Model => {
            let path = p.model.as_ref()?;
            models.request(server, path);
            if !p.visible && !editor { return None; }
            // its meshes come under it once the model has loaded (models::fill_models)
            Some(commands.spawn((tf, Visibility::default(), crate::models::ModelRoot { path: path.clone(), tint: p.color })).id())
        }
        s => {
            if !p.visible && !editor { return None; }
            if p.visible && !p.surfaces.is_empty() {
                // the faces, grouped by the surface that covers them (none: the part's colour)
                let count = shapes::build(s).faces.len() as u8;
                let mut groups: Vec<(Option<&Surface>, Vec<u8>)> = vec![];
                for f in 0..count {
                    let s = p.surface(f);
                    match groups.iter_mut().find(|g| g.0 == s) { Some(g) => g.1.push(f), None => groups.push((s, vec![f])) }
                }
                let parent = commands.spawn((tf, Visibility::default())).id();
                for (laid, faces) in groups {
                    let mat = laid.and_then(|s| Some(mats.surface(assets, server, materials.get(&s.material)?, s))).unwrap_or_else(|| mats.color(assets, p.color));
                    let mesh = mats.faces_mesh(meshes, s, p.size, &faces);
                    commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), Transform::default(), ChildOf(parent)));
                }
                return Some(parent);
            }
            if !p.visible { return Some(commands.spawn((tf, Mesh3d(lib.mesh(s)), MeshMaterial3d(mats.collider.clone()), EDITOR_ONLY)).id()); }
            Some(commands.spawn((tf, Mesh3d(lib.mesh(s)), MeshMaterial3d(mats.color(assets, p.color)))).id())
        }
    }
}
