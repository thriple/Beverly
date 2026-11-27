//! Custom shaders: any WESL file in the project's `assets/shaders/` can be a material's look. A header of comment
//! lines at the top of the file tells Beverly what the shader asks for:
//!
//! ```text
//! // @setting speed "Speed" 0..5 = 1          a number (a slider)
//! // @setting glow colour "Glow" = 1 0.5 0.2   a colour
//! // @setting waves toggle "Waves" = on        on or off
//! // @see-through                              the shader draws see-through (it can't go in virtual geometry)
//! // @raster vertex_offset 0.5                 it moves vertices, by at most this far (m)
//! // @raster alpha_mask                        it cuts its edges out (discard)
//! ```
//!
//! The raster hooks are functions the shader defines, run wherever the part is drawn (the main pass, and also the depth
//! and shadow passes, so shadows follow the moved or cut-out shape):
//!
//! ```text
//! fn vertex_offset(world: vec3<f32>, normal: vec3<f32>, uv: vec2<f32>) -> vec3<f32>   how far to move this vertex (world space)
//! fn alpha_mask(world: vec3<f32>, uv: vec2<f32>) -> f32                               below 0.5 the pixel is cut away
//! ```
//!
//! Beverly writes the vertex stage around `vertex_offset` (its normal is only known in the main pass), and gives the
//! part's bounds the declared reach so it is not culled at the screen's edge. A shader's own `fragment` calls
//! `alpha_mask` itself (`if alpha_mask(...) < 0.5 { discard; }`). The depth and shadow passes build the whole file but
//! have no `FragmentOutput`, so a shader with hooks puts its fragment imports and `fragment` under `@if(!PREPASS_PIPELINE)`
//! (see `tests/shaders/assets/shaders/lean.wesl`). `@see-through` makes an opaque material blend and
//! `@raster alpha_mask` makes it cut out. A part with a vertex offset is not skinned.
//!
//! Beverly adds, after the shader's own code, the settings' values and one function for each: `setting_speed()` is an
//! f32, `setting_glow()` a vec3, `setting_waves()` a bool. The shader is a fragment shader for Bevy's standard
//! material (see `paint.wesl`), so it can do whatever Bevy's lighting does. Values come from the material file
//! (`shader`, `shader_values`), by setting name, else the header's defaults. The file is read again whenever it
//! changes (about twice a second), so edits show at once; a file that doesn't parse keeps the last good one.
use crate::models::ProjectRoot;
use bevy::asset::uuid::Uuid;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline, MaterialPlugin};
use bevy::prelude::*;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::VisibilitySystems;
use bevy::material::descriptor::PipelineDescriptor;
use bevy::render::render_resource::{AsBindGroup, CachedPipelineState, PipelineCache, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError};
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::shader::{Shader, ShaderDefVal, ShaderRef};
use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// How many settings a shader can have (each takes a vec4 of the uniform).
pub const SLOTS: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub enum Kind { Number { min: f32, max: f32 }, Colour, Toggle }

#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub name: String,
    pub label: String,
    pub kind: Kind,
    /// A number or toggle in x; a colour in xyz.
    pub default: [f32; 3],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Header {
    pub settings: Vec<Setting>,
    pub see_through: bool,
    /// How far (m) the shader moves vertices, if it does.
    pub vertex_offset: Option<f32>,
    pub alpha_mask: bool,
}

/// The words of a header line: split on spaces, a quoted text is one word (marked with a leading quote mark).
fn words(s: &str) -> Result<Vec<String>, String> {
    let (mut out, mut cur, mut quoted) = (vec![], String::new(), false);
    for c in s.chars() {
        match c {
            '"' => { quoted = !quoted; if !quoted { out.push(format!("\"{}", std::mem::take(&mut cur))); } }
            c if c.is_whitespace() && !quoted => { if !cur.is_empty() { out.push(std::mem::take(&mut cur)); } }
            c => cur.push(c),
        }
    }
    if quoted { return Err("a quote is never closed".into()); }
    if !cur.is_empty() { out.push(cur); }
    Ok(out)
}

