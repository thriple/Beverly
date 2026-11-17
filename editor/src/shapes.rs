//! The basic shapes, built in a unit box (-0.5..0.5 on every axis) and scaled by the part's size. The same
//! triangles make the render mesh and the ray tests used for clicking, surface placement and (later) collision.
use crate::level::Shape;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

/// Triangles with a normal per corner, grouped into faces (a basic shape's; a model's have none).
#[derive(Default, Clone)]
pub struct Tris {
    pub pos: Vec<Vec3>,
    pub nrm: Vec<Vec3>,
    /// The face of each triangle: an index into `faces`.
    pub face: Vec<u8>,
    /// How a texture lies on each face.
    pub faces: Vec<Lay>,
}

/// How a texture lies on a face, in metres, so it keeps its size when the part is resized.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lay {
    /// A flat face (its normal in the unit box): across it, with up kept up on walls.
    Flat(Vec3),
    /// Wrapped around the line through `centre` along `axis` (0 x, 1 y, 2 z), radius `r` in the unit box: a
    /// cylinder's side, the arch's opening.
    Around { axis: usize, centre: Vec2, r: f32 },
    /// Wrapped over a ball.
    Ball,
}

impl Tris {
    /// Start a new face; the triangles added after this belong to it.
    fn face(&mut self, lay: Lay) { self.faces.push(lay); }
    fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3) {
        let n = (b - a).cross(c - a).normalize_or_zero();
        self.tri_n([a, b, c], [n, n, n]);
    }
    fn tri_n(&mut self, p: [Vec3; 3], n: [Vec3; 3]) {
        self.pos.extend(p);
        self.nrm.extend(n);
        self.face.push(self.faces.len().saturating_sub(1) as u8);
    }
    pub fn mesh(&self) -> Mesh {
        let uv: Vec<[f32; 2]> = self.pos.iter().zip(&self.nrm).map(|(p, n)| box_uv(*p, *n)).collect();
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos.clone())
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm.clone())
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
            .with_inserted_indices(Indices::U32((0..self.pos.len() as u32).collect()))
    }

    /// The triangles of the faces `keep` accepts, as a mesh whose UVs are metres on a part of this `size` (one
    /// texture copy is 1 x 1 until the material scales it).
    pub fn faces_mesh(&self, size: Vec3, keep: impl Fn(u8) -> bool) -> Mesh {
        let (mut pos, mut nrm, mut uv) = (vec![], vec![], vec![]);
        for (t, &f) in self.face.iter().enumerate().filter(|(_, f)| keep(**f)) {
            let k = t * 3..t * 3 + 3;
            pos.extend_from_slice(&self.pos[k.clone()]);
            nrm.extend_from_slice(&self.nrm[k.clone()]);
            uv.extend(self.face_uvs(self.faces[f as usize], [self.pos[k.start], self.pos[k.start + 1], self.pos[k.start + 2]], size));
        }
        let n = pos.len() as u32;
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
            .with_inserted_indices(Indices::U32((0..n).collect()))
    }

    /// Where the corners of one triangle fall on its face's texture, in metres.
    fn face_uvs(&self, lay: Lay, p: [Vec3; 3], size: Vec3) -> [[f32; 2]; 3] {
        // angles around the face, kept from jumping where they wrap
        let unwrap = |a: [f32; 3]| a.map(|x| if x - a[0] > PI { x - TAU } else if a[0] - x > PI { x + TAU } else { x });
        match lay {
            Lay::Flat(n) => {
                let (u, v) = flat_axes(n, size);
                p.map(|q| [(q * size).dot(u), (q * size).dot(v)])
            }
            Lay::Around { axis, centre, r } => {
                let (a, b) = ([1, 0, 0][axis], [2, 2, 1][axis]);
                let ang = unwrap(p.map(|q| (q[b] - centre.y).atan2(q[a] - centre.x)));
                let rm = r * (size[a] + size[b]) * 0.5;
                let down = if axis == 1 { -1.0 } else { 1.0 };
                [0, 1, 2].map(|i| [ang[i] * rm, p[i][axis] * size[axis] * down])
            }
            Lay::Ball => {
                let rm = (size.x + size.y + size.z) / 6.0;
                let th = unwrap(p.map(|q| q.z.atan2(q.x)));
                [0, 1, 2].map(|i| [th[i] * rm, (p[i].y * 2.0).clamp(-1.0, 1.0).acos() * rm])
            }
        }
    }

    /// A readable name for each face ("Top", "Front", "Round side"; repeats numbered), for the panels and the MCP.
    pub fn face_names(&self) -> Vec<String> {
        let base: Vec<&str> = self.faces.iter().map(|l| match *l {
            Lay::Flat(n) if n.y > 0.9 => "Top",
            Lay::Flat(n) if n.y < -0.9 => "Bottom",
            Lay::Flat(n) if n.y.abs() > 0.2 => "Slope",
            Lay::Flat(n) if n.x.abs() >= n.z.abs() => if n.x > 0.0 { "Right" } else { "Left" },
            Lay::Flat(n) => if n.z > 0.0 { "Back" } else { "Front" },
            Lay::Around { axis: 1, .. } => "Round side",
            Lay::Around { .. } => "Opening",
            Lay::Ball => "Surface",
        }).collect();
        base.iter().enumerate().map(|(i, b)| {
            let same = base.iter().filter(|x| *x == b).count();
            if same == 1 { b.to_string() } else { format!("{b} {}", base[..=i].iter().filter(|x| *x == b).count()) }
        }).collect()
    }
    /// Corners of the box around all the triangles (zero box when empty).
    pub fn bounds(&self) -> (Vec3, Vec3) {
        if self.pos.is_empty() { return (Vec3::ZERO, Vec3::ZERO); }
        self.pos.iter().fold((Vec3::MAX, Vec3::MIN), |(a, b), p| (a.min(*p), b.max(*p)))
    }

    /// Nearest hit of a ray (in the shape's unit space): distance, the triangle's normal and its face (if it has faces).
    pub fn raycast(&self, o: Vec3, d: Vec3) -> Option<(f32, Vec3, Option<u8>)> {
        let mut best: Option<(f32, Vec3, Option<u8>)> = None;
        for (i, t) in self.pos.chunks_exact(3).enumerate() {
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
            if tt > 1e-5 && best.is_none_or(|(bt, _, _)| tt < bt) {
                best = Some((tt, e1.cross(e2).normalize_or_zero(), self.face.get(i).copied()));
            }
        }
        best
    }
}

