//! The editor's frame and panels, drawn from the user's design: `assets/ui/editor.svg` (their docs/ui.svg cut into
//! parts by id, every shape, colour and coordinate kept). Each part is tessellated once (lyon) and drawn as a mesh.
//! The design is scaled to fit the window; a wider or taller window stretches only a few straight runs between the
//! corners and notches, so every curve, angle and the logo stay as drawn.
use bevy_egui::egui::{self, Color32, Pos2, Rect, pos2};
use lyon_tessellation::path::{Path as LPath, PathEvent, iterator::PathIterator};
use lyon_tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, StrokeOptions, StrokeTessellator, StrokeVertex, VertexBuffers, math::point};
use std::collections::HashMap;
use std::sync::OnceLock;
use usvg::tiny_skia_path::{PathSegment, Point, Transform};

/// The design's main artboard (the window), in its own units.
const ART: Rect = Rect { min: pos2(31.8, 36.6), max: pos2(1238.2, 764.4) };
/// The straight runs that stretch when the window is wider (two, shared equally) or taller than the design.
const STRETCH_X: [(f32, f32); 2] = [(200.0, 280.0), (990.0, 1070.0)];
const STRETCH_Y: (f32, f32) = (250.0, 550.0);

/// One part of the design: triangles in its own units, coloured, and for each triangle the edges of its shape
/// nearby (so each pixel's coverage comes from the real edges).
pub struct Part {
    pub verts: Vec<(Pos2, Color32)>,
    pub indices: Vec<u32>,
    pub bounds: Rect,
    /// Each edge vertex's outward direction (zero inside); the edge vertices step a pixel out when drawn, so the
    /// shader can fade the edge over the pixels on both sides of it.
    normals: Vec<egui::Vec2>,
    /// Per triangle, up to six edge segments near it (vertex pairs): those at its corners that step out first.
    edges: Vec<[Option<[u32; 2]>; 6]>,
}

pub struct Art {
    parts: HashMap<String, Part>,
    /// The view's outline (the frame's hole), in the design's units.
    pub view: Vec<Pos2>,
}

/// The design, read once.
pub fn art() -> &'static Art {
    static ART_ONCE: OnceLock<Art> = OnceLock::new();
    ART_ONCE.get_or_init(|| Art::read(include_str!("../../assets/ui/editor.svg")))
}

impl Art {
    fn read(svg: &str) -> Art {
        let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).expect("the editor's design is valid SVG");
        let mut parts = HashMap::new();
        let mut view = vec![];
        for node in tree.root().children() {
            let usvg::Node::Group(g) = node else { continue };
            let mut part = Part { verts: vec![], indices: vec![], bounds: Rect::NOTHING, normals: vec![], edges: vec![] };
            add_group(&mut part, g, 1.0, if g.id() == "frame" { Some(&mut view) } else { None });
            parts.insert(g.id().to_string(), part);
        }
        Art { parts, view }
    }

    pub fn part(&self, id: &str) -> &Part { &self.parts[id] }
}

