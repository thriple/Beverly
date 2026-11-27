//! Bucket fills (`level::Bucket`): which faces of which parts a fill reaches. A fill is the place clicked and a rule
//! (the same material, colour or surface tag, the part itself, or the floor at that height), so what it reaches is
//! worked out again whenever the level changes; each part it reaches gets a stroke covering those faces
//! (`Stroke::faces`), which the paint shader draws like any other stroke.
use crate::level::{Bucket, Fills, Level, Part, Tag};
use crate::world::part_affine;
use bevy::prelude::*;


/// The material laid on face `f` of `p`: the face's own, else the one laid all over it.
pub fn face_material(p: &Part, f: Option<u8>) -> Option<&str> {
    p.surfaces.iter().find(|s| s.face.is_some() && s.face == f).or_else(|| p.surfaces.iter().find(|s| s.face.is_none())).map(|s| s.material.as_str())
}

/// The colour face `f` of `p` shows: its material's tint, else the part's colour.
pub fn face_colour(level: &Level, p: &Part, f: Option<u8>) -> Vec3 {
    face_material(p, f).and_then(|m| level.materials.get(m)).map_or(Vec3::from(p.color), |m| Vec3::from(m.tint))
}

/// How far apart two colours are, 0 (the same) to 1 (black and white).
fn colour_gap(a: Vec3, b: Vec3) -> f32 { a.distance(b) / 3f32.sqrt() }

/// Each face of a part's shape in the world: its normal (how much it faces up) and its lowest and highest point.
fn faces_of(p: &Part) -> Vec<(f32, f32, f32)> {
    let tris = crate::shapes::build(p.shape);
    let (a, turn, size) = (part_affine(p), Quat::from_array(p.rot), Vec3::from(p.size));
    let mut out = vec![(f32::MIN, f32::MAX, f32::MIN); tris.faces.len()];
    for (t, &f) in tris.face.iter().enumerate() {
        let o = &mut out[f as usize];
        for k in t * 3..t * 3 + 3 {
            let y = a.transform_point3(tris.pos[k]).y;
            let up = (turn * (tris.nrm[k] / size.max(Vec3::splat(1e-4)))).normalize_or_zero().y;
            *o = (o.0.max(up), o.1.min(y), o.2.max(y));
        }
    }
    out
}

/// The box a part fills in the world.
fn bounds(p: &Part) -> (Vec3, Vec3) {
    let a = part_affine(p);
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    for k in 0..8 {
        let c = a.transform_point3(Vec3::new(if k & 1 == 0 { -0.5 } else { 0.5 }, if k & 2 == 0 { -0.5 } else { 0.5 }, if k & 4 == 0 { -0.5 } else { 0.5 }));
        (lo, hi) = (lo.min(c), hi.max(c));
    }
    (lo, hi)
}

/// Two parts' boxes touch: no further apart than `touching` (the project's, Connected only).
fn touch(a: (Vec3, Vec3), b: (Vec3, Vec3), touching: f32) -> bool { (a.0 - touching).cmple(b.1).all() && (b.0 - touching).cmple(a.1).all() }

