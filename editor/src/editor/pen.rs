//! The pen (P): draws curves (`curve.rs`) with Bezier handles on the surfaces under the pointer. Click for a corner,
//! drag for a smooth point (the drag pulls out its handles), click the first point to close the loop, Enter to finish
//! an open curve. Alt while dragging breaks a handle off from the other. Once drawn a curve stays editable: drag its
//! points and handles, click the line to put a point on it, Alt + click a point (or Delete over it) to take it out,
//! Delete elsewhere to take the whole curve away. Click another curve to edit that one. Esc or a right-click lets go
//! (a curve being drawn is finished first, then the pen is put away). A stretch of a curve (the line leaving a point)
//! is laid on the geometry it meets as its point's stretch says (`curve::Stretch`); a small button by the selected
//! point switches it (`ui::stretch_button`).
use super::view::View;
use super::{theme, Editor};
use crate::curve::{self, Curve, PenPoint, Stretch, Vert};
use crate::level::{Part, Tag};
use crate::world::{self, Geometry};
use bevy::prelude::*;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// What the pen is holding or pointing at: a point, or one of its two handles.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Spot { Point(usize), Into(usize), Out(usize) }

/// A point or handle held by the mouse: the plane it is dragged along (through where it was grabbed, with the
/// surface's normal), whether the other handle follows (a smooth point), and whether the point is brand new.
#[derive(Clone, Copy, Debug)]
pub struct Grab { spot: Spot, plane: (Vec3, Vec3), mirror: bool, fresh: bool }

/// What the pen is doing.
#[derive(Default)]
pub struct Pen {
    /// The curve being drawn or edited.
    pub curve: Option<u64>,
    /// The curve is still taking points (a click on a surface adds one).
    pub drawing: bool,
    pub grab: Option<Grab>,
    /// The point or handle of the curve under the pointer.
    pub hover: Option<Spot>,
    /// A surface under the pointer: the point and its normal.
    pub over: Option<(Vec3, Vec3)>,
    /// The point last picked or put down (its stretch is what the button switches).
    pub point: Option<usize>,
    /// Each curve's line as it is seen (laid on the geometry), with what it was worked out from.
    pub lines: HashMap<u64, (u64, Vec<Vert>)>,
    /// What each curve's strip strokes were last made from.
    pub stripped: HashMap<u64, u64>,
}

impl Pen {
    /// Put the pen away (what it is holding goes; the curves' lines and strips are kept).
    pub fn reset(&mut self) {
        *self = Pen { lines: std::mem::take(&mut self.lines), stripped: std::mem::take(&mut self.stripped), ..Pen::default() };
    }
}

/// How far (m) a path steps up onto the next surface (a half metre, and a little over so a step of exactly that is met), how far it drops down onto one, and how far over props an
/// Over everything stretch looks.
const STEP: f32 = 0.52;
const DROP: f32 = 2.0;
const OVER: f32 = 1.5;

/// Where a line's point `p` goes onto the surface it meets, given the last point laid (`from`): a Ground stretch takes
/// the walkable surface just under `from` (a part's top face or anything tagged Ground or Water, not one tagged Paths
/// ignore, invisible colliders never); Over everything takes anything visible within reach; Fall takes the first
/// surface it reaches. No surface within reach: None, and the line goes on straight over the gap.
fn surface(parts: &[Part], geo: &Geometry, p: Vec3, from: Vec3, s: Stretch) -> Option<Vec3> {
    let mut y = match s { Stretch::Ground => from.y + STEP, Stretch::Over => from.y + OVER, _ => p.y + STEP };
    for _ in 0..4 {
        let ray = Ray3d::new(Vec3::new(p.x, y, p.z), Dir3::NEG_Y);
        let h = world::raycast(parts, geo, ray, None, |q| q.visible && (s != Stretch::Ground || !q.tags.contains(&Tag::PathsIgnore)))?;
        let walkable = s != Stretch::Ground || h.normal.y >= 0.5 || h.part.and_then(|id| parts.iter().find(|q| q.id == id)).is_some_and(|q| q.tags.iter().any(|t| matches!(t, Tag::Ground | Tag::Water)));
        if walkable { return (s == Stretch::Fall || h.point.y >= from.y - DROP).then_some(h.point); }
        y = h.point.y - 0.01;
    }
    None
}

fn stamp<T: Hash>(h: &mut impl Hasher, v: &[T]) { for x in v { x.hash(h); } }

