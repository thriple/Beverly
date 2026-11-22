//! Where the grass grows. Every flat face of a part that has a grass-growing material on it (laid on the face, or
//! painted there with the brush, pen or bucket) is a sheet: the face's frame in the world and, over it, what grows
//! and the colour of the surface at each spot, worked out from the part's strokes whenever the part changes and
//! never stored. Tops, walls, ceilings and floors stacked over each other are each their own sheet.
use crate::level::{Level, Part};
use crate::shapes::Lay;
use bevy::prelude::*;

/// Metres per spot of a sheet's maps, and the most spots along a side (a bigger face gets bigger spots).
pub const TEXEL: f32 = 0.5;
pub const MOST: u32 = 512;
/// The most grass-growing materials a level can have (the shader's table).
pub const MOST_GROWERS: usize = 32;

/// One face's grass.
#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    pub face: u8,
    /// The face's corner the maps start at, its two directions across (unit) and its normal, in the world.
    pub origin: Vec3,
    pub u: Vec3,
    pub v: Vec3,
    pub normal: Vec3,
    /// Metres across, along `u` and `v`.
    pub size: Vec2,
    pub dims: UVec2,
    /// Per spot: which grass-growing material (an index into [`growers`]), how thick (0 to 255), 0, 0.
    pub grows: Vec<u8>,
    /// Per spot: the colour of the surface there (sRGB, 0 to 255), and 255.
    pub colour: Vec<u8>,
}

/// The level's grass-growing materials, in a fixed order (the shader's table of them).
pub fn growers(level: &Level) -> Vec<String> {
    level.materials.iter().filter(|(_, m)| !m.grass.is_empty()).map(|(k, _)| k.clone()).take(MOST_GROWERS).collect()
}

/// Across a face with this normal: along the world's x on floors and ceilings, level along walls; then up the face.
fn across(n: Vec3) -> (Vec3, Vec3) {
    let u = if n.y.abs() > 0.7 { (Vec3::X - n * n.x).normalize() } else { Vec3::Y.cross(n).normalize() };
    (u, n.cross(u))
}

/// How far a point is from a triangle, seen flat (0 inside).
fn off_triangle(p: Vec2, t: [Vec2; 3]) -> f32 {
    let side = |a: Vec2, b: Vec2| (b - a).perp_dot(p - a);
    let (s0, s1, s2) = (side(t[0], t[1]), side(t[1], t[2]), side(t[2], t[0]));
    if (s0 >= 0.0 && s1 >= 0.0 && s2 >= 0.0) || (s0 <= 0.0 && s1 <= 0.0 && s2 <= 0.0) { return 0.0; }
    let seg = |a: Vec2, b: Vec2| { let ab = b - a; let k = ((p - a).dot(ab) / ab.dot(ab).max(1e-12)).clamp(0.0, 1.0); p.distance(a + ab * k) };
    seg(t[0], t[1]).min(seg(t[1], t[2])).min(seg(t[2], t[0]))
}

fn srgb(c: Vec3) -> [u8; 3] { c.to_array().map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8) }

