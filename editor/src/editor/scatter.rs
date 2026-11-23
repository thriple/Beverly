//! Scatter: the items a pen curve scatters along or inside itself (`curve::Scatter`). They are ordinary parts, marked
//! with the curve, so they draw, collide and play like any part; they are made again whenever the curve, the scatter
//! or the ground changes, and the same every time for one seed. Ids are reused in order, so a part keeps its id (and
//! its selection) while the layout changes under it.
use super::pen::{curve_stamp, inside, surface, world_stamp};
use super::Editor;
use crate::curve::{Curve, Place, Scatter, Stretch, Vert};
use crate::level::Part;
use crate::world::Geometry;
use bevy::prelude::*;
use std::hash::{Hash, Hasher};

/// The most items one scatter makes.
pub const MOST: usize = 2000;

/// A small seeded random source (splitmix64): 0 to 1.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 40) as f32 / (1u64 << 24) as f32
    }
    fn between(&mut self, lo: f32, hi: f32) -> f32 { lo + (hi - lo) * self.next() }
}

/// What one item looks like: the part it is.
struct Made { name: String, shape: crate::level::Shape, pos: Vec3, rot: Quat, size: Vec3, colour: [f32; 3] }

/// The ground points the scatter puts items on: along the line in a band, or over the ground inside it.
fn spots(c: &Curve, sc: &Scatter, line: &[Vert], parts: &[Part], geo: &Geometry, rng: &mut Rng) -> Vec<Vec3> {
    let spacing = sc.spacing.max(0.1);
    let up = c.path.up();
    let mut out = vec![];
    match sc.place {
        Place::Along => {
            // walk the line, a step of about a spacing at a time, a place across the band for each
            let mut next = rng.between(0.0, spacing);
            let mut walked = 0.0;
            for w in line.windows(2) {
                let (a, b) = (w[0].at, w[1].at);
                let len = a.distance(b);
                if len < 1e-4 { continue; }
                let on_ground = !matches!(w[1].stretch, Stretch::Air | Stretch::Fall);
                let side = Vec3::new(-(b.z - a.z), 0.0, b.x - a.x).normalize_or_zero();
                while next <= walked + len && out.len() < MOST {
                    let p = a.lerp(b, (next - walked) / len);
                    next += spacing * rng.between(0.6, 1.4);
                    let at = p + side * rng.between(-0.5, 0.5) * sc.band;
                    if !on_ground { continue; }
                    if let Some(g) = surface(parts, geo, at, p, Stretch::Ground, up) { out.push(g); }
                }
                walked += len;
            }
        }
        Place::Inside => {
            let mut poly: Vec<Vec3> = line.iter().map(|v| v.at).collect();
            poly.dedup_by(|a, b| a.distance_squared(*b) < 1e-6);
            if poly.len() > 1 && poly[0].distance_squared(poly[poly.len() - 1]) < 1e-6 { poly.pop(); }
            if poly.len() < 3 { return out; }
            let (lo, hi) = poly.iter().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(lo, hi), p| (lo.min(Vec2::new(p.x, p.z)), hi.max(Vec2::new(p.x, p.z))));
            // a jittered grid; wider apart when the area would hold too many
            let step = spacing.max(((hi.x - lo.x) * (hi.y - lo.y) / MOST as f32).sqrt());
            let mut z = lo.y;
            while z < hi.y {
                let mut x = lo.x;
                while x < hi.x {
                    let (px, pz) = (x + rng.between(0.0, step), z + rng.between(0.0, step));
                    if inside(&poly, px, pz) {
                        let near = poly.iter().min_by(|a, b| (a.x - px).hypot(a.z - pz).total_cmp(&(b.x - px).hypot(b.z - pz))).map_or(0.0, |p| p.y);
                        if let Some(g) = surface(parts, geo, Vec3::new(px, near, pz), Vec3::new(px, near, pz), Stretch::Ground, up) { out.push(g); }
                    }
                    x += step;
                }
                z += step;
            }
        }
    }
    out
}

