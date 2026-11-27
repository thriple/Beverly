//! What the editor and the game player share about a level in 3D: where a part is, how it looks, ray tests
//! against it, and the stage (ground, sun).
use crate::level::{self, Part, Shape, Surface};
use crate::models::ModelLib;
use crate::shader::{self, ShaderLayer, ShaderMaterial, Shaders};
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

/// The box around a part in the world (its own box turned and placed: every corner).
pub fn world_box(geo: &Geometry, p: &Part) -> (Vec3, Vec3) {
    let (lo, hi) = geo.local_box(p);
    let a = part_affine(p);
    (0..8).map(|k| a.transform_point3(Vec3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z })))
        .fold((Vec3::MAX, Vec3::MIN), |(l, h), c| (l.min(c), h.max(c)))
}

/// Parts by where they are, seen from above (docs/orderliness.md, stage 6): a grid of cells, each listing the parts
/// whose box reaches it, so a ray or a question about one spot looks only at the parts there rather than at every
/// part. Built from the parts as they are; it holds their places in the list and their ids, not the parts.
pub struct Index {
    boxes: Vec<(Vec3, Vec3)>,
    ids: Vec<u64>,
    cells: HashMap<(i32, i32), Vec<u32>>,
    /// Parts too big to list cell by cell: looked at by every question.
    everywhere: Vec<u32>,
    /// The cells' corner-to-corner span, seen from above.
    span: (Vec2, Vec2),
}

/// Metres across a cell of the index, and the most cells one part is listed in.
const CELL: f32 = 8.0;
const MOST_CELLS: i64 = 4096;

fn cell_of(x: f32) -> i32 { (x / CELL).floor() as i32 }

impl Index {
    pub fn new(parts: &[Part], geo: &Geometry) -> Index {
        let mut ix = Index { boxes: Vec::with_capacity(parts.len()), ids: Vec::with_capacity(parts.len()), cells: HashMap::new(), everywhere: vec![], span: (Vec2::MAX, Vec2::MIN) };
        for (i, p) in parts.iter().enumerate() {
            let (lo, hi) = world_box(geo, p);
            ix.boxes.push((lo, hi));
            ix.ids.push(p.id);
            let (x0, x1, z0, z1) = (cell_of(lo.x), cell_of(hi.x), cell_of(lo.z), cell_of(hi.z));
            if !(lo.is_finite() && hi.is_finite()) || (x1 - x0 + 1) as i64 * (z1 - z0 + 1) as i64 > MOST_CELLS { ix.everywhere.push(i as u32); continue; }
            ix.span = (ix.span.0.min(lo.xz()), ix.span.1.max(hi.xz()));
            for x in x0..=x1 { for z in z0..=z1 { ix.cells.entry((x, z)).or_default().push(i as u32); } }
        }
        ix
    }