/// What the geometry is now, as one number (a change in it lays every curve again).
fn world_stamp(ed: &Editor) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    ed.geo.models.len().hash(&mut h);
    for p in &ed.level.parts {
        (p.id, p.visible, std::mem::discriminant(&p.shape), &p.model).hash(&mut h);
        for v in p.pos.iter().chain(&p.rot).chain(&p.size) { v.to_bits().hash(&mut h); }
        stamp(&mut h, &p.tags.iter().map(|t| *t as u8).collect::<Vec<_>>());
    }
    h.finish()
}

fn curve_stamp(c: &Curve, world: u64) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (world, c.closed).hash(&mut h);
    for p in &c.points {
        (p.stretch as u8).hash(&mut h);
        for v in p.at.iter().chain(&p.into).chain(&p.out) { v.to_bits().hash(&mut h); }
    }
    h.finish()
}

/// Lay each curve on the geometry again where it or the geometry changed (every frame; a curve that hasn't changed
/// costs a hash).
pub fn lay(ed: &mut Editor) {
    if ed.level.curves.is_empty() { ed.pen.lines.clear(); return; }
    let world = world_stamp(ed);
    let Editor { level, geo, pen, .. } = ed;
    pen.lines.retain(|id, _| level.curves.iter().any(|c| c.id == *id));
    for c in &level.curves {
        let now = curve_stamp(c, world);
        if pen.lines.get(&c.id).is_some_and(|(then, _)| *then == now) { continue; }
        pen.lines.insert(c.id, (now, c.laid(|p, from, s| surface(&level.parts, geo, p, from, s))));
    }
}

/// Make the strokes of each curve's strip again where the curve, its strip or the geometry changed, and take away
/// those of a curve or strip that is gone. Not while something is being dragged (the strokes would be made every
/// frame); `edit::commit` calls it as the change becomes an undo step.
pub fn strips(ed: &mut Editor) {
    if ed.level.curves.is_empty() && ed.pen.stripped.is_empty() { return; }
    let world = world_stamp(ed);
    let Editor { level, geo, pen, .. } = ed;
    let live: Vec<u64> = level.curves.iter().filter(|c| c.strip.is_some()).map(|c| c.id).collect();
    pen.stripped.retain(|id, _| live.contains(id));
    for p in level.parts.iter_mut().filter(|p| p.strokes.iter().any(|s| s.curve.is_some_and(|id| !live.contains(&id)))) {
        p.strokes.retain(|s| s.curve.is_none_or(|id| live.contains(&id)));
    }
    for c in &level.curves {
        let Some(strip) = &c.strip else { continue };
        let Some((_, line)) = pen.lines.get(&c.id) else { continue };
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (curve_stamp(c, world), &strip.material, strip.size.to_bits(), strip.hardness.to_bits(), strip.strength.to_bits()).hash(&mut h);
        let now = h.finish();
        if pen.stripped.get(&c.id) == Some(&now) { continue; }
        pen.stripped.insert(c.id, now);
        let made = if level.materials.contains_key(&strip.material) { strip_strokes(c.id, strip, line, &level.parts, geo) } else { vec![] };
        // each part gets the new strokes where its old ones were (so what was painted over or under them stays so)
        for p in &mut level.parts {
            let new: Vec<_> = made.iter().filter(|(id, _)| *id == p.id).map(|(_, s)| s.clone()).collect();
            let old: Vec<_> = p.strokes.iter().filter(|s| s.curve == Some(c.id)).collect();
            if old.len() == new.len() && old.iter().zip(&new).all(|(a, b)| *a == b) { continue; }
            let at = p.strokes.iter().position(|s| s.curve == Some(c.id)).unwrap_or(p.strokes.len());
            p.strokes.retain(|s| s.curve != Some(c.id));
            for (k, st) in new.into_iter().enumerate() { p.strokes.insert((at + k).min(p.strokes.len()), st); }
        }
    }
}

