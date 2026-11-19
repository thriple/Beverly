//! The move / rotate / scale widget and the outlines drawn over the view.
use super::*;

// ---------------------------------------------------------------- widget

#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct WidgetGizmos;

/// The selection outline: thin, drawn over everything.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct OutlineGizmos;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Grip {
    Axis(usize),
    Ring,
    /// A corner cube (bits: which end of x, y, z): scales the whole part.
    Corner(usize),
    /// A face cube (axis, the positive side?): stretches along that axis.
    Face(usize, bool),
}

pub struct Drag {
    pub handle: Grip,
    pub(super) start_pos: Vec3,
    pub(super) start_rot: Quat,
    pub(super) s0: f32,
    /// The part as it was, for Esc.
    pub(super) orig: Part,
    /// A number typed during the drag (Enter sets it exactly).
    pub typed: String,
    /// Finished by Enter or Esc: nothing more happens until the button is let go.
    pub done: bool,
}

/// The scale handles of a part: corner cubes and face cubes, in the world.
pub(super) fn scale_handles(p: &Part, (lo, hi): (Vec3, Vec3)) -> Vec<(Grip, Vec3)> {
    if p.shape == Shape::PlayerStart || p.shape.is_marker() { return vec![]; }
    let a = part_affine(p);
    let mut v: Vec<(Grip, Vec3)> = (0..8).map(|k| (Grip::Corner(k), a.transform_point3(corner_local(k, lo, hi)))).collect();
    for i in 0..3 {
        for pos in [false, true] { v.push((Grip::Face(i, pos), a.transform_point3(face_local(i, pos, lo, hi)))); }
    }
    v
}

pub(super) fn corner_local(k: usize, lo: Vec3, hi: Vec3) -> Vec3 {
    Vec3::new(if k & 1 != 0 { hi.x } else { lo.x }, if k & 2 != 0 { hi.y } else { lo.y }, if k & 4 != 0 { hi.z } else { lo.z })
}

pub(super) fn face_local(i: usize, pos: bool, lo: Vec3, hi: Vec3) -> Vec3 {
    let mut c = (lo + hi) * 0.5;
    c[i] = if pos { hi[i] } else { lo[i] };
    c
}

/// `orig` scaled by a scale handle: Corner by `amount` times, Face to `amount` metres along its axis. The opposite
/// corner or face stays put (with `centre`, the middle does).
pub(super) fn scaled(orig: &Part, (lo, hi): (Vec3, Vec3), grip: Grip, amount: f32, centre: bool) -> Part {
    let min = if orig.shape == Shape::Model { 0.001 } else { 0.02 };
    let size = Vec3::from(orig.size);
    let (new, pin) = match grip {
        Grip::Corner(k) => ((size * amount.max(0.01)).max(Vec3::splat(min)), corner_local(7 - k, lo, hi)),
        Grip::Face(i, pos) => {
            let mut s = size;
            s[i] = (amount / (hi[i] - lo[i]).max(1e-6)).max(min);
            (s, face_local(i, !pos, lo, hi))
        }
        _ => return orig.clone(),
    };
    let pin = if centre { (lo + hi) * 0.5 } else { pin };
    let mut p = orig.clone();
    p.size = new.to_array();
    p.pos = (Vec3::from(orig.pos) + Quat::from_array(orig.rot) * ((size - new) * pin)).to_array();
    p
}

pub(super) const AXES: [Vec3; 3] = [Vec3::X, Vec3::Y, Vec3::Z];
pub(super) fn axis_color(i: usize) -> bevy_egui::egui::Color32 { [theme::AXIS_X, theme::AXIS_Y, theme::AXIS_Z][i] }
pub(super) fn axis_name(i: usize) -> &'static str { ["X", "Y", "Z"][i] }
pub(super) fn c32(c: bevy_egui::egui::Color32) -> Color { Color::srgba_u8(c.r(), c.g(), c.b(), c.a()) }

pub(super) fn widget_len(view: &View, c: Vec3) -> f32 { view.pos.distance(c) * 0.16 }

/// Closest point on the line c + s·a to a ray, as s (None when the ray runs along the axis).
pub(super) fn axis_param(c: Vec3, a: Vec3, ray: Ray3d) -> Option<f32> {
    let d = *ray.direction;
    let w = c - ray.origin;
    let b = a.dot(d);
    let den = 1.0 - b * b;
    if den < 1e-3 { return None; }
    Some((b * d.dot(w) - a.dot(w)) / den)
}

pub(super) fn ring_angle(c: Vec3, ray: Ray3d) -> Option<f32> {
    let d = *ray.direction;
    if d.y.abs() < 1e-4 { return None; }
    let t = (c.y - ray.origin.y) / d.y;
    if t <= 0.0 { return None; }
    let q = ray.origin + d * t - c;
    Some(q.z.atan2(q.x))
}

