//! Imported glTF models. Files are read from the open project through an asset source named `project`
//! ("project://assets/models/tower.glb"), whose folder follows the project that is open. Each model's triangles
//! are gathered once it has loaded, for clicking, surface placement and collision. A model is spawned as its own
//! meshes and materials, not as a scene: one entity per mesh piece under the part, sharing the model's mesh and
//! material handles with every instance (so Colour, LOD, impostors and virtual geometry each take a mesh).
use crate::shapes::Tris;
use bevy::asset::io::{AssetReader, AssetReaderError, AssetSourceBuilder, PathStream, VecReader};
use bevy::gltf::{Gltf, GltfMaterial, GltfMesh, GltfNode};
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// The open project's folder, shared with the asset reader.
#[derive(Resource, Clone, Default)]
pub struct ProjectRoot(pub Arc<RwLock<PathBuf>>);

impl ProjectRoot {
    pub fn new(dir: PathBuf) -> Self { Self(Arc::new(RwLock::new(dir))) }
    pub fn set(&self, dir: PathBuf) { *self.0.write().unwrap() = dir; }
    pub fn get(&self) -> PathBuf { self.0.read().unwrap().clone() }
}

/// Register the `project://` asset source. Call before adding the default plugins.
pub fn register_source(app: &mut App, root: &ProjectRoot) {
    let r = root.clone();
    app.register_asset_source("project", AssetSourceBuilder::new(move || Box::new(ProjectReader(r.clone()))));
    app.insert_resource(root.clone());
}

struct ProjectReader(ProjectRoot);

impl AssetReader for ProjectReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<VecReader, AssetReaderError> {
        let full = self.0.get().join(path);
        std::fs::read(&full).map(VecReader::new).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound { AssetReaderError::NotFound(full) } else { AssetReaderError::Io(Arc::new(e)) }
        })
    }
    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<VecReader, AssetReaderError> { Err(AssetReaderError::NotFound(path.to_path_buf())) }
    async fn read_directory<'a>(&'a self, path: &'a Path) -> Result<Box<PathStream>, AssetReaderError> { Err(AssetReaderError::NotFound(path.to_path_buf())) }
    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> { Ok(self.0.get().join(path).is_dir()) }
}

/// Model files Beverly can place.
pub fn is_model(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.ends_with(".glb") || p.ends_with(".gltf")
}

/// One mesh of a model with the material it wears and where it sits in the model's space.
#[derive(Clone)]
pub struct Piece { pub mesh: Handle<Mesh>, pub material: Handle<StandardMaterial>, pub transform: Transform }

struct Entry {
    gltf: Handle<Gltf>,
    scene: Handle<WorldAsset>,
    geo: Option<Arc<Tris>>,
    /// The model's meshes, once loaded; None while loading or for a model with a skeleton (it stays a scene).
    pieces: Option<Arc<Vec<Piece>>>,
    skinned: bool,
}

/// A model part's root: its meshes are added under it once the model has loaded. `tint`: the part's Colour.
#[derive(Component)]
pub struct ModelRoot { pub path: String, pub tint: [f32; 3] }

/// The model's meshes are under the root.
#[derive(Component)]
pub struct ModelFilled;

/// The Colour a model part is placed with, which leaves the model as it is (parts made before Colour worked on
/// models were grey).
pub const UNTINTED: [f32; 3] = [0.62, 0.64, 0.70];

/// Loaded models by project-relative path.
#[derive(Resource, Default)]
pub struct ModelLib {
    entries: HashMap<String, Entry>,
    pub generation: u64,
    /// A model material in a part's Colour, made once so instances share it.
    tinted: HashMap<(AssetId<StandardMaterial>, [u8; 3]), Handle<StandardMaterial>>,
}

impl ModelLib {
    /// Start loading a model (once); returns its scene for spawning.
    pub fn request(&mut self, server: &AssetServer, path: &str) -> Handle<WorldAsset> {
        let e = self.entries.entry(path.to_owned()).or_insert_with(|| Entry {
            gltf: server.load(format!("project://{path}")),
            scene: server.load(GltfAssetLabel::Scene(0).from_asset(format!("project://{path}"))),
            geo: None,
            pieces: None,
            skinned: false,
        });
        e.scene.clone()
    }
    /// The model's triangles in its own space, once loaded.
    pub fn geo(&self, path: &str) -> Option<Arc<Tris>> { self.entries.get(path).and_then(|e| e.geo.clone()) }
    pub fn all_geo(&self) -> impl Iterator<Item = (&String, &Arc<Tris>)> { self.entries.iter().filter_map(|(k, e)| e.geo.as_ref().map(|g| (k, g))) }
    pub fn pending(&self) -> bool { self.entries.values().any(|e| e.geo.is_none()) }
    /// Forget everything (another project was opened).
    pub fn clear(&mut self) {
        self.entries.clear();
        self.tinted.clear();
        self.generation += 1;
    }
}