/// A strip's strokes: the laid line walked in steps of a tenth of the strip's width, each step put on the part under
/// it (a stretch in the air paints nothing), a stroke for each run along one part. (part id, stroke)
fn strip_strokes(curve: u64, strip: &curve::Strip, line: &[Vert], parts: &[Part], geo: &Geometry) -> Vec<(u64, crate::level::Stroke)> {
    let mut out = vec![];
    let mut run: Option<(u64, Vec<[f32; 3]>)> = None;
    let flush = |run: &mut Option<(u64, Vec<[f32; 3]>)>, out: &mut Vec<(u64, crate::level::Stroke)>| {
        if let Some((id, points)) = run.take() {
            out.push((id, crate::level::Stroke { material: strip.material.clone(), size: strip.size, hardness: strip.hardness, strength: strip.strength, points, lays: crate::level::Lays::Add, rules: Default::default(), curve: Some(curve) }));
        }
    };
    let step = (strip.size * 0.1).max(0.05);
    for w in line.windows(2) {
        let (a, b) = (w[0].at, w[1].at);
        let on_ground = !matches!(w[1].stretch, Stretch::Air | Stretch::Fall);
        let n = if on_ground { (a.distance(b) / step).ceil().max(1.0) as usize } else { 0 };
        for k in 0..=n {
            let p = a.lerp(b, k as f32 / n.max(1) as f32);
            // the part whose top face the point is on (a point inside a part, as under a prop, is on none)
            let ray = Ray3d::new(p + Vec3::Y * 0.05, Dir3::NEG_Y);
            let part = parts.iter().filter(|q| on_ground && crate::paint::paintable(q))
                .filter_map(|q| world::raycast(std::slice::from_ref(q), geo, ray, None, |_| true).filter(|h| h.part.is_some() && h.normal.y > 0.0).map(|h| (q, h.point.distance(p))))
                .filter(|(_, d)| *d < 0.15).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(q, _)| q);
            match (part, &mut run) {
                (Some(q), Some((id, points))) if q.id == *id => if k > 0 || points.is_empty() { points.push(q.to_local(p).to_array()) },
                (Some(q), _) => { flush(&mut run, &mut out); run = Some((q.id, vec![q.to_local(p).to_array()])); }
                (None, _) => flush(&mut run, &mut out),
            }
        }
    }
    flush(&mut run, &mut out);
    out
}

/// The line of curve `c` as it is seen.
fn line<'a>(ed: &'a Editor, c: &Curve) -> Option<&'a [Vert]> { ed.pen.lines.get(&c.id).map(|(_, l)| l.as_slice()) }

/// How near (m) a point or handle must be to the pointer to be picked, at this distance from the eye.
fn reach(view: &View, p: Vec3) -> f32 { (view.pos.distance(p) * 0.022).max(0.02) }

fn plane_point(ray: Ray3d, plane: (Vec3, Vec3)) -> Option<Vec3> {
    let n = Dir3::new(plane.1).ok()?;
    let t = ray.intersect_plane(plane.0, InfinitePlane3d::new(n))?;
    Some(ray.get_point(t)).filter(|p| p.distance(plane.0) < 500.0)
}

/// The point or handle of `c` that the ray passes within reach of (handles first, so they win where they overlap).
fn spot_at(c: &Curve, view: &View, ray: Ray3d) -> Option<Spot> {
    let near = |p: Vec3| curve::ray_to_point(ray, p) < reach(view, p);
    for (i, p) in c.points.iter().enumerate() {
        if p.out_handle() != Vec3::ZERO && near(p.pos() + p.out_handle()) { return Some(Spot::Out(i)); }
        if p.into_handle() != Vec3::ZERO && near(p.pos() + p.into_handle()) { return Some(Spot::Into(i)); }
    }
    c.points.iter().position(|p| near(p.pos())).map(Spot::Point)
}

/// The pen's part of `edit::interact`. Returns whether it took the pointer (always, while the pen is out).
pub fn input(ed: &mut Editor, view: &View, ray: Option<Ray3d>, mouse: &ButtonInput<MouseButton>, keys: &ButtonInput<KeyCode>) -> bool {
    if !ed.pen_on || ed.tab != super::audio::LEVEL { return false; }
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    // Enter finishes a curve; Esc or a right-click lets go: of the curve, then of the pen
    if keys.just_pressed(KeyCode::Enter) && ed.pen.drawing {
        finish(ed);
        return true;
    }
    if keys.just_pressed(KeyCode::Escape) || mouse.just_pressed(MouseButton::Right) {
        ed.pen.grab = None;
        if ed.pen.drawing { finish(ed); } else if ed.pen.curve.take().is_none() { ed.pen_on = false; ed.pen.reset(); }
        return true;
    }
    let hit = if view.over_ui { None } else { ray.and_then(|r| ed.hit(r)) };
    ed.pen.over = hit.as_ref().map(|h| (h.point, h.normal));
    if !mouse.pressed(MouseButton::Left) { ed.pen.grab = None; }
    if let (Some(g), Some(r)) = (ed.pen.grab, ray) {
        drag(ed, view, r, hit.as_ref().map(|h| h.point), alt, g);
        return true;
    }
    ed.pen.hover = match (ed.pen.curve.and_then(|id| ed.level.curve(id)), ray) {
        (Some(c), Some(r)) if !view.over_ui => spot_at(c, view, r),
        _ => None,
    };
    if view.keyboard_in_ui { return true; }
    if mouse.just_pressed(MouseButton::Left) && !view.over_ui { click(ed, view, ray, alt); }
    if (keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace)) && !ed.pen.drawing { delete(ed); }
    true
}