fn number(w: &str, what: &str) -> Result<f32, String> { w.parse::<f32>().map_err(|_| format!("{what} should be a number, not {w:?}")) }

/// Reads the header lines (`// @...`) of a shader file. The error says which line.
pub fn parse(src: &str) -> Result<Header, String> {
    let mut h = Header::default();
    for (n, line) in src.lines().enumerate() {
        let Some(rest) = line.trim().strip_prefix("//").map(str::trim).and_then(|l| l.strip_prefix('@')) else { continue };
        let at = |e: String| format!("line {}: {e}", n + 1);
        let w = words(rest).map_err(at)?;
        let at = |e: String| format!("line {}: {e}", n + 1);
        match w.first().map(String::as_str) {
            Some("setting") => {
                if h.settings.len() >= SLOTS { return Err(at(format!("a shader can have {SLOTS} settings"))); }
                let name = w.get(1).ok_or_else(|| at("a setting needs a name".into()))?.clone();
                if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') || name.chars().next().is_none_or(|c| c.is_ascii_digit()) {
                    return Err(at(format!("{name:?} can't be a name (letters, digits and _, not starting with a digit)")));
                }
                if h.settings.iter().any(|s| s.name == name) { return Err(at(format!("{name} is there twice"))); }
                let eq = w.iter().position(|x| x == "=");
                let (before, values) = match eq { Some(i) => (&w[2..i], &w[i + 1..]), None => (&w[2..], &w[w.len()..]) };
                // the label is the quoted word, the kind the other one
                let label = before.iter().find_map(|w| w.strip_prefix('"')).map_or_else(|| name.clone(), str::to_string);
                let kind = match before.iter().find(|w| !w.starts_with('"')).map(String::as_str) {
                    Some("colour") | Some("color") => Kind::Colour,
                    Some("toggle") => Kind::Toggle,
                    Some(r) if r.contains("..") => {
                        let (lo, hi) = r.split_once("..").unwrap_or_default();
                        let (min, max) = (number(lo, "the lowest").map_err(at)?, number(hi, "the highest").map_err(at)?);
                        if min >= max { return Err(at(format!("{min}..{max}: the lowest must be below the highest"))); }
                        Kind::Number { min, max }
                    }
                    _ => return Err(at(format!("{name}: say its kind: a range such as 0..5, colour or toggle"))),
                };
                let default = match &kind {
                    Kind::Number { min, max } => { let d = match values.first() { Some(v) => number(v, "the starting value").map_err(at)?, None => *min }; [d.clamp(*min, *max), 0.0, 0.0] }
                    Kind::Colour => {
                        let mut c = [1.0; 3];
                        if !values.is_empty() {
                            if values.len() != 3 { return Err(at("a colour starts as three numbers (red green blue, 0 to 1)".into())); }
                            for (i, v) in values.iter().enumerate() { c[i] = number(v, "the starting colour").map_err(at)?.clamp(0.0, 1.0); }
                        }
                        c
                    }
                    Kind::Toggle => [matches!(values.first().map(String::as_str), Some("on" | "true" | "yes" | "1")) as u8 as f32, 0.0, 0.0],
                };
                h.settings.push(Setting { name, label, kind, default });
            }
            Some("see-through") => h.see_through = true,
            Some("raster") => match w.get(1).map(String::as_str) {
                Some("vertex_offset") => h.vertex_offset = Some(number(w.get(2).map_or("", |s| s), "how far it moves vertices (m)").map_err(at)?.max(0.0)),
                Some("alpha_mask") => h.alpha_mask = true,
                other => return Err(at(format!("@raster {}: vertex_offset or alpha_mask", other.unwrap_or("")))),
            },
            other => return Err(at(format!("@{}: not something a shader header says (setting, see-through, raster)", other.unwrap_or("")))),
        }
    }
    Ok(h)
}

