//! The editor's frame and panels, drawn from the user's design: `assets/ui/editor.svg` (their docs/ui.svg cut into
//! parts by id, every shape, colour and coordinate kept). Each part is tessellated once (lyon) and drawn as a mesh.
//! The design is scaled to fit the window; a wider or taller window stretches only a few straight runs between the
//! corners and notches, so every curve, angle and the logo stay as drawn.
use bevy_egui::egui::{self, Color32, Mesh, Pos2, Rect, pos2};
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

/// One part of the design: triangles in its own units, coloured.
pub struct Part {
    pub verts: Vec<(Pos2, Color32)>,
    pub indices: Vec<u32>,
    pub bounds: Rect,
    /// The shapes' edges (vertex pairs) and each edge vertex's outward direction: drawn with a pixel-wide fade
    /// across them, so edges are smooth (as egui smooths its own shapes).
    rim: Vec<[u32; 2]>,
    normals: Vec<egui::Vec2>,
    /// Vertices on the edge of a filled shape (their fade is centred on the edge; a stroke's goes outward only, as
    /// strokes can be thinner than the fade).
    filled: Vec<bool>,
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
            let mut part = Part { verts: vec![], indices: vec![], bounds: Rect::NOTHING, rim: vec![], normals: vec![], filled: vec![] };
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
                    push(part, b, t, f.paint(), f.opacity().get() * opacity, true);
                }
                if let Some(s) = p.stroke() {
                    let mut b: VertexBuffers<Pos2, u32> = VertexBuffers::new();
                    StrokeTessellator::new()
                        .tessellate_path(&path, &StrokeOptions::tolerance(0.02).with_line_width(s.width().get()), &mut BuffersBuilder::new(&mut b, |v: StrokeVertex| pos2(v.position().x, v.position().y)))
                        .expect("the design's strokes tessellate");
                    push(part, b, t, s.paint(), s.opacity().get() * opacity, false);
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
fn push(part: &mut Part, b: VertexBuffers<Pos2, u32>, t: Transform, paint: &usvg::Paint, opacity: f32, fill: bool) {
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
        part.filled.push(fill);
    }
    part.indices.extend(b.indices.iter().map(|i| base + i));
    // the edges: sides of exactly one triangle, each with its outward normal (away from that triangle's third corner)
    let mut sides: HashMap<(u32, u32), (u32, u32, u32, usize)> = HashMap::new();
    for tri in b.indices.chunks_exact(3) {
        for (a, c, o) in [(tri[0], tri[1], tri[2]), (tri[1], tri[2], tri[0]), (tri[2], tri[0], tri[1])] {
            sides.entry((a.min(c), a.max(c))).or_insert((a, c, o, 0)).3 += 1;
        }
    }
    let at = |i: u32| part.verts[(base + i) as usize].0;
    let mut sums = vec![(egui::Vec2::ZERO, Vec::<egui::Vec2>::new()); b.vertices.len()];
    for (a, c, o, n) in sides.into_values() {
        if n != 1 { continue; }
        let d = at(c) - at(a);
        let mut nrm = egui::vec2(-d.y, d.x).normalized();
        if nrm.dot(at(o) - at(a)) > 0.0 { nrm = -nrm; }
        if !nrm.is_finite() { continue; }
        part.rim.push([base + a, base + c]);
        for k in [a, c] { sums[k as usize].0 += nrm; sums[k as usize].1.push(nrm); }
    }
    for (i, (sum, ns)) in sums.into_iter().enumerate() {
        let Some(n) = Some(sum.normalized()).filter(|n| n.is_finite() && !ns.is_empty()) else { continue };
        // corners: longer, so the fade keeps its width along both sides (up to twice)
        let k = ns.iter().map(|m| n.dot(*m)).fold(1.0f32, f32::min).max(0.5);
        part.normals[base as usize + i] = n / k;
    }
}

/// The pixel-wide fade along a part's edges, added to a mesh placed from it (its vertices in order).
fn feather(part: &Part, out: &mut Mesh) {
    let edge: Vec<Pos2> = out.vertices.iter().map(|v| v.pos).collect();
    // a filled shape's edge vertices step half a pixel in and the fade runs from there to half a pixel out; small
    // parts (an icon's thin strokes) keep their full width and fade outward only
    let centred = |i: usize| part.filled[i] && part.bounds.width().min(part.bounds.height()) > 12.0;
    for (i, v) in out.vertices.iter_mut().enumerate() {
        if centred(i) { v.pos -= part.normals[i] * 0.5; }
    }
    for [a, b] in &part.rim {
        let out_of = |i: u32| edge[i as usize] + part.normals[i as usize] * if centred(i as usize) { 0.5 } else { 1.0 };
        let n = out.vertices.len() as u32;
        out.colored_vertex(out_of(*a), Color32::TRANSPARENT);
        out.colored_vertex(out_of(*b), Color32::TRANSPARENT);
        out.add_triangle(*a, *b, n + 1);
        out.add_triangle(*a, n + 1, n);
    }
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

/// A part as a mesh in the window, moved by `shift` (window pixels) after placing.
pub fn mesh(part: &Part, m: &Map, fx: Fit, fy: Fit, shift: egui::Vec2, tint: Option<Color32>) -> Mesh {
    let mut out = Mesh::default();
    for (p, c) in &part.verts {
        let c = tint.map_or(*c, |t| Color32::from_rgba_unmultiplied(t.r(), t.g(), t.b(), ((c.a() as f32) * (t.a() as f32) / 255.0) as u8));
        out.colored_vertex(m.pos(*p, fx, fy) + shift, c);
    }
    out.indices = part.indices.clone();
    feather(part, &mut out);
    out
}

/// A part's mesh with its points first moved in the design's units (`f`), then placed (for the drawers' heights).
pub fn mesh_with(part: &Part, m: &Map, fx: Fit, fy: Fit, f: impl Fn(Pos2) -> Pos2) -> Mesh {
    let mut out = Mesh::default();
    for (p, c) in &part.verts { out.colored_vertex(m.pos(f(*p), fx, fy), *c); }
    out.indices = part.indices.clone();
    feather(part, &mut out);
    out
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
            // a pinned part keeps its size
            let r = bounds("assets-card", &m, Fit::Pin(1214.0), Fit::Pin(100.0));
            assert!((r.width() - 234.0 * m.s).abs() < 0.5);
        }
    }
}