fn add_group(part: &mut Part, g: &usvg::Group, opacity: f32, mut outline: Option<&mut Vec<Pos2>>) {
    let opacity = opacity * g.opacity().get();
    for node in g.children() {
        match node {
            usvg::Node::Group(c) => add_group(part, c, opacity, outline.as_deref_mut()),
            usvg::Node::Path(p) if p.is_visible() => {
                let (path, t) = (lyon_path(p.data()), p.abs_transform());
                if let Some(f) = p.fill() {
                    let rule = if f.rule() == usvg::FillRule::EvenOdd { lyon_tessellation::FillRule::EvenOdd } else { lyon_tessellation::FillRule::NonZero };
                    let mut b: VertexBuffers<Pos2, u32> = VertexBuffers::new();
                    FillTessellator::new()
                        .tessellate_path(&path, &FillOptions::tolerance(0.02).with_fill_rule(rule), &mut BuffersBuilder::new(&mut b, |v: FillVertex| pos2(v.position().x, v.position().y)))
                        .expect("the design's shapes tessellate");
                    push(part, b, t, f.paint(), f.opacity().get() * opacity);
                }
                if let Some(s) = p.stroke() {
                    let mut b: VertexBuffers<Pos2, u32> = VertexBuffers::new();
                    StrokeTessellator::new()
                        .tessellate_path(&path, &StrokeOptions::tolerance(0.02).with_line_width(s.width().get()), &mut BuffersBuilder::new(&mut b, |v: StrokeVertex| pos2(v.position().x, v.position().y)))
                        .expect("the design's strokes tessellate");
                    push(part, b, t, s.paint(), s.opacity().get() * opacity);
                }
                // the frame's hole is the view
                if let Some(o) = outline.as_deref_mut() {
                    let mut subs: Vec<Vec<Pos2>> = vec![];
                    for e in path.iter().flattened(0.05) {
                        match e {
                            PathEvent::Begin { at } => subs.push(vec![map(t, pos2(at.x, at.y))]),
                            PathEvent::Line { to, .. } => subs.last_mut().expect("begun").push(map(t, pos2(to.x, to.y))),
                            _ => {}
                        }
                    }
                    let area = |s: &Vec<Pos2>| Rect::from_points(s).area();
                    if let Some(inner) = subs.into_iter().min_by(|a, b| area(a).total_cmp(&area(b))) { *o = inner; }
                }
            }
            _ => {}
        }
    }
}

fn map(t: Transform, p: Pos2) -> Pos2 {
    let mut q = Point::from_xy(p.x, p.y);
    t.map_point(&mut q);
    pos2(q.x, q.y)
}

fn lyon_path(d: &usvg::tiny_skia_path::Path) -> LPath {
    let mut b = LPath::builder();
    let (mut open, mut last) = (false, point(0.0, 0.0));
    let pt = |p: Point| point(p.x, p.y);
    for s in d.segments() {
        match s {
            PathSegment::MoveTo(p) => {
                if open { b.end(false); }
                last = pt(p);
                b.begin(last);
                open = true;
            }
            PathSegment::Close => {
                if open { b.end(true); }
                open = false;
            }
            seg => {
                if !open { b.begin(last); open = true; }
                match seg {
                    PathSegment::LineTo(p) => { b.line_to(pt(p)); last = pt(p); }
                    PathSegment::QuadTo(c, p) => { b.quadratic_bezier_to(pt(c), pt(p)); last = pt(p); }
                    PathSegment::CubicTo(c1, c2, p) => { b.cubic_bezier_to(pt(c1), pt(c2), pt(p)); last = pt(p); }
                    _ => {}
                }
            }
        }
    }
    if open { b.end(false); }
    b.build()
}

