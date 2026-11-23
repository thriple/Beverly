//! The grass carpet, in the editor and the game: the user's grass system, ported from their game. Grass grows
//! wherever a material that grows grass (`level::Material::grass`: species and their shares) is laid on a face or
//! painted there with the brush, pen or bucket. Each flat face it grows on is a sheet (`sheets.rs`: its frame and
//! what grows at each spot, worked out from the strokes, never stored); each sheet is drawn as instanced tiles of one
//! lattice, every blade placed, shaped, coloured and swayed in the shader (`grass.wesl`), only within the carpet's
//! reach of the camera. It sways with its own wind (`GrassWind`) until the level has one.
mod defaults;
pub mod settings;
pub mod sheets;
pub mod species;

pub use settings::{GrassLighting, GrassSettings, GrassWind, TILE};
pub use species::{GrassSpecies, GrassSpeciesRegistry, MAX_SPECIES};

use crate::level::Level;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::ecs::system::SystemParam;
use bevy::image::{Image, ImageSampler};
use bevy::mesh::{Indices, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology};
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError, TextureDimension, TextureFormat, VertexFormat};
use bevy::shader::ShaderRef;
use sheets::Sheet;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

/// The names of the species a material can grow.
pub fn species_names(level: &Level) -> Vec<String> { GrassSpeciesRegistry::with(&level.foliage.species).species.into_iter().map(|s| s.name).collect() }

/// Each blade's place in its tile (the tile's own place arrives in the entity's translation).
pub const ATTRIBUTE_GRASS_CELL: MeshVertexAttribute = MeshVertexAttribute::new("GrassCell", 0x6772_6173, VertexFormat::Float32x2);

/// Tile sizes (blades along a side): a small face gets a small tile.
const SIZES: [u32; 4] = [8, 16, 32, TILE];

/// How long (seconds) a part that keeps changing waits between rebuilds of its grass (while a brush paints it).
const SETTLE: f64 = 0.15;

/// All the scalars, as `vec4`s, laid out as `Params` in grass.wesl.
#[derive(ShaderType, Clone, Default, PartialEq)]
pub struct GrassParams {
    origin: Vec4, across_u: Vec4, across_v: Vec4, normal: Vec4,
    lattice: Vec4, shape: Vec4, shape2: Vec4, look: Vec4, head: Vec4, misc: Vec4, grad: Vec4, grad2: Vec4,
    wind: Vec4, dark: Vec4, mid: Vec4, light: Vec4, sun_dir: Vec4, sun_color: Vec4, ambient: Vec4, flat_scale: Vec4,
}

/// A grass-growing material for the shader: up to four species and their shares, added up.
#[derive(ShaderType, Clone, Copy, Default, PartialEq)]
pub struct GpuGrower { species: Vec4, upto: Vec4 }

#[derive(ShaderType, Clone, Copy, PartialEq)]
pub struct GrowerTable { items: [GpuGrower; sheets::MOST_GROWERS] }

impl Default for GrowerTable { fn default() -> Self { GrowerTable { items: [GpuGrower::default(); sheets::MOST_GROWERS] } } }

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct GrassMaterial {
    #[uniform(0)]
    params: GrassParams,
    #[uniform(1)]
    species: species::SpeciesTable,
    #[uniform(2)]
    growers: GrowerTable,
    #[texture(3, sample_type = "float", filterable = false)]
    grows: Handle<Image>,
    #[texture(4, sample_type = "float", filterable = false)]
    colour: Handle<Image>,
    #[texture(5)]
    #[sampler(6)]
    gradients: Handle<Image>,
}