/// What Beverly adds after the shader's code: the values and a function for each setting.
pub fn prelude(h: &Header) -> String {
    let mut s = format!("\n// ---- added by Beverly: the settings ----\nstruct BeverlySettings {{ v: array<vec4<f32>, {SLOTS}> }}\n@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> beverly_settings: BeverlySettings;\n");
    for (i, set) in h.settings.iter().enumerate() {
        let (ty, get) = match set.kind { Kind::Number { .. } => ("f32", "x"), Kind::Colour => ("vec3<f32>", "xyz"), Kind::Toggle => ("bool", "x > 0.5") };
        s += &format!("fn setting_{}() -> {ty} {{ return beverly_settings.v[{i}].{get}; }}\n", set.name);
    }
    if h.vertex_offset.is_some() { s += VERTEX_STAGE; }
    if h.alpha_mask { s += PREPASS_FRAGMENT; }
    s
}

/// The vertex stage around a shader's `vertex_offset`, for the main pass and for the depth and shadow passes (Bevy's own
/// stages, without skinning and morph targets, which a shader that moves vertices does not do).
const VERTEX_STAGE: &str = "
@if(!PREPASS_PIPELINE)
@vertex
fn beverly_vertex(v: bevy_pbr::render::forward_io::Vertex) -> bevy_pbr::render::forward_io::VertexOutput {
    var out: bevy_pbr::render::forward_io::VertexOutput;
    let vertex = bevy_pbr::render::forward_io::decompress_vertex(v, v.instance_index);
    let world_from_local = bevy_pbr::render::mesh_functions::get_world_from_local(v.instance_index);
    let at = bevy_pbr::render::mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    var normal = vec3<f32>(0.0, 1.0, 0.0);
    var uv = vec2<f32>(0.0, 0.0);
@if(VERTEX_NORMALS)
    normal = bevy_pbr::render::mesh_functions::mesh_normal_local_to_world(vertex.normal, v.instance_index);
@if(VERTEX_UVS_A)
    uv = vertex.uv;
    let moved = at.xyz + vertex_offset(at.xyz, normal, uv);
    out.world_position = vec4<f32>(moved, at.w);
    out.position = bevy_pbr::render::view_transformations::position_world_to_clip(moved);
    out.world_normal = normal;
@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;
@if(VERTEX_TANGENTS)
    out.world_tangent = bevy_pbr::render::mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, v.instance_index);
@if(VERTEX_COLORS)
    out.color = vertex.color;
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = v.instance_index;
@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = bevy_pbr::render::mesh_functions::get_visibility_range_dither_level(v.instance_index, world_from_local[3]);
    return out;
}

@if(PREPASS_PIPELINE)
@vertex
fn beverly_vertex(v: bevy_pbr::prepass::io::Vertex) -> bevy_pbr::prepass::io::VertexOutput {
    var out: bevy_pbr::prepass::io::VertexOutput;
    let vertex = bevy_pbr::prepass::io::decompress_vertex(v, v.instance_index);
    let world_from_local = bevy_pbr::render::mesh_functions::get_world_from_local(v.instance_index);
    let at = bevy_pbr::render::mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));
    var normal = vec3<f32>(0.0, 1.0, 0.0);
    var uv = vec2<f32>(0.0, 0.0);
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS && VERTEX_NORMALS)
    normal = bevy_pbr::render::mesh_functions::mesh_normal_local_to_world(vertex.normal, v.instance_index);
@if(VERTEX_UVS_A)
    uv = vertex.uv;
    let moved = at.xyz + vertex_offset(at.xyz, normal, uv);
    out.world_position = vec4<f32>(moved, at.w);
    out.position = bevy_pbr::render::view_transformations::position_world_to_clip(moved);
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}
@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS) {
    out.world_normal = normal;
@if(VERTEX_TANGENTS)
    out.world_tangent = bevy_pbr::render::mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, v.instance_index);
}
@if(VERTEX_COLORS)
    out.color = vertex.color;
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = v.instance_index;
@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = bevy_pbr::render::mesh_functions::get_visibility_range_dither_level(v.instance_index, world_from_local[3]);
    return out;
}
";

