//! Brush strokes, drawn, in the editor and the game: each material painted on a part is a see-through copy of the
//! part (a child of its entity) whose shader (paint.wesl) works out, pixel by pixel, how much of that material shows
//! from the part's strokes in order: a stroke of it adds over what is there (or takes its own away, or sets it to
//! exactly its strength), any other stroke (another material, or erasing) covers it, and each stroke lands only where
//! its rules (slope, height) let it. Only the strokes are kept; nothing is painted into an image.
use crate::level::{Level, Part, Shape};
use crate::world::{Mats, PartId};
use bevy::ecs::system::SystemParam;
use bevy::light::NotShadowCaster;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;
use std::collections::HashMap;

/// How big one copy of a painted material's texture is (metres).
pub const TILE: f32 = 2.0;

pub type PaintMaterial = ExtendedMaterial<StandardMaterial, PaintLayer>;

/// One painted layer: which it is, where the part is, and the part's strokes as pieces.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct PaintLayer {
    #[uniform(100)]
    pub frame: Frame,
    #[storage(101, read_only)]
    pub pieces: Handle<ShaderBuffer>,
}

#[derive(ShaderType, Reflect, Debug, Clone, Copy, Default, PartialEq)]
pub struct Frame {
    /// The part's middle in the world.
    pub centre: Vec4,
    /// The turn from the world into the part's frame (a quaternion).
    pub unturn: Vec4,
    /// x: this layer, y: how many pieces.
    pub info: IVec4,
}

/// A piece of a stroke: from `a` to `b` (w: its radius, its hardness), `c`: strength, layer (-1 erases), stroke, how
/// it lays (0 add, 1 subtract, 2 set); `d`: its rules (the cosine of the steepest slope, below -1 any; lowest and
/// highest world height). A fill's outline is made of pieces too: each an edge from `a` to `b` in the world (`a.w`: the
/// width of the feathered edge), marked by `d.w` of 1.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Piece {
    pub a: Vec4,
    pub b: Vec4,
    pub c: Vec4,
    pub d: Vec4,
}

impl MaterialExtension for PaintLayer {
    fn fragment_shader() -> ShaderRef { "embedded://beverly/paint.wesl".into() }
}

pub struct PaintPlugin;

impl Plugin for PaintPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "paint.wesl");
        app.add_plugins(MaterialPlugin::<PaintMaterial>::default());
    }
}

/// Whether a part can be painted: a visible basic shape.
pub fn paintable(p: &Part) -> bool { p.visible && Shape::ALL.contains(&p.shape) }

/// A part's strokes as pieces for the shader (at least one, so the buffer is never empty).
pub fn pieces(p: &Part) -> Vec<Piece> {
    let layers = p.painted();
    let mut v = vec![];
    for (k, s) in p.strokes.iter().enumerate() {
        let layer = layers.iter().position(|m| *m == s.material).map_or(-1.0, |i| i as f32);
        let lays = match s.lays { crate::level::Lays::Add => 0.0, crate::level::Lays::Subtract => 1.0, crate::level::Lays::Set => 2.0 };
        let c = Vec4::new(s.strength.clamp(0.0, 1.0), layer, k as f32, lays);
        let (cos, lo, hi) = s.rules.shader();
        let d = Vec4::new(cos, lo, hi, 0.0);
        if s.fill {
            // the outline, edge by edge, closed (the shader works out which side is inside)
            let n = s.points.len();
            if n < 3 { continue; }
            for k in 0..n {
                v.push(Piece { a: Vec3::from(s.points[k]).extend(s.size.max(0.0)), b: Vec3::from(s.points[(k + 1) % n]).extend(0.0), c, d: Vec4::new(cos, lo, hi, 1.0) });
            }
            continue;
        }
        let r = (s.size * 0.5).max(0.005);
        let pts: Vec<Vec3> = s.points.iter().map(|q| Vec3::from(*q)).collect();
        let pairs: Vec<(Vec3, Vec3)> = match pts.len() {
            0 => continue,
            1 => vec![(pts[0], pts[0])],
            _ => pts.windows(2).map(|w| (w[0], w[1])).collect(),
        };
        for (a, b) in pairs { v.push(Piece { a: a.extend(r), b: b.extend(s.hardness.clamp(0.0, 1.0)), c, d }); }
    }
    if v.is_empty() { v.push(Piece::default()); }
    v
}

