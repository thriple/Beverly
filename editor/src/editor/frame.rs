//! The editor's frame and vector shapes, as the user's redesign draws them (docs/ui/HANDOFF.md, "The frame"): the
//! window with the 3D view set into it as one outline (rounded corners, a notch dipping in at the top for the tab's
//! topics and one rising at the bottom for the hints), and the icon set (docs/ui/icons, 24 x 24, 2 px round strokes).
//! Each shape is tessellated (lyon) and drawn by the vector renderer (vector.rs), which covers each pixel exactly.
use bevy_egui::egui::{self, Color32, Pos2, Rect, pos2};
pub(super) use lyon_tessellation::path::Path as LPath;
use lyon_tessellation::path::iterator::PathIterator;
use lyon_tessellation::{BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, LineCap, LineJoin, StrokeOptions, StrokeTessellator, StrokeVertex, VertexBuffers, math::point};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use usvg::tiny_skia_path::{PathSegment, Point, Transform};

/// The top bar's height, the rims along the sides and bottom, the view's corner radius, the notches' depth.
pub const TOP: f32 = 44.0;
pub const RIM: f32 = 8.0;
const VIEW_R: f32 = 14.0;
pub const NOTCH: f32 = 34.0;
/// Each notch's curved shoulder is this wide.
pub const SHOULDER: f32 = 44.0;

/// A shape: triangles in its own units, coloured, and its shapes' edges (each pixel's coverage is
/// worked out from them).
pub struct Part {
    pub verts: Vec<(Pos2, Color32)>,
    pub indices: Vec<u32>,
    pub bounds: Rect,
    /// Each edge vertex's outward direction (zero inside); the edge vertices step a pixel out when drawn, so the
    /// shader can fade the edge over the pixels on both sides of it.
    normals: Vec<egui::Vec2>,
    /// The shapes' edges (vertex pairs, each walked with its shape on the right), and per triangle which of them
    /// are its shape's (first, count).
    rims: Vec<[u32; 2]>,
    shape_of: Vec<[u32; 2]>,
}