/// A click: on a point or handle (hold it, close the loop, or with Alt take the point out), on the curve's line (put
/// a point there), on another curve (edit that one), or on a surface (the next point, or a new curve).
fn click(ed: &mut Editor, view: &View, ray: Option<Ray3d>, alt: bool) {
    let Some(ray) = ray else { return };
    let normal = ed.pen.over.map_or(Vec3::Y, |o| o.1);
    let active = ed.pen.curve;
    if let (Some(spot), Some(id)) = (ed.pen.hover, active) {
        let Some(c) = ed.level.curve(id) else { return };
        let pts = c.points.len();
        match spot {
            Spot::Point(0) if ed.pen.drawing && pts >= 3 => {
                ed.level.curve_mut(id).expect("found").closed = true;
                finish(ed);
            }
            Spot::Point(i) if alt && !ed.pen.drawing => remove_point(ed, id, i),
            Spot::Point(i) if !ed.pen.drawing => {
                let p = c.points[i].pos();
                ed.pen.point = Some(i);
                ed.pen.grab = Some(Grab { spot, plane: (p, normal), mirror: false, fresh: false });
            }
            Spot::Into(i) | Spot::Out(i) => {
                let p = &c.points[i];
                let (this, other) = if matches!(spot, Spot::Out(_)) { (p.out_handle(), p.into_handle()) } else { (p.into_handle(), p.out_handle()) };
                // a smooth point keeps its handles opposite; one already broken stays broken; Alt breaks it
                let mirror = !alt && (other.length() < 1e-4 || (this + other).length() < 0.05 * this.length().max(0.01));
                ed.pen.grab = Some(Grab { spot, plane: (p.pos() + this, normal), mirror, fresh: false });
            }
            _ => {}
        }
        return;
    }
    if !ed.pen.drawing {
        // the line of the curve being edited: a point goes onto it
        if let Some((id, c)) = active.and_then(|id| Some((id, ed.level.curve(id)?))) {
            if let Some((seg, t, d, at)) = line(ed, c).and_then(|l| curve::nearest(l, ray)) {
                if d < reach(view, at) {
                    let c = ed.level.curve_mut(id).expect("found");
                    let i = c.split(seg, t);
                    let p = c.points[i].pos();
                    ed.pen.point = Some(i);
                    ed.pen.grab = Some(Grab { spot: Spot::Point(i), plane: (p, normal), mirror: false, fresh: false });
                    return;
                }
            }
        }
        // another curve: edit that one
        let other = ed.level.curves.iter().filter(|c| Some(c.id) != active).find_map(|c| {
            let (_, _, d, at) = curve::nearest(line(ed, c)?, ray)?;
            (d < reach(view, at)).then_some(c.id)
        });
        if let Some(id) = other {
            ed.pen.curve = Some(id);
            ed.pen.point = None;
            return;
        }
    }
    let Some((mut at, n)) = ed.pen.over else { return };
    if ed.snap && n.y > 0.999 { at.x = (at.x / 0.25).round() * 0.25; at.z = (at.z / 0.25).round() * 0.25; }
    let i = match (ed.pen.drawing, active) {
        (true, Some(id)) if ed.level.curve(id).is_some() => {
            let c = ed.level.curve_mut(id).expect("checked");
            // a new point goes on as the last one went
            let stretch = c.points.last().map_or(Stretch::Ground, |p| p.stretch);
            c.points.push(PenPoint { stretch, ..PenPoint::corner(at) });
            c.points.len() - 1
        }
        _ => {
            let id = ed.level.add_curve(vec![PenPoint::corner(at)], false);
            ed.pen.curve = Some(id);
            ed.pen.drawing = true;
            0
        }
    };
    ed.pen.point = Some(i);
    // holding on drags the point's handles out of it (a click and no drag leaves a corner)
    ed.pen.grab = Some(Grab { spot: Spot::Out(i), plane: (at, n), mirror: !alt, fresh: true });
}

