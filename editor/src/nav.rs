//! Navigation areas: where characters such as enemies can walk, across floors, stairs and ramps, built from the
//! level with one button (Recast, through `bevy_rerecast`). The area is worked out from the level's still, solid
//! parts and the ground; it is never stored in files, only rebuilt from the level.
use crate::level::{Collision, Level, Navigation, Part, Shape};
use crate::world::{Geometry, part_affine};
use bevy::math::{Quat, UVec3, Vec2, Vec3, Vec3A};
use bevy_rerecast::rerecast::{AreaType, ConfigBuilder, ConvexVolume, DetailNavmesh, HeightfieldBuilder, TriMesh};
use bevy_rerecast::{Navmesh, NavmeshSettings};

/// The part of the level the area is worked out from: still, solid parts (loose and moving ones go around).
fn counts(p: &Part) -> bool { crate::physics::kind(p).is_some_and(|k| k.is_static()) && p.collision == Collision::Solid }

/// What the area depends on (to tell when it is out of date).
pub fn key(level: &Level) -> Vec<(Shape, Option<String>, [u32; 10])> {
    let bits = |p: &Part| {
        let mut b = [0; 10];
        for (i, x) in p.pos.iter().chain(&p.rot).chain(&p.size).enumerate() { b[i] = x.to_bits(); }
        b
    };
    let mut k: Vec<_> = level.parts.iter().filter(|p| counts(p)).map(|p| (p.shape, p.model.clone(), bits(p))).collect();
    let n = &level.navigation;
    k.push((Shape::Box, None, [n.width, n.height, n.step, n.slope, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0].map(f32::to_bits)));
    k
}

/// The level's still parts and the ground (reaching 10 m past them) as one triangle list, in the world.
fn triangles(level: &Level, geo: &Geometry) -> TriMesh {
    let mut t = TriMesh::default();
    let (mut lo, mut hi) = (Vec3::splat(-10.0), Vec3::splat(10.0));
    for p in level.parts.iter().filter(|p| counts(p)) {
        let Some(tris) = geo.tris(p) else { continue };
        let m = part_affine(p);
        for c in tris.pos.chunks_exact(3) {
            let base = t.vertices.len() as u32;
            for v in c {
                let w = m.transform_point3(*v);
                (lo, hi) = (lo.min(w - 10.0), hi.max(w + 10.0));
                t.vertices.push(Vec3A::from(w));
            }
            t.indices.push(UVec3::new(base, base + 1, base + 2));
            t.area_types.push(AreaType::NOT_WALKABLE);
        }
    }
    // the ground at y = 0
    let base = t.vertices.len() as u32;
    t.vertices.extend([Vec3::new(lo.x, 0.0, lo.z), Vec3::new(lo.x, 0.0, hi.z), Vec3::new(hi.x, 0.0, hi.z), Vec3::new(hi.x, 0.0, lo.z)].map(Vec3A::from));
    t.indices.extend([UVec3::new(base, base + 1, base + 2), UVec3::new(base, base + 2, base + 3)]);
    t.area_types.extend([AreaType::NOT_WALKABLE; 2]);
    t
}

/// The insides of upright boxes and cylinders, which are not walkable (Recast sees only surfaces, so a box looks
/// hollow, its floor tall enough to stand in).
fn insides(level: &Level) -> Vec<ConvexVolume> {
    level.parts.iter().filter(|p| counts(p) && matches!(p.shape, Shape::Box | Shape::Cylinder)).filter_map(|p| {
        let rot = Quat::from_array(p.rot);
        if (rot * Vec3::Y).y < 0.999 { return None; }
        let (c, s) = (Vec3::from(p.pos), Vec3::from(p.size));
        let n = if p.shape == Shape::Box { 4 } else { 16 };
        let vertices = (0..n).map(|k| {
            let a = std::f32::consts::TAU * (k as f32 + 0.5) / n as f32;
            let local = if p.shape == Shape::Box { Vec3::new(a.cos().signum(), 0.0, a.sin().signum()) * 0.5 } else { Vec3::new(a.cos(), 0.0, a.sin()) * 0.5 };
            let w = c + rot * (local * s);
            Vec2::new(w.x, w.z)
        }).collect();
        Some(ConvexVolume { vertices, min_y: c.y - s.y * 0.5 - 0.1, max_y: c.y + s.y * 0.5 - 0.25, area: AreaType::NOT_WALKABLE })
    }).collect()
}