impl Part {
    fn new() -> Part { Part { verts: vec![], indices: vec![], bounds: Rect::NOTHING, normals: vec![], rims: vec![], shape_of: vec![] } }
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

/// Add tessellated triangles (in the path's own space, placed by `t`) to a part, coloured by `colour`.
fn push(part: &mut Part, b: VertexBuffers<Pos2, u32>, t: Transform, colour: impl Fn(Pos2) -> Color32) {
    let base = part.verts.len() as u32;
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
    let mut sums = vec![(egui::Vec2::ZERO, Vec::<egui::Vec2>::new()); n];
    for (a, c, o, count) in sides.into_values() {
        if count != 1 { continue; }
        let d = at(c) - at(a);
        let mut out = egui::vec2(-d.y, d.x).normalized();
        let (a, c) = if out.dot(at(o) - at(a)) > 0.0 { out = -out; (c, a) } else { (a, c) };
        if !out.is_finite() { continue; }
        for k in [a, c] {
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
    // every triangle of this shape uses all its edges
    let range = [part.rims.len() as u32, rim.len() as u32];
    part.rims.extend(rim);
    part.shape_of.extend(std::iter::repeat_n(range, b.indices.len() / 3));
}

/// A filled outline (a letter's) as a white part, in the outline's own units.
pub(super) fn filled(path: &LPath) -> Part {
    let mut part = Part::new();
    let mut b: VertexBuffers<Pos2, u32> = VertexBuffers::new();
    if FillTessellator::new().tessellate_path(path, &FillOptions::tolerance(0.0005), &mut BuffersBuilder::new(&mut b, |v: FillVertex| pos2(v.position().x, v.position().y))).is_ok() {
        push(&mut part, b, Transform::identity(), |_| Color32::WHITE);
    }
    part
}

/// A part's triangles unplaced, for the GPU to place (letters): per corner its point and outward step, and which
/// edges are its shape's (counted from `first`); and the edges (ends), in the part's own units.
pub(super) fn unplaced(part: &Part, first: u32) -> (Vec<super::vector::LetterVertex>, Vec<[f32; 4]>) {
    let mut verts = Vec::with_capacity(part.indices.len());
    for (t, tri) in part.indices.chunks_exact(3).enumerate() {
        let r = part.shape_of[t];
        for &i in tri {
            let (p, n) = (part.verts[i as usize].0, part.normals[i as usize]);
            verts.push(super::vector::LetterVertex { pn: [p.x, p.y, n.x, n.y], range: [first + r[0], r[1]] });
        }
    }
    let edges = part.rims.iter().map(|[a, b]| { let (p, q) = (part.verts[*a as usize].0, part.verts[*b as usize].0); [p.x, p.y, q.x, q.y] }).collect();
    (verts, edges)
}

/// Draw a part: each design point placed in the window by `place`, its colours through `colour` (a tint, a fade).
pub fn paint(p: &egui::Painter, part: &Part, place: impl Fn(Pos2) -> Pos2, colour: impl Fn(Color32) -> Color32) {
    let ctx = p.ctx();
    let (verts, edges) = vertices(part, place, colour, ctx.content_rect(), ctx.pixels_per_point());
    super::vector::paint(p, verts, edges);
}

/// A part's vertices and edges for the vector shader, in a window `screen` (points) of `ppp` pixels per point.
fn vertices(part: &Part, place: impl Fn(Pos2) -> Pos2, colour: impl Fn(Color32) -> Color32, screen: Rect, ppp: f32) -> (Vec<super::vector::Vertex>, Vec<[f32; 4]>) {
    let size = (screen.size() * ppp).round();
    let px: Vec<Pos2> = part.verts.iter().map(|(v, _)| ((place(*v) - screen.min) * ppp).to_pos2()).collect();
    // the outward steps as placed (so a mirrored or stretched part still steps outward)
    let step: Vec<egui::Vec2> = part.verts.iter().zip(&part.normals).zip(&px).map(|(((v, _), n), q)| {
        if *n == egui::Vec2::ZERO { return egui::Vec2::ZERO; }
        let d = (((place(*v + *n * 0.01) - screen.min) * ppp).to_pos2() - *q).normalized();
        if d.is_finite() { d * n.length() } else { egui::Vec2::ZERO }
    }).collect();
    let mut out = Vec::with_capacity(part.indices.len());
    for (t, tri) in part.indices.chunks_exact(3).enumerate() {
        for &i in tri {
            let q = px[i as usize] + step[i as usize];
            out.push(super::vector::Vertex { pos: [q.x / size.x * 2.0 - 1.0, 1.0 - q.y / size.y * 2.0], color: colour(part.verts[i as usize].1).to_array(), range: part.shape_of[t] });
        }
    }
    let edges = part.rims.iter().map(|[a, b]| { let (p, q) = (px[*a as usize], px[*b as usize]); [p.x, p.y, q.x, q.y] }).collect();
    (out, edges)
}

/// Where the frame puts things in a window.
pub struct Layout {
    pub window: Rect,
    /// The 3D view's bounds and its shape (the notches cut in).
    pub view: Rect,
    pub outline: Vec<Pos2>,
    path: LPath,
    /// The flat floors of the two notches (where the topics and the hints sit).
    pub top_notch: Rect,
    pub bottom_notch: Rect,
}

/// The frame in a window, with a top notch `nt` wide (0: none) and a bottom notch `nb` wide, shoulders included.
pub fn layout(window: Rect, nt: f32, nb: f32) -> Layout {
    let view = Rect::from_min_max(window.min + egui::vec2(RIM, TOP), window.max - egui::vec2(RIM, RIM));
    let (x0, y0, x1, y1, r, d) = (view.left(), view.top(), view.right(), view.bottom(), VIEW_R, NOTCH);
    let cx = view.center().x;
    let (a1, a2, b1, b2) = (cx - nt / 2.0, cx + nt / 2.0, cx - nb / 2.0, cx + nb / 2.0);
    let mut b = LPath::builder();
    let p = |x: f32, y: f32| point(x, y);
    // a quarter circle from the current point to `to` around the corner `c`
    let k = 0.552_284_8;
    let quarter = |b: &mut lyon_tessellation::path::path::Builder, from: (f32, f32), c: (f32, f32), to: (f32, f32)| {
        b.cubic_bezier_to(p(from.0 + (c.0 - from.0) * k, from.1 + (c.1 - from.1) * k), p(to.0 + (c.0 - to.0) * k, to.1 + (c.1 - to.1) * k), p(to.0, to.1));
    };
    b.begin(p(x0 + r, y0));
    if nt > 0.0 {
        b.line_to(p(a1, y0));
        b.cubic_bezier_to(p(a1 + 20.0, y0), p(a1 + 22.0, y0 + d), p(a1 + SHOULDER, y0 + d));
        b.line_to(p(a2 - SHOULDER, y0 + d));
        b.cubic_bezier_to(p(a2 - 22.0, y0 + d), p(a2 - 20.0, y0), p(a2, y0));
    }
    b.line_to(p(x1 - r, y0));
    quarter(&mut b, (x1 - r, y0), (x1, y0), (x1, y0 + r));
    b.line_to(p(x1, y1 - r));
    quarter(&mut b, (x1, y1 - r), (x1, y1), (x1 - r, y1));
    if nb > 0.0 {
        b.line_to(p(b2, y1));
        b.cubic_bezier_to(p(b2 - 20.0, y1), p(b2 - 22.0, y1 - d), p(b2 - SHOULDER, y1 - d));
        b.line_to(p(b1 + SHOULDER, y1 - d));
        b.cubic_bezier_to(p(b1 + 22.0, y1 - d), p(b1 + 20.0, y1), p(b1, y1));
    }
    b.line_to(p(x0 + r, y1));
    quarter(&mut b, (x0 + r, y1), (x0, y1), (x0, y1 - r));
    b.line_to(p(x0, y0 + r));
    quarter(&mut b, (x0, y0 + r), (x0, y0), (x0 + r, y0));
    b.end(true);
    let path = b.build();
    let outline = path.iter().flattened(0.05).filter_map(|e| match e {
        lyon_tessellation::path::PathEvent::Begin { at } => Some(pos2(at.x, at.y)),
        lyon_tessellation::path::PathEvent::Line { to, .. } => Some(pos2(to.x, to.y)),
        _ => None,
    }).collect();
    Layout {
        window,
        view,
        outline,
        path,
        top_notch: Rect::from_min_max(pos2(a1 + SHOULDER, y0), pos2(a2 - SHOULDER, y0 + d)),
        bottom_notch: Rect::from_min_max(pos2(b1 + SHOULDER, y1 - d), pos2(b2 - SHOULDER, y1)),
    }
}

/// The frame's colours: its fill (the tab's frame colour at its alpha) and the edge along the view.
#[derive(Clone, Copy, PartialEq)]
pub struct Look { pub fill: Color32, pub alpha: f32, pub edge: Color32 }

type Drawn = (u64, Arc<(Part, Part)>);

/// Draw the frame: the window minus the view, in the tab's colour with a white sheen over its top 64 px, and a
/// 1 px edge along the view. (Kept from frame to frame until the window, the notches or the colours change.)
pub fn paint_frame(p: &egui::Painter, lay: &Layout, look: Look) {
    static LAST: Mutex<Option<Drawn>> = Mutex::new(None);
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for v in [lay.window.min.x, lay.window.min.y, lay.window.max.x, lay.window.max.y, lay.top_notch.width(), lay.bottom_notch.width(), look.alpha] { v.to_bits().hash(&mut h); }
        (look.fill.to_array(), look.edge.to_array()).hash(&mut h);
        h.finish()
    };
    let parts = {
        let mut last = LAST.lock().expect("not poisoned");
        match last.as_ref() {
            Some((k, parts)) if *k == key => parts.clone(),
            _ => {
                let parts = Arc::new(frame_parts(lay, look));
                *last = Some((key, parts.clone()));
                parts
            }
        }
    };
    paint(p, &parts.0, |q| q, |c| c);
    paint(p, &parts.1, |q| q, |c| c);
}

fn frame_parts(lay: &Layout, look: Look) -> (Part, Part) {
    let w = lay.window;
    // the window (with corners where the sheen ends, so its fade is exact down the sides) and the view as a hole
    let sheen_end = w.top() + 64.0;
    let mut b = LPath::builder();
    b.begin(point(w.left(), w.top()));
    for (x, y) in [(w.right(), w.top()), (w.right(), sheen_end), (w.right(), w.bottom()), (w.left(), w.bottom()), (w.left(), sheen_end)] { b.line_to(point(x, y)); }
    b.end(true);
    b.extend_from_paths(&[lay.path.as_slice()]);
    let path = b.build();
    let mut fill = Part::new();
    let mut buf: VertexBuffers<Pos2, u32> = VertexBuffers::new();
    FillTessellator::new()
        .tessellate_path(&path, &FillOptions::tolerance(0.02).with_fill_rule(FillRule::EvenOdd), &mut BuffersBuilder::new(&mut buf, |v: FillVertex| pos2(v.position().x, v.position().y)))
        .expect("the frame tessellates");
    let [r, g, bl, _] = look.fill.to_array();
    push(&mut fill, buf, Transform::identity(), |q| {
        // white at 16% fading to nothing over the top 64 px, over the frame colour
        let s = 0.16 * (1.0 - ((q.y - w.top()) / 64.0).clamp(0.0, 1.0));
        let a = s + look.alpha * (1.0 - s);
        let mix = |c: u8| ((255.0 * s + c as f32 * look.alpha * (1.0 - s)) / a.max(1e-6)).round() as u8;
        Color32::from_rgba_unmultiplied(mix(r), mix(g), mix(bl), (a * 255.0).round() as u8)
    });
    let mut edge = Part::new();
    let mut buf: VertexBuffers<Pos2, u32> = VertexBuffers::new();
    StrokeTessellator::new()
        .tessellate_path(&lay.path, &StrokeOptions::tolerance(0.02).with_line_width(1.0), &mut BuffersBuilder::new(&mut buf, |v: StrokeVertex| pos2(v.position().x, v.position().y)))
        .expect("the view's edge tessellates");
    push(&mut edge, buf, Transform::identity(), |_| look.edge);
    (fill, edge)
}

/// The icon set (docs/ui/icons): each icon's 2 px round stroke on its 24 x 24 grid, white.
fn icons() -> &'static HashMap<String, Part> {
    static ICONS: OnceLock<HashMap<String, Part>> = OnceLock::new();
    ICONS.get_or_init(|| {
        #[derive(serde::Deserialize)]
        struct Set { icons: std::collections::BTreeMap<String, String> }
        let set: Set = serde_json::from_str(include_str!("../../assets/ui/icons.json")).expect("the icon set is valid");
        set.icons.into_iter().map(|(name, d)| {
            let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="{d}"/></svg>"#);
            let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("an icon is valid SVG");
            let mut part = Part::new();
            for node in tree.root().children() {
                let usvg::Node::Path(p) = node else { continue };
                let mut buf: VertexBuffers<Pos2, u32> = VertexBuffers::new();
                let opts = StrokeOptions::tolerance(0.01).with_line_width(2.0).with_line_cap(LineCap::Round).with_line_join(LineJoin::Round);
                StrokeTessellator::new()
                    .tessellate_path(&lyon_path(p.data()), &opts, &mut BuffersBuilder::new(&mut buf, |v: StrokeVertex| pos2(v.position().x, v.position().y)))
                    .expect("an icon tessellates");
                push(&mut part, buf, p.abs_transform(), |_| Color32::WHITE);
            }
            (name, part)
        }).collect()
    })
}

/// Draw an icon `size` points across, centred at `c`, in `colour`.
pub fn paint_icon(p: &egui::Painter, name: &str, c: Pos2, size: f32, colour: Color32) {
    let Some(part) = icons().get(name) else { return };
    let k = size / 24.0;
    paint(p, part, |q| c + (q - pos2(12.0, 12.0)) * k, |_| colour);
}

/// Fill a closed polygon (any shape, in window points) in `colour`.
pub fn paint_polygon(p: &egui::Painter, pts: &[Pos2], colour: Color32) {
    let mut b = LPath::builder();
    b.begin(point(pts[0].x, pts[0].y));
    for q in &pts[1..] { b.line_to(point(q.x, q.y)); }
    b.end(true);
    let mut part = Part::new();
    let mut buf: VertexBuffers<Pos2, u32> = VertexBuffers::new();
    if FillTessellator::new().tessellate_path(&b.build(), &FillOptions::tolerance(0.01), &mut BuffersBuilder::new(&mut buf, |v: FillVertex| pos2(v.position().x, v.position().y))).is_ok() {
        push(&mut part, buf, Transform::identity(), |_| colour);
        paint(p, &part, |q| q, |c| c);
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_egui::egui::vec2;

    #[test]
    fn the_frame_and_icons_draw_in_any_window() {
        for (w, h) in [(1600.0, 960.0), (2560.0, 1080.0), (900.0, 700.0)] {
            let win = Rect::from_min_size(Pos2::ZERO, vec2(w, h));
            let lay = layout(win, 500.0, 400.0);
            let c = lay.view.center();
            assert!(inside(&lay.outline, c) && inside(&lay.outline, pos2(lay.view.left() + 30.0, lay.view.top() + 5.0)));
            assert!(!inside(&lay.outline, pos2(c.x, lay.view.top() + 10.0)), "the topics' notch is frame");
            assert!(!inside(&lay.outline, pos2(c.x, lay.view.bottom() - 10.0)), "the hints' notch is frame");
            assert!((lay.top_notch.width() - (500.0 - 2.0 * SHOULDER)).abs() < 0.01 && (lay.top_notch.height() - NOTCH).abs() < 0.01);
            let (fill, edge) = frame_parts(&lay, Look { fill: Color32::BLACK, alpha: 0.78, edge: Color32::WHITE });
            for part in [&fill, &edge] {
                let (v, e) = vertices(part, |p| p, |c| c, win, 1.0);
                assert!(!v.is_empty() && v.iter().all(|v| v.pos.iter().all(|x| x.is_finite() && x.abs() < 1.1)));
                assert!(v.iter().all(|v| v.range[0] + v.range[1] <= e.len() as u32));
            }
        }
        for name in ["cube", "mountain", "undo", "redo", "grid", "minus", "square", "x", "check", "sliders", "layers", "folder"] {
            assert!(icons().get(name).is_some_and(|p| !p.indices.is_empty()), "{name}");
        }
    }
}