/// Moving what is held.
fn drag(ed: &mut Editor, view: &View, ray: Ray3d, surface: Option<Vec3>, alt: bool, g: Grab) {
    let Some(id) = ed.pen.curve else { return };
    let snap = ed.snap;
    let Some(c) = ed.level.curve_mut(id) else { return };
    match g.spot {
        // a point follows the surface under the pointer, else its plane
        Spot::Point(i) => {
            let Some(mut p) = surface.or_else(|| plane_point(ray, g.plane)) else { return };
            if snap && g.plane.1.y > 0.999 { p.x = (p.x / 0.25).round() * 0.25; p.z = (p.z / 0.25).round() * 0.25; }
            if let Some(pt) = c.points.get_mut(i) { pt.at = p.to_array(); }
        }
        Spot::Out(i) | Spot::Into(i) => {
            let Some(target) = plane_point(ray, g.plane) else { return };
            let Some(pt) = c.points.get_mut(i) else { return };
            let mut h = target - pt.pos();
            // a click that barely moved stays a corner
            if g.fresh && h.length() < reach(view, pt.pos()) * 0.8 { h = Vec3::ZERO; }
            let (this, other) = if matches!(g.spot, Spot::Out(_)) { (&mut pt.out, &mut pt.into) } else { (&mut pt.into, &mut pt.out) };
            *this = h.to_array();
            if g.mirror && !(alt && !g.fresh) { *other = (-h).to_array(); }
        }
    }
}

/// Take point `i` out of curve `id`; a curve of one point goes too.
fn remove_point(ed: &mut Editor, id: u64, i: usize) {
    let Some(c) = ed.level.curve_mut(id) else { return };
    c.remove(i);
    if c.points.len() < 2 { drop_curve(ed, id); }
    ed.pen.hover = None;
    ed.pen.point = None;
}

fn drop_curve(ed: &mut Editor, id: u64) {
    ed.level.curves.retain(|c| c.id != id);
    if ed.pen.curve == Some(id) { ed.pen.curve = None; ed.pen.drawing = false; ed.pen.point = None; }
}

/// Delete: the point under the pointer, else the whole curve being edited.
fn delete(ed: &mut Editor) {
    let Some(id) = ed.pen.curve else { return };
    match ed.pen.hover {
        Some(Spot::Point(i)) => remove_point(ed, id, i),
        _ => {
            let name = ed.level.curve(id).map(|c| c.name.clone()).unwrap_or_default();
            drop_curve(ed, id);
            ed.island.show("✕", format!("Deleted {name}"), "Ctrl+Z to undo", theme::Tone::Danger);
        }
    }
}

/// Stop taking points: a curve of one point is nothing and goes; any other stays, to edit.
fn finish(ed: &mut Editor) {
    let Some(id) = ed.pen.curve else { ed.pen.drawing = false; return };
    ed.pen.drawing = false;
    ed.pen.grab = None;
    match ed.level.curve(id).map(|c| (c.points.len(), c.name.clone(), c.closed)) {
        Some((n, name, closed)) if n >= 2 => ed.island.show("✎", format!("{name} · {n} points{}", if closed { " · closed" } else { "" }), "Drag points and handles · click the line to add a point · Alt + click a point removes it", theme::Tone::Accent),
        _ => drop_curve(ed, id),
    }
}