/// Add tessellated triangles (in the path's own space) to a part, coloured by its paint.
fn push(part: &mut Part, b: VertexBuffers<Pos2, u32>, t: Transform, paint: &usvg::Paint, opacity: f32) {
    let base = part.verts.len() as u32;
    let colour = |local: Pos2| -> Color32 {
        let (c, a) = match paint {
            usvg::Paint::Color(c) => (*c, 1.0),
            usvg::Paint::LinearGradient(g) => {
                let mut q = Point::from_xy(local.x, local.y);
                if let Some(inv) = g.transform().invert() { inv.map_point(&mut q); }
                let (dx, dy) = (g.x2() - g.x1(), g.y2() - g.y1());
                let k = (((q.x - g.x1()) * dx + (q.y - g.y1()) * dy) / (dx * dx + dy * dy).max(1e-6)).clamp(0.0, 1.0);
                let stops = g.stops();
                let i = stops.iter().position(|s| s.offset().get() >= k).unwrap_or(stops.len() - 1);
                let (s0, s1) = (&stops[i.saturating_sub(1)], &stops[i]);
                let span = (s1.offset().get() - s0.offset().get()).max(1e-6);
                let u = ((k - s0.offset().get()) / span).clamp(0.0, 1.0);
                let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * u).round() as u8;
                let (a0, a1) = (s0.color(), s1.color());
                (usvg::Color::new_rgb(l(a0.red, a1.red), l(a0.green, a1.green), l(a0.blue, a1.blue)), s0.opacity().get() + (s1.opacity().get() - s0.opacity().get()) * u)
            }
            _ => (usvg::Color::black(), 1.0),
        };
        Color32::from_rgba_unmultiplied(c.red, c.green, c.blue, (a * opacity * 255.0).round() as u8)
    };
    for v in &b.vertices {
        let w = map(t, *v);
        part.bounds = part.bounds.union(Rect::from_pos(w));
        part.verts.push((w, colour(*v)));
        part.normals.push(egui::Vec2::ZERO);
    }
    part.indices.extend(b.indices.iter().map(|i| base + i));
    // the edges: sides of exactly one triangle, each walked so its outside is on its left
    let mut sides: HashMap<(u32, u32), (u32, u32, u32, usize)> = HashMap::new();
    for tri in b.indices.chunks_exact(3) {
        for (a, c, o) in [(tri[0], tri[1], tri[2]), (tri[1], tri[2], tri[0]), (tri[2], tri[0], tri[1])] {
            sides.entry((a.min(c), a.max(c))).or_insert((a, c, o, 0)).3 += 1;
        }
    }
    let at = |i: u32| part.verts[(base + i) as usize].0;
    let n = b.vertices.len();
    let mut rim: Vec<[u32; 2]> = vec![];
    let mut touching: Vec<Vec<usize>> = vec![vec![]; n];
    let mut sums = vec![(egui::Vec2::ZERO, Vec::<egui::Vec2>::new()); n];
    for (a, c, o, count) in sides.into_values() {
        if count != 1 { continue; }
        let d = at(c) - at(a);
        let mut out = egui::vec2(-d.y, d.x).normalized();
        let (a, c) = if out.dot(at(o) - at(a)) > 0.0 { out = -out; (c, a) } else { (a, c) };
        if !out.is_finite() { continue; }
        for k in [a, c] {
            touching[k as usize].push(rim.len());
            sums[k as usize].0 += out;
            sums[k as usize].1.push(out);
        }
        rim.push([base + a, base + c]);
    }
    for (i, (sum, outs)) in sums.into_iter().enumerate() {
        let Some(d) = Some(sum.normalized()).filter(|d| d.is_finite() && !outs.is_empty()) else { continue };
        // corners step further, so the edges either side move a whole pixel (at most twice as far)
        let k = outs.iter().map(|m| d.dot(*m)).fold(1.0f32, f32::min).max(0.5);
        part.normals[base as usize + i] = d / k;
    }
    // each triangle's nearby edges: its own first (the pixels pushed out past them are its), then those at its
    // corners that step out, nearest first
    for tri in b.indices.chunks_exact(3) {
        let mid = (at(tri[0]).to_vec2() + at(tri[1]).to_vec2() + at(tri[2]).to_vec2()) / 3.0;
        let mut near: Vec<usize> = tri.iter().flat_map(|v| touching[*v as usize].iter().copied()).collect();
        near.sort_unstable();
        near.dedup();
        let dist = |e: usize| {
            let (p, q) = (part.verts[rim[e][0] as usize].0.to_vec2(), part.verts[rim[e][1] as usize].0.to_vec2());
            let t = ((mid - p).dot(q - p) / (q - p).length_sq().max(1e-9)).clamp(0.0, 1.0);
            (mid - (p + (q - p) * t)).length()
        };
        let own = |e: usize| tri.contains(&(rim[e][0] - base)) && tri.contains(&(rim[e][1] - base));
        near.sort_by(|x, y| own(*y).cmp(&own(*x)).then(dist(*x).total_cmp(&dist(*y))));
        let mut slots = [None; 6];
        for (k, e) in near.into_iter().take(6).enumerate() { slots[k] = Some(rim[e]); }
        part.edges.push(slots);
    }
}

/// Draw a part: each design point placed in the window by `place`, its colours through `colour` (a tint, a fade).
pub fn paint(p: &egui::Painter, part: &Part, place: impl Fn(Pos2) -> Pos2, colour: impl Fn(Color32) -> Color32) {
    let ctx = p.ctx();
    super::vector::paint(p, vertices(part, place, colour, ctx.content_rect(), ctx.pixels_per_point()));
}

