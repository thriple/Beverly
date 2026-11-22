//! A dense procedural test mesh: an icosphere subdivided `levels` times (20 × 4^levels triangles; 8 gives
//! 1,310,720) and pushed in and out by seeded noise. Indexed, welded (one vertex per position), with smooth
//! normals and zero UVs: exactly the POSITION, NORMAL and UV_0 that `MeshletMesh::from_mesh` takes.
use bevy_asset::RenderAssetUsages;
use bevy_mesh::{Indices, Mesh, PrimitiveTopology};
use std::collections::HashMap;

/// The test mesh at `levels` subdivisions, its bumps chosen by `seed` (the same seed gives the same mesh).
pub fn test_mesh(levels: u32, seed: u64) -> Mesh {
    let (mut positions, triangles) = icosphere(levels);
    for p in &mut positions {
        let r = 1.0 + 0.15 * fbm(*p, seed);
        *p = p.map(|c| c * r);
    }
    let normals = smooth_normals(&positions, &triangles);
    let uvs = vec![[0.0f32; 2]; positions.len()];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(triangles.into_iter().flatten().collect()))
}

/// A flat square, -1..1 in X and Y facing +Z, of `cells` × `cells` cells split into two triangles each, with the
/// inner corners moved up to 0.3 of a cell across and 0.02 in depth by seeded noise (the border stays straight). Every inner edge is shared
/// by two triangles at uneven sub-pixel positions: the software rasterizer's fill-rule check (`vg-render ... grid`).
pub fn jittered_grid(cells: u32, seed: u64) -> Mesh {
    let n = cells + 1;
    let step = 2.0 / cells as f32;
    let mut positions = Vec::with_capacity((n * n) as usize);
    for y in 0..n {
        for x in 0..n {
            let inner = x > 0 && y > 0 && x < cells && y < cells;
            let jitter = |axis: i64| if inner { 0.3 * step * lattice(x as i64, y as i64, axis, seed) } else { 0.0 };
            // small depth bumps too: a flat grid would simplify losslessly and be drawn at its coarsest level
            let depth = if inner { 0.02 * lattice(x as i64, y as i64, 2, seed) } else { 0.0 };
            positions.push([-1.0 + x as f32 * step + jitter(0), -1.0 + y as f32 * step + jitter(1), depth]);
        }
    }
    let mut indices = Vec::with_capacity((cells * cells * 6) as usize);
    for y in 0..cells {
        for x in 0..cells {
            let (a, b, c, d) = (y * n + x, y * n + x + 1, (y + 1) * n + x, (y + 1) * n + x + 1);
            indices.extend([a, b, d, a, d, c]);
        }
    }
    let count = positions.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32; 2]; count])
        .with_inserted_indices(Indices::U32(indices))
}

/// Unit icosphere: positions and triangles (counter-clockwise from outside), edges' midpoints shared.
fn icosphere(levels: u32) -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let t = (1.0 + 5f32.sqrt()) / 2.0;
    let mut positions: Vec<[f32; 3]> = [
        [-1.0, t, 0.0], [1.0, t, 0.0], [-1.0, -t, 0.0], [1.0, -t, 0.0],
        [0.0, -1.0, t], [0.0, 1.0, t], [0.0, -1.0, -t], [0.0, 1.0, -t],
        [t, 0.0, -1.0], [t, 0.0, 1.0], [-t, 0.0, -1.0], [-t, 0.0, 1.0],
    ]
    .into_iter()
    .map(normalize)
    .collect();
    let mut triangles: Vec<[u32; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];
    for _ in 0..levels {
        let mut midpoints: HashMap<(u32, u32), u32> = HashMap::with_capacity(triangles.len() * 3 / 2);
        let mut mid = |a: u32, b: u32, positions: &mut Vec<[f32; 3]>| {
            *midpoints.entry((a.min(b), a.max(b))).or_insert_with(|| {
                let (pa, pb) = (positions[a as usize], positions[b as usize]);
                positions.push(normalize([pa[0] + pb[0], pa[1] + pb[1], pa[2] + pb[2]]));
                positions.len() as u32 - 1
            })
        };
        let mut next = Vec::with_capacity(triangles.len() * 4);
        for [a, b, c] in triangles {
            let (ab, bc, ca) = (mid(a, b, &mut positions), mid(b, c, &mut positions), mid(c, a, &mut positions));
            next.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        triangles = next;
    }
    (positions, triangles)
}

