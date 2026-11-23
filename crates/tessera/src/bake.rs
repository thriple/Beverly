//! Baking: a Bevy `Mesh` (such as one glTF primitive) into a `MeshletMesh` and its bytes, and back.
//!
//! The cache is the editor's (`editor/src/bake.rs`, `Bakes`): this module names the bake's [`KIND`] and the
//! [`key_parts`] it depends on, and turns meshes into bytes; the editor stores them.
use bevy_asset::RenderAssetUsages;
use bevy_mesh::{Indices, Mesh, PrimitiveTopology, VertexAttributeValues};
use bevy_pbr::experimental::meshlet::{
    MeshToMeshletMeshConversionError, MeshletMesh, MeshletMeshSaveOrLoadError,
    MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR, MESHLET_MESH_ASSET_VERSION,
};
use bevy_tasks::{AsyncComputeTaskPool, TaskPool};
use std::collections::HashMap;

/// The bake cache's kind for these bakes.
pub const KIND: &str = "meshlet";

/// What this module does to a mesh before Bevy's builder; bumped when that changes, so old bakes are made again.
pub const PREPARE_VERSION: u32 = 1;

/// How a mesh is baked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BakeSettings {
    /// Positions are rounded to 1 / 2^this centimetres (Bevy's `vertex_position_quantization_factor`; meshes placed
    /// next to each other need the same value to meet without cracks).
    pub position_quantization: u8,
}

impl Default for BakeSettings {
    fn default() -> Self {
        Self { position_quantization: MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR }
    }
}

/// Why a mesh couldn't be baked or its bytes read.
#[derive(Debug)]
pub enum BakeError {
    /// Only triangle lists can be baked.
    NotTriangles,
    /// The mesh has no `Float32x3` positions.
    NoPositions,
    /// Bevy's builder refused the prepared mesh.
    Build(MeshToMeshletMeshConversionError),
    /// Writing or reading the bytes failed, or they are of another format version.
    Bytes(MeshletMeshSaveOrLoadError),
}