/// The depth and shadow passes' fragment stage around a shader's `alpha_mask`.
const PREPASS_FRAGMENT: &str = "
@if(PREPASS_PIPELINE && PREPASS_FRAGMENT)
@fragment
fn beverly_prepass_fragment(in: bevy_pbr::prepass::io::VertexOutput) -> bevy_pbr::prepass::io::FragmentOutput {
    var uv = vec2<f32>(0.0, 0.0);
@if(VERTEX_UVS_A)
    uv = in.uv;
    if alpha_mask(in.world_position.xyz, uv) < 0.5 { discard; }
    var out: bevy_pbr::prepass::io::FragmentOutput;
@if(NORMAL_PREPASS)
    out.normal = vec4<f32>(in.world_normal * 0.5 + vec3<f32>(0.5), 1.0);
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    out.frag_depth = in.unclipped_depth;
    return out;
}

@if(PREPASS_PIPELINE && !PREPASS_FRAGMENT)
@fragment
fn beverly_prepass_fragment(in: bevy_pbr::prepass::io::VertexOutput) {
    var uv = vec2<f32>(0.0, 0.0);
@if(VERTEX_UVS_A)
    uv = in.uv;
    if alpha_mask(in.world_position.xyz, uv) < 0.5 { discard; }
}
";

/// The values a shader gets for a material: its own where it set them, else the header's defaults.
pub fn values(h: &Header, set: &BTreeMap<String, Vec<f32>>) -> [Vec4; SLOTS] {
    let mut v = [Vec4::ZERO; SLOTS];
    for (i, s) in h.settings.iter().enumerate() {
        let d = s.default;
        let mut x = [d[0], d[1], d[2]];
        if let Some(own) = set.get(&s.name) {
            for (k, o) in own.iter().take(if s.kind == Kind::Colour { 3 } else { 1 }).enumerate() { x[k] = *o; }
        }
        if let Kind::Number { min, max } = s.kind { x[0] = x[0].clamp(min, max); }
        v[i] = Vec4::new(x[0], x[1], x[2], 0.0);
    }
    v
}

#[derive(ShaderType, Clone, Copy, Debug, Default)]
pub struct Values { pub v: [Vec4; SLOTS] }

/// The extension that gives a material its shader and values.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
#[bind_group_data(ShaderKey)]
pub struct ShaderLayer {
    #[uniform(100)]
    pub values: Values,
    /// The shader file this look came from and the values the material set (to work the values out again when the
    /// header changes).
    pub path: String,
    pub set: BTreeMap<String, Vec<f32>>,
    pub shader: Handle<Shader>,
    /// Which raster hooks its header declares (a vertex offset, an alpha mask), and the look's own alpha mode before the
    /// header made it blend or cut out.
    pub vertex: bool,
    pub mask: bool,
    pub alpha: AlphaMode,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderKey { id: Option<Uuid>, vertex: bool, mask: bool }

impl From<&ShaderLayer> for ShaderKey {
    fn from(l: &ShaderLayer) -> Self { ShaderKey { id: match &l.shader { Handle::Uuid(u, _) => Some(*u), _ => None }, vertex: l.vertex, mask: l.mask } }
}

pub type ShaderMaterial = ExtendedMaterial<StandardMaterial, ShaderLayer>;

impl MaterialExtension for ShaderLayer {
    /// The depth and shadow passes use Bevy's own stages, which don't read the material, unless the material names one.
    /// Naming Bevy's own keeps them as they are and lets the shader's hooks read its settings there.
    fn prepass_vertex_shader() -> ShaderRef { "embedded://bevy_pbr/prepass/prepass.wesl".into() }