/// The items of a scatter, one for each spot.
fn made(c: &Curve, sc: &Scatter, line: &[Vert], parts: &[Part], geo: &Geometry) -> Vec<Made> {
    if sc.items.is_empty() { return vec![]; }
    let mut rng = Rng((sc.seed as u64).wrapping_mul(0x2545_F491_4F6C_DD1D) ^ c.id);
    let total: f32 = sc.items.iter().map(|i| i.weight.max(0.0)).sum();
    let mut out = vec![];
    for (n, ground) in spots(c, sc, line, parts, geo, &mut rng).into_iter().enumerate() {
        // which item, by share
        let mut pick = rng.next() * if total > 0.0 { total } else { sc.items.len() as f32 };
        let item = sc.items.iter().find(|i| { pick -= if total > 0.0 { i.weight.max(0.0) } else { 1.0 }; pick < 0.0 }).unwrap_or(&sc.items[0]);
        let scale = (1.0 + rng.between(-1.0, 1.0) * sc.size_var).max(0.2);
        let size = Vec3::from(item.size) * scale;
        let lean = Quat::from_euler(EulerRot::XYZ, rng.between(-1.0, 1.0) * sc.tilt.to_radians(), 0.0, rng.between(-1.0, 1.0) * sc.tilt.to_radians());
        let rot = Quat::from_rotation_y(rng.next() * std::f32::consts::TAU) * lean * if item.on_side { Quat::from_rotation_z(std::f32::consts::FRAC_PI_2) } else { Quat::IDENTITY };
        // its height above the ground when it sits on it, less how far it is sunk
        let tall = if item.on_side { size.x } else { size.y };
        let shade = rng.between(0.9, 1.1);
        out.push(Made {
            name: format!("{} {}", item.name, n + 1), shape: item.shape,
            pos: ground + Vec3::Y * (tall * 0.5 - tall * sc.sink), rot, size,
            colour: item.colour.map(|v| (v * shade).clamp(0.0, 1.0)),
        });
    }
    out
}

/// Make each curve's scattered items again where the curve, its scatter or the ground changed, and take away those of
/// a curve or scatter that is gone. `edit::commit` calls it as the change becomes an undo step.
pub fn scatters(ed: &mut Editor) {
    if ed.level.curves.is_empty() && !ed.level.parts.iter().any(|p| p.scatter.is_some()) { return; }
    let world = world_stamp(ed);
    let Editor { level, geo, pen, .. } = ed;
    let live: Vec<u64> = level.curves.iter().filter(|c| c.scatter.is_some()).map(|c| c.id).collect();
    pen.scattered.retain(|id, _| live.contains(id));
    let gone: Vec<u64> = level.parts.iter().filter(|p| p.scatter.is_some_and(|id| !live.contains(&id))).map(|p| p.id).collect();
    for id in gone { level.remove_part(id); }
    for c in level.curves.clone().iter().filter(|c| c.scatter.is_some()) {
        let (Some(sc), Some((_, line))) = (c.scatter.as_ref(), pen.lines.get(&c.id)) else { continue };
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (curve_stamp(c, world), sc.place as u8, [sc.spacing, sc.band, sc.size_var, sc.tilt, sc.sink, sc.seed].map(f32::to_bits)).hash(&mut h);
        for i in &sc.items { (&i.name, i.shape as u8, i.size.map(f32::to_bits), i.colour.map(f32::to_bits), i.weight.to_bits(), i.on_side).hash(&mut h); }
        let now = h.finish();
        if pen.scattered.get(&c.id) == Some(&now) { continue; }
        pen.scattered.insert(c.id, now);
        let want = made(c, sc, line, &level.parts, geo);
        let have: Vec<u64> = level.parts.iter().filter(|p| p.scatter == Some(c.id)).map(|p| p.id).collect();
        let same = have.len() == want.len() && have.iter().zip(&want).all(|(id, m)| level.get(*id).is_some_and(|p| matches(p, m)));
        if same { continue; }
        for (k, m) in want.iter().enumerate() {
            let mut p = Part::placed(m.shape, None, m.name.clone(), m.pos.to_array(), m.rot.to_array(), m.size.to_array());
            p.color = m.colour;
            p.scatter = Some(c.id);
            match have.get(k).and_then(|id| level.get_mut(*id)) {
                Some(old) => { p.id = old.id; p.tags = old.tags.clone(); if *old != p { *old = p; } }
                None => { level.add(p); }
            }
        }
        for id in have.iter().skip(want.len()) { level.remove_part(*id); }
    }
}

