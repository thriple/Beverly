//! Imported glTF models. Files are read from the open project through an asset source named `project`
//! ("project://assets/models/tower.glb"), whose folder follows the project that is open. Each model's triangles
//! are gathered once it has loaded, for clicking, surface placement and collision.
use crate::shapes::Tris;
use bevy::asset::io::{AssetReader, AssetReaderError, AssetSourceBuilder, PathStream, VecReader};
use bevy::gltf::{Gltf, GltfMesh, GltfNode};
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

struct Entry { gltf: Handle<Gltf>, scene: Handle<WorldAsset>, geo: Option<Arc<Tris>> }

/// Loaded models by project-relative path.
#[derive(Resource, Default)]
pub struct ModelLib { entries: HashMap<String, Entry>, pub generation: u64 }

impl ModelLib {
    /// Start loading a model (once); returns its scene for spawning.
    pub fn request(&mut self, server: &AssetServer, path: &str) -> Handle<WorldAsset> {
        let e = self.entries.entry(path.to_owned()).or_insert_with(|| Entry {
            gltf: server.load(format!("project://{path}")),
            scene: server.load(GltfAssetLabel::Scene(0).from_asset(format!("project://{path}"))),
            geo: None,
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
        self.generation += 1;
    }
}

/// Gather each newly loaded model's triangles (all nodes, with their transforms).
pub fn build_geometry(mut lib: ResMut<ModelLib>, gltfs: Res<Assets<Gltf>>, nodes: Res<Assets<GltfNode>>, gmeshes: Res<Assets<GltfMesh>>, meshes: Res<Assets<Mesh>>) {
    let mut built = false;
    for e in lib.entries.values_mut().filter(|e| e.geo.is_none()) {
        let Some(g) = gltfs.get(&e.gltf) else { continue };
        let Some(tris) = gather(g, &nodes, &gmeshes, &meshes) else { continue };
        e.geo = Some(Arc::new(tris));
        built = true;
    }
    if built { lib.generation += 1; }
}

fn gather(g: &Gltf, nodes: &Assets<GltfNode>, gmeshes: &Assets<GltfMesh>, meshes: &Assets<Mesh>) -> Option<Tris> {
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