    /// The material's own fragment shader in the main pass, its vertex stage when it moves vertices, and in the depth
    /// and shadow passes its vertex stage and (when it cuts its edges out) its fragment stage.
    fn specialize(_: &MaterialExtensionPipeline, descriptor: &mut RenderPipelineDescriptor, _: &MeshVertexBufferLayoutRef, key: MaterialExtensionKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        let k = key.bind_group_data;
        let Some(uuid) = k.id else { return Ok(()) };
        let shader: Handle<Shader> = Handle::Uuid(uuid, PhantomData);
        // the depth and shadow passes say so in their shader definitions (the key's bits are no sure sign: an opaque
        // look that cuts holes sets the same one in the main pass)
        let prepass = descriptor.vertex.shader_defs.iter().any(|d| matches!(d, ShaderDefVal::Bool(n, true) if n == "PREPASS_PIPELINE"));
        if k.vertex {
            descriptor.vertex.shader = shader.clone();
            descriptor.vertex.entry_point = Some("beverly_vertex".into());
        }
        if let Some(fragment) = descriptor.fragment.as_mut() {
            if !prepass {
                fragment.shader = shader;
            } else if k.mask {
                fragment.shader = shader;
                fragment.entry_point = Some("beverly_prepass_fragment".into());
            }
        }
        Ok(())
    }
}

struct Entry {
    handle: Handle<Shader>,
    header: Option<Header>,
    error: Option<String>,
    stamp: Option<SystemTime>,
}

/// The project's shaders in use: the last good header of each, and any error in its file now.
#[derive(Resource, Default)]
pub struct Shaders {
    entries: HashMap<String, Entry>,
    /// Counts up whenever a shader's header changes.
    pub generation: u64,
    checked: f64,
    /// What the graphics side could not build from each shader's code (found there, by shader).
    compile: Arc<Mutex<HashMap<Uuid, String>>>,
}

impl Shaders {
    /// The shader at this project path (loaded on the next check if it isn't yet).
    pub fn request(&mut self, path: &str) -> Handle<Shader> {
        self.entries.entry(path.to_owned()).or_insert_with(|| {
            let hash = crate::update::sha256_hex(path.as_bytes());
            Entry { handle: Handle::Uuid(Uuid::from_u128(u128::from_str_radix(&hash[..32], 16).unwrap_or(1)), PhantomData), header: None, error: None, stamp: None }
        }).handle.clone()
    }
    pub fn header(&self, path: &str) -> Option<&Header> { self.entries.get(path).and_then(|e| e.header.as_ref()) }
    /// What is wrong with the file now: its header or reading it (the last good one is still in use), or the code the
    /// graphics side could not build.
    pub fn error(&self, path: &str) -> Option<String> {
        let e = self.entries.get(path)?;
        e.error.clone().or_else(|| match e.handle { Handle::Uuid(u, _) => self.compile.lock().ok()?.get(&u).map(|m| format!("{path}: {m}")), _ => None })
    }
    pub fn paths(&self) -> impl Iterator<Item = &String> { self.entries.keys() }
}

/// The shader files in a project, by path.
pub fn list(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir.join("assets/shaders")).into_iter().flatten().flatten()
        .filter_map(|e| e.file_name().to_str().filter(|n| n.ends_with(".wesl")).map(|n| format!("assets/shaders/{n}")))
        .collect();
    v.sort();
    v
}