/// Curves, in the Level tab: every curve's line, the one being edited with its points and handles, and while drawing
/// the line to where the next point would go.
pub fn draw(ed: &Editor, view: &View, g: &mut Gizmos<super::edit::WidgetGizmos>) {
    let line = Color::srgba(0.35, 0.85, 0.7, 0.7);
    let hot = Color::srgb(0.62, 1.0, 0.88);
    for c in &ed.level.curves {
        let col = if ed.pen.curve == Some(c.id) { hot } else { line };
        match self::line(ed, c) {
            Some(l) => stretches(g, l, col),
            None => g.linestrip(c.sample(), col),
        }
    }
    if !ed.pen_on { return; }
    if let Some((at, n)) = ed.pen.over.filter(|_| ed.pen.grab.is_none()) {
        g.circle(Isometry3d::new(at + n * 0.02, Quat::from_rotation_arc(Vec3::Z, n)), reach(view, at) * 0.7, hot).resolution(24);
    }
    let Some(c) = ed.pen.curve.and_then(|id| ed.level.curve(id)) else { return };
    let lit = |s: Spot| ed.pen.hover == Some(s) || ed.pen.grab.is_some_and(|g| g.spot == s);
    let cube = |g: &mut Gizmos<super::edit::WidgetGizmos>, at: Vec3, s: f32, col: Color| { g.cube(Transform { translation: at, rotation: Quat::IDENTITY, scale: Vec3::splat(s) }, col); };
    for (i, p) in c.points.iter().enumerate() {
        let r = reach(view, p.pos());
        let (size, col) = if ed.pen.point == Some(i) { (1.25, Color::WHITE) } else { (0.9, if lit(Spot::Point(i)) { Color::WHITE } else { hot }) };
        cube(g, p.pos(), r * size, col);
        for (h, spot) in [(p.out_handle(), Spot::Out(i)), (p.into_handle(), Spot::Into(i))] {
            if h == Vec3::ZERO { continue; }
            let end = p.pos() + h;
            g.line(p.pos(), end, line);
            g.circle(Isometry3d::new(end, Quat::from_rotation_arc(Vec3::Z, -view.forward())), r * 0.55, if lit(spot) { Color::WHITE } else { hot }).resolution(16);
        }
    }
    if ed.pen.drawing {
        // the first point is where a click closes the loop
        if c.points.len() >= 3 {
            let p = c.points[0].pos();
            g.circle(Isometry3d::new(p, Quat::from_rotation_arc(Vec3::Z, -view.forward())), reach(view, p) * 1.6, hot).resolution(24);
        }
        // the line to where the next point would go
        if let (Some(last), Some((at, _)), None) = (c.points.last(), ed.pen.over, ed.pen.grab) {
            let b = [last.pos(), last.pos() + last.out_handle(), at, at];
            g.linestrip((0..=curve::PIECES).map(|k| curve::at(&b, k as f32 / curve::PIECES as f32)), hot.with_alpha(0.45));
        }
    }
}

/// A laid line: ground stretches solid, air and fall ones dashed.
fn stretches(g: &mut Gizmos<super::edit::WidgetGizmos>, line: &[Vert], col: Color) {
    let mut on = 0.0f32;
    for w in line.windows(2) {
        let (a, b) = (w[0].at, w[1].at);
        if !matches!(w[1].stretch, Stretch::Air | Stretch::Fall) { g.line(a, b, col); on = 0.0; continue; }
        // dashes 0.3 m long with 0.2 m between, carried on from piece to piece (`on` is how far the dashes have run)
        let len = a.distance(b);
        let mut k = (on / 0.5).floor();
        while k * 0.5 - on < len {
            let (from, to) = ((k * 0.5 - on).max(0.0), (k * 0.5 + 0.3 - on).min(len));
            if to > from { g.line(a.lerp(b, from / len), a.lerp(b, to / len), col); }
            k += 1.0;
        }
        on += len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::Shape;

    fn part(id: u64, shape: Shape, pos: [f32; 3], size: [f32; 3]) -> Part {
        let mut p = Part::placed(shape, None, format!("P{id}"), pos, [0.0, 0.0, 0.0, 1.0], size);
        p.id = id;
        p
    }

    #[test]
    fn a_strip_paints_the_part_under_the_line_and_not_one_the_line_passes_through() {
        // a floor 0.1 thick (top at 0.1), a big box standing on it, and a line along the floor through the box
        let parts = vec![part(1, Shape::Plane, [0.0, 0.05, 0.0], [20.0, 0.1, 20.0]), part(2, Shape::Box, [0.0, 1.1, 0.0], [2.0, 2.0, 2.0])];
        let geo = Geometry::default();
        let c = Curve { id: 9, name: "T".into(), points: vec![PenPoint::corner(Vec3::new(-6.0, 0.1, 0.0)), PenPoint::corner(Vec3::new(6.0, 0.1, 0.0))], closed: false, strip: None };
        let line = c.laid(|_, _, _| None);
        let strip = curve::Strip { material: "assets/materials/moss.bev".into(), size: 1.0, hardness: 0.5, strength: 1.0 };
        let made = strip_strokes(9, &strip, &line, &parts, &geo);
        // the box stands on the line: the floor under it is painted all the way along, the box (the line is inside it) not at all
        assert_eq!(made.iter().map(|m| m.0).collect::<Vec<_>>(), vec![1]);
        assert!(made.iter().all(|(_, s)| s.curve == Some(9) && s.size == 1.0 && s.points.len() > 100));
        // a line in the air paints nothing
        let mut air = c.clone();
        air.points[0].stretch = Stretch::Air;
        assert!(strip_strokes(9, &strip, &air.laid(|_, _, _| None), &parts, &geo).is_empty());
    }
}