/// Whether a part is what an item would make.
fn matches(p: &Part, m: &Made) -> bool {
    p.name == m.name && p.shape == m.shape && p.pos == m.pos.to_array() && p.rot == m.rot.to_array() && p.size == m.size.to_array() && p.color == m.colour
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::PenPoint;
    use crate::level::Shape;

    fn floor() -> Vec<Part> {
        let mut p = Part::placed(Shape::Plane, None, "Floor".into(), [0.0, 0.05, 0.0], [0.0, 0.0, 0.0, 1.0], [40.0, 0.1, 40.0]);
        p.id = 1;
        vec![p]
    }

    fn curve(points: &[(f32, f32)], closed: bool, sc: Scatter) -> Curve {
        Curve { id: 9, name: "S".into(), points: points.iter().map(|(x, z)| PenPoint::corner(Vec3::new(*x, 0.1, *z))).collect(), closed, strip: None, fill: None, scatter: Some(sc), path: Default::default() }
    }

    #[test]
    fn along_a_line_they_stay_in_the_band_and_the_same_seed_gives_the_same_layout() {
        let sc = Scatter { place: Place::Along, spacing: 2.0, band: 4.0, ..Default::default() };
        let c = curve(&[(-10.0, 0.0), (10.0, 0.0)], false, sc.clone());
        let line = c.laid(|_, _, _| None);
        let (parts, geo) = (floor(), Geometry::default());
        let a = made(&c, &sc, &line, &parts, &geo);
        assert!(a.len() >= 8 && a.len() <= 14, "about one a spacing along 20 m: {}", a.len());
        assert!(a.iter().all(|m| m.pos.z.abs() <= 2.05 && m.pos.x.abs() <= 10.5), "inside the band");
        assert!(a.iter().all(|m| m.pos.y > 0.1 && m.pos.y < 1.5), "on the floor, not floating or buried: {:?}", a.iter().map(|m| m.pos.y).collect::<Vec<_>>());
        let b = made(&c, &sc, &line, &parts, &geo);
        assert!(a.iter().zip(&b).all(|(x, y)| x.pos == y.pos && x.size == y.size && x.name == y.name), "the same every time");
        let other = Scatter { seed: 2.0, ..sc.clone() };
        let d = made(&c, &other, &line, &parts, &geo);
        assert!(a.iter().zip(&d).any(|(x, y)| x.pos != y.pos), "another seed, another layout");
    }

    #[test]
    fn inside_an_outline_they_stay_inside_and_an_item_with_no_share_is_never_picked() {
        let mut sc = Scatter { place: Place::Inside, spacing: 2.0, ..Default::default() };
        sc.items[0].weight = 0.0;
        let c = curve(&[(-6.0, -6.0), (6.0, -6.0), (6.0, 6.0), (-6.0, 6.0)], true, sc.clone());
        let line = c.laid(|_, _, _| None);
        let a = made(&c, &sc, &line, &floor(), &Geometry::default());
        assert!(a.len() >= 20 && a.len() <= 50, "about a 12 m square over 2 m: {}", a.len());
        assert!(a.iter().all(|m| m.pos.x.abs() <= 6.0 && m.pos.z.abs() <= 6.0));
        assert!(a.iter().all(|m| !m.name.starts_with("Stump")), "the stump has no share");
        assert!(a.iter().any(|m| m.name.starts_with("Log")) && a.iter().any(|m| m.name.starts_with("Boulder")));
    }
}