    /// The part in slot `i`, if the list is still the one the index was built from there.
    fn part<'a>(&self, parts: &'a [Part], i: u32) -> Option<&'a Part> {
        let id = self.ids[i as usize];
        parts.get(i as usize).filter(|p| p.id == id).or_else(|| parts.iter().find(|p| p.id == id))
    }

    /// The parts whose box reaches the spot (x, z) seen from above (a superset: their boxes, not their shapes).
    pub fn at<'a>(&'a self, parts: &'a [Part], at: Vec2) -> impl Iterator<Item = &'a Part> + 'a {
        let cell = self.cells.get(&(cell_of(at.x), cell_of(at.y))).map_or(&[][..], |v| v.as_slice());
        cell.iter().chain(&self.everywhere).filter(move |i| { let (lo, hi) = self.boxes[**i as usize]; at.cmpge(lo.xz()).all() && at.cmple(hi.xz()).all() }).filter_map(move |i| self.part(parts, *i))
    }

    /// As [`raycast`], walking only the cells the ray crosses, nearest first, and stopping once nothing nearer can be
    /// found. The same answer as testing every part.
    pub fn raycast(&self, parts: &[Part], geo: &Geometry, ray: Ray3d, skip: Option<u64>, keep: impl Fn(&Part) -> bool) -> Option<Hit> {
        let (o, d) = (ray.origin, *ray.direction);
        let mut best: Option<Hit> = None;
        let mut seen = std::collections::HashSet::new();
        let mut test = |i: u32, best: &mut Option<Hit>| {
            if !seen.insert(i) { return; }
            let Some(t0) = slab(o, d, self.boxes[i as usize]) else { return };
            if best.as_ref().is_some_and(|b| b.t < t0) { return; }
            let Some(p) = self.part(parts, i) else { return };
            if Some(p.id) == skip || !keep(p) { return; }
            if let Some(h) = hit_part(geo, p, o, d) { if best.as_ref().is_none_or(|b| h.t < b.t) { *best = Some(h); } }
        };
        for i in &self.everywhere { test(*i, &mut best); }
        // the stretch of the ray over the cells' span, seen from above
        let (flat, (lo, hi)) = (d.xz(), self.span);
        let range = if lo.x > hi.x { None } else if flat.length_squared() < crate::guard::LEN2 {
            (o.xz().cmpge(lo).all() && o.xz().cmple(hi).all()).then_some((0.0f32, f32::MAX))
        } else {
            let (a, b) = ((lo - o.xz()) / flat, (hi - o.xz()) / flat);
            let (t0, t1) = (a.min(b).max_element().max(0.0), a.max(b).min_element());
            (t0 <= t1).then_some((t0, t1))
        };
        if let Some((t0, t1)) = range {
            // step from cell to cell along the ray (seen from above)
            let start = o.xz() + flat * t0;
            let (mut cx, mut cz) = (cell_of(start.x), cell_of(start.y));
            let step = |v: f32| if v > 0.0 { 1 } else { -1 };
            let next = |c: i32, o: f32, v: f32| if v.abs() < crate::guard::LEN2 { f32::MAX } else { (((c + i32::from(v > 0.0)) as f32 * CELL) - o) / v };
            let (mut nx, mut nz) = (next(cx, o.x, d.x), next(cz, o.z, d.z));
            let (dx, dz) = (if d.x.abs() < crate::guard::LEN2 { f32::MAX } else { CELL / d.x.abs() }, if d.z.abs() < crate::guard::LEN2 { f32::MAX } else { CELL / d.z.abs() });
            let mut enter = t0;
            loop {
                if best.as_ref().is_some_and(|b| b.t < enter) { break; }
                if let Some(v) = self.cells.get(&(cx, cz)) { for i in v { test(*i, &mut best); } }
                if flat.length_squared() < crate::guard::LEN2 { break; }
                if nx < nz { enter = nx; nx += dx; cx += step(d.x); } else { enter = nz; nz += dz; cz += step(d.z); }
                if enter > t1 { break; }
            }
        }
        ground(o, d, &mut best);
        best
    }
}

/// Where a ray first meets a box (its distance), if it does.
fn slab(o: Vec3, d: Vec3, (lo, hi): (Vec3, Vec3)) -> Option<f32> {
    let inv = d.recip();
    let (a, b) = ((lo - crate::guard::SHORT - o) * inv, (hi + crate::guard::SHORT - o) * inv);
    let (near, far) = (a.min(b).max_element(), a.max(b).min_element());
    (far >= near.max(0.0)).then_some(near.max(0.0))
}


/// A ray against one part's shape, in the world.
fn hit_part(geo: &Geometry, p: &Part, o: Vec3, d: Vec3) -> Option<Hit> {
    let tris = geo.tris(p)?;
    let m = part_affine(p);
    let inv = m.inverse();
    let (t, n, face) = tris.raycast(inv.transform_point3(o), inv.transform_vector3(d))?;
    let nw = (Mat3::from(m.matrix3).inverse().transpose() * n).normalize_or_zero();
    Some(Hit { t, point: o + d * t, normal: nw, part: Some(p.id), face })
}

/// The ground (y = 0), seen from above, when it is nearer.
fn ground(o: Vec3, d: Vec3, best: &mut Option<Hit>) {
    if d.y < -crate::guard::ZERO && o.y > 0.0 {
        let t = -o.y / d.y;
        if best.as_ref().is_none_or(|b| t < b.t) { *best = Some(Hit { t, point: o + d * t, normal: Vec3::Y, part: None, face: None }); }
    }
}

/// The parts of a level that lines are laid on and items land on, with their shapes and an [`Index`] of them.
pub struct Ground<'a> {
    pub parts: &'a [Part],
    pub geo: &'a Geometry,
    pub index: Index,
}

impl<'a> Ground<'a> {
    pub fn new(parts: &'a [Part], geo: &'a Geometry) -> Self { Ground { parts, geo, index: Index::new(parts, geo) } }
    pub fn raycast(&self, ray: Ray3d, skip: Option<u64>, keep: impl Fn(&Part) -> bool) -> Option<Hit> { self.index.raycast(self.parts, self.geo, ray, skip, keep) }
    /// The parts whose box reaches (x, z), seen from above.
    pub fn at(&self, at: Vec2) -> impl Iterator<Item = &Part> + '_ { self.index.at(self.parts, at) }
}

