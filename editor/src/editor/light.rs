//! Light parts' handles. A point light shows the sphere it reaches, with a handle to size it; a spot light shows
//! its cone: drag the tip to aim it and set how far it reaches, drag the rim to make it wider or narrower.
use super::theme;
use super::view::View;
use super::Editor;
use crate::level::{Part, Shape};
use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Grip {
    /// A point light's sphere.
    Reach,
    /// The end of a spot's cone.
    Tip,
    /// The edge of a spot's cone.
    Rim,
}

/// The way a spot light shines: the part's up.
pub fn aim(p: &Part) -> Vec3 { Quat::from_array(p.rot) * Vec3::Y }

/// The selected light part.
fn active(ed: &Editor) -> Option<Part> { ed.selected.and_then(|id| ed.level.get(id)).filter(|p| p.shape.is_light() && p.light.is_some()).cloned() }

/// Across the screen, for handles that should face the viewer.
fn across(view: &View, d: Vec3) -> Vec3 {
    let a = d.cross(view.forward());
    if a.length_squared() > 1e-4 { a.normalize() } else { d.any_orthonormal_vector() }
}

/// Where the handles are: the sphere's (to the right as seen), or the cone's tip and rim.
fn grips(p: &Part, view: &View) -> Vec<(Grip, Vec3)> {
    let l = p.light.as_ref().expect("a light");
    let c = Vec3::from(p.pos);
    if p.shape == Shape::PointLight { return vec![(Grip::Reach, c + across(view, Vec3::Y) * l.reach)]; }
    let d = aim(p);
    let tip = c + d * l.reach;
    vec![(Grip::Tip, tip), (Grip::Rim, tip + across(view, d) * l.reach * (l.cone * 0.5).to_radians().tan())]
}

/// Where a ray meets the plane through `at` with normal `n`.
fn on_plane(ray: Ray3d, at: Vec3, n: Vec3) -> Option<Vec3> {
    let den = n.dot(*ray.direction);
    if den.abs() < 1e-6 { return None; }
    let t = n.dot(at - ray.origin) / den;
    (t > 0.0).then(|| ray.origin + *ray.direction * t)
}

/// Hover, start and follow a drag of the selected light's handles. Returns true while they have the mouse.
#[allow(clippy::too_many_arguments)]
pub fn handles(ed: &mut Editor, view: &View, cam: &Camera, gtf: &GlobalTransform, ray: Option<Ray3d>, pointer: Option<Vec2>, pressed: bool, just_pressed: bool, snap: bool) -> bool {
    ed.light_hot = None;
    let Some(p) = active(ed) else { ed.light_drag = None; return false };
    let l = p.light.clone().expect("a light");
    let c = Vec3::from(p.pos);
    if let Some(grip) = ed.light_drag {
        if !pressed {
            ed.light_drag = None;
            return false;
        }
        let Some(ray) = ray else { return true };
        let grid = ed.level.grid();
        let step = |x: f32, by: f32| if snap { ((x / by).round() * by).max(by) } else { x };
        let mut q = p.clone();
        let light = q.light.as_mut().expect("a light");
        let text = match grip {
            Grip::Reach => {
                let Some(at) = on_plane(ray, c, view.forward()) else { return true };
                light.reach = step(at.distance(c).max(0.1), grid.step);
                format!("Reach · {:.2} m", light.reach)
            }
            Grip::Tip => {
                // onto what is under the pointer, else level with the tip as it was
                let at = ed.hit_skip(ray, Some(p.id)).map(|h| h.point).or_else(|| on_plane(ray, c + aim(&p) * l.reach, view.forward()));
                let Some(at) = at.filter(|a| a.distance(c) > 0.1) else { return true };
                q.rot = Quat::from_rotation_arc(Vec3::Y, (at - c).normalize()).to_array();
                let light = q.light.as_mut().expect("a light");
                light.reach = step(at.distance(c), grid.step);
                format!("Aim · reach {:.2} m", light.reach)
            }
            Grip::Rim => {
                let d = aim(&p);
                let tip = c + d * l.reach;
                let Some(at) = on_plane(ray, tip, d) else { return true };
                let deg = (2.0 * at.distance(tip).atan2(l.reach).to_degrees()).clamp(1.0, 170.0);
                light.cone = if snap { ((deg / 5.0).round() * 5.0).clamp(5.0, 170.0) } else { deg };
                format!("Cone · {:.0}°", light.cone)
            }
        };
        if let Some(m) = ed.level.get_mut(p.id) { *m = q; }
        if let Some(ptr) = pointer { ed.label = Some((ptr + vec2(18.0, 14.0), text, theme::highlight())); }
        return true;
    }
    let Some(ptr) = pointer else { return false };
    let mut best: Option<(f32, Grip)> = None;
    for (g, at) in grips(&p, view) {
        if let Some(s) = view.to_screen(cam, gtf, at) {
            let d = s.distance(ptr);
            if d < 10.0 && best.is_none_or(|(b, _)| d < b) { best = Some((d, g)); }
        }
    }
    let Some((_, grip)) = best else { return false };
    ed.light_hot = Some(grip);
    let hint = match grip {
        Grip::Reach => "How far the light reaches",
        Grip::Tip => "Aim the light (and how far it reaches)",
        Grip::Rim => "How wide the cone is",
    };
    ed.label = Some((ptr + vec2(18.0, 14.0), hint.into(), theme::highlight()));
    if just_pressed { ed.light_drag = Some(grip); }
    true
}

