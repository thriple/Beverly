//! Textures on parts: drag one from the Assets panel onto a face (Shift: the whole part), then fit it with the
//! violet handles lying on the face: the middle slides it, the corners size one copy of it (Shift keeps its shape).
use super::theme::{self, Tone};
use super::view::View;
use super::Editor;
use crate::level::{Part, Shape, Surface};
use crate::shapes::{self, Lay, Tris};
use crate::world::part_affine;
use bevy::prelude::*;
use std::collections::HashMap;

/// A texture handle: the middle (slide) or a corner of one copy (size), counter-clockwise from the start corner.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Grip { Slide, Corner(usize) }

/// A handle being dragged: the surface as it was, and where on the face the drag began (metres).
pub struct Drag { grip: Grip, orig: Surface, start: Vec2 }

/// Texture files Beverly can lay on parts.
pub fn is_texture(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.ends_with(".png") || p.ends_with(".jpg") || p.ends_with(".jpeg")
}

/// Parts that take textures: the basic shapes.
pub fn takes_textures(p: &Part) -> bool { Shape::ALL.contains(&p.shape) }

/// A flat face of a part, to go between the world and metres on the face (the texture's across / down).
struct Plane { rot: Quat, pos: Vec3, u: Vec3, v: Vec3, n: Vec3, d: f32, centre: Vec2 }

impl Plane {
    fn of(p: &Part, tris: &Tris, face: u8) -> Option<Plane> {
        let Lay::Flat(n0) = *tris.faces.get(face as usize)? else { return None };
        let size = Vec3::from(p.size);
        let (u, v) = shapes::flat_axes(n0, size);
        let n = (n0 / size).normalize_or_zero();
        let pts: Vec<Vec3> = tris.face.iter().enumerate().filter(|(_, f)| **f == face).flat_map(|(t, _)| tris.pos[t * 3..t * 3 + 3].iter().map(|q| *q * size)).collect();
        let centre = pts.iter().map(|q| vec2(q.dot(u), q.dot(v))).sum::<Vec2>() / pts.len().max(1) as f32;
        Some(Plane { rot: Quat::from_array(p.rot), pos: Vec3::from(p.pos), u, v, n, d: pts.first()?.dot(n), centre })
    }
    fn world(&self, m: Vec2) -> Vec3 { self.pos + self.rot * (self.u * m.x + self.v * m.y + self.n * self.d) }
    fn metres(&self, ray: Ray3d) -> Option<Vec2> {
        let inv = self.rot.inverse();
        let (o, dir) = (inv * (ray.origin - self.pos), inv * *ray.direction);
        let den = self.n.dot(dir);
        if den.abs() < 1e-6 { return None; }
        let x = o + dir * ((self.d - self.n.dot(o)) / den);
        Some(vec2(x.dot(self.u), x.dot(self.v)))
    }
    /// The corners of the texture copy nearest the middle of the face, counter-clockwise from its start.
    fn copy(&self, s: &Surface) -> [Vec2; 4] {
        let (tile, off) = (Vec2::from(s.tile), Vec2::from(s.offset));
        let k = ((self.centre - off) / tile).floor();
        [vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0)].map(|c| off + (k + c) * tile)
    }
}

/// The outline of one face: the edges its triangles don't share, in the world.
fn face_outline(p: &Part, tris: &Tris, face: u8) -> Vec<(Vec3, Vec3)> {
    let key = |q: Vec3| (q * 1e4).round().as_ivec3();
    let mut count: HashMap<(IVec3, IVec3), (usize, Vec3, Vec3)> = HashMap::new();
    for (t, _) in tris.face.iter().enumerate().filter(|(_, f)| **f == face) {
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let (pa, pb) = (tris.pos[t * 3 + a], tris.pos[t * 3 + b]);
            let k = if key(pa).to_array() < key(pb).to_array() { (key(pa), key(pb)) } else { (key(pb), key(pa)) };
            count.entry(k).or_insert((0, pa, pb)).0 += 1;
        }
    }
    let m = part_affine(p);
    count.into_values().filter(|(n, ..)| *n == 1).map(|(_, a, b)| (m.transform_point3(a), m.transform_point3(b))).collect()
}

fn face_name(tris: &Tris, face: Option<u8>) -> String {
    face.and_then(|f| tris.face_names().get(f as usize).cloned()).unwrap_or_else(|| "all".into())
}