/// The texture's across and down directions on a flat face with unit-box normal `n`, on a part of this `size`
/// (metres in, metres out). Floors and ceilings: x across, z down; walls and slopes: level across, downhill down.
pub fn flat_axes(n: Vec3, size: Vec3) -> (Vec3, Vec3) {
    let n = (n / size).normalize_or_zero();
    if n.y.abs() > 0.99 { return (Vec3::X, Vec3::Z); }
    let u = Vec3::Y.cross(n).normalize();
    (u, u.cross(n))
}

fn box_uv(p: Vec3, n: Vec3) -> [f32; 2] {
    let a = n.abs();
    if a.y >= a.x && a.y >= a.z { [p.x + 0.5, p.z + 0.5] } else if a.x >= a.z { [p.z + 0.5, 0.5 - p.y] } else { [p.x + 0.5, 0.5 - p.y] }
}

/// The unit-box triangles of a basic shape (a model's come from its file: see `models.rs`).
pub fn build(shape: Shape) -> Tris {
    match shape {
        Shape::Box | Shape::Plane | Shape::FixedCamera | Shape::OrbitCamera | Shape::FollowCamera => prism(&[vec2(-0.5, -0.5), vec2(0.5, -0.5), vec2(0.5, 0.5), vec2(-0.5, 0.5)], None),
        Shape::Ramp => prism(&[vec2(-0.5, -0.5), vec2(0.5, -0.5), vec2(0.5, 0.5)], None),
        Shape::Stairs => prism(&stairs_profile(6), None),
        Shape::Arch => prism(&arch_profile(), Some((2..26, vec2(0.0, ARCH_SPRING), ARCH_HALF))),
        Shape::Cylinder | Shape::PlayerStart | Shape::Interact => cylinder(40),
        Shape::Model => Tris::default(),
        Shape::Sphere => sphere(32, 18),
        Shape::PointLight => sphere(16, 10),
        Shape::SpotLight => lampshade(20),
        // a speaker seen from the side, facing +x
        Shape::Sound => prism(&[vec2(-0.5, -0.2), vec2(-0.1, -0.2), vec2(0.5, -0.5), vec2(0.5, 0.5), vec2(-0.1, 0.2), vec2(-0.5, 0.2)], None),
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
const ARCH_HALF: f32 = 0.3;
const ARCH_SPRING: f32 = -0.05;
fn arch_profile() -> Vec<Vec2> {
    let (w, ys) = (ARCH_HALF, ARCH_SPRING);
    let mut v = vec![vec2(-0.5, -0.5), vec2(-w, -0.5), vec2(-w, ys)];
    for i in 1..24 {
        let a = PI - PI * i as f32 / 24.0;
        v.push(vec2(a.cos() * w, ys + a.sin() * w));
    }
    v.extend([vec2(w, ys), vec2(w, -0.5), vec2(0.5, -0.5), vec2(0.5, 0.5), vec2(-0.5, 0.5)]);
    v
}

/// A counter-clockwise profile in x/y, extruded along z from -0.5 to 0.5. Each side is a face, except the sides
/// `round` names (edge i runs from point i to i + 1), which make one face wrapped around its centre and radius.
fn prism(profile: &[Vec2], round: Option<(std::ops::Range<usize>, Vec2, f32)>) -> Tris {
    let mut t = Tris::default();
    let n = profile.len();
    for i in 0..n {
        let a = profile[i];
        let b = profile[(i + 1) % n];
        match &round {
            Some((edges, centre, r)) if edges.contains(&i) => if i == edges.start { t.face(Lay::Around { axis: 2, centre: *centre, r: *r }) },
            _ => t.face(Lay::Flat(-(b - a).normalize().perp().extend(0.0))),
        }
        let (a0, b0, a1, b1) = (a.extend(-0.5), b.extend(-0.5), a.extend(0.5), b.extend(0.5));
        t.tri(a0, b0, b1);
        t.tri(a0, b1, a1);
    }
    let tris = ear_clip(profile);
    t.face(Lay::Flat(Vec3::Z));
    for &[i, j, k] in &tris { t.tri(profile[i].extend(0.5), profile[j].extend(0.5), profile[k].extend(0.5)); }
    t.face(Lay::Flat(Vec3::NEG_Z));
    for &[i, j, k] in &tris { t.tri(profile[k].extend(-0.5), profile[j].extend(-0.5), profile[i].extend(-0.5)); }
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
    let p = |c: f32, s: f32, y: f32| vec3(c * r, y, s * r);
    let at = |i: usize| { let a = TAU * i as f32 / seg as f32; (a.cos(), a.sin()) };
    t.face(Lay::Around { axis: 1, centre: Vec2::ZERO, r });
    for i in 0..seg {
        let ((c0, s0), (c1, s1)) = (at(i), at(i + 1));
        let (n0, n1) = (vec3(c0, 0.0, s0), vec3(c1, 0.0, s1));
        t.tri_n([p(c0, s0, -0.5), p(c1, s1, 0.5), p(c1, s1, -0.5)], [n0, n1, n1]);
        t.tri_n([p(c0, s0, -0.5), p(c0, s0, 0.5), p(c1, s1, 0.5)], [n0, n0, n1]);
    }
    t.face(Lay::Flat(Vec3::Y));
    for i in 0..seg {
        let ((c0, s0), (c1, s1)) = (at(i), at(i + 1));
        t.tri(vec3(0.0, 0.5, 0.0), p(c1, s1, 0.5), p(c0, s0, 0.5));
    }
    t.face(Lay::Flat(Vec3::NEG_Y));
    for i in 0..seg {
        let ((c0, s0), (c1, s1)) = (at(i), at(i + 1));
        t.tri(vec3(0.0, -0.5, 0.0), p(c0, s0, -0.5), p(c1, s1, -0.5));
    }
    t
}

/// A spot light's stand-in: a cone opening toward +Y (the way it shines), closed at the wide end.
fn lampshade(seg: usize) -> Tris {
    let mut t = Tris::default();
    let apex = vec3(0.0, -0.5, 0.0);
    let rim = |i: usize| { let a = TAU * i as f32 / seg as f32; vec3(a.cos() * 0.5, 0.5, a.sin() * 0.5) };
    t.face(Lay::Around { axis: 1, centre: Vec2::ZERO, r: 0.25 });
    for i in 0..seg { t.tri(apex, rim(i), rim(i + 1)); }
    t.face(Lay::Flat(Vec3::Y));
    for i in 0..seg { t.tri(vec3(0.0, 0.5, 0.0), rim(i + 1), rim(i)); }
    t
}

fn sphere(seg: usize, rings: usize) -> Tris {
    let mut t = Tris::default();
    t.face(Lay::Ball);
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
    let meshes = Shape::built().map(|s| (s, meshes.add(build(s).mesh()))).collect();
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

    #[test]
    fn faces_have_names_and_textures_lie_in_metres() {
        for s in Shape::built() {
            let t = build(s);
            assert_eq!(t.face.len() * 3, t.pos.len(), "{s:?}: a face for every triangle");
            assert!(t.face.iter().all(|&f| (f as usize) < t.faces.len()), "{s:?}");
        }
        let mut names = build(Shape::Box).face_names();
        names.sort();
        assert_eq!(names, ["Back", "Bottom", "Front", "Left", "Right", "Top"]);
        assert_eq!(build(Shape::Cylinder).face_names(), ["Round side", "Top", "Bottom"]);
        assert!(build(Shape::Arch).face_names().contains(&"Opening".to_string()));
        assert!(build(Shape::Ramp).face_names().contains(&"Slope".to_string()));
        // the front of a 4 x 2 x 1 box spans 4 m across and 2 m down, whatever the unit box says
        let t = build(Shape::Box);
        let front = t.face_names().iter().position(|n| n == "Front").unwrap() as u8;
        let m = t.faces_mesh(vec3(4.0, 2.0, 1.0), |f| f == front);
        let Some(bevy::mesh::VertexAttributeValues::Float32x2(uv)) = m.attribute(Mesh::ATTRIBUTE_UV_0) else { panic!() };
        let span = |k: usize| uv.iter().map(|c| c[k]).fold(f32::MIN, f32::max) - uv.iter().map(|c| c[k]).fold(f32::MAX, f32::min);
        assert!((span(0) - 4.0).abs() < 1e-4 && (span(1) - 2.0).abs() < 1e-4, "{} x {}", span(0), span(1));
        // around a cylinder of 2 m across: the way round is about 2 pi m
        let t = build(Shape::Cylinder);
        let m = t.faces_mesh(vec3(2.0, 3.0, 2.0), |f| f == 0);
        let Some(bevy::mesh::VertexAttributeValues::Float32x2(uv)) = m.attribute(Mesh::ATTRIBUTE_UV_0) else { panic!() };
        let round = uv.chunks(3).map(|c| c.iter().map(|x| x[0]).fold(f32::MIN, f32::max) - c.iter().map(|x| x[0]).fold(f32::MAX, f32::min)).sum::<f32>() / 2.0;
        assert!((round - TAU).abs() < 0.05, "{round}");
    }
}
