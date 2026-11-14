//! The basic shapes, built in a unit box (-0.5..0.5 on every axis) and scaled by the part's size. The same
//! triangles make the render mesh and the ray tests used for clicking, surface placement and (later) collision.
use crate::level::Shape;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

/// Triangles with a normal per corner.
#[derive(Default, Clone)]
pub struct Tris { pub pos: Vec<Vec3>, pub nrm: Vec<Vec3> }

impl Tris {
    fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3) {
        let n = (b - a).cross(c - a).normalize_or_zero();
        self.pos.extend([a, b, c]);
        self.nrm.extend([n, n, n]);
    }
    fn tri_n(&mut self, p: [Vec3; 3], n: [Vec3; 3]) {
        self.pos.extend(p);
        self.nrm.extend(n);
    }
    pub fn mesh(&self) -> Mesh {
        let uv: Vec<[f32; 2]> = self.pos.iter().zip(&self.nrm).map(|(p, n)| box_uv(*p, *n)).collect();
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos.clone())
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm.clone())
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
            .with_inserted_indices(Indices::U32((0..self.pos.len() as u32).collect()))
    }
    /// Nearest hit of a ray (in the shape's unit space) and the triangle's normal there.
    pub fn raycast(&self, o: Vec3, d: Vec3) -> Option<(f32, Vec3)> {
        let mut best: Option<(f32, Vec3)> = None;
        for t in self.pos.chunks_exact(3) {
            let (a, b, c) = (t[0], t[1], t[2]);
            let e1 = b - a;
            let e2 = c - a;
            let p = d.cross(e2);
            let det = e1.dot(p);
            if det.abs() < 1e-9 { continue; }
            let inv = 1.0 / det;
            let s = o - a;
            let u = s.dot(p) * inv;
            if !(0.0..=1.0).contains(&u) { continue; }
            let q = s.cross(e1);
            let v = d.dot(q) * inv;
            if v < 0.0 || u + v > 1.0 { continue; }
            let tt = e2.dot(q) * inv;
            if tt > 1e-5 && best.is_none_or(|(bt, _)| tt < bt) {
                best = Some((tt, e1.cross(e2).normalize_or_zero()));
            }
        }
        best
    }
}

fn box_uv(p: Vec3, n: Vec3) -> [f32; 2] {
    let a = n.abs();
    if a.y >= a.x && a.y >= a.z { [p.x + 0.5, p.z + 0.5] } else if a.x >= a.z { [p.z + 0.5, 0.5 - p.y] } else { [p.x + 0.5, 0.5 - p.y] }
}

pub fn build(shape: Shape) -> Tris {
    match shape {
        Shape::Box | Shape::Plane => prism(&[vec2(-0.5, -0.5), vec2(0.5, -0.5), vec2(0.5, 0.5), vec2(-0.5, 0.5)]),
        Shape::Ramp => prism(&[vec2(-0.5, -0.5), vec2(0.5, -0.5), vec2(0.5, 0.5)]),
        Shape::Stairs => prism(&stairs_profile(6)),
        Shape::Arch => prism(&arch_profile()),
        Shape::Cylinder => cylinder(40),
        Shape::Sphere => sphere(32, 18),
    }
}

/// Steps rising toward +x.
fn stairs_profile(n: usize) -> Vec<Vec2> {
    let top = |k: usize| -0.5 + (k + 1) as f32 / n as f32;
    let mut v = vec![vec2(-0.5, -0.5), vec2(0.5, -0.5), vec2(0.5, 0.5)];
    for k in (0..n).rev() {
        let x = -0.5 + k as f32 / n as f32;
        v.push(vec2(x, top(k)));
        if k > 0 { v.push(vec2(x, top(k - 1))); }
    }
    v
}

/// A block with a round-topped opening through it.
fn arch_profile() -> Vec<Vec2> {
    let (w, ys) = (0.3, -0.05);
    let mut v = vec![vec2(-0.5, -0.5), vec2(-w, -0.5), vec2(-w, ys)];
    for i in 1..24 {
        let a = PI - PI * i as f32 / 24.0;
        v.push(vec2(a.cos() * w, ys + a.sin() * w));
    }
    v.extend([vec2(w, ys), vec2(w, -0.5), vec2(0.5, -0.5), vec2(0.5, 0.5), vec2(-0.5, 0.5)]);
    v
}

/// A counter-clockwise profile in x/y, extruded along z from -0.5 to 0.5.
fn prism(profile: &[Vec2]) -> Tris {
    let mut t = Tris::default();
    let n = profile.len();
    for i in 0..n {
        let a = profile[i];
        let b = profile[(i + 1) % n];
        let (a0, b0, a1, b1) = (a.extend(-0.5), b.extend(-0.5), a.extend(0.5), b.extend(0.5));
        t.tri(a0, b0, b1);
        t.tri(a0, b1, a1);
    }
    for [i, j, k] in ear_clip(profile) {
        let (a, b, c) = (profile[i], profile[j], profile[k]);
        t.tri(a.extend(0.5), b.extend(0.5), c.extend(0.5));
        t.tri(c.extend(-0.5), b.extend(-0.5), a.extend(-0.5));
    }
    t
}