/// The sheets of a part (none when nothing on it grows grass). Curved faces (a cylinder's side, a ball) grow none.
pub fn sheets(level: &Level, p: &Part, growers: &[String]) -> Vec<Sheet> {
    if !crate::paint::paintable(p) || growers.is_empty() { return vec![]; }
    let grows = |m: &str| growers.iter().position(|g| g == m).map(|i| (i, level.materials[m].grass_density.clamp(0.0, 1.0)));
    let layers = p.painted();
    let painted: Vec<(Option<(usize, f32)>, Vec3)> = layers.iter().map(|m| (grows(m), level.materials.get(*m).map_or(Vec3::ONE, |x| Vec3::from(x.tint)))).collect();
    let tris = crate::shapes::build(p.shape);
    let base_grows = |f: u8| crate::bucket::face_material(p, Some(f)).and_then(grows);
    if painted.iter().all(|l| l.0.is_none()) && (0..tris.faces.len() as u8).all(|f| base_grows(f).is_none()) { return vec![]; }
    let pieces = crate::paint::pieces(p);
    let count = if p.strokes.is_empty() { 0 } else { pieces.len() };
    let (a, turn, size) = (crate::world::part_affine(p), Quat::from_array(p.rot), Vec3::from(p.size));
    let mut out = vec![];
    for (f, lay) in tris.faces.iter().enumerate() {
        let Lay::Flat(_) = lay else { continue };
        let f = f as u8;
        let corners: Vec<Vec3> = tris.face.iter().enumerate().filter(|(_, x)| **x == f).flat_map(|(t, _)| (0..3).map(move |k| t * 3 + k)).map(|k| a.transform_point3(tris.pos[k])).collect();
        let Some(k0) = tris.face.iter().position(|x| *x == f) else { continue };
        let normal = (turn * (tris.nrm[k0 * 3] / size.max(Vec3::splat(1e-4)))).normalize_or_zero();
        if normal == Vec3::ZERO { continue; }
        let (u, v) = across(normal);
        let flat: Vec<Vec2> = corners.iter().map(|c| Vec2::new(c.dot(u), c.dot(v))).collect();
        let (lo, hi) = flat.iter().fold((Vec2::MAX, Vec2::MIN), |(lo, hi), q| (lo.min(*q), hi.max(*q)));
        let span = hi - lo;
        if span.min_element() < 1e-3 { continue; }
        let texel = TEXEL.max(span.max_element() / MOST as f32);
        let dims = (span / texel).ceil().as_uvec2().max(UVec2::ONE);
        let origin = corners[0] + u * (lo.x - flat[0].x) + v * (lo.y - flat[0].y);
        let tri2: Vec<[Vec2; 3]> = flat.chunks_exact(3).map(|t| [t[0] - lo, t[1] - lo, t[2] - lo]).collect();
        let base = base_grows(f);
        let base_colour = crate::bucket::face_colour(level, p, Some(f));
        let n = (dims.x * dims.y) as usize;
        let (mut g, mut c) = (vec![0u8; n * 4], vec![255u8; n * 4]);
        let mut any = false;
        for j in 0..dims.y {
            for i in 0..dims.x {
                let at = (Vec2::new(i as f32, j as f32) + 0.5) * texel;
                if !tri2.iter().any(|t| off_triangle(at, *t) <= texel * 0.5) { continue; }
                let w = origin + u * at.x + v * at.y;
                let local = p.to_local(w);
                let (mut left, mut colour, mut best, mut thick) = (1.0f32, Vec3::ZERO, (0.0f32, 0usize), 0.0f32);
                for (l, (gr, tint)) in painted.iter().enumerate() {
                    let cov = crate::paint::coverage(&pieces, count, l as i32, local, f, normal.y, w.y);
                    if cov <= 0.0 { continue; }
                    left -= cov;
                    colour += *tint * cov;
                    if let Some((gi, d)) = gr {
                        thick += cov * d;
                        if cov * d > best.0 { best = (cov * d, *gi); }
                    }
                }
                let left = left.max(0.0);
                colour += base_colour * left;
                if let Some((gi, d)) = base {
                    thick += left * d;
                    if left * d > best.0 { best = (left * d, gi); }
                }
                let o = ((j * dims.x + i) * 4) as usize;
                c[o..o + 3].copy_from_slice(&srgb(colour));
                if thick > 0.0 {
                    g[o] = best.1 as u8;
                    g[o + 1] = (thick.min(1.0) * 255.0).round() as u8;
                    any |= g[o + 1] > 0;
                }
            }
        }
        if any { out.push(Sheet { face: f, origin, u, v, normal, size: dims.as_vec2() * texel, dims, grows: g, colour: c }); }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{Grows, Material, Shape, Stroke, Surface};

    fn meadow() -> Material { Material { tint: [0.4, 0.3, 0.2], grass: vec![Grows { species: "grass".into(), share: 1.0 }], ..Default::default() } }

    #[test]
    fn a_painted_top_grows_where_painted_and_a_face_laid_with_it_grows_all_over() {
        let mut level = Level::default();
        level.materials.insert("assets/materials/meadow.bev".into(), meadow());
        let mut p = Part::placed(Shape::Box, None, "Floor".into(), [0.0, 0.5, 0.0], [0.0, 0.0, 0.0, 1.0], [10.0, 1.0, 10.0]);
        // a round dab of meadow 2 m across on the top, at x = -3
        p.strokes = vec![Stroke { material: "assets/materials/meadow.bev".into(), size: 2.0, hardness: 1.0, strength: 1.0, points: vec![[-3.0, 0.5, 0.0]], lays: Default::default(), rules: Default::default(), curve: None, fill: false, faces: vec![], bucket: None }];
        let g = growers(&level);
        let s = sheets(&level, &p, &g);
        assert_eq!(s.len(), 1, "only the top: {:?}", s.iter().map(|s| s.face).collect::<Vec<_>>());
        let top = &s[0];
        assert!(top.normal.y > 0.99 && (top.origin.y - 1.0).abs() < 1e-4, "{:?}", top.origin);
        assert_eq!(top.dims, UVec2::new(20, 20));
        let at = |x: f32, z: f32| { let q = Vec3::new(x, 1.0, z) - top.origin; let (i, j) = ((q.dot(top.u) / TEXEL) as u32, (q.dot(top.v) / TEXEL) as u32); let o = ((j * top.dims.x + i) * 4) as usize; (top.grows[o + 1], [top.colour[o], top.colour[o + 1], top.colour[o + 2]]) };
        assert_eq!(at(-3.0, 0.0).0, 255, "thick where painted");
        assert_eq!(at(3.0, 0.0).0, 0, "bare elsewhere");
        assert_eq!(at(-3.0, 0.0).1, srgb(Vec3::new(0.4, 0.3, 0.2)), "the roots take the meadow's colour");
        // lay meadow all over the part instead: every flat face grows
        p.strokes.clear();
        p.surfaces = vec![Surface { face: None, material: "assets/materials/meadow.bev".into(), tile: [1.0; 2], offset: [0.0; 2] }];
        assert_eq!(sheets(&level, &p, &g).len(), 6);
        // nothing grows grass: no sheets
        level.materials.get_mut("assets/materials/meadow.bev").unwrap().grass.clear();
        assert!(sheets(&level, &p, &growers(&level)).is_empty());
    }

    #[test]
    fn a_ramps_side_is_a_triangle_and_grows_only_inside_it() {
        let mut level = Level::default();
        level.materials.insert("assets/materials/meadow.bev".into(), meadow());
        let mut p = Part::placed(Shape::Ramp, None, "Ramp".into(), [0.0; 3], [0.0, 0.0, 0.0, 1.0], [4.0, 4.0, 4.0]);
        p.surfaces = vec![Surface { face: None, material: "assets/materials/meadow.bev".into(), tile: [1.0; 2], offset: [0.0; 2] }];
        let s = sheets(&level, &p, &growers(&level));
        let side = s.iter().find(|s| s.normal.z.abs() > 0.99).expect("a side (the ramp rises toward +x)");
        let thick = side.grows.chunks_exact(4).filter(|g| g[1] > 0).count() as f32 / (side.dims.x * side.dims.y) as f32;
        assert!(thick > 0.4 && thick < 0.75, "about half of its box: {thick}");
    }
}