/// A part's vertices for the vector shader, in a window `screen` (points) of `ppp` pixels per point.
fn vertices(part: &Part, place: impl Fn(Pos2) -> Pos2, colour: impl Fn(Color32) -> Color32, screen: Rect, ppp: f32) -> Vec<super::vector::Vertex> {
    let size = (screen.size() * ppp).round();
    let px: Vec<Pos2> = part.verts.iter().map(|(v, _)| ((place(*v) - screen.min) * ppp).to_pos2()).collect();
    // the outward directions as placed (so a mirrored or stretched part still knows its outside)
    let out_dir: Vec<egui::Vec2> = part.verts.iter().zip(&part.normals).zip(&px).map(|(((v, _), n), q)| {
        if *n == egui::Vec2::ZERO { return egui::Vec2::ZERO; }
        let d = (((place(*v + *n * 0.01) - screen.min) * ppp).to_pos2() - *q).normalized();
        if d.is_finite() { d * n.length() } else { egui::Vec2::ZERO }
    }).collect();
    let angle = |i: u32| { let d = out_dir[i as usize]; d.y.atan2(d.x) };
    let mut out = Vec::with_capacity(part.indices.len());
    for (t, tri) in part.indices.chunks_exact(3).enumerate() {
        let mut edges = [[0.0f32; 6]; 6];
        for (k, e) in part.edges[t].iter().enumerate() {
            let Some([a, b]) = e else { continue };
            let (pa, pb) = (px[*a as usize], px[*b as usize]);
            edges[k] = [pa.x, pa.y, pb.x, pb.y, angle(*a), angle(*b)];
        }
        for &i in tri {
            let q = px[i as usize] + out_dir[i as usize];
            out.push(super::vector::Vertex { pos: [q.x / size.x * 2.0 - 1.0, 1.0 - q.y / size.y * 2.0], color: colour(part.verts[i as usize].1).to_array(), edges });
        }
    }
    out
}

/// How the design's units land in the window: scaled to fit, with the extra width and height going to the
/// stretching runs.
#[derive(Clone, Copy)]
pub struct Map { pub window: Rect, pub s: f32, ex: f32, ey: f32 }

/// How a part follows the window across (or down): stretched with the design, or kept whole and moved with the
/// design at one of its own points (a pill pinned by its outer end, say).
#[derive(Clone, Copy)]
pub enum Fit { Stretch, Pin(f32) }

impl Map {
    pub fn new(window: Rect) -> Self {
        let s = (window.width() / ART.width()).min(window.height() / ART.height());
        Map { window, s, ex: (window.width() - ART.width() * s).max(0.0) / 2.0, ey: (window.height() - ART.height() * s).max(0.0) }
    }
    fn gx(&self, x: f32) -> f32 { STRETCH_X.iter().map(|(a, b)| self.ex * ((x - a) / (b - a)).clamp(0.0, 1.0)).sum() }
    fn gy(&self, y: f32) -> f32 { self.ey * ((y - STRETCH_Y.0) / (STRETCH_Y.1 - STRETCH_Y.0)).clamp(0.0, 1.0) }
    pub fn x(&self, x: f32, fit: Fit) -> f32 { self.window.left() + (x - ART.left()) * self.s + self.gx(match fit { Fit::Stretch => x, Fit::Pin(at) => at }) }
    pub fn y(&self, y: f32, fit: Fit) -> f32 { self.window.top() + (y - ART.top()) * self.s + self.gy(match fit { Fit::Stretch => y, Fit::Pin(at) => at }) }
    pub fn pos(&self, p: Pos2, fx: Fit, fy: Fit) -> Pos2 { pos2(self.x(p.x, fx), self.y(p.y, fy)) }
    pub fn rect(&self, r: Rect, fx: Fit, fy: Fit) -> Rect { Rect::from_min_max(self.pos(r.min, fx, fy), self.pos(r.max, fx, fy)) }
    /// A length in the design's units.
    pub fn len(&self, l: f32) -> f32 { l * self.s }
}

/// How the pills and the Assets card follow a wider window: kept whole, pinned at their outer ends.
pub const LEFT: Fit = Fit::Pin(56.0);
pub const RIGHT: Fit = Fit::Pin(1214.0);
/// The bottom panels and drawers follow the window's bottom.
pub const BOTTOM: Fit = Fit::Pin(764.4);