/// Triangulate a simple counter-clockwise polygon.
fn ear_clip(p: &[Vec2]) -> Vec<[usize; 3]> {
    let mut idx: Vec<usize> = (0..p.len()).collect();
    let mut out = Vec::new();
    let cross = |a: Vec2, b: Vec2, c: Vec2| (b - a).perp_dot(c - a);
    let mut guard = 0;
    while idx.len() > 3 && guard < 10_000 {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (p[ia], p[ib], p[ic]);
            if cross(a, b, c) <= 1e-9 { continue; }
            let inside = idx.iter().any(|&j| {
                if j == ia || j == ib || j == ic { return false; }
                let q = p[j];
                cross(a, b, q) >= 0.0 && cross(b, c, q) >= 0.0 && cross(c, a, q) >= 0.0
            });
            if inside { continue; }
            out.push([ia, ib, ic]);
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped { break; }
    }
    if idx.len() == 3 { out.push([idx[0], idx[1], idx[2]]); }
    out
}

fn cylinder(seg: usize) -> Tris {
    let mut t = Tris::default();
    let r = 0.5;
    for i in 0..seg {
        let a0 = TAU * i as f32 / seg as f32;
        let a1 = TAU * (i + 1) as f32 / seg as f32;
        let (c0, s0, c1, s1) = (a0.cos(), a0.sin(), a1.cos(), a1.sin());
        let p = |c: f32, s: f32, y: f32| vec3(c * r, y, s * r);
        let (n0, n1) = (vec3(c0, 0.0, s0), vec3(c1, 0.0, s1));
        t.tri_n([p(c0, s0, -0.5), p(c1, s1, 0.5), p(c1, s1, -0.5)], [n0, n1, n1]);
        t.tri_n([p(c0, s0, -0.5), p(c0, s0, 0.5), p(c1, s1, 0.5)], [n0, n0, n1]);
        t.tri(vec3(0.0, 0.5, 0.0), p(c1, s1, 0.5), p(c0, s0, 0.5));
        t.tri(vec3(0.0, -0.5, 0.0), p(c0, s0, -0.5), p(c1, s1, -0.5));
    }
    t
}

fn sphere(seg: usize, rings: usize) -> Tris {
    let mut t = Tris::default();
    let pt = |i: usize, j: usize| {
        let th = TAU * i as f32 / seg as f32;
        let ph = PI * j as f32 / rings as f32;
        vec3(ph.sin() * th.cos(), ph.cos(), ph.sin() * th.sin())
    };
    for j in 0..rings {
        for i in 0..seg {
            let (a, b, c, d) = (pt(i, j), pt(i + 1, j), pt(i + 1, j + 1), pt(i, j + 1));
            if j > 0 { t.tri_n([a * 0.5, b * 0.5, c * 0.5], [a, b, c]); }
            if j + 1 < rings { t.tri_n([a * 0.5, c * 0.5, d * 0.5], [a, c, d]); }
        }
    }
    t
}

/// One render mesh per shape, shared by every part of that shape (Bevy batches them).
#[derive(Resource)]
pub struct ShapeLib { pub meshes: Vec<(Shape, Handle<Mesh>)> }

impl ShapeLib {
    pub fn mesh(&self, s: Shape) -> Handle<Mesh> { self.meshes.iter().find(|(k, _)| *k == s).unwrap().1.clone() }
}

pub fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let meshes = Shape::ALL.iter().map(|&s| (s, meshes.add(build(s).mesh()))).collect();
    commands.insert_resource(ShapeLib { meshes });
}

/// Edges of the unit box, for outlines.
pub const BOX_EDGES: [(usize, usize); 12] = [(0, 1), (1, 3), (3, 2), (2, 0), (4, 5), (5, 7), (7, 6), (6, 4), (0, 4), (1, 5), (2, 6), (3, 7)];
pub fn box_corner(i: usize) -> Vec3 {
    vec3(if i & 1 == 0 { -0.5 } else { 0.5 }, if i & 2 == 0 { -0.5 } else { 0.5 }, if i & 4 == 0 { -0.5 } else { 0.5 })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shapes_are_closed_and_hit() {
        for s in Shape::ALL {
            let t = build(s);
            assert!(t.pos.len() % 3 == 0 && !t.pos.is_empty(), "{s:?}");
            // a ray from above, down through the middle of the top (arch: through a leg), hits something
            let x = if s == Shape::Arch { -0.4 } else { 0.1 };
            let hit = t.raycast(vec3(x, 2.0, 0.05), Vec3::NEG_Y);
            assert!(hit.is_some(), "{s:?} not hit");
            assert!(hit.unwrap().1.y > 0.0, "{s:?} normal faces down");
        }
        // the arch is open in the middle at the bottom
        assert!(build(Shape::Arch).raycast(vec3(0.0, -0.4, 2.0), Vec3::NEG_Z).is_none());
    }
}