/// Gather each newly loaded model's triangles (all nodes, with their transforms) and its meshes with their materials
/// as Beverly's.
#[allow(clippy::too_many_arguments)]
pub fn build_geometry(mut lib: ResMut<ModelLib>, gltfs: Res<Assets<Gltf>>, nodes: Res<Assets<GltfNode>>, gmeshes: Res<Assets<GltfMesh>>, meshes: Res<Assets<Mesh>>, gmats: Res<Assets<GltfMaterial>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let mut built = false;
    for e in lib.entries.values_mut().filter(|e| e.geo.is_none()) {
        let Some(g) = gltfs.get(&e.gltf) else { continue };
        let mut found = vec![];
        let Some(tris) = gather(g, &nodes, &gmeshes, &meshes, &mut found) else { continue };
        // each glTF material becomes one StandardMaterial (none: glTF's default), shared by every piece that wears it
        let mut made: HashMap<Option<AssetId<GltfMaterial>>, Handle<StandardMaterial>> = HashMap::new();
        let pieces = found.into_iter().filter_map(|(mesh, material, m)| {
            let id = material.as_ref().map(|h| h.id());
            if !made.contains_key(&id) {
                let src = match &material { Some(h) => gmats.get(h)?.clone(), None => GltfMaterial::default() };
                made.insert(id, mats.add(bevy::pbr::gltf::standard_material_from_gltf_material(&src)));
            }
            Some(Piece { mesh, material: made[&id].clone(), transform: Transform::from_matrix(m) })
        }).collect();
        e.skinned = !g.skins.is_empty();
        e.pieces = Some(Arc::new(pieces));
        e.geo = Some(Arc::new(tris));
        built = true;
    }
    if built { lib.generation += 1; }
}

/// Put a model part's meshes under its root once the model has loaded (a model with a skeleton stays a scene).
pub fn fill_models(mut commands: Commands, mut lib: ResMut<ModelLib>, mut assets: ResMut<Assets<StandardMaterial>>, roots: Query<(Entity, &ModelRoot), Without<ModelFilled>>) {
    for (root, m) in &roots {
        let Some(e) = lib.entries.get(&m.path) else { continue };
        let Some(pieces) = e.pieces.clone() else { continue };
        commands.entity(root).insert(ModelFilled);
        if e.skinned {
            commands.spawn((WorldAssetRoot(e.scene.clone()), ChildOf(root)));
            continue;
        }
        let tint = if m.tint == UNTINTED { [1.0; 3] } else { m.tint };
        for p in pieces.iter() {
            let material = if tint == [1.0; 3] { p.material.clone() } else { lib.tinted(&mut assets, &p.material, tint) };
            commands.spawn((Mesh3d(p.mesh.clone()), MeshMaterial3d(material), p.transform, ChildOf(root)));
        }
    }
}

impl ModelLib {
    fn tinted(&mut self, assets: &mut Assets<StandardMaterial>, base: &Handle<StandardMaterial>, c: [f32; 3]) -> Handle<StandardMaterial> {
        let key = (base.id(), c.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8));
        if let Some(h) = self.tinted.get(&key) { return h.clone(); }
        let mut m = assets.get(base).cloned().unwrap_or_default();
        let t = LinearRgba::from(Color::srgb_u8(key.1[0], key.1[1], key.1[2]));
        let b = m.base_color.to_linear();
        m.base_color = Color::LinearRgba(LinearRgba { red: b.red * t.red, green: b.green * t.green, blue: b.blue * t.blue, alpha: b.alpha });
        let h = assets.add(m);
        self.tinted.insert(key, h.clone());
        h
    }
}

fn gather(g: &Gltf, nodes: &Assets<GltfNode>, gmeshes: &Assets<GltfMesh>, meshes: &Assets<Mesh>, found: &mut Vec<(Handle<Mesh>, Option<Handle<GltfMaterial>>, Mat4)>) -> Option<Tris> {
    // roots: nodes that are nobody's child
    let mut child = std::collections::HashSet::new();
    for h in &g.nodes {
        for c in &nodes.get(h)?.children { child.insert(c.id()); }
    }
    let mut out = Tris::default();
    let mut stack: Vec<(Handle<GltfNode>, Mat4)> = g.nodes.iter().filter(|h| !child.contains(&h.id())).map(|h| (h.clone(), Mat4::IDENTITY)).collect();
    while let Some((h, parent)) = stack.pop() {
        let n = nodes.get(&h)?;
        let m = parent * n.transform.to_matrix();
        if let Some(mh) = &n.mesh {
            for prim in &gmeshes.get(mh)?.primitives {
                let mesh = meshes.get(&prim.mesh)?;
                found.push((prim.mesh.clone(), prim.material.clone(), m));
                let Some(VertexAttributeValues::Float32x3(pos)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { continue };
                let pos: Vec<Vec3> = pos.iter().map(|p| m.transform_point3(Vec3::from(*p))).collect();
                let idx: Vec<usize> = match mesh.indices() {
                    Some(i) => i.iter().collect(),
                    None => (0..pos.len()).collect(),
                };
                for t in idx.chunks_exact(3) {
                    out.pos.extend([pos[t[0]], pos[t[1]], pos[t[2]]]);
                }
            }
        }
        for c in &n.children { stack.push((c.clone(), m)); }
    }
    Some(out)
}