/// Where the design's parts land in a window.
pub struct Layout {
    pub m: Map,
    /// The 3D view: its bounds (the camera's viewport) and its shape.
    pub view: Rect,
    pub outline: Vec<Pos2>,
    pub left_pill: Rect,
    pub right_pill: Rect,
    pub assets: Rect,
    pub drop: Rect,
    /// The bottom panels, closed: the Level topic's settings, the inspector, the scene list.
    pub panels: [Rect; 3],
}

pub fn layout(window: Rect) -> Layout {
    let m = Map::new(window);
    let a = art();
    let outline: Vec<Pos2> = a.view.iter().map(|p| m.pos(*p, Fit::Stretch, Fit::Stretch)).collect();
    Layout {
        view: Rect::from_points(&outline),
        outline,
        left_pill: bounds("pill-left", &m, LEFT, Fit::Stretch),
        right_pill: bounds("pill-right", &m, RIGHT, Fit::Stretch),
        assets: bounds("assets-card", &m, RIGHT, Fit::Stretch),
        drop: bounds("drop-files", &m, RIGHT, Fit::Stretch),
        panels: ["panel-mode", "panel-inspector", "panel-scene"].map(|id| bounds(id, &m, Fit::Stretch, BOTTOM)),
        m,
    }
}

/// Whether `p` is inside the outline.
pub fn inside(outline: &[Pos2], p: Pos2) -> bool {
    let mut on = false;
    let n = outline.len();
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) { on = !on; }
    }
    on
}

/// The part's bounds in the window.
pub fn bounds(id: &str, m: &Map, fx: Fit, fy: Fit) -> Rect { m.rect(art().part(id).bounds, fx, fy) }

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_egui::egui::vec2;

    #[test]
    fn the_design_reads_into_its_parts_and_fits_any_window() {
        let a = art();
        for id in ["frame", "logo", "pill-left", "pill-right", "tab-band", "assets-card", "drop-files", "panel-mode", "panel-inspector", "panel-scene", "drawer-mode", "drawer-inspector", "drawer-scene", "grip", "toggle", "undo-icon", "menu-level", "menu-effects"] {
            assert!(!a.part(id).indices.is_empty(), "{id}");
        }
        // the view: the frame's hole, from its rounded top left to the inspector's step
        let v = Rect::from_points(&a.view);
        assert!((v.left() - 46.2).abs() < 0.2 && (v.top() - 51.9).abs() < 0.2 && (v.bottom() - 668.1).abs() < 0.3, "{v:?}");
        assert!(inside(&a.view, v.center()) && !inside(&a.view, pos2(635.0, 60.0)), "the tabs' notch is frame");
        for (w, h) in [(1600.0, 960.0), (2560.0, 1080.0), (1300.0, 1300.0)] {
            let m = Map::new(Rect::from_min_size(Pos2::ZERO, vec2(w, h)));
            // the design's corners reach the window's
            assert!(m.pos(ART.min, Fit::Stretch, Fit::Stretch).distance(Pos2::ZERO) < 0.01);
            assert!(m.pos(ART.max, Fit::Stretch, Fit::Stretch).distance(pos2(w, h)) < 0.01, "{w}×{h}");
            // every part's vertices land in or just around the window
            let win = Rect::from_min_size(Pos2::ZERO, vec2(w, h));
            for id in ["frame", "logo", "pill-left", "assets-card", "drop-files", "undo-icon", "panel-mode"] {
                let v = vertices(a.part(id), |p| m.pos(p, Fit::Stretch, Fit::Stretch), |c| c, win, 1.0);
                assert!(v.iter().all(|v| v.pos.iter().all(|x| x.is_finite() && x.abs() < 1.1)), "{id}: {:?}", v.iter().map(|v| v.pos).find(|p| !(p[0].abs() < 1.1 && p[1].abs() < 1.1)));
                assert!(v.iter().all(|v| v.edges.iter().flatten().all(|x| x.is_finite())), "{id}: edges");
            }
            // a pinned part keeps its size
            let r = bounds("assets-card", &m, Fit::Pin(1214.0), Fit::Pin(100.0));
            assert!((r.width() - 234.0 * m.s).abs() < 0.5);
        }
    }
}
