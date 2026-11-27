//! Camera parts in the editor (views for cinematics; the game never looks through them while playing). Each shows its
//! view as a wire pyramid; the selected Orbit camera also shows its circle and a handle on its target, a Follow camera
//! a line to the player start. Look through (L) puts the editor's view in the camera to aim it by eye.
use super::theme;
use super::view::{Mode, View};
use super::Editor;
use crate::level::{self, Part, Shape};
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

/// Looking through a camera part: which one, and the view to go back to.
#[derive(Clone)]
pub struct Looking {
    pub id: u64,
    back: (Vec3, f32, f32, Mode),
    /// The view it started from (the camera changes only once the view moves away from it).
    from: (Vec3, f32, f32),
    /// An Orbit camera's distance to its target, kept while it is aimed.
    dist: f32,
}

/// How high a placed camera stands above what it is placed on.
pub const EYE: f32 = 1.6;
/// The pyramid's length and its picture's shape (wide).
const LENGTH: f32 = 1.2;
pub const ASPECT: f32 = 16.0 / 9.0;

/// A camera part placed on `ground`, facing `yaw`: at eye height; an Orbit camera's target 6 m ahead on the ground.
pub fn place(p: &mut Part, ground: Vec3, yaw: f32) {
    let pos = ground + Vec3::Y * EYE;
    p.pos = pos.to_array();
    p.rot = Quat::from_rotation_y(yaw).to_array();
    if let (Shape::OrbitCamera, Some(c)) = (p.shape, p.camera.as_mut()) {
        let target = ground + Quat::from_rotation_y(yaw) * Vec3::NEG_Z * 6.0;
        c.target = target.to_array();
        p.rot = level::looking(pos, target).unwrap_or(Quat::IDENTITY).to_array();
    }
}

/// Point a camera part at `at` (an Orbit camera's target becomes `at`).
pub fn aim(p: &mut Part, at: Vec3) -> Result<(), &'static str> {
    let rot = level::looking(Vec3::from(p.pos), at).ok_or("it can't look at itself")?;
    p.rot = rot.to_array();
    if let (Shape::OrbitCamera, Some(c)) = (p.shape, p.camera.as_mut()) { c.target = at.to_array(); }
    Ok(())
}

fn camera(ed: &Editor, id: u64) -> Option<&Part> { ed.level.get(id).filter(|p| p.shape.is_camera() && p.camera.is_some()) }

/// Start or stop looking through the selected camera part.
pub fn toggle(ed: &mut Editor, view: &mut View) {
    if let Some(l) = ed.looking.take() {
        (view.pos, view.yaw, view.pitch, view.mode) = l.back;
        ed.commit();
        return;
    }
    let Some(p) = ed.selected.and_then(|id| camera(ed, id)).cloned() else { return };
    let (pos, rot) = level::camera_view(&p, 0.0, Vec3::ZERO, Vec3::ZERO);
    let dist = p.camera.as_ref().map_or(0.0, |c| pos.distance(Vec3::from(c.target)).max(0.5));
    let (yaw, pitch, _) = rot.to_euler(EulerRot::YXZ);
    ed.looking = Some(Looking { id: p.id, back: (view.pos, view.yaw, view.pitch, view.mode), from: (pos, yaw, pitch), dist });
    (view.pos, view.yaw, view.pitch, view.mode) = (pos, yaw, pitch, Mode::Builder);
}

/// While looking through a camera part, it goes where the view goes.
pub fn follow(ed: &mut Editor, view: &View) {
    let Some(l) = ed.looking.clone() else { return };
    let Some(mut p) = camera(ed, l.id).cloned() else { ed.looking = None; return };
    if (view.pos, view.yaw, view.pitch) == l.from { return; }
    p.pos = view.pos.to_array();
    p.rot = view.rot().to_array();
    if let (Shape::OrbitCamera, Some(c)) = (p.shape, p.camera.as_mut()) { c.target = (view.pos + view.forward() * l.dist).to_array(); }
    if ed.level.get(l.id) != Some(&p) { *ed.level.get_mut(l.id).expect("found above") = p; }
}

