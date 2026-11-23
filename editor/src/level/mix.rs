//! Mixes: a material that grows plants and lays a ground under them (`Material::ground`, with its spread, soft edge
//! and thickness). A stroke of a Mix paints the ground, wider and softer than its plants, as the surface; the plants
//! come from the stroke itself (`grass/sheets.rs`). The ground is worked out from the strokes whenever they are drawn,
//! never stored, so changing the Mix's ground changes every stroke that used it. A stroke can choose another ground
//! (`Stroke::ground`), or none (empty).
use super::*;
use std::borrow::Cow;

/// The stroke that lays `s`'s ground, if it has one: the stroke itself, with the ground's material, spread, softened and,
/// when the Mix says so, as thin as its plants.
fn ground_stroke(level: &Level, s: &Stroke) -> Option<Stroke> {
    let m = level.materials.get(&s.material)?;
    let g = match &s.ground { Some(g) => g.as_str(), None => m.ground.as_deref()? };
    if !level.materials.contains_key(g) { return None; }
    Some(Stroke {
        material: g.into(),
        size: s.size * s.ground_look.and_then(|l| l.width).unwrap_or(m.spread).max(1.0),
        hardness: s.hardness * (1.0 - m.soft.clamp(0.0, 1.0)),
        strength: s.strength * s.ground_look.map_or(1.0, |l| l.strength.clamp(0.0, 1.0)) * if m.follows { m.grass_density.clamp(0.0, 1.0) } else { 1.0 },
        ground: None,
        ground_look: None,
        ..s.clone()
    })
}

/// The part as it looks: each Mix stroke is its ground. The same part when nothing on it is a Mix.
pub fn shown<'a>(level: &Level, p: &'a Part) -> Cow<'a, Part> {
    if !p.strokes.iter().any(|s| ground_stroke(level, s).is_some()) { return Cow::Borrowed(p); }
    Cow::Owned(Part { strokes: p.strokes.iter().map(|s| ground_stroke(level, s).unwrap_or_else(|| s.clone())).collect(), ..p.clone() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(m: &str) -> Stroke {
        Stroke { material: m.into(), size: 2.0, hardness: 0.8, strength: 1.0, points: vec![[0.0, 0.5, 0.0]], lays: Default::default(), rules: Default::default(), curve: None, fill: false, faces: vec![], bucket: None, ground: None, ground_look: None }
    }

    #[test]
    fn a_mixs_stroke_is_its_ground_wider_and_softer() {
        let mut level = Level::default();
        level.materials.insert("soil".into(), Material::default());
        level.materials.insert("mud".into(), Material::default());
        level.materials.insert("meadow".into(), Material { ground: Some("soil".into()), spread: 2.0, soft: 0.5, ..Default::default() });
        let mut p = Part::placed(Shape::Box, None, "P".into(), [0.0; 3], [0.0, 0.0, 0.0, 1.0], [4.0; 3]);
        p.strokes = vec![stroke("meadow"), stroke("soil"), stroke("")];
        let s = shown(&level, &p);
        assert_eq!((s.strokes[0].material.as_str(), s.strokes[0].size, s.strokes[0].hardness), ("soil", 4.0, 0.4));
        assert_eq!(s.strokes[1], p.strokes[1], "a plain material is itself");
        assert_eq!(s.strokes[2], p.strokes[2], "erasing too");
        // this stroke's own ground, or none at all
        p.strokes[0].ground = Some("mud".into());
        assert_eq!(shown(&level, &p).strokes[0].material, "mud");
        p.strokes[0].ground = Some(String::new());
        assert_eq!(shown(&level, &p).strokes[0].material, "meadow", "no ground: the material's own colour");
        // this stroke's own width and strength
        p.strokes[0].ground = None;
        p.strokes[0].ground_look = Some(GroundLook { width: Some(3.0), strength: 0.5 });
        let one = shown(&level, &p).strokes[0].clone();
        assert_eq!((one.size, one.strength), (6.0, 0.5));
        p.strokes[0].ground_look = None;
        // sparse plants lay sparse ground when the Mix follows sparseness
        level.materials.get_mut("meadow").unwrap().follows = true;
        level.materials.get_mut("meadow").unwrap().grass_density = 0.25;
        assert_eq!(shown(&level, &p).strokes[0].strength, 0.25);
        // nothing a Mix: the part itself
        p.strokes = vec![stroke("soil")];
        assert!(matches!(shown(&level, &p), Cow::Borrowed(_)));
    }
}