/// Reads the shaders in use again when their files change.
pub fn watch(time: Res<Time<Real>>, root: Res<ProjectRoot>, mut shaders: ResMut<Shaders>, mut assets: ResMut<Assets<Shader>>) {
    let now = time.elapsed_secs_f64();
    if now - shaders.checked < 0.5 { return; }
    shaders.checked = now;
    let dir = root.get();
    let mut changed = false;
    for (path, e) in shaders.entries.iter_mut() {
        let file = dir.join(path);
        let stamp = std::fs::metadata(&file).and_then(|m| m.modified()).ok();
        if stamp == e.stamp && (e.header.is_some() || e.error.is_some()) { continue; }
        e.stamp = stamp;
        let src = match std::fs::read_to_string(&file) {
            Ok(s) => s,
            Err(err) => { e.error = Some(format!("{path}: {err}")); continue; }
        };
        match parse(&src) {
            Ok(h) => {
                let code = format!("{src}{}", prelude(&h));
                let _ = assets.insert(e.handle.id(), Shader::from_wesl(code, format!("project://{path}")));
                e.error = None;
                if e.header.as_ref() != Some(&h) { changed = true; }
                e.header = Some(h);
            }
            Err(err) => e.error = Some(format!("{path}: {err}")),
        }
    }
    if changed { shaders.generation += 1; }
}

/// After a header changed: work every shader material's values, hooks and alpha mode out again.
pub fn refresh(shaders: Res<Shaders>, mut mats: ResMut<Assets<ShaderMaterial>>, mut seen: Local<u64>) {
    if *seen == shaders.generation { return; }
    *seen = shaders.generation;
    for (_, m) in mats.iter_mut() {
        let Some(h) = shaders.header(&m.extension.path) else { continue };
        let own = m.extension.alpha.clone();
        m.extension.values = Values { v: values(h, &m.extension.set) };
        m.extension.vertex = h.vertex_offset.is_some();
        m.extension.mask = h.alpha_mask;
        m.base.alpha_mode = match own {
            AlphaMode::Opaque if h.see_through => AlphaMode::Blend,
            AlphaMode::Opaque if h.alpha_mask => AlphaMode::Mask(0.5),
            other => other,
        };
    }
}

/// How far a part's bounds were grown for its shader's vertex offset, so it isn't culled while its moved vertices are
/// still on screen.
#[derive(Component)]
pub struct Reach(f32);

pub fn grow_bounds(shaders: Res<Shaders>, mats: Res<Assets<ShaderMaterial>>, mut commands: Commands, mut parts: Query<(Entity, &MeshMaterial3d<ShaderMaterial>, &mut Aabb, Option<&Reach>)>) {
    for (e, m, mut aabb, had) in &mut parts {
        let want = mats.get(&m.0).and_then(|m| shaders.header(&m.extension.path)).and_then(|h| h.vertex_offset).unwrap_or(0.0);
        // a new box (Bevy worked it out again) has none of the growth
        let had = if aabb.is_added() { 0.0 } else { had.map_or(0.0, |r| r.0) };
        if (want - had).abs() > crate::guard::SHORT {
            aabb.half_extents += Vec3A::splat(want - had);
            commands.entity(e).insert(Reach(want));
        }
    }
}

/// The render side: which shaders' pipelines failed, and why.
fn report(cache: Res<PipelineCache>, found: Res<Found>, mut tick: Local<u32>) {
    *tick += 1;
    if *tick % 20 != 1 { return; }
    let mut errors = HashMap::new();
    for p in cache.pipelines() {
        let (CachedPipelineState::Err(e), PipelineDescriptor::RenderPipelineDescriptor(d)) = (&p.state, &p.descriptor) else { continue };
        for h in [Some(&d.vertex.shader), d.fragment.as_ref().map(|f| &f.shader)].into_iter().flatten() {
            if let Handle::Uuid(u, _) = h { errors.insert(*u, brief(&e.to_string())); }
        }
    }
    if let Ok(mut f) = found.0.lock() { *f = errors; }
}

/// The part of a shader build error worth showing: what is wrong, and at which line of the file when it says.
fn brief(e: &str) -> String {
    let what = e.lines().find_map(|l| l.trim().strip_prefix("error: ")).unwrap_or_else(|| e.lines().next().unwrap_or(e)).trim();
    let line = e.lines().find_map(|l| l.trim().split_once(" |").and_then(|(n, _)| n.trim().parse::<u32>().ok()));
    match line { Some(n) => format!("line {n}: {what}"), None => what.to_string() }
}