/// The largest picture of the camera's shape that fits in `r`.
pub fn frame(r: Rect) -> Rect {
    let (w, h) = if r.width() / r.height().max(1.0) > ASPECT { (r.height() * ASPECT, r.height()) } else { (r.width(), r.width() / ASPECT) };
    Rect::from_center_size(r.center(), vec2(w, h))
}

/// The editor view's lens: the camera's while looking through it (so its picture fills [`frame`], showing only what
/// the camera sees: not the editor's own layer), else the usual.
pub fn lens(ed: Res<Editor>, view: Res<View>, cam: Single<(&mut Projection, &mut RenderLayers), With<super::view::MainCamera>>) {
    let (mut proj, mut layers) = cam.into_inner();
    let want = if ed.looking.is_some() { RenderLayers::layer(0) } else { RenderLayers::from_layers(&[0, 1]) };
    if *layers != want { *layers = want; }
    let fov = match ed.looking.as_ref().and_then(|l| camera(&ed, l.id)).and_then(|p| p.camera.as_ref()) {
        Some(c) => {
            let k = view.rect.height() / frame(view.rect).height().max(1.0);
            2.0 * ((c.fov.to_radians() * 0.5).tan() * k).atan()
        }
        None => std::f32::consts::FRAC_PI_4,
    };
    if let Projection::Perspective(p) = &mut *proj {
        if (p.fov - fov).abs() > 1e-5 { p.fov = fov; }
    }
}

/// Where an Orbit camera's target handle is.
fn target(p: &Part) -> Option<Vec3> { p.camera.as_ref().filter(|_| p.shape == Shape::OrbitCamera).map(|c| Vec3::from(c.target)) }

/// Hover, start and follow a drag of the selected Orbit camera's target handle. Returns true while it has the mouse.
pub fn handles(ed: &mut Editor, view: &View, cam: &Camera, gtf: &GlobalTransform, ray: Option<Ray3d>, pointer: Option<Vec2>, pressed: bool, just_pressed: bool) -> bool {
    ed.camera_hot = false;
    let Some(p) = ed.selected.and_then(|id| camera(ed, id)).cloned() else { ed.camera_drag = false; return false };
    let Some(at) = target(&p) else { ed.camera_drag = false; return false };
    if ed.camera_drag {
        if !pressed { ed.camera_drag = false; return false; }
        let Some(ray) = ray else { return true };
        // onto what is under the pointer, else level with the target as it was
        let n = view.forward();
        let to = ed.hit_skip(ray, Some(p.id)).map(|h| h.point).or_else(|| {
            let den = n.dot(*ray.direction);
            (den.abs() > crate::guard::ZERO).then(|| ray.origin + *ray.direction * (n.dot(at - ray.origin) / den))
        });
        let Some(to) = to.filter(|t| t.distance(Vec3::from(p.pos)) > 0.2) else { return true };
        let mut q = p.clone();
        let _ = aim(&mut q, to);
        if let Some(m) = ed.level.get_mut(p.id) { *m = q; }
        if let Some(ptr) = pointer { ed.label = Some((ptr + vec2(18.0, 14.0), format!("Target · {:.1} m away", to.distance(Vec3::from(p.pos))), theme::highlight())); }
        return true;
    }
    let Some(ptr) = pointer else { return false };
    if view.to_screen(cam, gtf, at).is_none_or(|s| s.distance(ptr) > 10.0) { return false; }
    ed.camera_hot = true;
    ed.label = Some((ptr + vec2(18.0, 14.0), "What it circles and looks at".into(), theme::highlight()));
    if just_pressed { ed.camera_drag = true; }
    true
}

