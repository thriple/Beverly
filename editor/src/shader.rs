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
//! Beverly adds, after the shader's own code, the settings' values and one function for each: `setting_speed()` is an
//! f32, `setting_glow()` a vec3, `setting_waves()` a bool. The shader is a fragment shader for Bevy's standard
//! material (see `paint.wesl`), so it can do whatever Bevy's lighting does. Values come from the material file
//! (`shader`, `shader_values`), by setting name, else the header's defaults. The file is read again whenever it
//! changes (about twice a second), so edits show at once; a file that doesn't parse keeps the last good one.
use crate::models::ProjectRoot;
use bevy::asset::uuid::Uuid;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline, MaterialPlugin, MeshPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::shader::Shader;
use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
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
    s
}

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
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderKey(Option<Uuid>);

impl From<&ShaderLayer> for ShaderKey {
    fn from(l: &ShaderLayer) -> Self { ShaderKey(match &l.shader { Handle::Uuid(u, _) => Some(*u), _ => None }) }
}

pub type ShaderMaterial = ExtendedMaterial<StandardMaterial, ShaderLayer>;

impl MaterialExtension for ShaderLayer {
    /// The material's own fragment shader in the main passes (the prepasses keep Bevy's).
    fn specialize(_: &MaterialExtensionPipeline, descriptor: &mut RenderPipelineDescriptor, _: &MeshVertexBufferLayoutRef, key: MaterialExtensionKey<Self>) -> Result<(), SpecializedMeshPipelineError> {
        if let (Some(uuid), Some(fragment)) = (key.bind_group_data.0, descriptor.fragment.as_mut())
            && !key.mesh_key.intersects(MeshPipelineKey::ALL_PREPASS_BITS)
        {
            fragment.shader = Handle::Uuid(uuid, PhantomData);
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
    /// What is wrong with the file now (the last good one is still in use).
    pub fn error(&self, path: &str) -> Option<&str> { self.entries.get(path).and_then(|e| e.error.as_deref()) }
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

/// After a header changed: work every shader material's values out again.
pub fn refresh(shaders: Res<Shaders>, mut mats: ResMut<Assets<ShaderMaterial>>, mut seen: Local<u64>) {
    if *seen == shaders.generation { return; }
    *seen = shaders.generation;
    for (_, m) in mats.iter_mut() {
        if let Some(h) = shaders.header(&m.extension.path) { m.extension.values = Values { v: values(h, &m.extension.set) }; }
    }
}

pub struct ShaderPlugin;

impl Plugin for ShaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ShaderMaterial>::default()).init_resource::<Shaders>().add_systems(Update, (watch, refresh).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