/// Work out the level's navigation area.
pub fn build(level: &Level, geo: &Geometry) -> Result<Navmesh, String> {
    let n: &Navigation = &level.navigation;
    let mut trimesh = triangles(level, geo);
    let mut cfg = ConfigBuilder {
        agent_radius: (n.width * 0.5).max(0.05),
        agent_height: n.height.max(0.1),
        walkable_climb: n.step.max(0.0),
        walkable_slope_angle: n.slope.clamp(0.0, 89.0).to_radians(),
        ..Default::default()
    };
    cfg.aabb = trimesh.compute_aabb().ok_or("nothing to build from")?;
    let config = cfg.clone().build();
    let e = |e: &dyn std::fmt::Display| e.to_string();
    trimesh.mark_walkable_triangles(config.walkable_slope_angle);
    let mut field = HeightfieldBuilder { aabb: config.aabb, cell_size: config.cell_size, cell_height: config.cell_height }.build().map_err(|x| e(&x))?;
    field.rasterize_triangles(&trimesh, config.walkable_climb).map_err(|x| e(&x))?;
    field.filter_low_hanging_walkable_obstacles(config.walkable_climb);
    field.filter_ledge_spans(config.walkable_height, config.walkable_climb);
    field.filter_walkable_low_height_spans(config.walkable_height);
    let mut compact = field.into_compact(config.walkable_height, config.walkable_climb).map_err(|x| e(&x))?;
    compact.erode_walkable_area(config.walkable_radius);
    for v in insides(level) { compact.mark_convex_poly_area(&v); }
    compact.build_distance_field();
    compact.build_regions(config.border_size, config.min_region_area, config.merge_region_area).map_err(|x| e(&x))?;
    let contours = compact.build_contours(config.max_simplification_error, config.max_edge_len, config.contour_flags);
    let polygon = contours.into_polygon_mesh(config.max_vertices_per_polygon).map_err(|x| e(&x))?;
    let detail = DetailNavmesh::new(&polygon, &compact, config.detail_sample_dist, config.detail_sample_max_error).map_err(|x| e(&x))?;
    let settings = NavmeshSettings { agent_radius: cfg.agent_radius, agent_height: cfg.agent_height, walkable_climb: cfg.walkable_climb, walkable_slope_angle: cfg.walkable_slope_angle, ..Default::default() };
    Ok(Navmesh { polygon, detail, settings })
}

/// The area's surface as triangles in the world (to draw it).
pub fn surface(m: &Navmesh) -> Vec<[Vec3; 3]> {
    let d = &m.detail;
    let mut out = vec![];
    for s in &d.meshes {
        let v = &d.vertices[s.base_vertex_index as usize..][..s.vertex_count as usize];
        for t in &d.triangles[s.base_triangle_index as usize..][..s.triangle_count as usize] {
            out.push([v[t[0] as usize], v[t[1] as usize], v[t[2] as usize]]);
        }
    }
    out
}

/// How much ground the area covers, in square metres.
pub fn area(m: &Navmesh) -> f32 { surface(m).iter().map(|[a, b, c]| (*b - *a).cross(*c - *a).length() * 0.5).sum() }

/// The area's pieces that join up (walking from one to another along the area), as groups of polygon indices.
pub fn islands(m: &Navmesh) -> Vec<Vec<usize>> {
    let p = &m.polygon;
    let nvp = p.max_vertices_per_polygon as usize;
    let n = p.polygon_count();
    let mut seen = vec![false; n];
    let mut out = vec![];
    for s in 0..n {
        if seen[s] { continue; }
        seen[s] = true;
        let (mut group, mut todo) = (vec![], vec![s]);
        while let Some(i) = todo.pop() {
            group.push(i);
            for &nb in &p.polygon_neighbors[i * nvp..(i + 1) * nvp] {
                // (high bits flag edges between tiles; this area is one tile)
                let j = (nb & 0x7fff) as usize;
                if nb != bevy_rerecast::rerecast::PolygonNavmesh::NO_CONNECTION && j < n && !seen[j] {
                    seen[j] = true;
                    todo.push(j);
                }
            }
        }
        out.push(group);
    }
    out
}

/// A polygon's middle, in the world.
pub fn middle(m: &Navmesh, i: usize) -> Vec3 {
    let p = &m.polygon;
    let nvp = p.max_vertices_per_polygon as usize;
    let vs: Vec<Vec3> = p.polygons[i * nvp..(i + 1) * nvp].iter().take_while(|&&v| v != bevy_rerecast::rerecast::PolygonNavmesh::NO_INDEX)
        .map(|&v| { let u = p.vertices[v as usize]; p.aabb.min + Vec3::new(u.x as f32 * p.cell_size, u.y as f32 * p.cell_height, u.z as f32 * p.cell_size) })
        .collect();
    vs.iter().sum::<Vec3>() / vs.len().max(1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stairs_join_the_ground_and_a_landing_into_one_area_and_a_tall_block_is_an_island() {
        let geo = Geometry::default();
        let mut lvl = Level::default();
        let up = bevy::math::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2).to_array();
        lvl.add(Part::placed(Shape::Stairs, None, "Stairs".into(), [0.0, 1.0, 2.0], up, [4.0, 2.0, 2.0]));
        lvl.add(Part::placed(Shape::Box, None, "Landing".into(), [0.0, 1.0, -2.0], bevy::math::Quat::IDENTITY.to_array(), [2.0, 2.0, 4.0]));
        let m = build(&lvl, &geo).unwrap();
        let groups = islands(&m);
        let top = |g: &Vec<usize>| g.iter().any(|&i| middle(&m, i).y > 1.8);
        let ground = |g: &Vec<usize>| g.iter().any(|&i| middle(&m, i).y < 0.2);
        assert!(groups.iter().any(|g| top(g) && ground(g)), "the landing is reached from the ground by the stairs");
        assert!(!surface(&m).is_empty());
        // a box 3 m tall with no stairs: its top is walkable, but on its own
        lvl.parts.clear();
        lvl.add(Part::placed(Shape::Box, None, "Block".into(), [0.0, 1.5, 0.0], bevy::math::Quat::IDENTITY.to_array(), [4.0, 3.0, 4.0]));
        let m = build(&lvl, &geo).unwrap();
        let groups = islands(&m);
        let high = |g: &Vec<usize>| g.iter().any(|&i| middle(&m, i).y > 2.8);
        assert!(groups.iter().any(|g| high(g)) && !groups.iter().any(|g| high(g) && g.iter().any(|&i| middle(&m, i).y < 0.2)), "the block's top is an island");
        assert_eq!(groups.len(), 2, "the ground and the top: not the floor inside the block");
    }
}
