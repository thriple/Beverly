//! The bucket fill (G): click a part and everything that counts as the same (`level::Fills`: the same material,
//! colour or surface tag, the part itself, or the floor at that height) gets the brush's material, strength and Lays,
//! over whole faces. Hold Alt to erase instead. The fill stays a rule (`bucket.rs` works out what it reaches), so its
//! strokes are made again whenever the level changes; filling the same area again replaces the earlier fill. Esc or
//! a right-click puts it away. Each fill is one undo step.
use super::view::View;
use super::{theme, Editor};
use crate::level::{Bucket, Fills, Stroke, Tag};
use crate::settings::Setting;
use bevy::prelude::*;
use bevy::reflect::std_traits::ReflectDefault;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// The bucket's section in its panel, and the one for Colour's tolerance (shown only for Colour).
pub const BUCKET: &str = "BUCKET";
pub const TOLERANCE: &str = "TOLERANCE";
/// The layer's section: what of the brush's the bucket lays.
pub const LAYER: &str = "LAYER";

/// How much the bucket lays, and how: the brush's strength and Lays, as its panel shows them.
#[derive(Clone, Copy, PartialEq, Debug, Default, Reflect)]
#[reflect(Default)]
pub struct Layer {
    #[reflect(@Setting::feel("Strength", &["Faint", "Half", "Full"], 0.05, 1.0, "").section(LAYER).help("How much it covers (with Set: exactly how much); the brush's"))]
    pub strength: f32,
    #[reflect(@Setting::choice("Lays").section(LAYER).help("Add: over what is there  ·  Subtract: takes its own material away  ·  Set: exactly its strength; the brush's"))]
    pub lays: crate::level::Lays,
}

/// How the bucket fills (it lays the brush's material, strength and Lays).
#[derive(Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
pub struct BucketTool {
    #[reflect(@Setting::choice("Fills").section(BUCKET).help("What else counts as the same as what you click: the same material, the same colour, the same surface tag, the part itself, or the walkable faces at that height"))]
    pub fills: Fills,
    #[reflect(@Setting::feel("Tolerance", &["Exact", "Close", "Loose", "Any"], 0.0, 1.0, "").section(TOLERANCE).help("How far a colour may be from the one clicked"))]
    pub tolerance: f32,
    #[reflect(@Setting::toggle("Connected only").section(BUCKET).help("Only what touches the part clicked, part to part: it stops at gaps"))]
    pub connected: bool,
    /// For Tag: the tag it fills (None: the clicked part's first).
    pub tag: Option<Tag>,
}

impl Default for BucketTool {
    fn default() -> Self { BucketTool { fills: Fills::Material, tolerance: 0.1, connected: false, tag: None } }
}

/// What the bucket is doing: what it is over and what a click there would fill, and what each fill's strokes were
/// last made from.
#[derive(Default)]
pub struct Filling {
    /// Over a part: the fill a click would make, and the faces it would reach.
    pub over: Option<(Bucket, Vec<(u64, Vec<u8>)>)>,
    /// What `over` was worked out from (so it isn't every frame).
    over_from: u64,
    /// Alt is held: it erases.
    pub erasing: bool,
    /// What each fill's strokes were last made from.
    made: HashMap<u64, u64>,
}

/// What the parts, materials and fills are now, as one number, leaving out the strokes (a fill makes those).
fn level_stamp(ed: &Editor) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in ed.level.parts.iter() {
        (p.id, p.visible, std::mem::discriminant(&p.shape), p.tags.iter().map(|t| *t as u8).collect::<Vec<_>>()).hash(&mut h);
        for v in p.pos.iter().chain(&p.rot).chain(&p.size).chain(&p.color) { v.to_bits().hash(&mut h); }
        for s in &p.surfaces { (s.face, &s.material).hash(&mut h); }
    }
    // Floor reads the walker profile, Connected only the precision
    [ed.level.profile().step, ed.level.profile().slope, ed.level.precision().0].map(f32::to_bits).hash(&mut h);
    for (k, m) in ed.level.materials.iter() { (k, m.tint.map(f32::to_bits)).hash(&mut h); }
    h.finish()
}

fn bucket_stamp(b: &Bucket, world: u64) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (world, b.part, b.face, b.at.map(f32::to_bits), b.fills as u8, b.tolerance.to_bits(), b.tag.map(|t| t as u8), b.connected, &b.material, b.strength.to_bits(), b.lays as u8).hash(&mut h);
    h.finish()
}