pub(super) fn seg_dist(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

pub(super) fn snapf(v: f32, step: f32) -> f32 { (v / step).round() * step }

/// Outlines (selection, hidden colliders) and the widget.
pub fn draw(mut ed: ResMut<Editor>, view: Res<View>, mut g: Gizmos, mut w: Gizmos<WidgetGizmos>, mut o: Gizmos<OutlineGizmos>) {
    if ed.testing.is_some() { return; }
    if ed.tab != crate::editor::audio::LEVEL {
        crate::editor::audio::draw(&ed, &view, &mut w);
        return;
    }
    // the curves are laid on the geometry again when it or they changed (not a change that sync or the undo log should see)
    pen::lay(ed.bypass_change_detection());
    texture::draw(&ed, &view, &mut w);
    brush::draw(&ed, &mut w);
    pen::draw(&ed, &view, &mut w);
    light::draw(&ed, &view, &mut w);
    camera::draw(&ed, &view, &mut g, &mut w);
    // a selected Sound part: where it is heard at full volume, and how far it carries (as seen, and level)
    if let Some((p, f)) = ed.selected.and_then(|id| ed.level.get(id)).filter(|_| ed.placing.is_none()).and_then(|p| Some((p, ed.level.sounds.get(&p.sound.as_ref()?.sound)?))) {
        let c = Vec3::from(p.pos);
        let col = Color::srgba_u8(0x3D, 0x7B, 0xFF, 160);
        for r in [f.near, f.far] {
            w.circle(Isometry3d::new(c, Quat::from_rotation_arc(Vec3::Z, -view.forward())), r, col).resolution(64);
            w.circle(Isometry3d::new(c, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), r, col).resolution(64);
        }
    }
    // a tried path: its ends, and the way (yellow), or a red line where there is none
    if let Some(t) = &ed.path_try {
        let up = Vec3::Y * 0.12;
        for (at, c) in [(t.from, Color::srgb(0.45, 0.85, 0.55)), (t.to, Color::srgb(0.96, 0.86, 0.29))] {
            if let Some(at) = at { w.circle(Isometry3d::new(at + up, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), 0.3, c); }
        }
        match (&t.found, t.from, t.to) {
            (Some(Ok(p)), ..) => w.linestrip(p.iter().map(|v| *v + up), Color::srgb(0.96, 0.86, 0.29)),
            (Some(Err(_)), Some(a), Some(b)) => w.line(a + up, b + up, Color::srgb(1.0, 0.37, 0.23)),
            _ => {}
        }
    }
    // the part's box: unit box for shapes, the model's bounds for models
    let corner = |p: &Part, i: usize| {
        let (lo, hi) = ed.geo.local_box(p);
        part_affine(p).transform_point3(lo + (hi - lo) * (shapes::box_corner(i) + Vec3::splat(0.5)))
    };
    let outline = |g: &mut Gizmos<_>, p: &Part, color: Color| {
        for (a, b) in shapes::BOX_EDGES { g.line(corner(p, a), corner(p, b), color); }
    };
    for p in ed.level.parts.iter().filter(|p| !p.visible && Some(p.id) != ed.selected) {
        for (a, b) in shapes::BOX_EDGES { g.line(corner(p, a), corner(p, b), Color::srgba(0.96, 0.86, 0.29, 0.45)); }
    }
    // moving parts: where they go (an outline at the end, and the path there)
    for p in &ed.level.parts {
        let Some(m) = &p.mover else { continue };
        let a = if Some(p.id) == ed.selected { 0.9 } else { 0.35 };
        let col = Color::srgba(0.36, 0.61, 0.94, a);
        let mut end = p.clone();
        end.pos = (Vec3::from(p.pos) + Vec3::from(m.to)).to_array();
        for (i, j) in shapes::BOX_EDGES { g.line(corner(&end, i), corner(&end, j), col); }
        g.arrow(Vec3::from(p.pos), Vec3::from(end.pos), col);
    }
    // wires: arcs from the part that sends to the part that receives
    for w in &ed.level.wires {
        let (Some(a), Some(b)) = (ed.level.get(w.from), ed.level.get(w.to)) else { continue };
        let (a, b) = (Vec3::from(a.pos), Vec3::from(b.pos));
        let lift = Vec3::Y * (a.distance(b) * 0.3 + 0.5);
        let hot = ed.selected == Some(w.from) || ed.selected == Some(w.to);
        let col = Color::srgba(0.94, 0.55, 0.26, if hot { 1.0 } else { 0.55 });
        let pts: Vec<Vec3> = (0..=24).map(|k| {
            let t = k as f32 / 24.0;
            a.lerp(b, t) + lift * 4.0 * t * (1.0 - t)
        }).collect();
        g.linestrip(pts.iter().copied(), col);
        let tip = pts[23];
        g.arrow(tip, b, col);
    }
    // while asking "Change all?": the instances it would change
    for q in ed.ask.iter().flat_map(|(_, them)| them).filter_map(|id| ed.level.get(*id)) { outline(&mut o, q, Color::srgba(0.96, 0.86, 0.29, 0.4)); }
    let Some(p) = ed.selected.and_then(|id| ed.level.get(id)) else { return };
    outline(&mut o, p, Color::srgb(0.96, 0.86, 0.29));
    if ed.placing.is_some() { return; }
    let c = Vec3::from(p.pos);
    let len = widget_len(&view, c);
    let active = ed.drag.as_ref().map(|d| d.handle).or(ed.hover_handle);
    for (i, a) in AXES.iter().enumerate() {
        let hot = active == Some(Grip::Axis(i));
        let col = if hot { Color::WHITE } else { c32(axis_color(i)) };
        w.arrow(c, c + *a * len, col);
    }
    let hot = active == Some(Grip::Ring);
    let col = if hot { Color::WHITE } else { Color::srgba(0.96, 0.86, 0.29, 0.85) };
    w.circle(Isometry3d::new(c, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), len * 0.75, col).resolution(64);
    // scale cubes: corners scale the whole part, faces stretch it along their axis
    let rot = Quat::from_array(p.rot);
    for (g, at) in scale_handles(p, ed.geo.local_box(p)) {
        let hot = active == Some(g);
        let (s, col) = match g {
            Grip::Face(i, _) => (len * 0.045, c32(axis_color(i))),
            _ => (len * 0.06, Color::srgb(0.96, 0.86, 0.29)),
        };
        let s = if hot { s * 1.4 } else { s };
        w.cube(Transform { translation: at, rotation: rot, scale: Vec3::splat(s) }, if hot { Color::WHITE } else { col });
    }
}
