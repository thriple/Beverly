//! The pen (P): draws curves (`curve.rs`) with Bezier handles on the surfaces under the pointer. Click for a corner,
//! drag for a smooth point (the drag pulls out its handles), click the first point to close the loop, Enter to finish
//! an open curve. Alt while dragging breaks a handle off from the other. Once drawn a curve stays editable: drag its
//! points and handles, click the line to put a point on it, Alt + click a point (or Delete over it) to take it out,
//! Delete elsewhere to take the whole curve away. Click another curve to edit that one. Esc or a right-click lets go
//! (a curve being drawn is finished first, then the pen is put away).
use super::view::View;
use super::{theme, Editor};
use crate::curve::{self, Curve, PenPoint};
use bevy::prelude::*;

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
}

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
        if ed.pen.drawing { finish(ed); } else if ed.pen.curve.take().is_none() { ed.pen_on = false; ed.pen = Pen::default(); }
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
            if let Some((seg, t, d)) = c.nearest(ray) {
                if d < reach(view, curve::at(&c.bezier(seg), t)) {
                    let c = ed.level.curve_mut(id).expect("found");
                    let i = c.split(seg, t);
                    let p = c.points[i].pos();
                    ed.pen.grab = Some(Grab { spot: Spot::Point(i), plane: (p, normal), mirror: false, fresh: false });
                    return;
                }
            }
        }
        // another curve: edit that one
        let other = ed.level.curves.iter().filter(|c| Some(c.id) != active).find_map(|c| {
            let (seg, t, d) = c.nearest(ray)?;
            (d < reach(view, curve::at(&c.bezier(seg), t))).then_some(c.id)
        });
        if let Some(id) = other {
            ed.pen.curve = Some(id);
            return;
        }
    }
    let Some((mut at, n)) = ed.pen.over else { return };
    if ed.snap && n.y > 0.999 { at.x = (at.x / 0.25).round() * 0.25; at.z = (at.z / 0.25).round() * 0.25; }
    let i = match (ed.pen.drawing, active) {
        (true, Some(id)) if ed.level.curve(id).is_some() => {
            let c = ed.level.curve_mut(id).expect("checked");
            c.points.push(PenPoint::corner(at));
            c.points.len() - 1
        }
        _ => {
            let id = ed.level.add_curve(vec![PenPoint::corner(at)], false);
            ed.pen.curve = Some(id);
            ed.pen.drawing = true;
            0
        }
    };
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
}

fn drop_curve(ed: &mut Editor, id: u64) {
    ed.level.curves.retain(|c| c.id != id);
    if ed.pen.curve == Some(id) { ed.pen.curve = None; ed.pen.drawing = false; }
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
        let on = ed.pen.curve == Some(c.id);
        g.linestrip(c.sample(), if on { hot } else { line });
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
        cube(g, p.pos(), r * 0.9, if lit(Spot::Point(i)) { Color::WHITE } else { hot });
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