fn normalize([x, y, z]: [f32; 3]) -> [f32; 3] {
    let l = (x * x + y * y + z * z).sqrt();
    [x / l, y / l, z / l]
}

/// Area-weighted vertex normals.
fn smooth_normals(positions: &[[f32; 3]], triangles: &[[u32; 3]]) -> Vec<[f32; 3]> {
    let mut normals = vec![[0.0f32; 3]; positions.len()];
    for tri in triangles {
        let [a, b, c] = tri.map(|i| positions[i as usize]);
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        for i in tri {
            let m = &mut normals[*i as usize];
            (m[0], m[1], m[2]) = (m[0] + n[0], m[1] + n[1], m[2] + n[2]);
        }
    }
    normals.into_iter().map(normalize).collect()
}

/// Five octaves of value noise, about -1..1.
fn fbm(p: [f32; 3], seed: u64) -> f32 {
    let (mut sum, mut amp, mut freq) = (0.0, 0.5, 3.0);
    for octave in 0..5 {
        sum += amp * value_noise(p.map(|c| c * freq), seed.wrapping_add(octave));
        (amp, freq) = (amp * 0.5, freq * 2.0);
    }
    sum
}

/// Trilinear value noise on the integer lattice, smoothstepped, -1..1.
fn value_noise(p: [f32; 3], seed: u64) -> f32 {
    let cell = p.map(|c| c.floor());
    let f = [0, 1, 2].map(|i| {
        let t = p[i] - cell[i];
        t * t * (3.0 - 2.0 * t)
    });
    let corner = |dx: i64, dy: i64, dz: i64| lattice(cell[0] as i64 + dx, cell[1] as i64 + dy, cell[2] as i64 + dz, seed);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = lerp(corner(0, 0, 0), corner(1, 0, 0), f[0]);
    let x10 = lerp(corner(0, 1, 0), corner(1, 1, 0), f[0]);
    let x01 = lerp(corner(0, 0, 1), corner(1, 0, 1), f[0]);
    let x11 = lerp(corner(0, 1, 1), corner(1, 1, 1), f[0]);
    lerp(lerp(x00, x10, f[1]), lerp(x01, x11, f[1]), f[2])
}

/// A lattice point's value, -1..1, from a SplitMix64 hash of its coordinates and the seed.
fn lattice(x: i64, y: i64, z: i64, seed: u64) -> f32 {
    let mut h = seed ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (z as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    (h >> 40) as f32 / (1u64 << 23) as f32 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_and_welded() {
        let (positions, triangles) = icosphere(3);
        assert_eq!(triangles.len(), 20 * 4usize.pow(3));
        // a closed genus-0 mesh: V - E + F = 2, with E = 3F/2
        assert_eq!(positions.len() + triangles.len() - triangles.len() * 3 / 2, 2);
    }

    #[test]
    fn grid_counts_and_straight_border() {
        let mesh = jittered_grid(4, 1);
        assert_eq!(mesh.indices().unwrap().len(), 4 * 4 * 6);
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap();
        for p in positions {
            assert!(p[0].abs() <= 1.0 && p[1].abs() <= 1.0);
            if p[0].abs() == 1.0 || p[1].abs() == 1.0 {
                assert!(((p[0] + 1.0) / 0.5).fract() == 0.0 && ((p[1] + 1.0) / 0.5).fract() == 0.0);
            }
        }
        // counter-clockwise from +Z, so the camera in front sees the front faces
        let i = mesh.indices().unwrap().iter().take(3).collect::<Vec<_>>();
        let [a, b, c] = [i[0], i[1], i[2]].map(|i| positions[i]);
        assert!((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) > 0.0);
    }

    #[test]
    fn same_seed_same_mesh() {
        let a = test_mesh(3, 7);
        let b = test_mesh(3, 7);
        let c = test_mesh(3, 8);
        let pos = |m: &Mesh| m.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap().to_vec();
        assert_eq!(pos(&a), pos(&b));
        assert_ne!(pos(&a), pos(&c));
    }
}