impl Editor {
    /// Lay a texture on a part's face (`None`: all of it), keeping the size the texture had there. Returns what
    /// happened, for the island and the MCP.
    pub fn lay_texture(&mut self, id: u64, face: Option<u8>, texture: &str) -> Result<String, String> {
        let p = self.level.get(id).ok_or("No such part")?;
        if !takes_textures(p) { return Err(format!("{} can't take textures (basic shapes only)", p.name)); }
        let tris = &self.geo.shapes[&p.shape];
        if face.is_some_and(|f| f as usize >= tris.faces.len()) { return Err("No such face".into()); }
        let tile = face.and_then(|f| p.surface(f)).or(p.surfaces.first()).map_or([1.0, 1.0], |s| s.tile);
        let what = format!("{} of {}", face_name(tris, face), p.name);
        // a material file is laid as it is; a texture through its own material
        let material = if texture.ends_with(".bev") {
            if !self.level.materials.contains_key(texture) { return Err(format!("No material {texture}")); }
            texture.to_string()
        } else {
            self.level.material_for_texture(texture)
        };
        self.level.get_mut(id).unwrap().lay(Surface { face, material, tile, offset: [0.0, 0.0] });
        self.selected = Some(id);
        self.tex_face = face;
        Ok(what)
    }
}

/// While a texture is dragged from the Assets panel: show the face under the pointer, lay it there when let go.
/// Returns true while it has the mouse.
pub fn dropping(ed: &mut Editor, ray: Option<Ray3d>, pointer: Option<Vec2>, released: bool, cancel: bool, shift: bool) -> bool {
    let Some(tex) = ed.texturing.clone() else { return false };
    let over_view = pointer.is_some();
    if let Some(ptr) = pointer {
        let name = tex.rsplit('/').next().unwrap_or_default();
        ed.label = Some((ptr + vec2(18.0, 14.0), format!("▦ {name} · drop on a face (Shift: the whole part)"), theme::VIOLET));
    }
    let hit = ray.and_then(|r| ed.hit(r)).and_then(|h| Some((h.part?, h.face)));
    ed.tex_hover = hit.filter(|(id, _)| ed.level.get(*id).is_some_and(takes_textures)).map(|(id, f)| (id, if shift { None } else { f }));
    if cancel {
        ed.texturing = None;
        ed.tex_hover = None;
    } else if released {
        ed.texturing = None;
        match ed.tex_hover.take() {
            Some((id, face)) if over_view => match ed.lay_texture(id, face, &tex) {
                Ok(what) => ed.island.show("▦", format!("Textured {what}"), "Drag the violet handles to fit it · Shift when dropping: the whole part", Tone::Accent),
                Err(e) => ed.island.show("!", "Couldn't lay the texture", e, Tone::Danger),
            },
            _ if over_view => ed.island.show("▦", "Drop a texture on a part", "Basic shapes take textures · Shift: the whole part", Tone::Plain),
            _ => {}
        }
    }
    true
}

/// The selected part's texture handles: hover, start and follow a drag. Returns true when they have the mouse.
#[allow(clippy::too_many_arguments)]
pub fn handles(ed: &mut Editor, view: &View, cam: &Camera, gtf: &GlobalTransform, ray: Option<Ray3d>, pointer: Option<Vec2>, pressed: bool, just_pressed: bool, shift: bool, snap: bool) -> bool {
    ed.tex_hot = None;
    let Some((p, face, s)) = active(ed) else { ed.tex_drag = None; return false };
    let Some(plane) = Plane::of(&p, &ed.geo.shapes[&p.shape], face) else { return false };
    if let Some(d) = &ed.tex_drag {
        if !pressed {
            ed.tex_drag = None;
            return false;
        }
        let Some(now) = ray.and_then(|r| plane.metres(r)) else { return true };
        let o = &d.orig;
        let (tile0, off0) = (Vec2::from(o.tile), Vec2::from(o.offset));
        let by = ed.level.grid().scale;
        let step = |x: Vec2, by: f32| if snap { (x / by).round() * by } else { x };
        let (tile, offset) = match d.grip {
            Grip::Slide => (tile0, step(off0 + now - d.start, by)),
            Grip::Corner(c) => {
                // the opposite corner stays put
                let copy = plane.copy(o);
                let pin = copy[(c + 2) % 4];
                let mut t = (now - pin).abs().max(Vec2::splat(0.05));
                if shift { t = tile0 * (t / tile0).max_element(); }
                let t = step(t, by).max(Vec2::splat(by));
                let k = ((plane.centre - off0) / tile0).floor();
                let side = [vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0)][(c + 2) % 4];
                (t, pin - (k + side) * t)
            }
        };
        let grip = d.grip;
        if let Some(q) = ed.level.get_mut(p.id) {
            if let Some(m) = q.surfaces.iter_mut().find(|m| *m == &s) {
                m.tile = tile.to_array();
                m.offset = offset.to_array();
            }
        }
        if let Some(ptr) = pointer {
            let text = match grip { Grip::Slide => format!("Slide texture · {:.2}, {:.2} m", offset.x, offset.y), Grip::Corner(_) => format!("Texture size · {:.2} × {:.2} m", tile.x, tile.y) };
            ed.label = Some((ptr + vec2(18.0, 14.0), text, theme::VIOLET));
        }
        return true;
    }
    let Some(ptr) = pointer else { return false };
    let sc = |m: Vec2| view.to_screen(cam, gtf, plane.world(m));
    let copy = plane.copy(&s);
    let mut best: Option<(f32, Grip)> = None;
    let mid = (copy[0] + copy[2]) * 0.5;
    for (g, m) in [(Grip::Slide, mid)].into_iter().chain((0..4).map(|c| (Grip::Corner(c), copy[c]))) {
        if let Some(at) = sc(m) {
            let dd = at.distance(ptr);
            if dd < 10.0 && best.is_none_or(|(b, _)| dd < b) { best = Some((dd, g)); }
        }
    }
    let Some((_, grip)) = best else { return false };
    ed.tex_hot = Some(grip);
    ed.label = Some((ptr + vec2(18.0, 14.0), match grip { Grip::Slide => "Slide the texture".into(), Grip::Corner(_) => "Size the texture (Shift keeps its shape)".into() }, theme::VIOLET));
    if just_pressed {
        if let Some(start) = ray.and_then(|r| plane.metres(r)) {
            ed.tex_drag = Some(Drag { grip, orig: s, start });
        }
    }
    true
}