impl Material for GrassMaterial {
    fn vertex_shader() -> ShaderRef { "embedded://beverly/grass/grass.wesl".into() }
    fn fragment_shader() -> ShaderRef { "embedded://beverly/grass/grass.wesl".into() }
    // the vertex shader is its own, so the depth prepass and shadows (which would use the standard one) are off: the
    // carpet casts no shadows, and effects that read the prepass (SSAO, TAA, depth of field) don't see it
    fn enable_prepass() -> bool { false }
    fn enable_shadows() -> bool { false }
    fn specialize(_: &MaterialPipeline, descriptor: &mut RenderPipelineDescriptor, layout: &MeshVertexBufferLayoutRef, _: MaterialPipelineKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None; // blades have two sides
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0), ATTRIBUTE_GRASS_CELL.at_shader_location(1)])?];
        Ok(())
    }
}

pub struct GrassPlugin;

impl Plugin for GrassPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "grass.wesl");
        app.add_plugins(MaterialPlugin::<GrassMaterial>::default())
            .init_resource::<GrassSettings>()
            .init_resource::<GrassSpeciesRegistry>()
            .init_resource::<GrassWind>()
            .init_resource::<GrassLighting>();
    }
}

/// `n` x `n` blades, each two crossed quads of 5 corners: position (x: -0.5 to 0.5 across, y: along the blade, z:
/// which quad); the shader makes the rest.
fn tile_mesh(n: u32) -> Mesh {
    let (mut pos, mut cell, mut idx) = (Vec::<[f32; 3]>::new(), Vec::<[f32; 2]>::new(), Vec::<u32>::new());
    for j in 0..n {
        for i in 0..n {
            for plane in 0..2u32 {
                let base = pos.len() as u32;
                for t in [0.0f32, 0.5] {
                    pos.push([-0.5, t, plane as f32]);
                    pos.push([0.5, t, plane as f32]);
                }
                pos.push([0.0, 1.0, plane as f32]);
                cell.extend(std::iter::repeat_n([i as f32, j as f32], 5));
                for q in [[0, 1, 2], [2, 1, 3], [2, 3, 4]] { idx.extend(q.iter().map(|k| base + k)); }
            }
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(ATTRIBUTE_GRASS_CELL, cell)
        .with_inserted_indices(Indices::U32(idx))
}

/// Marks a tile of the carpet.
#[derive(Component)]
pub struct GrassTile;

/// A sheet as drawn: its material and the tiles out now.
struct Drawn {
    sheet: Sheet,
    material: Handle<GrassMaterial>,
    images: [Handle<Image>; 2],
    /// Blades along a tile's side, and the tiles out (by place in tiles) with their entities.
    n: u32,
    tiles: HashMap<(u32, u32), Entity>,
}

/// A part's grass: what it was made from, when, and its sheets.
struct PartGrass { key: u64, made: f64, sheets: Vec<Drawn> }

#[derive(Default)]
pub struct State {
    meshes: HashMap<u32, Handle<Mesh>>,
    gradients: Option<Handle<Image>>,
    parts: HashMap<u64, PartGrass>,
    /// What every material's params were last made from.
    shared: Option<(GrassParams, species::SpeciesTable, GrowerTable)>,
    /// The level's parts and materials as last looked at (their stamps), and whether a part's rebuild is waiting.
    seen: Option<(u64, u64)>,
    /// The species as the level has them, and the foliage stamp they were made at.
    species: Option<(u64, GrassSpeciesRegistry)>,
    waiting: bool,
}

/// The carpet kept in step with a level (see [`GrassSync::run`]).
#[derive(SystemParam)]
pub struct GrassSync<'w, 's> {
    commands: Commands<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<GrassMaterial>>,
    images: ResMut<'w, Assets<Image>>,
    settings: Res<'w, GrassSettings>,
    wind: Res<'w, GrassWind>,
    lighting: ResMut<'w, GrassLighting>,
    time: Res<'w, Time>,
    focus: Query<'w, 's, &'static GlobalTransform, With<crate::play::PlayCamera>>,
    sun: Query<'w, 's, (&'static DirectionalLight, &'static GlobalTransform)>,
    state: Local<'s, State>,
}

/// The growers as the shader reads them: each one's species (by index in the registry) and their shares, added up.
fn grower_table(level: &Level, growers: &[String], reg: &GrassSpeciesRegistry) -> GrowerTable {
    let mut t = GrowerTable::default();
    for (g, path) in t.items.iter_mut().zip(growers) {
        let list: Vec<(u8, f32)> = level.materials[path].grass.iter().filter_map(|w| Some((reg.id_of(&w.species)?, w.share.max(0.0)))).take(4).collect();
        let total: f32 = list.iter().map(|x| x.1).sum();
        let (mut ids, mut upto, mut sum) = ([0.0; 4], [1.0; 4], 0.0);
        for (k, (id, share)) in list.iter().enumerate() {
            sum += if total > 0.0 { share / total } else { 1.0 / list.len() as f32 };
            ids[k] = *id as f32;
            upto[k] = if k + 1 == list.len() { 1.0 } else { sum };
        }
        // unused slots repeat the last species and are never reached
        for k in list.len().max(1)..4 { ids[k] = ids[k - 1]; }
        *g = GpuGrower { species: Vec4::from(ids), upto: Vec4::from(upto) };
    }
    t
}

/// What a part's grass is made from, as one number (0: nothing on it grows grass).
fn part_key(level: &Level, p: &crate::level::Part, growers: &[String], table: &GrowerTable, spacing: f32) -> u64 {
    if !p.painted().into_iter().chain(p.surfaces.iter().map(|s| s.material.as_str())).any(|m| growers.iter().any(|g| g == m)) { return 0; }
    // the strokes as they look: a Mix's are its ground
    let shown = crate::level::shown(level, p);
    let used: Vec<&str> = p.painted().into_iter().chain(shown.painted()).chain(p.surfaces.iter().map(|s| s.material.as_str())).collect();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (p.visible, std::mem::discriminant(&p.shape)).hash(&mut h);
    for v in p.pos.iter().chain(&p.rot).chain(&p.size).chain(&p.color) { v.to_bits().hash(&mut h); }
    for s in &p.surfaces { (s.face, &s.material).hash(&mut h); }
    for s in p.strokes.iter().chain(&shown.strokes) {
        (&s.material, s.size.to_bits(), s.hardness.to_bits(), s.strength.to_bits(), s.lays as u8, s.fill, &s.faces, format!("{:?}", s.rules)).hash(&mut h);
        for q in &s.points { q.map(f32::to_bits).hash(&mut h); }
    }
    for m in used {
        if let Some(m) = level.materials.get(m) { (m.tint.map(f32::to_bits), m.grass_density.to_bits()).hash(&mut h); }
    }
    growers.hash(&mut h);
    spacing.to_bits().hash(&mut h);
    for g in &table.items { (g.species.to_array().map(f32::to_bits), g.upto.to_array().map(f32::to_bits)).hash(&mut h); }
    h.finish()
}

fn lin(c: [f32; 3]) -> Vec4 { let l = Color::srgb(c[0], c[1], c[2]).to_linear(); Vec4::new(l.red, l.green, l.blue, 1.0) }

impl GrassSync<'_, '_> {
    fn shared_params(&mut self, s: &GrassSettings, species: usize) -> GrassParams {
        if self.lighting.sync_directional_light && let Some((l, t)) = self.sun.iter().next() {
            let c = l.color.to_linear();
            let (dir, colour) = (-Vec3::from(t.forward()), Vec3::new(c.red, c.green, c.blue) * 0.9);
            if self.lighting.sun_dir != dir || self.lighting.sun_color != colour { (self.lighting.sun_dir, self.lighting.sun_color) = (dir, colour); }
        }
        let (wind, light) = (&*self.wind, &*self.lighting);
        let strength = s.sway * (0.22 + wind.speed * 0.052).min(1.0) * 0.16 * 0.5;
        GrassParams {
            lattice: Vec4::new(s.radius, s.edge, s.min_scale, s.keep()),
            shape: Vec4::new(s.height, s.height_var, s.width, s.bend),
            shape2: Vec4::new(s.jitter, s.clump, s.clump_size, s.min_width),
            look: Vec4::new(s.flat, s.ground, s.patch, s.tip),
            head: Vec4::new(s.head_radius, s.head_var, s.head_var_hi, s.plume_at),
            misc: Vec4::new(s.pad_lift, s.ivy_normal_bias, s.ivy_direct_mul, s.ivy_ground_tint),
            grad: Vec4::new(s.gradient_amount, s.gradient_alt, s.gradient_lo, s.gradient_hi),
            grad2: Vec4::new(s.gradient_pow, species.min(MAX_SPECIES) as f32, 0.0, 0.0),
            wind: Vec4::new(wind.direction.cos(), wind.direction.sin(), strength, s.gust),
            dark: lin(s.dark), mid: lin(s.mid), light: lin(s.light),
            sun_dir: light.sun_dir.extend(0.0), sun_color: light.sun_color.extend(0.0), ambient: light.ambient.extend(0.0), flat_scale: light.flat_scale.extend(0.0),
            ..default()
        }
    }

    fn sheet_params(base: &GrassParams, s: &Sheet, spacing: f32, seed: f32) -> GrassParams {
        GrassParams { origin: s.origin.extend(spacing), across_u: s.u.extend(s.size.x), across_v: s.v.extend(s.size.y), normal: s.normal.extend(seed), ..base.clone() }
    }

    fn image(&mut self, s: &Sheet, data: Vec<u8>, format: TextureFormat) -> Handle<Image> {
        self.images.add(Image::new(Extent3d { width: s.dims.x, height: s.dims.y, depth_or_array_layers: 1 }, TextureDimension::D2, data, format, RenderAssetUsages::RENDER_WORLD))
    }

    fn drop_part(&mut self, g: PartGrass) {
        for d in g.sheets {
            for (_, e) in d.tiles { self.commands.entity(e).try_despawn(); }
            self.materials.remove(&d.material);
            for i in d.images { self.images.remove(&i); }
        }
    }

    /// Make the carpet match `level`: a part whose grass changed gets its sheets again (no more often than every
    /// `SETTLE` seconds while it keeps changing), and the tiles within the carpet's reach of the camera are out.
    pub fn run(&mut self, level: &Level) {
        let now = self.time.elapsed_secs_f64();
        let growers = sheets::growers(level);
        // the level's reach and quality, and its species with the built-in ones
        let settings = GrassSettings { radius: level.foliage.reach, tiles: level.foliage.tiles(), ..(*self.settings).clone() };
        if self.state.species.as_ref().map(|s| s.0) != Some(level.foliage.stamp()) { self.state.species = Some((level.foliage.stamp(), GrassSpeciesRegistry::with(&level.foliage.species))); }
        let species = self.state.species.take().expect("just made");
        let table = grower_table(level, &growers, &species.1);
        let spacing = settings.spacing();
        let base = self.shared_params(&settings, species.1.species.len());
        let shared = (base.clone(), species.1.pack(), table);
        self.state.species = Some(species);
        let shared_changed = self.state.shared.as_ref() != Some(&shared);
        if self.state.gradients.is_none() {
            let mut atlas = Image::new(Extent3d { width: defaults::GRADIENT_W, height: defaults::GRADIENT_H, depth_or_array_layers: 1 }, TextureDimension::D2, defaults::GRADIENT_RGBA.to_vec(), TextureFormat::Rgba8Unorm, RenderAssetUsages::RENDER_WORLD);
            atlas.sampler = ImageSampler::linear();
            self.state.gradients = Some(self.images.add(atlas));
        }
        let stamps = (level.parts.stamp(), level.materials.stamp());
        let look = shared_changed || self.state.waiting || self.state.seen != Some(stamps);
        self.state.seen = Some(stamps);
        self.state.waiting = false;
        // gone: the part, or all its grass
        let live: HashSet<u64> = level.parts.iter().map(|p| p.id).collect();
        let gone: Vec<u64> = if look { self.state.parts.keys().filter(|id| !live.contains(id)).copied().collect() } else { vec![] };
        for id in gone { let g = self.state.parts.remove(&id).unwrap(); self.drop_part(g); }
        for p in level.parts.iter().filter(|_| look) {
            let key = part_key(level, p, &growers, &shared.2, spacing);
            match self.state.parts.get(&p.id) {
                Some(g) if g.key == key => continue,
                Some(g) if now - g.made < SETTLE => { self.state.waiting = true; continue }
                None if key == 0 => continue,
                _ => {}
            }
            if let Some(g) = self.state.parts.remove(&p.id) { self.drop_part(g); }
            let mut drawn = vec![];
            for s in sheets::sheets(level, p, &growers) {
                let grows = self.image(&s, s.grows.clone(), TextureFormat::Rgba8Unorm);
                let colour = self.image(&s, s.colour.clone(), TextureFormat::Rgba8UnormSrgb);
                let seed = ((p.id * 48 + s.face as u64) % 4093) as f32 * 1.618;
                let material = self.materials.add(GrassMaterial { params: Self::sheet_params(&base, &s, spacing, seed), species: shared.1, growers: shared.2, grows: grows.clone(), colour: colour.clone(), gradients: self.state.gradients.clone().unwrap() });
                let cells = (s.size / spacing).ceil().max_element() as u32;
                let n = SIZES.iter().copied().find(|n| *n >= cells).unwrap_or(TILE);
                drawn.push(Drawn { sheet: s, material, images: [grows, colour], n, tiles: HashMap::new() });
            }
            self.state.parts.insert(p.id, PartGrass { key, made: now, sheets: drawn });
        }
        if shared_changed {
            for d in self.state.parts.values().flat_map(|g| &g.sheets) {
                if let Some(mut m) = self.materials.get_mut(&d.material) {
                    m.params = Self::sheet_params(&base, &d.sheet, spacing, m.params.normal.w);
                    (m.species, m.growers) = (shared.1, shared.2);
                }
            }
            self.state.shared = Some(shared);
        }
        // the tiles within reach of the camera
        let Some(focus) = self.focus.iter().next().map(|t| t.translation()) else { return };
        let reach = settings.radius;
        let mut spawn: Vec<(u64, usize, (u32, u32), u32)> = vec![];
        let mut despawn = vec![];
        for (id, g) in self.state.parts.iter_mut() {
            for (k, d) in g.sheets.iter_mut().enumerate() {
                let s = &d.sheet;
                let side = d.n as f32 * spacing;
                let count = (s.size / side).ceil().as_uvec2();
                let q = focus - s.origin;
                let (fu, fv, fn_) = (q.dot(s.u), q.dot(s.v), q.dot(s.normal));
                let mut want = HashSet::new();
                for tj in 0..count.y {
                    for ti in 0..count.x {
                        let (u0, v0) = (ti as f32 * side, tj as f32 * side);
                        let du = (u0 - fu).max(fu - u0 - side).max(0.0);
                        let dv = (v0 - fv).max(fv - v0 - side).max(0.0);
                        if (du * du + dv * dv + fn_ * fn_).sqrt() <= reach + spacing { want.insert((ti, tj)); }
                    }
                }
                d.tiles.retain(|t, e| { let keep = want.contains(t); if !keep { despawn.push(*e); } keep });
                for t in want { if !d.tiles.contains_key(&t) { spawn.push((*id, k, t, d.n)); } }
            }
        }
        for e in despawn { self.commands.entity(e).try_despawn(); }
        for (id, k, (ti, tj), n) in spawn {
            let mesh = self.state.meshes.entry(n).or_insert_with(|| self.meshes.add(tile_mesh(n))).clone();
            let d = &mut self.state.parts.get_mut(&id).unwrap().sheets[k];
            let e = self.commands.spawn((GrassTile, Mesh3d(mesh), MeshMaterial3d(d.material.clone()), Transform::from_xyz((ti * n) as f32, 0.0, (tj * n) as f32), NoFrustumCulling)).id();
            d.tiles.insert((ti, tj), e);
        }
    }
}