/// Every camera part's pyramid, and the selected one's circle, target handle or line to the player start.
pub fn draw(ed: &Editor, view: &View, g: &mut Gizmos, w: &mut Gizmos<super::edit::WidgetGizmos>) {
    if ed.placing.is_some() { return; }
    let colour = |a: f32| Color::srgba(0.37, 0.78, 0.85, a);
    let start = ed.level.parts.iter().find(|p| p.shape == Shape::PlayerStart).map(|p| Vec3::from(p.pos) - Vec3::Y * p.size[1] * 0.5);
    for p in ed.level.parts.iter().filter(|p| p.shape.is_camera()) {
        if ed.looking.as_ref().is_some_and(|l| l.id == p.id) { continue; }
        let Some(c) = p.camera.as_ref() else { continue };
        let selected = ed.selected == Some(p.id);
        let line = colour(if selected { 0.95 } else { 0.5 });
        let (pos, rot) = level::camera_view(p, 0.0, Vec3::ZERO, Vec3::ZERO);
        let h = LENGTH * (c.fov.to_radians() * 0.5).tan();
        let corner = |x: f32, y: f32| pos + rot * vec3(x * h * ASPECT, y * h, -LENGTH);
        let cs = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)];
        for i in 0..4 {
            g.line(pos, cs[i], line);
            g.line(cs[i], cs[(i + 1) % 4], line);
        }
        // which way is up in its picture
        let top = pos + rot * vec3(0.0, h * 1.35, -LENGTH);
        g.line(cs[2], top, line);
        g.line(top, cs[3], line);
        if !selected { continue; }
        match p.shape {
            Shape::OrbitCamera => {
                let t = Vec3::from(c.target);
                let centre = vec3(t.x, pos.y, t.z);
                let r = vec2(pos.x - t.x, pos.z - t.z).length();
                g.circle(Isometry3d::new(centre, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), r, colour(0.45)).resolution(64);
                g.line(pos, t, colour(0.45));
                let s = view.pos.distance(t) * 0.012 * if ed.camera_hot || ed.camera_drag { 1.5 } else { 1.0 };
                let col = if ed.camera_hot || ed.camera_drag { Color::WHITE } else { colour(1.0) };
                let facing = Isometry3d::new(t, Quat::from_rotation_arc(Vec3::Z, -view.forward()));
                w.circle(facing, s, col);
                w.circle(facing, s * 0.5, col);
            }
            Shape::FollowCamera => {
                if let Some(s) = start { g.line(pos, s, colour(0.45)); }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placed_cameras_stand_at_eye_height_and_an_orbit_camera_looks_at_its_target() {
        let mut p = Part::placed(Shape::OrbitCamera, None, "Orbit camera 1".into(), [0.0; 3], Quat::IDENTITY.to_array(), Shape::OrbitCamera.default_size());
        place(&mut p, Vec3::ZERO, 0.0);
        assert_eq!(p.pos, [0.0, EYE, 0.0]);
        assert_eq!(p.camera.as_ref().unwrap().target, [0.0, 0.0, -6.0]);
        let (_, rot) = level::camera_view(&p, 0.0, Vec3::ZERO, Vec3::ZERO);
        assert!((rot * Vec3::NEG_Z).distance(vec3(0.0, -EYE, -6.0).normalize()) < 1e-4);
        aim(&mut p, vec3(3.0, EYE, 0.0)).unwrap();
        assert_eq!(p.camera.as_ref().unwrap().target, [3.0, EYE, 0.0]);
        let here = Vec3::from(p.pos);
        assert!(aim(&mut p, here).is_err());
    }

    #[test]
    fn the_picture_frame_fits_the_view() {
        let f = frame(Rect::new(0.0, 0.0, 2000.0, 900.0));
        assert_eq!((f.width(), f.height()), (1600.0, 900.0));
        let f = frame(Rect::new(0.0, 0.0, 800.0, 900.0));
        assert_eq!((f.width(), f.height()), (800.0, 450.0));
    }
}
