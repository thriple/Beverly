//! Baking: a Bevy `Mesh` (such as one glTF primitive) into a `MeshletMesh` and its bytes, and back.
//!
//! The cache is the editor's (`editor/src/bake.rs`, `Bakes`): this module names the bake's [`KIND`] and the
//! [`key_parts`] it depends on, and turns meshes into bytes; the editor stores them.
//!
//! The bytes: `TSRA`, [`FORMAT_VERSION`] (u32), then Bevy's `.meshlet_mesh` bytes (u64 length first), then the
//! proxy mesh ([`proxy_mesh`]): vertex count (u64), positions, normals and UVs as f32s, index count (u64) and u32
//! indices. Little-endian, as Bevy's format is.
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

/// The version of these bytes and of what this module does to a mesh before Bevy's builder; bumped when either
/// changes, so old bakes are made again.
pub const FORMAT_VERSION: u32 = 3;

const MAGIC: &[u8; 4] = b"TSRA";

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
    /// Writing or reading Bevy's part of the bytes failed, or it is of another version.
    Bytes(MeshletMeshSaveOrLoadError),
    /// The bytes aren't a bake, are cut short, or are of another [`FORMAT_VERSION`] (the number found).
    Format(Option<u32>),
}

impl std::fmt::Display for BakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotTriangles => write!(f, "only triangle meshes can be baked"),
            Self::NoPositions => write!(f, "the mesh has no positions"),
            Self::Build(e) => write!(f, "{e}"),
            Self::Bytes(e) => write!(f, "{e}"),
            Self::Format(Some(found)) => write!(f, "a bake of format {found}, not {FORMAT_VERSION}"),
            Self::Format(None) => write!(f, "not a bake, or cut short"),
        }
    }
}

impl std::error::Error for BakeError {}

/// What a bake depends on besides its kind, in order, for the cache key (`bake::key(KIND, parts)` in the editor):
/// the source bytes (e.g. the model file), the settings, and the format version (Bevy's asset version and
/// [`FORMAT_VERSION`]).
pub fn key_parts(source: &[u8], settings: &BakeSettings) -> [Vec<u8>; 3] {
    let version = [MESHLET_MESH_ASSET_VERSION.to_le_bytes().as_slice(), &FORMAT_VERSION.to_le_bytes()].concat();
    [source.to_vec(), vec![settings.position_quantization], version]
}

/// Bakes a mesh into the bytes the cache keeps (the `MeshletMesh` and its proxy).
pub fn bake(mesh: &Mesh, settings: &BakeSettings) -> Result<Vec<u8>, BakeError> {
    let (meshlet, proxy) = bake_mesh(mesh, settings)?;
    let mut meshlet_bytes = Vec::new();
    meshlet.write(&mut meshlet_bytes).map_err(BakeError::Bytes)?;

    let mut bytes = Vec::with_capacity(meshlet_bytes.len() + 64);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&(meshlet_bytes.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&meshlet_bytes);
    let float3 = |id| proxy.attribute(id).and_then(|a: &VertexAttributeValues| a.as_float3()).unwrap_or(&[]);
    let positions = float3(Mesh::ATTRIBUTE_POSITION);
    let uvs = match proxy.attribute(Mesh::ATTRIBUTE_UV_0) {
        Some(VertexAttributeValues::Float32x2(uvs)) => uvs.as_slice(),
        _ => &[],
    };
    let indices: Vec<u32> = proxy.indices().map_or(vec![], |i| i.iter().map(|i| i as u32).collect());
    bytes.extend_from_slice(&(positions.len() as u64).to_le_bytes());
    bytes.extend_from_slice(bytemuck::cast_slice(positions));
    bytes.extend_from_slice(bytemuck::cast_slice(float3(Mesh::ATTRIBUTE_NORMAL)));
    bytes.extend_from_slice(bytemuck::cast_slice(uvs));
    bytes.extend_from_slice(&(indices.len() as u64).to_le_bytes());
    bytes.extend_from_slice(bytemuck::cast_slice(&indices));
    Ok(bytes)
}

/// Bakes a mesh into a `MeshletMesh` and its proxy (see [`proxy_mesh`]).
pub fn bake_mesh(mesh: &Mesh, settings: &BakeSettings) -> Result<(MeshletMesh, Mesh), BakeError> {
    // Bevy's builder runs on its task pools, which only exist inside an app
    AsyncComputeTaskPool::get_or_init(TaskPool::default);
    MeshletMesh::from_mesh_with_proxy(&prepare(mesh)?, settings.position_quantization).map_err(BakeError::Build)
}