impl std::fmt::Display for BakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotTriangles => write!(f, "only triangle meshes can be baked"),
            Self::NoPositions => write!(f, "the mesh has no positions"),
            Self::Build(e) => write!(f, "{e}"),
            Self::Bytes(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BakeError {}

/// What a bake depends on besides its kind, in order, for the cache key (`bake::key(KIND, parts)` in the editor):
/// the source bytes (e.g. the model file), the settings, and the format version (Bevy's asset version and
/// [`PREPARE_VERSION`]).
pub fn key_parts(source: &[u8], settings: &BakeSettings) -> [Vec<u8>; 3] {
    let version = [MESHLET_MESH_ASSET_VERSION.to_le_bytes().as_slice(), &PREPARE_VERSION.to_le_bytes()].concat();
    [source.to_vec(), vec![settings.position_quantization], version]
}

/// Bakes a mesh into the bytes the cache keeps.
pub fn bake(mesh: &Mesh, settings: &BakeSettings) -> Result<Vec<u8>, BakeError> {
    let meshlet = bake_mesh(mesh, settings)?;
    let mut bytes = Vec::new();
    meshlet.write(&mut bytes).map_err(BakeError::Bytes)?;
    Ok(bytes)
}

/// Bakes a mesh into a `MeshletMesh`.
pub fn bake_mesh(mesh: &Mesh, settings: &BakeSettings) -> Result<MeshletMesh, BakeError> {
    // Bevy's builder runs on its task pools, which only exist inside an app
    AsyncComputeTaskPool::get_or_init(TaskPool::default);
    MeshletMesh::from_mesh(&prepare(mesh)?, settings.position_quantization).map_err(BakeError::Build)
}

/// Reads baked bytes back; bytes of another format version are an error (bake again).
pub fn load(bytes: &[u8]) -> Result<MeshletMesh, BakeError> {
    MeshletMesh::read(&mut &bytes[..]).map_err(BakeError::Bytes)
}

/// The mesh as Bevy's builder takes it: exactly positions, normals and UVs, indexed, with identical vertices welded.
/// Missing normals are made smooth (weighted by corner angle) and missing UVs are zero; other attributes are dropped.
pub fn prepare(mesh: &Mesh) -> Result<Mesh, BakeError> {
    if mesh.primitive_topology() != PrimitiveTopology::TriangleList {
        return Err(BakeError::NotTriangles);
    }
    let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        return Err(BakeError::NoPositions);
    };
    let normals = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(n)) if n.len() == positions.len() => Some(n),
        _ => None,
    };
    let uvs = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
        Some(VertexAttributeValues::Float32x2(uv)) if uv.len() == positions.len() => Some(uv),
        _ => None,
    };
    let corners: Vec<usize> = match mesh.indices() {
        Some(indices) => indices.iter().collect(),
        None => (0..positions.len()).collect(),
    };

    // Weld: one vertex per distinct (position, normal, UV); without normals, per (position, UV), so the
    // smoothed normals are shared across triangles
    let mut welded: HashMap<[u32; 8], u32> = HashMap::new();
    let (mut out_positions, mut out_normals, mut out_uvs) = (Vec::new(), Vec::new(), Vec::new());
    let mut indices = Vec::with_capacity(corners.len() - corners.len() % 3);
    for &i in &corners[..corners.len() - corners.len() % 3] {
        let p = positions[i];
        let n = normals.map_or([0.0; 3], |n| n[i]);
        let uv = uvs.map_or([0.0; 2], |uv| uv[i]);
        let key = [p[0], p[1], p[2], n[0], n[1], n[2], uv[0], uv[1]].map(f32::to_bits);
        let index = *welded.entry(key).or_insert_with(|| {
            out_positions.push(p);
            out_normals.push(n);
            out_uvs.push(uv);
            out_positions.len() as u32 - 1
        });
        indices.push(index);
    }

    let mut prepared = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, out_positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, out_uvs)
        .with_inserted_indices(Indices::U32(indices));
    if normals.is_some() {
        prepared.insert_attribute(Mesh::ATTRIBUTE_NORMAL, out_normals);
    } else {
        prepared.compute_smooth_normals();
    }
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_mesh::test_mesh;

    #[test]
    fn same_bytes_every_time() {
        let mesh = test_mesh(4, 1);
        assert_eq!(bake(&mesh, &BakeSettings::default()).unwrap(), bake(&mesh, &BakeSettings::default()).unwrap());
    }

    #[test]
    fn save_load_round_trip() {
        let bytes = bake(&test_mesh(3, 2), &BakeSettings::default()).unwrap();
        let mut again = Vec::new();
        load(&bytes).unwrap().write(&mut again).unwrap();
        assert_eq!(bytes, again);
    }

    #[test]
    fn wrong_version_is_rejected() {
        let mut bytes = bake(&test_mesh(3, 2), &BakeSettings::default()).unwrap();
        // the version follows the 8-byte magic number
        bytes[8..16].copy_from_slice(&(MESHLET_MESH_ASSET_VERSION + 1).to_le_bytes());
        assert!(matches!(load(&bytes), Err(BakeError::Bytes(MeshletMeshSaveOrLoadError::WrongVersion { .. }))));
    }

    #[test]
    fn fills_in_and_drops_attributes() {
        // an unindexed mesh with only positions and an extra colour attribute
        let full = test_mesh(2, 3);
        let positions = full.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
        let unindexed: Vec<[f32; 3]> = full.indices().unwrap().iter().map(|i| positions[i]).collect();
        let count = unindexed.len();
        let bare = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, unindexed)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32; 4]; count]);
        let prepared = prepare(&bare).unwrap();
        let ids: Vec<_> = prepared.attributes().map(|(a, _)| a.id).collect();
        assert_eq!(ids, [Mesh::ATTRIBUTE_POSITION.id, Mesh::ATTRIBUTE_NORMAL.id, Mesh::ATTRIBUTE_UV_0.id]);
        // welded back to one vertex per position
        assert_eq!(prepared.count_vertices(), positions.len());
        assert!(bake(&bare, &BakeSettings::default()).is_ok());
    }

    #[test]
    fn key_parts_change_with_settings() {
        let a = key_parts(b"model", &BakeSettings::default());
        let b = key_parts(b"model", &BakeSettings { position_quantization: 2 });
        assert_eq!(a[0], b[0]);
        assert_ne!(a[1], b[1]);
    }
}
