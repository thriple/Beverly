//! The brush (B): paints the chosen material onto parts in strokes, with a soft or hard edge (`paint.rs` draws them).
//! The size and hardness show as two rings on the surface under the pointer. [ and ] make it smaller and larger,
//! Shift + [ and ] softer and harder, as in Photoshop; Alt + right-drag sizes it (across) and softens it (up and
//! down). Hold Alt to erase. Esc or a right-click puts it away. A stroke is one undo step.
use super::view::View;
use super::{theme, Editor};
use crate::level::Stroke;
use crate::settings::Setting;
use bevy::prelude::*;
use bevy::reflect::std_traits::ReflectDefault;

/// The brush's section in its panel.
pub const BRUSH: &str = "BRUSH";

/// The brush: what it paints and how.
#[derive(Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
pub struct Brush {
    /// The material it paints (a file in assets/materials).
    pub material: String,
    #[reflect(@Setting::feel("Size", &["Dab", "Small", "Large", "Forest"], 0.1, 500.0, "m").log().section(BRUSH).help("How wide it paints  ·  [ and ] smaller and larger"))]
    pub size: f32,
    #[reflect(@Setting::feel("Hardness", &["Soft", "Medium", "Hard"], 0.0, 1.0, "").section(BRUSH).help("Soft fades out to its edge; hard stops sharply  ·  Shift + [ and ]"))]
    pub hardness: f32,
    #[reflect(@Setting::feel("Strength", &["Faint", "Half", "Full"], 0.05, 1.0, "").section(BRUSH).help("How much one stroke covers"))]
    pub strength: f32,
}

impl Default for Brush {
    fn default() -> Self { Brush { material: String::new(), size: 1.0, hardness: 0.6, strength: 1.0 } }
}

/// What the brush is doing: where it is over a part, the stroke being painted, and Alt + right-drag sizing.
#[derive(Default)]
pub struct Painting {
    /// Over a part: its id, the point and the surface's normal.
    pub over: Option<(u64, Vec3, Vec3)>,
    /// The stroke being painted: the part and which of its strokes.
    pub stroke: Option<(u64, usize)>,
    /// Alt + right-drag: where the pointer was last.
    pub sizing: Option<Vec2>,
    /// Alt is held: it erases.
    pub erasing: bool,
}

/// Size steps for [ and ].
const STEP: f32 = 1.25;

/// The brush's part of `edit::interact`. Returns whether it took the pointer (always, while the brush is out).
pub fn input(ed: &mut Editor, view: &View, ray: Option<Ray3d>, mouse: &ButtonInput<MouseButton>, keys: &ButtonInput<KeyCode>) -> bool {
    if !ed.brush_on { return false; }
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    ed.painting.erasing = alt;
    // [ ] size, Shift + [ ] hardness (not while typing)
    if !view.keyboard_in_ui {
        for (k, up) in [(KeyCode::BracketLeft, false), (KeyCode::BracketRight, true)] {
            if !keys.just_pressed(k) { continue; }
            let b = &mut ed.brush;
            if shift { b.hardness = (b.hardness + if up { 0.1 } else { -0.1 }).clamp(0.0, 1.0); } else { b.size = (if up { b.size * STEP } else { b.size / STEP }).clamp(0.05, 500.0); }
        }
    }
    // Alt + right-drag: across sizes it, up and down softens and hardens it
    if alt && mouse.pressed(MouseButton::Right) {
        if let (Some(at), Some(last)) = (view.cursor, ed.painting.sizing) {
            let d = at - last;
            ed.brush.size = (ed.brush.size * (d.x * 0.01).exp()).clamp(0.05, 500.0);
            ed.brush.hardness = (ed.brush.hardness - d.y * 0.005).clamp(0.0, 1.0);
        }
        ed.painting.sizing = view.cursor;
        return true;
    }
    ed.painting.sizing = None;
    // a plain right-click or Esc puts it away
    if (mouse.just_pressed(MouseButton::Right) && !alt) || keys.just_pressed(KeyCode::Escape) {
        ed.brush_on = false;
        ed.painting = Painting::default();
        return true;
    }
    let hit = if view.over_ui { None } else { ray.and_then(|r| ed.hit(r)) };
    ed.painting.over = hit.as_ref().and_then(|h| Some((h.part?, h.point, h.normal))).filter(|(id, ..)| ed.level.get(*id).is_some_and(crate::paint::paintable));
    if !mouse.pressed(MouseButton::Left) {
        ed.painting.stroke = None;
        return true;
    }
    let material = if alt { String::new() } else { ed.brush.material.clone() };
    if !alt && !ed.level.materials.contains_key(&material) {
        if mouse.just_pressed(MouseButton::Left) { ed.island.show("!", "Pick a material to paint with", "in the brush's panel, or click one in Assets", theme::Tone::Plain); }
        return true;
    }
    let Some((id, point, _)) = ed.painting.over else { return true };
    let b = ed.brush.clone();
    let spacing = (b.size * 0.15).max(0.01);
    let p = ed.level.get_mut(id).expect("checked above");
    let local = p.to_local(point).to_array();
    match ed.painting.stroke {
        // on along the same part: a point every so often
        Some((pid, k)) if pid == id && p.strokes.get(k).is_some_and(|s| s.material == material) => {
            let s = &mut p.strokes[k];
            if s.points.last().is_none_or(|q| Vec3::from(*q).distance(Vec3::from(local)) >= spacing) { s.points.push(local); }
        }
        // a new stroke (or onto another part: a stroke of its own there)
        _ => {
            p.strokes.push(Stroke { material, size: b.size, hardness: b.hardness, strength: b.strength, points: vec![local] });
            ed.painting.stroke = Some((id, p.strokes.len() - 1));
        }
    }
    true
}

/// The brush's rings on the surface under the pointer: its size (outer) and where its edge starts to fade (inner).
pub fn draw(ed: &Editor, g: &mut Gizmos<super::edit::WidgetGizmos>) {
    if !ed.brush_on { return; }
    let Some((_, at, n)) = ed.painting.over else { return };
    let col = if ed.painting.erasing { Color::srgb(1.0, 0.54, 0.6) } else { Color::srgb_u8(0x8C, 0x7D, 0xFF) };
    let iso = Isometry3d::new(at + n * 0.02, Quat::from_rotation_arc(Vec3::Z, n));
    let r = ed.brush.size * 0.5;
    g.circle(iso, r, col).resolution(64);
    if ed.brush.hardness > 0.02 { g.circle(iso, r * ed.brush.hardness.min(0.98), col.with_alpha(0.5)).resolution(48); }
}