/// Reads the `MeshletMesh` back from baked bytes; bytes of another format version are an error (bake again).
pub fn load(bytes: &[u8]) -> Result<MeshletMesh, BakeError> {
    let (meshlet_bytes, _) = split(bytes)?;
    MeshletMesh::read(&mut &meshlet_bytes[..]).map_err(BakeError::Bytes)
}

/// Reads the proxy back from baked bytes: the coarsest level of the cluster hierarchy as an ordinary triangle mesh
/// (positions, normals, UVs; indexed), covering the whole model with few triangles. For ray tracing against a
/// stand-in (`realtime-gi`) and far, low-poly backdrops (`backdrops`).
pub fn proxy_mesh(bytes: &[u8]) -> Result<Mesh, BakeError> {
    let (_, mut rest) = split(bytes)?;
    let mut take = |n: usize| -> Result<&[u8], BakeError> {
        let (head, tail) = rest.split_at_checked(n).ok_or(BakeError::Format(None))?;
        rest = tail;
        Ok(head)
    };
    let count = |b: &[u8]| u64::from_le_bytes(b.try_into().unwrap()) as usize;
    let vertices = count(take(8)?);
    let positions: Vec<[f32; 3]> = bytemuck::pod_collect_to_vec(take(vertices * 12)?);
    let normals: Vec<[f32; 3]> = bytemuck::pod_collect_to_vec(take(vertices * 12)?);
    let uvs: Vec<[f32; 2]> = bytemuck::pod_collect_to_vec(take(vertices * 8)?);
    let index_count = count(take(8)?);
    let indices: Vec<u32> = bytemuck::pod_collect_to_vec(take(index_count * 4)?);
    Ok(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices)))
}

/// Checks the header and splits baked bytes into Bevy's `.meshlet_mesh` bytes and the proxy's.
fn split(bytes: &[u8]) -> Result<(&[u8], &[u8]), BakeError> {
    let header = bytes.get(..16).ok_or(BakeError::Format(None))?;
    if &header[..4] != MAGIC {
        return Err(BakeError::Format(None));
    }
    let version = u32::from_le_bytes(header[4..8].try_into().unwrap());
    if version != FORMAT_VERSION {
        return Err(BakeError::Format(Some(version)));
    }
    let length = u64::from_le_bytes(header[8..16].try_into().unwrap()) as usize;
    bytes[16..].split_at_checked(length).ok_or(BakeError::Format(None))
}