/// The faces each part the fill reaches: (part id, faces), in the level's order. Only parts that can be painted.
/// Floor takes the walker profile's slope and step (a walkable face within a step of the height clicked is the same
/// floor); Connected only takes the project's touching.
pub fn reach(level: &Level, b: &Bucket) -> Vec<(u64, Vec<u8>)> {
    let Some(clicked) = level.get(b.part) else { return vec![] };
    let (walkable, floor, touching) = (level.profile().up(), level.profile().step, level.precision().touching());
    let at_y = clicked.to_world(Vec3::from(b.at)).y;
    let key_material = face_material(clicked, b.face).map(str::to_string);
    let key_colour = face_colour(level, clicked, b.face);
    let tag: Option<Tag> = b.tag.or_else(|| clicked.tags.first().copied());
    let mut found: Vec<(u64, Vec<u8>)> = vec![];
    for p in level.parts.iter().filter(|p| crate::paint::paintable(p)) {
        let n = crate::shapes::build(p.shape).faces.len() as u8;
        let all = || (0..n).collect::<Vec<u8>>();
        let faces: Vec<u8> = match b.fills {
            Fills::Part => if p.id == b.part { all() } else { vec![] },
            Fills::Tag => match tag {
                Some(t) if p.tags.contains(&t) => all(),
                // nothing tagged where it was clicked: only that part
                None if p.id == b.part => all(),
                _ => vec![],
            },
            Fills::Material if key_material.is_some() => (0..n).filter(|f| face_material(p, Some(*f)) == key_material.as_deref()).collect(),
            // Material with none laid where clicked: faces with none laid, of the same colour
            Fills::Material => (0..n).filter(|f| face_material(p, Some(*f)).is_none() && colour_gap(face_colour(level, p, Some(*f)), key_colour) <= 0.02).collect(),
            Fills::Colour => (0..n).filter(|f| colour_gap(face_colour(level, p, Some(*f)), key_colour) <= b.tolerance.max(0.02)).collect(),
            Fills::Floor => {
                let ground = p.tags.iter().any(|t| matches!(t, Tag::Ground | Tag::Water));
                faces_of(p).iter().enumerate().filter(|(_, (up, lo, hi))| (*up >= walkable || (ground && *up > 0.0)) && at_y >= lo - floor && at_y <= hi + floor).map(|(f, _)| f as u8).collect()
            }
        };
        if !faces.is_empty() { found.push((p.id, faces)); }
    }
    if b.connected && b.fills != Fills::Part {
        // part to part from the clicked one, through parts that touch
        let boxes: Vec<(u64, (Vec3, Vec3))> = found.iter().filter_map(|(id, _)| Some((*id, bounds(level.get(*id)?)))).collect();
        let mut keep = vec![b.part];
        let mut grew = true;
        while grew {
            grew = false;
            for (id, bx) in &boxes {
                if keep.contains(id) { continue; }
                if boxes.iter().any(|(k, kb)| keep.contains(k) && touch(*bx, *kb, touching)) { keep.push(*id); grew = true; }
            }
        }
        found.retain(|(id, _)| keep.contains(id));
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{Shape, Surface};

    fn part(id: u64, shape: Shape, pos: [f32; 3], size: [f32; 3], color: [f32; 3]) -> Part {
        let mut p = Part::placed(shape, None, format!("P{id}"), pos, [0.0, 0.0, 0.0, 1.0], size);
        (p.id, p.color) = (id, color);
        p
    }

    fn bucket(part: u64, at: [f32; 3], fills: Fills) -> Bucket {
        Bucket { id: 99, part, face: None, at, fills, tolerance: 0.0, tag: None, connected: false, material: "moss".into(), strength: 1.0, lays: Default::default() }
    }

    fn level(parts: Vec<Part>) -> Level {
        let mut l = Level::default();
        *l.parts = parts;
        l
    }

    #[test]
    fn colour_fills_every_face_close_enough_and_part_fills_one() {
        let grey = [0.5, 0.5, 0.5];
        let blue = [0.1, 0.2, 0.9];
        let l = level(vec![part(1, Shape::Box, [0.0; 3], [1.0; 3], blue), part(2, Shape::Box, [5.0, 0.0, 0.0], [1.0; 3], [0.15, 0.2, 0.85]), part(3, Shape::Box, [9.0, 0.0, 0.0], [1.0; 3], grey)]);
        let mut b = bucket(1, [0.0, 0.5, 0.0], Fills::Colour);
        b.tolerance = 0.05;
        let r = reach(&l, &b);
        assert_eq!(r.iter().map(|x| x.0).collect::<Vec<_>>(), vec![1, 2], "the nearly-blue box too, not the grey one");
        assert_eq!(r[0].1.len(), 6, "every face of a box");
        b.tolerance = 0.0;
        assert_eq!(reach(&l, &b).len(), 1, "exactly blue: only the box clicked");
        b.fills = Fills::Part;
        assert_eq!(reach(&l, &b), vec![(1, (0..6).collect())]);
    }

    #[test]
    fn material_fills_the_faces_it_is_laid_on_and_follows_when_it_changes() {
        let mut a = part(1, Shape::Box, [0.0; 3], [1.0; 3], [0.5; 3]);
        let mut b2 = part(2, Shape::Box, [3.0, 0.0, 0.0], [1.0; 3], [0.5; 3]);
        a.surfaces = vec![Surface { face: None, material: "brick".into(), tile: [1.0; 2], offset: [0.0; 2] }];
        b2.surfaces = vec![Surface { face: Some(2), material: "brick".into(), tile: [1.0; 2], offset: [0.0; 2] }];
        let mut l = level(vec![a, b2]);
        let mut b = bucket(1, [0.0, 0.5, 0.0], Fills::Material);
        b.face = Some(0);
        let r = reach(&l, &b);
        assert_eq!(r, vec![(1, (0..6).collect()), (2, vec![2])], "all of the brick box, one face of the other");
        // lay stone on the clicked box instead: the fill now finds stone, which nothing else has
        l.parts[0].surfaces[0].material = "stone".into();
        assert_eq!(reach(&l, &b), vec![(1, (0..6).collect())]);
    }

    #[test]
    fn floor_takes_the_walkable_faces_at_that_height_and_connected_stops_at_gaps() {
        // two floors at the same height side by side, a third far away at that height, and one a storey up
        let grey = [0.5; 3];
        let l = level(vec![
            part(1, Shape::Plane, [0.0, 0.05, 0.0], [4.0, 0.1, 4.0], grey),
            part(2, Shape::Plane, [4.0, 0.05, 0.0], [4.0, 0.1, 4.0], grey),
            part(3, Shape::Plane, [20.0, 0.05, 0.0], [4.0, 0.1, 4.0], grey),
            part(4, Shape::Plane, [0.0, 3.05, 0.0], [4.0, 0.1, 4.0], grey),
        ]);
        let mut b = bucket(1, [0.0, 0.05, 0.0], Fills::Floor);
        let r = reach(&l, &b);
        assert_eq!(r.iter().map(|x| x.0).collect::<Vec<_>>(), vec![1, 2, 3], "not the storey above");
        assert!(r.iter().all(|(_, f)| f.len() == 1), "only the top of each: {r:?}");
        b.connected = true;
        assert_eq!(reach(&l, &b).iter().map(|x| x.0).collect::<Vec<_>>(), vec![1, 2], "the far one is across a gap");
        // colour, connected: the grey storey above doesn't touch
        b.fills = Fills::Colour;
        assert_eq!(reach(&l, &b).iter().map(|x| x.0).collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn floor_reads_the_walker_profile_and_connected_reads_the_precision() {
        // a platform 0.42 m up, and a ramp face 50° steep (the same height as the floor clicked)
        let grey = [0.5; 3];
        let ramp = part(3, Shape::Ramp, [-6.0, 0.0, 0.0], [2.0, 2.4, 2.0], grey);
        let mut l = level(vec![part(1, Shape::Plane, [0.0, 0.05, 0.0], [4.0, 0.1, 4.0], grey), part(2, Shape::Box, [6.0, 0.27, 0.0], [2.0, 0.3, 2.0], grey), ramp]);
        let b = bucket(1, [0.0, 0.05, 0.0], Fills::Floor);
        let ids = |l: &Level| reach(l, &b).iter().map(|x| x.0).collect::<Vec<_>>();
        assert_eq!(ids(&l), vec![1, 2], "the platform is within the profile's step (0.45 m); the ramp is steeper than its slope (45°)");
        l.navigation.step = 0.3;
        l.navigation.slope = 60.0;
        assert_eq!(ids(&l), vec![1, 3], "a lower step leaves the platform out; a steeper slope takes the ramp in");
        // Connected only: touching is 50 x the precision
        let mut l = level(vec![part(1, Shape::Plane, [0.0, 0.05, 0.0], [4.0, 0.1, 4.0], grey), part(2, Shape::Plane, [4.04, 0.05, 0.0], [4.0, 0.1, 4.0], grey)]);
        let b = Bucket { connected: true, ..bucket(1, [0.0, 0.05, 0.0], Fills::Floor) };
        assert_eq!(reach(&l, &b).len(), 2, "4 cm apart touch at 1 mm");
        *l.precision = crate::level::Precision(0.0005);
        assert_eq!(reach(&l, &b).len(), 1, "but not at 0.5 mm (touching 2.5 cm)");
    }

    #[test]
    fn tag_fills_every_part_with_it() {
        let mut a = part(1, Shape::Box, [0.0; 3], [1.0; 3], [0.5; 3]);
        let mut c = part(3, Shape::Box, [8.0, 0.0, 0.0], [1.0; 3], [0.2; 3]);
        a.tags = vec![Tag::Slippery];
        c.tags = vec![Tag::Climbable, Tag::Slippery];
        let l = level(vec![a, part(2, Shape::Box, [4.0, 0.0, 0.0], [1.0; 3], [0.5; 3]), c]);
        assert_eq!(reach(&l, &bucket(1, [0.0; 3], Fills::Tag)).iter().map(|x| x.0).collect::<Vec<_>>(), vec![1, 3]);
    }
}