/// Make each fill's strokes again where the fill or the level changed, and take away those of a fill that is gone.
/// `Editor::commit` calls it as a change becomes an undo step, so a fill and its strokes are one step (step 4 of
/// `docs/orderliness.md` C3, after the pen: it owns the strokes marked with a fill, and no other).
pub fn fills(ed: &mut Editor) {
    if ed.level.buckets.is_empty() && ed.filling.made.is_empty() { return; }
    let world = level_stamp(ed);
    let live: Vec<u64> = ed.level.buckets.iter().map(|b| b.id).collect();
    ed.filling.made.retain(|id, _| live.contains(id));
    let stale: Vec<u64> = ed.level.parts.iter().filter(|p| p.strokes.iter().any(|s| s.bucket.is_some_and(|id| !live.contains(&id)))).map(|p| p.id).collect();
    for id in stale {
        if let Some(p) = ed.level.get_mut(id) { p.strokes.retain(|s| s.bucket.is_none_or(|b| live.contains(&b))); }
    }
    let buckets: Vec<Bucket> = ed.level.buckets.to_vec();
    for b in &buckets {
        let now = bucket_stamp(b, world);
        if ed.filling.made.get(&b.id) == Some(&now) { continue; }
        ed.filling.made.insert(b.id, now);
        let reach = crate::bucket::reach(&ed.level, b);
        let stroke = |faces: &Vec<u8>| Stroke { material: b.material.clone(), size: 0.0, hardness: 1.0, strength: b.strength, points: vec![], lays: b.lays, rules: Default::default(), curve: None, fill: false, faces: faces.clone(), bucket: Some(b.id), ground: None, ground_look: None };
        let ids: Vec<u64> = ed.level.parts.iter().map(|p| p.id).collect();
        for id in ids {
            let want: Option<Stroke> = reach.iter().find(|(pid, _)| *pid == id).map(|(_, f)| stroke(f));
            let p = ed.level.get(id).expect("listed");
            let old: Vec<&Stroke> = p.strokes.iter().filter(|s| s.bucket == Some(b.id)).collect();
            if old.len() == usize::from(want.is_some()) && old.first().copied() == want.as_ref() { continue; }
            // the new stroke goes where the old one was (so what was painted over or under it stays so), else on top
            let p = ed.level.get_mut(id).expect("listed");
            let at = p.strokes.iter().position(|s| s.bucket == Some(b.id)).unwrap_or(p.strokes.len());
            p.strokes.retain(|s| s.bucket != Some(b.id));
            if let Some(s) = want { p.strokes.insert(at.min(p.strokes.len()), s); }
        }
    }
}

/// Add a fill (or replace the one that reaches the same faces) and make its strokes. Returns its id.
pub fn add(ed: &mut Editor, mut b: Bucket) -> u64 {
    let reach = crate::bucket::reach(&ed.level, &b);
    // filling the same area again replaces the earlier fill
    let same: Vec<u64> = ed.level.buckets.iter().filter(|o| crate::bucket::reach(&ed.level, o) == reach).map(|o| o.id).collect();
    ed.level.buckets.retain(|o| !same.contains(&o.id));
    ed.level.next_id += 1;
    b.id = ed.level.next_id;
    ed.level.buckets.push(b);
    fills(ed);
    ed.level.next_id
}

/// The bucket's part of `edit::interact`. Returns whether it took the pointer (always, while it is out).
pub fn input(ed: &mut Editor, view: &View, ray: Option<Ray3d>, mouse: &ButtonInput<MouseButton>, keys: &ButtonInput<KeyCode>) -> bool {
    if !ed.bucket_on || ed.tab != super::audio::LEVEL { return false; }
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    ed.filling.erasing = alt;
    if mouse.just_pressed(MouseButton::Right) || keys.just_pressed(KeyCode::Escape) {
        ed.bucket_on = false;
        ed.filling.over = None;
        return true;
    }
    let hit = if view.over_ui { None } else { ray.and_then(|r| ed.hit(r)) };
    let target = hit.as_ref().and_then(|h| Some((h.part?, h.face, h.point))).filter(|(id, ..)| ed.level.get(*id).is_some_and(crate::paint::paintable));
    let Some((id, face, point)) = target else { ed.filling.over = None; return true };
    let t = ed.bucket.clone();
    let material = if alt { String::new() } else { ed.brush.material.clone() };
    let at = ed.level.get(id).expect("checked").to_local(point).to_array();
    let lays = if alt { Default::default() } else { ed.brush.lays };
    let b = Bucket { id: 0, part: id, face, at, fills: t.fills, tolerance: t.tolerance, tag: t.tag, connected: t.connected, material, strength: ed.brush.strength, lays };
    // what a click would fill: worked out again when the place, the settings or the level change
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (bucket_stamp(&b, level_stamp(ed)), if t.fills == Fills::Floor { (point.y * 10.0).round() as i32 } else { 0 }).hash(&mut h);
    let from = h.finish();
    if ed.filling.over.is_none() || ed.filling.over_from != from {
        let reach = crate::bucket::reach(&ed.level, &b);
        ed.filling.over = Some((b.clone(), reach));
        ed.filling.over_from = from;
    }
    if mouse.just_pressed(MouseButton::Left) && !view.keyboard_in_ui {
        if !alt && !ed.level.materials.contains_key(&b.material) {
            ed.island.show("!", "Pick a material to fill with", "in the bucket's panel, or click one in Assets", theme::Tone::Plain);
            return true;
        }
        let reached = ed.filling.over.as_ref().map_or(0, |o| o.1.len());
        let word = if alt { "Erased" } else { "Filled" };
        add(ed, b);
        ed.island.show("◪", format!("{word} {reached} part{}", if reached == 1 { "" } else { "s" }), "Ctrl+Z to undo · fill it again to replace", theme::Tone::Accent);
    }
    true
}

/// What a click would fill: the faces it reaches, outlined.
pub fn draw(ed: &Editor, g: &mut Gizmos<super::edit::WidgetGizmos>) {
    if !ed.bucket_on { return; }
    let Some((_, reach)) = &ed.filling.over else { return };
    let col = if ed.filling.erasing { Color::srgb(1.0, 0.54, 0.6) } else { Color::srgb_u8(0x8C, 0x7D, 0xFF) };
    for (id, faces) in reach {
        let Some(p) = ed.level.get(*id) else { continue };
        let tris = crate::shapes::build(p.shape);
        let a = crate::world::part_affine(p);
        let turn = Quat::from_array(p.rot);
        for (t, f) in tris.face.iter().enumerate() {
            if !faces.contains(f) { continue; }
            let n = (turn * (tris.nrm[t * 3] / Vec3::from(p.size).max(Vec3::splat(1e-4)))).normalize_or_zero() * 0.01;
            let c: Vec<Vec3> = (0..3).map(|k| a.transform_point3(tris.pos[t * 3 + k]) + n).collect();
            g.linestrip([c[0], c[1], c[2], c[0]], col.with_alpha(0.55));
        }
    }
}