/// The mesh as Bevy's builder takes it: positions, normals, UVs and (when it has them) vertex colours, indexed, with
/// identical vertices welded. Missing normals are made smooth (weighted by corner angle) and missing UVs are zero;
/// other attributes are dropped.
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
    let colors = match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
        Some(VertexAttributeValues::Float32x4(c)) if c.len() == positions.len() => Some(c),
        _ => None,
    };
    let corners: Vec<usize> = match mesh.indices() {
        Some(indices) => indices.iter().collect(),
        None => (0..positions.len()).collect(),
    };

    // Weld: one vertex per distinct (position, normal, UV, colour); without normals, per (position, UV, colour), so
    // the smoothed normals are shared across triangles
    let mut welded: HashMap<[u32; 12], u32> = HashMap::new();
    let (mut out_positions, mut out_normals, mut out_uvs, mut out_colors) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut indices = Vec::with_capacity(corners.len() - corners.len() % 3);
    for &i in &corners[..corners.len() - corners.len() % 3] {
        let p = positions[i];
        let n = normals.map_or([0.0; 3], |n| n[i]);
        let uv = uvs.map_or([0.0; 2], |uv| uv[i]);
        let c = colors.map_or([1.0; 4], |c| c[i]);
        let key = [p[0], p[1], p[2], n[0], n[1], n[2], uv[0], uv[1], c[0], c[1], c[2], c[3]].map(f32::to_bits);
        let index = *welded.entry(key).or_insert_with(|| {
            out_positions.push(p);
            out_normals.push(n);
            out_uvs.push(uv);
            out_colors.push(c);
            out_positions.len() as u32 - 1
        });
        indices.push(index);
    }

    let mut prepared = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, out_positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, out_uvs)
        .with_inserted_indices(Indices::U32(indices));
    if colors.is_some() {
        prepared.insert_attribute(Mesh::ATTRIBUTE_COLOR, out_colors);
    }
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
        // on several threads at once too, as the editor's bake jobs can run (METIS has global state)
        let mesh = test_mesh(4, 1);
        let first = bake(&mesh, &BakeSettings::default()).unwrap();
        let others: Vec<Vec<u8>> = std::thread::scope(|scope| {
            let jobs: Vec<_> = (0..4).map(|_| scope.spawn(|| bake(&mesh, &BakeSettings::default()).unwrap())).collect();
            jobs.into_iter().map(|job| job.join().unwrap()).collect()
        });
        assert!(others.iter().all(|bytes| *bytes == first));
    }

    #[test]
    fn save_load_round_trip() {
        let bytes = bake(&test_mesh(3, 2), &BakeSettings::default()).unwrap();
        let (meshlet_bytes, _) = split(&bytes).unwrap();
        let mut again = Vec::new();
        load(&bytes).unwrap().write(&mut again).unwrap();
        assert_eq!(meshlet_bytes, again.as_slice());
    }

    #[test]
    fn wrong_version_is_rejected() {
        let bytes = bake(&test_mesh(3, 2), &BakeSettings::default()).unwrap();
        // ours: after the 4-byte magic
        let mut ours = bytes.clone();
        ours[4..8].copy_from_slice(&(FORMAT_VERSION + 1).to_le_bytes());
        assert!(matches!(load(&ours), Err(BakeError::Format(Some(_)))));
        assert!(matches!(proxy_mesh(&ours), Err(BakeError::Format(Some(_)))));
        // Bevy's: after its own 8-byte magic, inside ours (16 bytes of header)
        let mut bevys = bytes.clone();
        bevys[24..32].copy_from_slice(&(MESHLET_MESH_ASSET_VERSION + 1).to_le_bytes());
        assert!(matches!(load(&bevys), Err(BakeError::Bytes(MeshletMeshSaveOrLoadError::WrongVersion { .. }))));
        // cut short
        assert!(matches!(proxy_mesh(&bytes[..bytes.len() - 1]), Err(BakeError::Format(None))));
    }

    #[test]
    fn proxy_is_coarse_and_covers_the_mesh() {
        let mesh = test_mesh(6, 4);
        let source_triangles = mesh.indices().unwrap().len() / 3;
        let proxy = proxy_mesh(&bake(&mesh, &BakeSettings::default()).unwrap()).unwrap();
        let triangles = proxy.indices().unwrap().len() / 3;
        assert!(triangles > 0 && triangles * 10 < source_triangles, "{triangles} of {source_triangles}");
        // about the same size: the proxy's furthest point from the centre is near the source's
        let radius = |m: &Mesh| {
            let p = m.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
            p.iter().map(|p| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()).fold(0.0f32, f32::max)
        };
        assert!((radius(&proxy) / radius(&mesh) - 1.0).abs() < 0.1);
        assert_eq!(proxy.attributes().count(), 3);
    }

    #[test]
    fn fills_in_and_drops_attributes() {
        // an unindexed mesh with only positions and an extra tangent attribute
        let full = test_mesh(2, 3);
        let positions = full.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
        let unindexed: Vec<[f32; 3]> = full.indices().unwrap().iter().map(|i| positions[i]).collect();
        let count = unindexed.len();
        let bare = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, unindexed)
            .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, vec![[1.0f32, 0.0, 0.0, 1.0]; count]);
        let prepared = prepare(&bare).unwrap();
        let ids: Vec<_> = prepared.attributes().map(|(a, _)| a.id).collect();
        assert_eq!(ids, [Mesh::ATTRIBUTE_POSITION.id, Mesh::ATTRIBUTE_NORMAL.id, Mesh::ATTRIBUTE_UV_0.id]);
        // welded back to one vertex per position
        assert_eq!(prepared.count_vertices(), positions.len());
        assert!(bake(&bare, &BakeSettings::default()).is_ok());
    }

    #[test]
    fn keeps_vertex_colours() {
        let mut mesh = test_mesh(3, 5);
        let count = mesh.count_vertices();
        let colors: Vec<[f32; 4]> = (0..count).map(|i| [i as f32 / count as f32, 0.5, 0.25, 1.0]).collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors.clone());
        let prepared = prepare(&mesh).unwrap();
        let ids: Vec<_> = prepared.attributes().map(|(a, _)| a.id).collect();
        assert_eq!(
            ids,
            [Mesh::ATTRIBUTE_POSITION.id, Mesh::ATTRIBUTE_NORMAL.id, Mesh::ATTRIBUTE_UV_0.id, Mesh::ATTRIBUTE_COLOR.id]
        );
        // baked, the colours are kept (more bytes than without), and read back
        let with = bake(&mesh, &BakeSettings::default()).unwrap();
        let without = bake(&test_mesh(3, 5), &BakeSettings::default()).unwrap();
        assert!(with.len() > without.len());
        assert!(load(&with).is_ok());
    }

    #[test]
    fn key_parts_change_with_settings() {
        let a = key_parts(b"model", &BakeSettings::default());
        let b = key_parts(b"model", &BakeSettings { position_quantization: 2 });
        assert_eq!(a[0], b[0]);
        assert_ne!(a[1], b[1]);
    }
}