#[derive(Resource)]
struct Found(Arc<Mutex<HashMap<Uuid, String>>>);

pub struct ShaderPlugin;

impl Plugin for ShaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ShaderMaterial>::default()).init_resource::<Shaders>()
            .add_systems(Update, (watch, refresh).chain())
            .add_systems(PostUpdate, grow_bounds.after(VisibilitySystems::CalculateBounds));
        let found = Found(app.world().resource::<Shaders>().compile.clone());
        if let Some(render) = app.get_sub_app_mut(RenderApp) { render.insert_resource(found).add_systems(Render, report.in_set(RenderSystems::Cleanup)); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_error_is_cut_down_to_what_and_where() {
        let e = "failed to process shader error:\nerror: cannot find declaration of `nope`\n   |\n25 |     return nope(cell);\n   |            ^^^^^^^^^^ cannot find declaration of `nope`\n";
        assert_eq!(brief(e), "line 25: cannot find declaration of `nope`");
        assert_eq!(brief("Shader import not yet available."), "Shader import not yet available.");
    }

    #[test]
    fn a_header_gives_settings_with_their_kinds_and_defaults() {
        let h = parse("// a comment\n// @setting speed \"Speed\" 0..5 = 1\n// @setting glow colour \"Glow\" = 1 0.5 0.2\n// @setting waves toggle \"Waves\" = on\n// @see-through\n// @raster vertex_offset 0.5\n// @raster alpha_mask\nfn x() {}").unwrap();
        assert_eq!(h.settings.len(), 3);
        assert_eq!((h.settings[0].kind.clone(), h.settings[0].default[0], h.settings[0].label.as_str()), (Kind::Number { min: 0.0, max: 5.0 }, 1.0, "Speed"));
        assert_eq!(h.settings[1].default, [1.0, 0.5, 0.2]);
        assert_eq!(h.settings[2].default[0], 1.0);
        assert!(h.see_through && h.alpha_mask && h.vertex_offset == Some(0.5));
    }

    #[test]
    fn a_bad_header_line_says_which_line() {
        assert!(parse("\n// @setting a 5..1 = 1").unwrap_err().starts_with("line 2:"));
        assert!(parse("// @setting 1a 0..1").unwrap_err().contains("can't be a name"));
        assert!(parse("// @setting a 0..1 = x").unwrap_err().contains("should be a number"));
        assert!(parse("// @setting a 0..1\n// @setting a 0..1").unwrap_err().contains("twice"));
        assert!(parse("// @wobble").unwrap_err().contains("not something"));
        assert!(parse("// @setting a \"Open").unwrap_err().contains("quote"));
    }

    #[test]
    fn values_come_from_the_material_else_the_defaults_kept_in_range() {
        let h = parse("// @setting speed \"Speed\" 0..5 = 1\n// @setting glow colour \"Glow\" = 1 0 0").unwrap();
        let none = values(&h, &BTreeMap::new());
        assert_eq!((none[0].x, none[1].xyz()), (1.0, Vec3::new(1.0, 0.0, 0.0)));
        let own = values(&h, &BTreeMap::from([("speed".into(), vec![99.0]), ("glow".into(), vec![0.0, 1.0, 0.0])]));
        assert_eq!((own[0].x, own[1].xyz()), (5.0, Vec3::new(0.0, 1.0, 0.0)));
    }

    #[test]
    fn the_prelude_has_a_function_per_setting() {
        let p = prelude(&parse("// @setting speed 0..5\n// @setting glow colour\n// @setting on toggle").unwrap());
        assert!(p.contains("fn setting_speed() -> f32 { return beverly_settings.v[0].x; }"));
        assert!(p.contains("fn setting_glow() -> vec3<f32> { return beverly_settings.v[1].xyz; }"));
        assert!(p.contains("fn setting_on() -> bool { return beverly_settings.v[2].x > 0.5; }"));
    }
}