/// The selected part's surface whose handles show: on the face last clicked or textured, if it is flat.
fn active(ed: &Editor) -> Option<(Part, u8, Surface)> {
    let p = ed.selected.and_then(|id| ed.level.get(id)).filter(|p| takes_textures(p) && !p.surfaces.is_empty())?;
    let tris = &ed.geo.shapes[&p.shape];
    // the face clicked, else the first flat face the surfaces cover
    let face = ed.tex_face.filter(|f| p.surface(*f).is_some())
        .or_else(|| (0..tris.faces.len() as u8).find(|f| p.surface(*f).is_some() && matches!(tris.faces[*f as usize], Lay::Flat(_))))?;
    Some((p.clone(), face, p.surface(face)?.clone()))
}

/// Draw the face a texture would land on, and the selected part's texture handles.
pub fn draw(ed: &Editor, view: &View, g: &mut Gizmos<super::edit::WidgetGizmos>) {
    let violet = |a: f32| Color::srgba_u8(theme::VIOLET.r(), theme::VIOLET.g(), theme::VIOLET.b(), (a * 255.0) as u8);
    if let Some((id, face)) = ed.tex_hover {
        let Some(p) = ed.level.get(id) else { return };
        let tris = &ed.geo.shapes[&p.shape];
        let lines = match face {
            Some(f) => face_outline(p, tris, f),
            None => (0..tris.faces.len() as u8).flat_map(|f| face_outline(p, tris, f)).collect(),
        };
        for (a, b) in lines { g.line(a, b, violet(1.0)); }
        return;
    }
    if ed.placing.is_some() || ed.testing.is_some() { return; }
    let Some((p, face, s)) = active(ed) else { return };
    let Some(plane) = Plane::of(&p, &ed.geo.shapes[&p.shape], face) else { return };
    let copy = plane.copy(&s).map(|m| plane.world(m));
    for i in 0..4 { g.line(copy[i], copy[(i + 1) % 4], violet(0.9)); }
    let hot = |grip: Grip| ed.tex_drag.as_ref().map(|d| d.grip).or(ed.tex_hot) == Some(grip);
    let size = |at: Vec3| view.pos.distance(at) * 0.012;
    let iso = |at: Vec3| Isometry3d::new(at, plane.rot * Quat::from_rotation_arc(Vec3::Z, plane.n));
    let mid = (copy[0] + copy[2]) * 0.5;
    g.circle(iso(mid), size(mid) * if hot(Grip::Slide) { 1.6 } else { 1.0 }, if hot(Grip::Slide) { Color::WHITE } else { violet(1.0) });
    for (c, at) in copy.iter().enumerate() {
        let k = if hot(Grip::Corner(c)) { 1.5 } else { 1.0 };
        g.rect(iso(*at), Vec2::splat(size(*at) * 1.4 * k), if hot(Grip::Corner(c)) { Color::WHITE } else { violet(1.0) });
    }
}