/// The nearest surface along a ray: a part (those `keep` accepts, except `skip`), or the ground (y = 0) seen from above.
pub fn raycast(parts: &[Part], geo: &Geometry, ray: Ray3d, skip: Option<u64>, keep: impl Fn(&Part) -> bool) -> Option<Hit> {
    let (o, d) = (ray.origin, *ray.direction);
    let mut best: Option<Hit> = None;
    for p in parts {
        if Some(p.id) == skip || !keep(p) { continue; }
        if let Some(h) = hit_part(geo, p, o, d) { if best.as_ref().is_none_or(|b| h.t < b.t) { best = Some(h); } }
    }
    ground(o, d, &mut best);
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
    shaded: HashMap<(String, [u32; 4]), Handle<ShaderMaterial>>,
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
        let h = assets.add(self.surface_material(server, m, s));
        self.laid.insert(key, h.clone());
        h
    }

    /// A surface whose material has a shader (shader.rs): the same look with the shader's fragment code and values.
    pub fn shaded(&mut self, assets: &mut Assets<ShaderMaterial>, shaders: &mut Shaders, server: &AssetServer, m: &level::Material, s: &Surface) -> Option<Handle<ShaderMaterial>> {
        let path = m.shader.as_deref()?;
        let key = (format!("{m:?}"), [s.tile[0], s.tile[1], s.offset[0], s.offset[1]].map(f32::to_bits));
        if let Some(h) = self.shaded.get(&key) { return Some(h.clone()); }
        let shader = shaders.request(path);
        let values = shaders.header(path).map(|h| shader::values(h, &m.shader_values)).unwrap_or([Vec4::ZERO; shader::SLOTS]);
        let base = self.surface_material(server, m, s);
        let alpha = base.alpha_mode;
        let h = assets.add(ShaderMaterial {
            base,
            extension: ShaderLayer { values: shader::Values { v: values }, path: path.to_string(), set: m.shader_values.clone(), shader, vertex: false, mask: false, alpha },
        });
        self.shaded.insert(key, h.clone());
        Some(h)
    }

    fn surface_material(&mut self, server: &AssetServer, m: &level::Material, s: &Surface) -> StandardMaterial {
        let texture = m.texture.as_ref().map(|t| self.image(server, t));
        let tile = Vec2::from(s.tile).max(Vec2::splat(0.01));
        let tint = Color::srgb(m.tint[0], m.tint[1], m.tint[2]);
        let see_through = m.opacity < 0.999;
        StandardMaterial {
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
        }
    }
}

/// The stage: a dark ground plane at y = 0 and the sun (`shadows` off for slow software renderers). Returns the
/// shared materials for the caller to keep as a resource.
pub fn setup_stage(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, shadows: bool) -> Mats {
    let see_through = |c: Color| StandardMaterial { base_color: c, alpha_mode: AlphaMode::Blend, unlit: true, cull_mode: None, double_sided: true, ..default() };
    let collider = mats.add(see_through(Color::srgba(0.96, 0.86, 0.29, 0.10)));
    let ghost = mats.add(see_through(Color::srgba(0.96, 0.86, 0.29, 0.28)));
    let shared = Mats { cache: HashMap::new(), collider, ghost, laid: HashMap::new(), shaded: HashMap::new(), images: HashMap::new(), faces: HashMap::new() };
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
    pub shaded: &'a mut Assets<ShaderMaterial>,
    pub shaders: &'a mut Shaders,
    pub meshes: &'a mut Assets<Mesh>,
    pub materials: &'a std::collections::BTreeMap<String, level::Material>,
    /// Lights may cast shadows (off for slow software renderers).
    pub shadows: bool,
}