/// The selected light's reach (sphere or cone) and its handles.
pub fn draw(ed: &Editor, view: &View, g: &mut Gizmos<super::edit::WidgetGizmos>) {
    if ed.placing.is_some() || ed.testing.is_some() { return; }
    let Some(p) = active(ed) else { return };
    let l = p.light.as_ref().expect("a light");
    let c = Vec3::from(p.pos);
    let line = Color::srgba(0.96, 0.86, 0.29, 0.55);
    let facing = |at: Vec3| Isometry3d::new(at, Quat::from_rotation_arc(Vec3::Z, -view.forward()));
    if p.shape == Shape::PointLight {
        // the outline as seen, and the level ring
        g.circle(facing(c), l.reach, line).resolution(64);
        g.circle(Isometry3d::new(c, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), l.reach, line).resolution(64);
    } else {
        let d = aim(&p);
        let tip = c + d * l.reach;
        let r = l.reach * (l.cone * 0.5).to_radians().tan();
        g.circle(Isometry3d::new(tip, Quat::from_rotation_arc(Vec3::Z, d)), r, line).resolution(64);
        let side = across(view, d);
        let up = d.cross(side);
        for k in 0..8 {
            let a = std::f32::consts::TAU * k as f32 / 8.0;
            g.line(c, tip + (side * a.cos() + up * a.sin()) * r, line);
        }
        g.line(c, tip, line);
    }
    let hot = |grip: Grip| ed.light_drag.or(ed.light_hot) == Some(grip);
    for (grip, at) in grips(&p, view) {
        let s = view.pos.distance(at) * 0.012 * if hot(grip) { 1.5 } else { 1.0 };
        let col = if hot(grip) { Color::WHITE } else { Color::srgb(0.96, 0.86, 0.29) };
        g.circle(facing(at), s, col);
        g.circle(facing(at), s * 0.5, col);
    }
}

/// How many shadowed lights may reach one place before the game slows down.
pub const BUDGET: usize = 4;

/// The most shadowed lights that reach any one place, and which they are. Checked at each light and in the middle
/// of where each two of them overlap.
pub fn budget(level: &crate::level::Level) -> Vec<u64> {
    let lit: Vec<(u64, Vec3, f32)> = level.parts.iter()
        .filter_map(|p| p.light.as_ref().filter(|l| l.shadows && p.shape.is_light()).map(|l| (p.id, Vec3::from(p.pos), l.reach)))
        .collect();
    let mut spots: Vec<Vec3> = lit.iter().map(|l| l.1).collect();
    for (i, a) in lit.iter().enumerate() {
        for b in &lit[i + 1..] {
            let d = a.1.distance(b.1);
            if d > 1e-4 && d < a.2 + b.2 { spots.push(a.1 + (b.1 - a.1) / d * ((d - b.2 + a.2) * 0.5)); }
        }
    }
    spots.iter()
        .map(|s| lit.iter().filter(|l| l.1.distance(*s) <= l.2 + 1e-4).map(|l| l.0).collect::<Vec<_>>())
        .max_by_key(|g| g.len())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::Level;

    #[test]
    fn the_budget_counts_shadowed_lights_reaching_one_place() {
        let mut lvl = Level::default();
        let lamp = |lvl: &mut Level, x: f32, shadows: bool| {
            let mut p = Part::placed(Shape::PointLight, None, format!("Lamp {x}"), [x, 1.0, 0.0], Quat::IDENTITY.to_array(), [0.3; 3]);
            p.light.as_mut().unwrap().reach = 3.0;
            p.light.as_mut().unwrap().shadows = shadows;
            lvl.add(p)
        };
        // a chain: 0 and 4 each overlap 2 but not each other, so no place has all three
        let a = lamp(&mut lvl, 0.0, true);
        let b = lamp(&mut lvl, 4.0, true);
        lamp(&mut lvl, 8.0, true);
        assert_eq!(budget(&lvl).len(), 2);
        // one more over the first two: three reach the place between them; one without shadows doesn't count
        let c = lamp(&mut lvl, 2.0, true);
        lamp(&mut lvl, 2.0, false);
        let mut worst = budget(&lvl);
        worst.sort();
        assert_eq!(worst, vec![a, b, c]);
        assert!(budget(&Level::default()).is_empty());
    }
}