/// Pieces as the shader reads them.
fn upload(pieces: Vec<Piece>) -> ShaderBuffer {
    let flat: Vec<[f32; 16]> = pieces.iter().map(|p| {
        let mut f = [0.0; 16];
        for (i, v) in [p.a, p.b, p.c, p.d].iter().enumerate() { f[i * 4..i * 4 + 4].copy_from_slice(&v.to_array()); }
        f
    }).collect();
    ShaderBuffer::from(flat)
}

/// Whether a fill outline's edge from `a` to `b` crosses the line going on from `p` in +x (seen from above), and how
/// near the edge is to `p`: as in paint.wesl.
fn edge(a: Vec3, b: Vec3, p: Vec3) -> (bool, f32) {
    let crosses = (a.z > p.z) != (b.z > p.z) && p.x < a.x + (p.z - a.z) * (b.x - a.x) / (b.z - a.z);
    let (ab, ap) = (Vec2::new(b.x - a.x, b.z - a.z), Vec2::new(p.x - a.x, p.z - a.z));
    let t = (ap.dot(ab) / ab.dot(ab).max(1e-8)).clamp(0.0, 1.0);
    (crosses, (ap - ab * t).length())
}

/// How much of layer `layer` shows at `at` (in the part's frame, `world` in the world), on a surface whose normal
/// points `up` this much (1: flat) at height `y` in the world: the shader's sum, for tests and the MCP.
pub fn coverage(pieces: &[Piece], count: usize, layer: i32, at: Vec3, world: Vec3, up: f32, y: f32) -> f32 {
    let dab = |p: &Piece| {
        let (a, b) = (p.a.truncate(), p.b.truncate());
        let ab = b - a;
        let t = ((at - a).dot(ab) / ab.dot(ab).max(1e-8)).clamp(0.0, 1.0);
        let d = at.distance(a + ab * t);
        let (lo, hi) = (p.a.w * p.b.w.min(0.98), p.a.w);
        let x = ((d - lo) / (hi - lo)).clamp(0.0, 1.0);
        1.0 - x * x * (3.0 - 2.0 * x)
    };
    let (mut cov, mut i) = (0.0, 0);
    while i < count {
        let (stroke, l, strength, lays, d) = (pieces[i].c.z, pieces[i].c.y as i32, pieces[i].c.x, pieces[i].c.w, pieces[i].d);
        let mut m: f32 = 0.0;
        let (mut fill, mut inside, mut near, mut feather) = (false, false, f32::MAX, 0.0f32);
        while i < count && pieces[i].c.z == stroke {
            let p = &pieces[i];
            if p.d.w > 0.5 {
                let (crossed, d) = edge(p.a.truncate(), p.b.truncate(), world);
                (fill, inside, near, feather) = (true, inside ^ crossed, near.min(d), p.a.w);
            } else {
                m = m.max(dab(p));
            }
            i += 1;
        }
        if fill {
            let x = (near / feather.max(1e-3)).clamp(0.0, 1.0);
            m = if inside { x * x * (3.0 - 2.0 * x) } else { 0.0 };
        }
        let rules = crate::level::Rules { slope: if d.x < -1.0 { 90.0 } else { d.x.clamp(-1.0, 1.0).acos().to_degrees() }, band: (d.y > -1e8).then_some([d.y, d.z]), on: None };
        m *= rules.allows(up, y);
        let f = m * strength;
        cov = lay(cov, l == layer, l < 0, lays, m, f, strength);
    }
    cov
}

/// One stroke over a layer's coverage `cov`: `own` when it is this layer's, `erase` when it erases; `m` how much of it
/// reaches here (its edge and rules), `f` that times its strength. As in paint.wesl.
fn lay(cov: f32, own: bool, erase: bool, lays: f32, m: f32, f: f32, strength: f32) -> f32 {
    match (own, lays as i32) {
        (true, 1) => cov * (1.0 - f),
        (true, 2) => cov + (strength - cov) * m,
        (true, _) => cov + f * (1.0 - cov),
        // taking its own layer away leaves the others
        (false, 1) if !erase => cov,
        (false, _) => cov * (1.0 - f),
    }
}

/// A painted part as drawn: its entity, what its layers were made from, and their materials and pieces.
struct Drawn {
    root: Entity,
    look: (Shape, [u32; 3], Vec<(String, Option<crate::level::Material>)>),
    strokes: Vec<crate::level::Stroke>,
    frame: (Vec3, Quat),
    kids: Vec<Entity>,
    layers: Vec<Handle<PaintMaterial>>,
    buffer: Handle<ShaderBuffer>,
}