/// Spawn how a part looks. `editor`: hidden colliders and the player start show as see-through stand-ins, lights as
/// bulbs, cameras as small bodies and sounds as speakers, all on the editor's own layer (only its view shows them, not pictures through
/// a camera); in the game they are not drawn. A textured shape is an entity with a child per material it shows.
pub fn spawn_visual(commands: &mut Commands, p: &Part, k: &mut Looks, editor: bool) -> Option<Entity> {
    let Looks { lib, models, server, mats, assets, shaded, shaders, meshes, materials, shadows } = k;
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
        // a Zone part: a see-through box in the trigger colour in the editor (seen from outside; clicked to select it)
        Shape::Zone => editor.then(|| commands.spawn((tf, Mesh3d(lib.mesh(Shape::Zone)), MeshMaterial3d(assets.add(StandardMaterial { base_color: Color::srgba(0.98, 0.62, 0.22, 0.14), unlit: true, alpha_mode: AlphaMode::Blend, ..default() })), bevy::light::NotShadowCaster, EDITOR_ONLY)).id()),
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
                    let mesh = mats.faces_mesh(meshes, s, p.size, &faces);
                    // a material with a shader is drawn by it
                    if let Some(sh) = laid.and_then(|s| mats.shaded(shaded, shaders, server, materials.get(&s.material)?, s)) {
                        commands.spawn((Mesh3d(mesh), MeshMaterial3d(sh), Transform::default(), ChildOf(parent)));
                        continue;
                    }
                    let mat = laid.and_then(|s| Some(mats.surface(assets, server, materials.get(&s.material)?, s))).unwrap_or_else(|| mats.color(assets, p.color));
                    commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), Transform::default(), ChildOf(parent)));
                }
                return Some(parent);
            }
            if !p.visible { return Some(commands.spawn((tf, Mesh3d(lib.mesh(s)), MeshMaterial3d(mats.collider.clone()), EDITOR_ONLY)).id()); }
            Some(commands.spawn((tf, Mesh3d(lib.mesh(s)), MeshMaterial3d(mats.color(assets, p.color)))).id())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The index walks only the cells a ray crosses: it must find what testing every part finds, ray for ray.
    #[test]
    fn the_index_finds_what_testing_every_part_finds() {
        let geo = Geometry::default();
        let mut seed = 7u64;
        let mut rnd = move || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed % 10_000) as f32 / 10_000.0 };
        let shapes = [Shape::Box, Shape::Ramp, Shape::Cylinder, Shape::Sphere, Shape::Arch, Shape::Stairs];
        let mut parts: Vec<Part> = (0..300).map(|k| {
            let mut p = Part::placed(shapes[k % shapes.len()], None, format!("P{k}"), [rnd() * 200.0 - 100.0, rnd() * 20.0, rnd() * 200.0 - 100.0], Quat::from_euler(EulerRot::YXZ, rnd() * 6.0, rnd() - 0.5, rnd() - 0.5).to_array(), [0.2 + rnd() * 12.0, 0.2 + rnd() * 6.0, 0.2 + rnd() * 12.0]);
            p.id = k as u64 + 1;
            p
        }).collect();
        // a floor too big to list cell by cell, and a long thin wall
        let mut floor = Part::placed(Shape::Plane, None, "Floor".into(), [0.0, -0.05, 0.0], Quat::IDENTITY.to_array(), [5000.0, 0.1, 5000.0]);
        floor.id = 1000;
        parts.push(floor);
        let mut wall = Part::placed(Shape::Box, None, "Wall".into(), [0.0, 2.0, 30.0], Quat::from_rotation_y(0.3).to_array(), [300.0, 4.0, 0.3]);
        wall.id = 1001;
        parts.push(wall);
        let ix = Index::new(&parts, &geo);
        assert!(!ix.everywhere.is_empty(), "the big floor is looked at by every question");
        let mut checked = 0;
        for k in 0..3000 {
            let o = Vec3::new(rnd() * 300.0 - 150.0, rnd() * 60.0 - 5.0, rnd() * 300.0 - 150.0);
            // oblique, straight down, straight along x, and upward rays
            let d = match k % 4 { 0 => Vec3::new(rnd() - 0.5, rnd() - 0.8, rnd() - 0.5), 1 => Vec3::NEG_Y, 2 => Vec3::X * if rnd() > 0.5 { 1.0 } else { -1.0 }, _ => Vec3::new(rnd() - 0.5, rnd(), rnd() - 0.5) };
            let Ok(d) = Dir3::new(d) else { continue };
            let ray = Ray3d::new(o, d);
            let keep = |p: &Part| p.id % 7 != 3;
            let (a, b) = (raycast(&parts, &geo, ray, Some(5), keep), ix.raycast(&parts, &geo, ray, Some(5), keep));
            assert_eq!(a.as_ref().map(|h| h.part), b.as_ref().map(|h| h.part), "ray {k}: {ray:?}");
            if let (Some(a), Some(b)) = (a, b) { assert!((a.t - b.t).abs() < 1e-4, "ray {k}"); checked += 1; }
            // a spot: every part whose shape is straight above or below it is among those the index gives
            let at = o.xz();
            let down = raycast(&parts, &geo, Ray3d::new(Vec3::new(at.x, 500.0, at.y), Dir3::NEG_Y), None, |_| true);
            if let Some(id) = down.and_then(|h| h.part) { assert!(ix.at(&parts, at).any(|p| p.id == id), "spot {k}"); }
        }
        assert!(checked > 1000, "{checked}");
    }
}
