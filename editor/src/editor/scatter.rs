//! Scatter: the items a pen curve scatters along or inside itself (`curve::Scatter`). They are ordinary parts, marked
//! with the curve, so they draw, collide and play like any part; they are made again whenever the curve, the scatter
//! or the ground changes, and the same every time for one seed. Ids are reused in order, so a part keeps its id (and
//! its selection) while the layout changes under it. The level file keeps only the curves and their settings (`grow`
//! makes the items when a level opens, in the editor and in the game).
use super::pen::{curve_stamp, inside, surface, surface_hit, world_stamp};
use super::Editor;
use crate::curve::{Curve, Place, Scatter, Stretch, Vert};
use crate::level::{Level, Mover, Part, Tag};
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
struct Made { name: String, shape: crate::level::Shape, pos: Vec3, rot: Quat, size: Vec3, colour: [f32; 3], mover: Option<Mover> }

/// How far a point is inside a polygon's outline, seen from above (the nearest edge).
fn depth(poly: &[Vec3], x: f32, z: f32) -> f32 {
    let p = Vec2::new(x, z);
    (0..poly.len()).map(|i| {
        let (a, b) = (Vec2::new(poly[i].x, poly[i].z), Vec2::new(poly[(i + 1) % poly.len()].x, poly[(i + 1) % poly.len()].z));
        let ab = b - a;
        p.distance(a + ab * ((p - a).dot(ab) / ab.dot(ab).max(1e-12)).clamp(0.0, 1.0))
    }).fold(f32::MAX, f32::min)
}

/// How far a point is from a line, seen from above.
fn off_line(line: &[Vec3], p: Vec3) -> f32 {
    let q = Vec2::new(p.x, p.z);
    line.windows(2).map(|w| {
        let (a, b) = (Vec2::new(w[0].x, w[0].z), Vec2::new(w[1].x, w[1].z));
        let ab = b - a;
        q.distance(a + ab * ((q - a).dot(ab) / ab.dot(ab).max(1e-12)).clamp(0.0, 1.0))
    }).fold(f32::MAX, f32::min)
}

/// How far a point is from the footprint of a part, seen from above (0 over it).
fn off_part(part: &Part, at: Vec3) -> f32 {
    let (l, h) = (part.to_local(at), Vec3::from(part.size) * 0.5);
    Vec2::new((l.x.abs() - h.x).max(0.0), (l.z.abs() - h.z).max(0.0)).length()
}

/// The paths a scatter keeps clear of: the lines of the other curves that lay a strip (a path, a road, a stream).
fn paths_of(level: &Level, c: &Curve, lines: impl Fn(&Curve) -> Option<Vec<Vec3>>) -> Vec<Vec<Vec3>> {
    if c.scatter.as_ref().is_none_or(|s| s.clear <= 0.0) { return vec![]; }
    level.curves.iter().filter(|o| o.id != c.id && o.strip.is_some()).filter_map(lines).collect()
}