/// Painted layers kept in step with a level (see [`PaintSync::run`]).
#[derive(SystemParam)]
pub struct PaintSync<'w, 's> {
    commands: Commands<'w, 's>,
    roots: Query<'w, 's, (Entity, &'static PartId, &'static GlobalTransform)>,
    mats: ResMut<'w, Mats>,
    meshes: ResMut<'w, Assets<Mesh>>,
    paints: ResMut<'w, Assets<PaintMaterial>>,
    buffers: ResMut<'w, Assets<ShaderBuffer>>,
    server: Res<'w, AssetServer>,
    drawn: Local<'s, HashMap<u64, Drawn>>,
}

/// Marks a painted layer's entity.
#[derive(Component)]
pub struct Painted;

impl PaintSync<'_, '_> {
    /// Make the painted layers match `level`: new strokes reach the shader, a part that changed shape or size gets
    /// new layers, and the layers follow their part as it moves.
    pub fn run(&mut self, level: &Level) {
        let roots: HashMap<u64, (Entity, Vec3, Quat)> = self.roots.iter().map(|(e, id, g)| {
            let (_, r, t) = g.to_scale_rotation_translation();
            (id.0, (e, t, r))
        }).collect();
        // gone: the part, its strokes, or its entity
        let keep: Vec<u64> = level.parts.iter().filter(|p| !p.strokes.is_empty() && paintable(p) && roots.contains_key(&p.id)).map(|p| p.id).collect();
        let gone: Vec<u64> = self.drawn.keys().filter(|id| !keep.contains(id)).copied().collect();
        for id in gone {
            let d = self.drawn.remove(&id).unwrap();
            for k in d.kids { self.commands.entity(k).try_despawn(); }
        }
        for p in level.parts.iter().filter(|p| keep.contains(&p.id)) {
            let (root, centre, turn) = roots[&p.id];
            let layers = p.painted();
            let look = (p.shape, p.size.map(f32::to_bits), layers.iter().map(|m| (m.to_string(), level.materials.get(*m).cloned())).collect::<Vec<_>>());
            let frame = Frame { centre: centre.extend(0.0), unturn: Vec4::from(turn.inverse()), info: IVec4::ZERO };
            match self.drawn.get_mut(&p.id) {
                Some(d) if d.root == root && d.look == look => {
                    let strokes_changed = d.strokes != p.strokes;
                    let moved = d.frame != (centre, turn);
                    if !strokes_changed && !moved { continue; }
                    let count = if strokes_changed {
                        let pieces = pieces(p);
                        let n = if p.strokes.is_empty() { 0 } else { pieces.len() };
                        self.buffers.insert(&d.buffer, upload(pieces)).ok();
                        d.strokes = p.strokes.clone();
                        Some(n)
                    } else { None };
                    d.frame = (centre, turn);
                    for h in &d.layers {
                        if let Some(mut m) = self.paints.get_mut(h) {
                            let info = IVec4::new(m.extension.frame.info.x, count.map_or(m.extension.frame.info.y, |n| n as i32), 0, 0);
                            m.extension.frame = Frame { info, ..frame };
                        }
                    }
                }
                _ => {
                    if let Some(d) = self.drawn.remove(&p.id) { for k in d.kids { self.commands.entity(k).try_despawn(); } }
                    let pieces = pieces(p);
                    let n = pieces.len() as i32;
                    let buffer = self.buffers.add(upload(pieces));
                    let faces: Vec<u8> = (0..crate::shapes::build(p.shape).faces.len() as u8).collect();
                    let mesh = self.mats.faces_mesh(&mut self.meshes, p.shape, p.size, &faces);
                    let (mut kids, mut handles) = (vec![], vec![]);
                    for (i, (_, m)) in look.2.iter().enumerate() {
                        let base = self.mats.painted(&self.server, &m.clone().unwrap_or_default(), TILE);
                        let h = self.paints.add(PaintMaterial { base, extension: PaintLayer { frame: Frame { info: IVec4::new(i as i32, n, 0, 0), ..frame }, pieces: buffer.clone() } });
                        kids.push(self.commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(h.clone()), Transform::default(), NotShadowCaster, Painted, ChildOf(root))).id());
                        handles.push(h);
                    }
                    self.drawn.insert(p.id, Drawn { root, look, strokes: p.strokes.clone(), frame: (centre, turn), kids, layers: handles, buffer });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::Stroke;

    #[test]
    fn later_strokes_cover_earlier_ones_and_erasing_uncovers() {
        let mut p = Part::placed(Shape::Plane, None, "P".into(), [0.0; 3], [0.0, 0.0, 0.0, 1.0], [10.0, 0.1, 10.0]);
        let s = |m: &str, x: f32, hard: f32| Stroke { material: m.into(), size: 2.0, hardness: hard, strength: 1.0, points: vec![[x - 1.0, 0.05, 0.0], [x, 0.05, 0.0], [x + 1.0, 0.05, 0.0]], lays: Default::default(), rules: Default::default(), curve: None, fill: false };
        p.strokes = vec![s("moss", 0.0, 1.0), s("sand", 3.0, 1.0), s("", -3.0, 1.0)];
        let pc = pieces(&p);
        let at = |x: f32, layer: i32| coverage(&pc, pc.len(), layer, Vec3::new(x, 0.05, 0.0), Vec3::new(x, 0.05, 0.0), 1.0, 0.0);
        assert_eq!(p.painted(), ["moss", "sand"]);
        // moss shows where only it went, sand covers it where sand went after, erasing took moss away on the left
        assert!((at(0.0, 0) - 1.0).abs() < 1e-4, "{}", at(0.0, 0));
        assert!(at(2.5, 0) < 1e-4 && (at(2.5, 1) - 1.0).abs() < 1e-4);
        assert!(at(1.5, 0) < 1e-4, "sand, painted after, covers the moss where they meet");
        assert!(at(-1.5, 0) < 1e-4);
        assert_eq!(at(0.0, 1), 0.0);
        // overlapping pieces of one stroke count once; a soft edge fades out
        p.strokes = vec![Stroke { strength: 0.5, ..s("moss", 0.0, 1.0) }];
        let pc = pieces(&p);
        assert!((coverage(&pc, pc.len(), 0, Vec3::new(0.0, 0.05, 0.0), Vec3::new(0.0, 0.05, 0.0), 1.0, 0.0) - 0.5).abs() < 1e-4);
        p.strokes = vec![s("moss", 0.0, 0.0)];
        let pc = pieces(&p);
        let mid = coverage(&pc, pc.len(), 0, Vec3::new(0.0, 0.05, 0.5), Vec3::new(0.0, 0.05, 0.5), 1.0, 0.0);
        assert!(mid > 0.2 && mid < 0.8, "{mid}");
    }

    #[test]
    fn subtract_takes_only_its_own_and_set_holds_its_strength() {
        use crate::level::{Lays, Rules};
        let mut p = Part::placed(Shape::Plane, None, "P".into(), [0.0; 3], [0.0, 0.0, 0.0, 1.0], [10.0, 0.1, 10.0]);
        let s = |m: &str, strength: f32, lays: Lays| Stroke { material: m.into(), size: 2.0, hardness: 1.0, strength, points: vec![[0.0, 0.05, 0.0]], lays, rules: Rules::default(), curve: None, fill: false };
        let at = |p: &Part, layer: i32, up: f32, y: f32| { let pc = pieces(p); coverage(&pc, pc.len(), layer, Vec3::new(0.0, 0.05, 0.0), Vec3::new(0.0, 0.05, 0.0), up, y) };
        // moss, then sand half over it, then half the moss taken away: the sand stays
        p.strokes = vec![s("moss", 1.0, Lays::Add), s("sand", 0.5, Lays::Add), s("moss", 0.5, Lays::Subtract)];
        assert!((at(&p, 0, 1.0, 0.0) - 0.25).abs() < 1e-4, "{}", at(&p, 0, 1.0, 0.0));
        assert!((at(&p, 1, 1.0, 0.0) - 0.5).abs() < 1e-4);
        // Set: exactly its strength, however often
        p.strokes = vec![s("moss", 0.3, Lays::Set), s("moss", 0.3, Lays::Set), s("moss", 0.3, Lays::Set)];
        assert!((at(&p, 0, 1.0, 0.0) - 0.3).abs() < 1e-4);
        p.strokes = vec![s("moss", 1.0, Lays::Add), s("moss", 0.3, Lays::Set)];
        assert!((at(&p, 0, 1.0, 0.0) - 0.3).abs() < 1e-4, "set brings it down too");
        // rules: tops only (slope up to 30 degrees) leaves walls bare; a height band leaves what is above it
        p.strokes = vec![Stroke { rules: Rules { slope: 30.0, band: Some([0.0, 2.0]), on: None }, ..s("moss", 1.0, Lays::Add) }];
        assert!((at(&p, 0, 1.0, 1.0) - 1.0).abs() < 1e-4);
        assert!(at(&p, 0, 0.0, 1.0) < 1e-4, "a wall");
        assert!(at(&p, 0, 1.0, 3.0) < 1e-4, "above the band");
    }
}