/// The points the scatter puts items at: along the line in a band, over the ground inside it, or through the space above it.
fn spots(c: &Curve, sc: &Scatter, line: &[Vert], parts: &[Part], geo: &Geometry, paths: &[Vec<Vec3>], rng: &mut Rng) -> Vec<Vec3> {
    let spacing = sc.spacing.max(0.1);
    let up = c.path.up();
    // the ground at a spot, if the rules let an item land there: steep enough, high enough, on the right part, clear of paths and water
    let land = |at: Vec3, from: Vec3| -> Option<Vec3> {
        let h = surface_hit(parts, geo, at, from, Stretch::Ground, up)?;
        let on = |t: Tag| h.part.and_then(|id| parts.iter().find(|q| q.id == id)).is_some_and(|q| q.tags.contains(&t));
        let clear = sc.clear <= 0.0 || (paths.iter().all(|l| off_line(l, h.point) >= sc.clear) && parts.iter().filter(|q| q.tags.contains(&Tag::Water)).all(|q| off_part(q, h.point) >= sc.clear));
        (sc.rules.on.is_none_or(on) && sc.rules.allows(h.normal.y, h.point.y) >= 0.5 && clear).then_some(h.point)
    };
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
                    if let Some(g) = land(at, p) { out.push(g); }
                }
                walked += len;
            }
        }
        Place::Inside | Place::Volume => {
            let mut poly: Vec<Vec3> = line.iter().map(|v| v.at).collect();
            poly.dedup_by(|a, b| a.distance_squared(*b) < 1e-6);
            if poly.len() > 1 && poly[0].distance_squared(poly[poly.len() - 1]) < 1e-6 { poly.pop(); }
            if poly.len() < 3 { return out; }
            let (lo, hi) = poly.iter().fold((Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)), |(lo, hi), p| (lo.min(Vec2::new(p.x, p.z)), hi.max(Vec2::new(p.x, p.z))));
            let volume = sc.place == Place::Volume;
            let (floor, high) = (poly.iter().map(|p| p.y).fold(f32::MAX, f32::min), sc.height.max(0.5));
            // a jittered grid (of cubes in a volume); wider apart when the space would hold too many
            let space = (hi.x - lo.x) * (hi.y - lo.y) * if volume { high } else { 1.0 };
            let step = spacing.max((space / MOST as f32).powf(if volume { 1.0 / 3.0 } else { 0.5 }));
            // thinning toward the edge: it takes this far in to be as thick as the middle
            let fade = sc.falloff.clamp(0.0, 1.0) * 0.5 * (hi.x - lo.x).min(hi.y - lo.y).min(if volume { high } else { f32::MAX });
            let mut z = lo.y;
            while z < hi.y {
                let mut x = lo.x;
                while x < hi.x {
                    let mut y = 0.0;
                    while y < if volume { high } else { 1e-3 } {
                        let (px, pz) = (x + rng.between(0.0, step), z + rng.between(0.0, step));
                        let py = y + if volume { rng.between(0.0, step) } else { 0.0 };
                        let walls = depth(&poly, px, pz).min(if volume { py.min(high - py) } else { f32::MAX });
                        let kept = fade <= 0.0 || { let t = (walls / fade).clamp(0.0, 1.0); rng.next() < t * t * (3.0 - 2.0 * t) };
                        if kept && py < high && inside(&poly, px, pz) {
                            if volume { if sc.rules.allows(1.0, floor + py) >= 0.5 { out.push(Vec3::new(px, floor + py, pz)); } }
                            else {
                                let near = poly.iter().min_by(|a, b| (a.x - px).hypot(a.z - pz).total_cmp(&(b.x - px).hypot(b.z - pz))).map_or(0.0, |p| p.y);
                                if let Some(g) = land(Vec3::new(px, near, pz), Vec3::new(px, near, pz)) { out.push(g); }
                            }
                        }
                        y += step;
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
fn made(c: &Curve, sc: &Scatter, line: &[Vert], parts: &[Part], geo: &Geometry, paths: &[Vec<Vec3>]) -> Vec<Made> {
    if sc.items.is_empty() { return vec![]; }
    let mut rng = Rng((sc.seed as u64).wrapping_mul(0x2545_F491_4F6C_DD1D) ^ c.id);
    let total: f32 = sc.items.iter().map(|i| i.weight.max(0.0)).sum();
    let mut out = vec![];
    for (n, ground) in spots(c, sc, line, parts, geo, paths, &mut rng).into_iter().enumerate() {
        // which item, by share
        let mut pick = rng.next() * if total > 0.0 { total } else { sc.items.len() as f32 };
        let item = sc.items.iter().find(|i| { pick -= if total > 0.0 { i.weight.max(0.0) } else { 1.0 }; pick < 0.0 }).unwrap_or(&sc.items[0]);
        let scale = (1.0 + rng.between(-1.0, 1.0) * sc.size_var).max(0.2);
        let size = Vec3::from(item.size) * scale;
        let volume = sc.place == Place::Volume;
        let lean = Quat::from_euler(EulerRot::XYZ, rng.between(-1.0, 1.0) * sc.tilt.to_radians(), 0.0, rng.between(-1.0, 1.0) * sc.tilt.to_radians());
        let rot = if volume {
            // nothing is upright in the air: any way round
            Quat::from_euler(EulerRot::XYZ, rng.next() * std::f32::consts::TAU, rng.next() * std::f32::consts::TAU, rng.next() * std::f32::consts::TAU)
        } else {
            Quat::from_rotation_y(rng.next() * std::f32::consts::TAU) * lean * if item.on_side { Quat::from_rotation_z(std::f32::consts::FRAC_PI_2) } else { Quat::IDENTITY }
        };
        // its height above the ground when it sits on it, less how far it is sunk (in a volume the spot is its middle)
        let tall = if volume { 0.0 } else if item.on_side { size.x } else { size.y };
        let shade = rng.between(0.9, 1.1);
        // slowly turning and drifting, each its own way (the random numbers come last, so a scatter that doesn't move lays out as it did)
        let mover = (sc.spin > 0.0 || sc.drift > 0.0).then(|| {
            let mut way = || Vec3::new(rng.between(-1.0, 1.0), rng.between(-1.0, 1.0), rng.between(-1.0, 1.0));
            let (spin, drift) = (way() * sc.spin, way() * sc.drift);
            Mover { to: [0.0; 3], spin: spin.to_array(), drift: drift.to_array(), ..Default::default() }
        });
        out.push(Made {
            name: format!("{} {}", item.name, n + 1), shape: item.shape,
            pos: ground + Vec3::Y * (tall * 0.5 - tall * sc.sink), rot, size,
            colour: item.colour.map(|v| (v * shade).clamp(0.0, 1.0)), mover,
        });
    }
    out
}

/// The part an item is.
fn part(m: &Made, curve: u64) -> Part {
    let mut p = Part::placed(m.shape, None, m.name.clone(), m.pos.to_array(), m.rot.to_array(), m.size.to_array());
    p.color = m.colour;
    p.scatter = Some(curve);
    p.mover = m.mover.clone();
    p
}

/// Make the items of every scattering curve of a level that was just opened (the file keeps none; one that has some, from
/// before, loses them for the same ones). The editor and the game both call it, so the layout is the same in both.
pub fn grow(level: &mut Level, geo: &Geometry) {
    level.parts.retain(|p| p.scatter.is_none());
    for c in level.curves.clone().iter() {
        let Some(sc) = c.scatter.as_ref() else { continue };
        let lay = |o: &Curve| Some(o.laid(|p, from, s| surface(&level.parts, geo, p, from, s, o.path.up())).iter().map(|v| v.at).collect::<Vec<_>>());
        let line = c.laid(|p, from, s| surface(&level.parts, geo, p, from, s, c.path.up()));
        for m in made(c, sc, &line, &level.parts, geo, &paths_of(level, c, lay)) { level.add(part(&m, c.id)); }
    }
}

/// Turn a curve's scattered items into ordinary parts: the curve stops scattering and they stay where they are, free to
/// move, change or delete one by one. Returns how many.
pub fn turn_into_items(ed: &mut Editor, curve: u64) -> usize {
    let Some(c) = ed.level.curve_mut(curve) else { return 0 };
    if c.scatter.take().is_none() { return 0; }
    ed.pen.scattered.remove(&curve);
    let mut n = 0;
    for p in ed.level.parts.iter_mut().filter(|p| p.scatter == Some(curve)) { p.scatter = None; n += 1; }
    n
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
        let paths = paths_of(level, c, |o| pen.lines.get(&o.id).map(|(_, l)| l.iter().map(|v| v.at).collect()));
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (curve_stamp(c, world), sc.place as u8, [sc.spacing, sc.band, sc.size_var, sc.tilt, sc.sink, sc.height, sc.falloff, sc.spin, sc.drift, sc.clear, sc.seed].map(f32::to_bits)).hash(&mut h);
        (sc.rules.slope.to_bits(), sc.rules.band.map(|b| b.map(f32::to_bits)), sc.rules.on.map(|t| t as u8)).hash(&mut h);
        for l in &paths { for v in l { v.to_array().map(f32::to_bits).hash(&mut h); } }
        for i in &sc.items { (&i.name, i.shape as u8, i.size.map(f32::to_bits), i.colour.map(f32::to_bits), i.weight.to_bits(), i.on_side).hash(&mut h); }
        let now = h.finish();
        if pen.scattered.get(&c.id) == Some(&now) { continue; }
        pen.scattered.insert(c.id, now);
        let want = made(c, sc, line, &level.parts, geo, &paths);
        let have: Vec<u64> = level.parts.iter().filter(|p| p.scatter == Some(c.id)).map(|p| p.id).collect();
        let same = have.len() == want.len() && have.iter().zip(&want).all(|(id, m)| level.get(*id).is_some_and(|p| matches(p, m)));
        if same { continue; }
        for (k, m) in want.iter().enumerate() {
            let mut p = part(m, c.id);
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
    p.name == m.name && p.shape == m.shape && p.pos == m.pos.to_array() && p.rot == m.rot.to_array() && p.size == m.size.to_array() && p.color == m.colour && p.mover == m.mover
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
        let a = made(&c, &sc, &line, &parts, &geo, &[]);
        assert!(a.len() >= 8 && a.len() <= 14, "about one a spacing along 20 m: {}", a.len());
        assert!(a.iter().all(|m| m.pos.z.abs() <= 2.05 && m.pos.x.abs() <= 10.5), "inside the band");
        assert!(a.iter().all(|m| m.pos.y > 0.1 && m.pos.y < 1.5), "on the floor, not floating or buried: {:?}", a.iter().map(|m| m.pos.y).collect::<Vec<_>>());
        let b = made(&c, &sc, &line, &parts, &geo, &[]);
        assert!(a.iter().zip(&b).all(|(x, y)| x.pos == y.pos && x.size == y.size && x.name == y.name), "the same every time");
        let other = Scatter { seed: 2.0, ..sc.clone() };
        let d = made(&c, &other, &line, &parts, &geo, &[]);
        assert!(a.iter().zip(&d).any(|(x, y)| x.pos != y.pos), "another seed, another layout");
    }

    #[test]
    fn inside_an_outline_they_stay_inside_and_an_item_with_no_share_is_never_picked() {
        let mut sc = Scatter { place: Place::Inside, spacing: 2.0, ..Default::default() };
        sc.items[0].weight = 0.0;
        let c = curve(&[(-6.0, -6.0), (6.0, -6.0), (6.0, 6.0), (-6.0, 6.0)], true, sc.clone());
        let line = c.laid(|_, _, _| None);
        let a = made(&c, &sc, &line, &floor(), &Geometry::default(), &[]);
        assert!(a.len() >= 20 && a.len() <= 50, "about a 12 m square over 2 m: {}", a.len());
        assert!(a.iter().all(|m| m.pos.x.abs() <= 6.0 && m.pos.z.abs() <= 6.0));
        assert!(a.iter().all(|m| !m.name.starts_with("Stump")), "the stump has no share");
        assert!(a.iter().any(|m| m.name.starts_with("Log")) && a.iter().any(|m| m.name.starts_with("Boulder")));
    }

    #[test]
    fn the_file_keeps_the_curve_and_the_items_are_made_again_the_same_when_it_opens() {
        let sc = Scatter { place: Place::Along, spacing: 2.0, band: 4.0, ..Default::default() };
        let mut level = Level { parts: floor().into(), next_id: 1, ..Default::default() };
        level.curves.push(curve(&[(-10.0, 0.0), (10.0, 0.0)], false, sc));
        let geo = Geometry::default();
        grow(&mut level, &geo);
        let items = |l: &Level| l.parts.iter().filter(|p| p.scatter.is_some()).map(|p| (p.name.clone(), p.pos, p.size)).collect::<Vec<_>>();
        let first = items(&level);
        assert!(first.len() >= 8, "items were made: {}", first.len());
        let file = level.to_file();
        assert!(file.parts.iter().all(|p| p.scatter.is_none()), "the file has none of them");
        assert_eq!(file.curves.len(), 1);
        let mut opened = file;
        grow(&mut opened, &geo);
        assert_eq!(items(&opened), first, "the same layout when it opens");
        // growing a level that already has them (an older file) doesn't double them
        grow(&mut opened, &geo);
        assert_eq!(items(&opened), first);
    }

    #[test]
    fn a_volume_fills_the_space_above_its_outline_thinner_at_the_edge_and_its_items_spin_and_drift() {
        let square = [(-8.0, -8.0), (8.0, -8.0), (8.0, 8.0), (-8.0, 8.0)];
        let sc = Scatter { place: Place::Volume, spacing: 2.0, height: 8.0, spin: 30.0, drift: 2.0, ..Default::default() };
        let c = curve(&square, true, sc.clone());
        let line = c.laid(|_, _, _| None);
        let a = made(&c, &sc, &line, &[], &Geometry::default(), &[]);
        assert!(a.len() >= 200 && a.len() <= 400, "a 16 x 16 x 8 m space over 2 m cubes: {}", a.len());
        assert!(a.iter().all(|m| m.pos.x.abs() <= 8.0 && m.pos.z.abs() <= 8.0 && m.pos.y >= 0.1 && m.pos.y <= 8.1), "inside the space");
        assert!(a.iter().any(|m| m.pos.y > 6.0) && a.iter().any(|m| m.pos.y < 2.0), "all the way up");
        let m = a[0].mover.as_ref().expect("it moves");
        assert!(m.to == [0.0; 3] && m.spin.iter().any(|v| *v != 0.0) && m.drift.iter().any(|v| *v != 0.0));
        assert!(a.iter().all(|m| m.mover.as_ref().is_some_and(|v| v.spin.iter().all(|s| s.abs() <= 30.0) && v.drift.iter().all(|d| d.abs() <= 2.0))));
        // falloff takes the ones at the edge: the middle keeps its thickness
        let soft = Scatter { falloff: 1.0, ..sc.clone() };
        let b = made(&c, &soft, &line, &[], &Geometry::default(), &[]);
        let edge = |v: &[Made]| v.iter().filter(|m| m.pos.x.abs() > 6.0 || m.pos.z.abs() > 6.0 || m.pos.y > 6.0 || m.pos.y < 2.0).count() as f32 / v.len() as f32;
        assert!(b.len() < a.len() && edge(&b) < edge(&a) * 0.7, "thinner at the edge: {} of {} vs {} of {}", edge(&b), b.len(), edge(&a), a.len());
        // one that doesn't move has no mover, and the ground layouts are as before
        let still = Scatter { spin: 0.0, drift: 0.0, ..sc };
        assert!(made(&c, &still, &line, &[], &Geometry::default(), &[]).iter().all(|m| m.mover.is_none()));
    }

    #[test]
    fn rules_keep_items_off_steep_or_low_or_untagged_ground_and_clear_of_paths_and_water() {
        let sc = Scatter { place: Place::Along, spacing: 1.0, band: 6.0, ..Default::default() };
        let c = curve(&[(-10.0, 0.0), (10.0, 0.0)], false, sc.clone());
        let line = c.laid(|_, _, _| None);
        let (parts, geo) = (floor(), Geometry::default());
        let count = |sc: &Scatter, parts: &[Part], paths: &[Vec<Vec3>]| made(&c, sc, &line, parts, &geo, paths).len();
        let all = count(&sc, &parts, &[]);
        assert!(all > 10);
        // the height band: the floor's top is at 0.1 m
        let high = Scatter { rules: crate::level::Rules { band: Some([5.0, 9.0]), ..Default::default() }, ..sc.clone() };
        assert_eq!(count(&high, &parts, &[]), 0, "nothing that high");
        let low = Scatter { rules: crate::level::Rules { band: Some([-1.0, 1.0]), ..Default::default() }, ..sc.clone() };
        assert_eq!(count(&low, &parts, &[]), all, "all of it inside the band");
        // only on a tagged part
        let tagged = Scatter { rules: crate::level::Rules { on: Some(Tag::Diggable), ..Default::default() }, ..sc.clone() };
        assert_eq!(count(&tagged, &parts, &[]), 0, "the floor isn't diggable");
        let mut dig = parts.clone();
        dig[0].tags.push(Tag::Diggable);
        assert_eq!(count(&tagged, &dig, &[]), all, "now it is");
        // keep clear of a path along the line itself, and of water
        let clear = Scatter { clear: 2.0, ..sc.clone() };
        let path = vec![vec![Vec3::new(-10.0, 0.1, 0.0), Vec3::new(10.0, 0.1, 0.0)]];
        let off = made(&c, &clear, &line, &parts, &geo, &path);
        assert!(!off.is_empty() && off.len() < all && off.iter().all(|m| m.pos.z.abs() >= 2.0 - 1e-3), "none within 2 m of the path: {:?}", off.iter().map(|m| m.pos.z).fold(9.0f32, |a, b| a.min(b.abs())));
        let mut wet = parts.clone();
        let mut pond = Part::placed(Shape::Box, None, "Pond".into(), [5.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], [4.0, 0.1, 4.0]);
        pond.id = 2;
        pond.tags.push(Tag::Water);
        wet.push(pond);
        let near_pond = made(&c, &clear, &line, &wet, &geo, &[]);
        assert!(!near_pond.is_empty() && near_pond.iter().all(|m| (m.pos.x - 5.0).abs() >= 2.0 || m.pos.z.abs() >= 2.0 + 2.0 - 1e-3 || { let (dx, dz) = ((m.pos.x - 5.0).abs() - 2.0, m.pos.z.abs() - 2.0); dx.max(0.0).hypot(dz.max(0.0)) >= 2.0 - 1e-3 }));
    }

    #[test]
    fn turning_a_scatter_into_items_leaves_ordinary_parts_and_no_scatter() {
        let mut ed = Editor::at(std::env::temp_dir());
        ed.level.parts = floor().into();
        let id = ed.level.add_curve(vec![PenPoint::corner(Vec3::new(-10.0, 0.1, 0.0)), PenPoint::corner(Vec3::new(10.0, 0.1, 0.0))], false);
        ed.level.curve_mut(id).unwrap().scatter = Some(Scatter { spacing: 2.0, ..Default::default() });
        super::super::pen::lay(&mut ed);
        scatters(&mut ed);
        let made = ed.level.parts.iter().filter(|p| p.scatter == Some(id)).count();
        assert!(made >= 8, "{made}");
        assert_eq!(turn_into_items(&mut ed, id), made);
        assert!(ed.level.curve(id).unwrap().scatter.is_none() && ed.level.parts.iter().all(|p| p.scatter.is_none()));
        scatters(&mut ed);
        assert_eq!(ed.level.parts.len(), made + 1, "they stay, and nothing is made again");
        assert_eq!(turn_into_items(&mut ed, id), 0);
    }
}
