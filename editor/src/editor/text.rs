//! All the editor's text as vector letters (no rasters in the GUI): after egui lays a frame out, each piece of text
//! it would draw from its letter atlas is drawn instead from the letters' outlines in the font files (read with
//! skrifa, tessellated once per letter, kept on the GPU) at the very places egui laid them out, with the same exact
//! edges as the design's parts (vector.rs). Backgrounds and underlines of text stay egui's own shapes.
use super::vector::{GlyphBatch, Instance};
use bevy::prelude::*;
use bevy_egui::egui::emath::GuiRounding;
use bevy_egui::egui::epaint::{ClippedShape, Mesh, Shape, TextShape, Vertex, WHITE_UV};
use bevy_egui::egui::emath::Rot2;
use bevy_egui::egui::epaint::text::ByteIndex;
use bevy_egui::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontTweak, Pos2, Rect, vec2};
use bevy_egui::{EguiContext, EguiFullOutput, EguiPostUpdateSet};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::TableProvider;
use skrifa::{FontRef, MetadataProvider};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

pub struct TextPlugin;

impl Plugin for TextPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Letters>()
            .add_systems(PostUpdate, vector_text.after(EguiPostUpdateSet::EndPass).before(EguiPostUpdateSet::ProcessOutput));
    }
}

/// The fonts egui was given (theme.rs sets them).
static FONTS: OnceLock<Faces> = OnceLock::new();

pub fn set_fonts(defs: &FontDefinitions) { let _ = FONTS.set(Faces::new(defs)); }

struct Face { font: FontRef<'static>, tweak: FontTweak, upem: f32 }

struct Faces { faces: Vec<Face>, families: HashMap<FontFamily, Vec<usize>> }

impl Faces {
    fn new(defs: &FontDefinitions) -> Self {
        let mut faces = vec![];
        let mut index = HashMap::new();
        for (name, data) in &defs.font_data {
            let data: &FontData = data;
            // the font files live as long as the editor
            let bytes: &'static [u8] = match &data.font {
                std::borrow::Cow::Borrowed(b) => b,
                std::borrow::Cow::Owned(v) => Box::leak(v.clone().into_boxed_slice()),
            };
            let Ok(font) = FontRef::from_index(bytes, data.index) else { continue };
            let upem = font.head().map(|h| h.units_per_em() as f32).unwrap_or(1000.0);
            index.insert(name.clone(), faces.len());
            faces.push(Face { font, tweak: data.tweak.clone(), upem });
        }
        let families = defs.families.iter().map(|(f, names)| (f.clone(), names.iter().filter_map(|n| index.get(n).copied()).collect())).collect();
        Faces { faces, families }
    }

    /// The face that draws `c` in `family` (the first that has it, as egui chooses), and the letter's id there.
    fn find(&self, family: &FontFamily, c: char) -> Option<(usize, u32)> {
        let list = self.families.get(family)?;
        for &f in list {
            if let Some(g) = self.faces[f].font.charmap().map(c) { if g.to_u32() != 0 { return Some((f, g.to_u32())); } }
        }
        None
    }
}

/// Letters' shapes, tessellated once each (in ems, y down), all in one list the GPU keeps.
#[derive(Resource, Default)]
pub struct Letters {
    at: HashMap<(usize, u32), Option<(u32, u32)>>,
    pub store: Arc<(Vec<super::vector::LetterVertex>, Vec<[f32; 4]>)>,
    pub version: u64,
}

impl Letters {
    /// Where letter `g` of face `f` is in the store (first vertex, count); None if it has no outline.
    fn get(&mut self, faces: &Faces, f: usize, g: u32) -> Option<(u32, u32)> {
        if let Some(r) = self.at.get(&(f, g)) { return *r; }
        let face = &faces.faces[f];
        let mut pen = Pen { b: super::frame::LPath::builder(), k: 1.0 / face.upem, open: false };
        let r = face.font.outline_glyphs().get(skrifa::GlyphId::new(g)).and_then(|o| {
            o.draw(DrawSettings::unhinted(Size::unscaled(), LocationRef::default()), &mut pen).ok()?;
            if pen.open { pen.b.end(true); }
            let store = Arc::make_mut(&mut self.store);
            let (verts, edges) = super::frame::unplaced(&super::frame::filled(&pen.b.build()), store.1.len() as u32);
            if verts.is_empty() { return None; }
            let start = store.0.len() as u32;
            store.0.extend(verts);
            store.1.extend(edges);
            self.version += 1;
            Some((start, store.0.len() as u32 - start))
        });
        self.at.insert((f, g), r);
        r
    }
}

/// A letter's outline into lyon, in ems with y down.
struct Pen { b: lyon_tessellation::path::path::Builder, k: f32, open: bool }

impl Pen {
    fn p(&self, x: f32, y: f32) -> lyon_tessellation::math::Point { lyon_tessellation::math::point(x * self.k, -y * self.k) }
}

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.open { self.b.end(true); }
        self.b.begin(self.p(x, y));
        self.open = true;
    }
    fn line_to(&mut self, x: f32, y: f32) { self.b.line_to(self.p(x, y)); }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) { self.b.quadratic_bezier_to(self.p(cx, cy), self.p(x, y)); }
    fn curve_to(&mut self, c0x: f32, c0y: f32, c1x: f32, c1y: f32, x: f32, y: f32) { self.b.cubic_bezier_to(self.p(c0x, c0y), self.p(c1x, c1y), self.p(x, y)); }
    fn close(&mut self) {
        if self.open { self.b.end(true); }
        self.open = false;
    }
}

/// Every text shape of the frame becomes vector letters (runs of text under one clip share one draw).
fn vector_text(mut q: Query<(&mut EguiContext, &mut EguiFullOutput)>, mut letters: ResMut<Letters>) {
    let Some(faces) = FONTS.get() else { return };
    for (mut ctx, mut out) in &mut q {
        let screen = ctx.get_mut().content_rect();
        let Some(full) = out.0.as_mut() else { continue };
        let ppp = full.pixels_per_point;
        let mut shapes = Vec::with_capacity(full.shapes.len());
        let mut run: Option<(Rect, Vec<((u32, u32), Instance)>)> = None;
        let flush = |shapes: &mut Vec<ClippedShape>, run: &mut Option<(Rect, Vec<((u32, u32), Instance)>)>, letters: &Letters| {
            if let Some((clip, list)) = run.take() {
                if list.is_empty() { return; }
                let batch = GlyphBatch::new(list, letters.store.clone(), letters.version);
                shapes.push(ClippedShape { clip_rect: clip, shape: Shape::Callback(bevy_egui::render::EguiBevyPaintCallback::new_paint_callback(screen, batch)) });
            }
        };
        for ClippedShape { clip_rect, shape } in std::mem::take(&mut full.shapes) {
            let mut flat = vec![];
            flatten(shape, &mut flat);
            for s in flat {
                match s {
                    Shape::Text(t) => {
                        if run.as_ref().is_some_and(|(c, _)| *c != clip_rect) { flush(&mut shapes, &mut run, &letters); }
                        // its backgrounds and underline stay egui's (drawn first)
                        let extras = decorations(&t, ppp);
                        if !extras.is_empty() {
                            flush(&mut shapes, &mut run, &letters);
                            shapes.extend(extras.into_iter().map(|shape| ClippedShape { clip_rect, shape }));
                        }
                        let list = &mut run.get_or_insert_with(|| (clip_rect, vec![])).1;
                        letters_of(&t, ppp, screen, faces, &mut letters, list);
                    }
                    other => {
                        flush(&mut shapes, &mut run, &letters);
                        shapes.push(ClippedShape { clip_rect, shape: other });
                    }
                }
            }
        }
        flush(&mut shapes, &mut run, &letters);
        full.shapes = shapes;
    }
}

fn flatten(s: Shape, out: &mut Vec<Shape>) {
    match s {
        Shape::Vec(v) => { for s in v { flatten(s, out); } }
        Shape::Noop => {}
        s => out.push(s),
    }
}

/// Where the text's galley lands (as egui's tessellator puts it: on a whole pixel).
fn galley_pos(t: &TextShape, ppp: f32) -> Pos2 { t.pos.round_to_pixels(ppp) }

/// A text's letters: each one's place in the store and where it goes.
fn letters_of(t: &TextShape, ppp: f32, screen: Rect, faces: &Faces, letters: &mut Letters, out: &mut Vec<((u32, u32), Instance)>) {
    if t.opacity_factor <= 0.0 || t.galley.is_empty() { return; }
    let job = &t.galley.job;
    let rot = Rot2::from_angle(t.angle);
    let base = galley_pos(t, ppp);
    // which section each letter is from: walk the text alongside the letters
    let mut chars = job.text.char_indices().peekable();
    for row in &t.galley.rows {
        let row_pos = base + rot * row.pos.to_vec2();
        for g in &row.glyphs {
            let mut byte = None;
            while let Some((i, c)) = chars.next() {
                if c == g.chr { byte = Some(i); break; }
            }
            let section = byte.and_then(|b| job.sections.iter().find(|s| s.byte_range.contains(&ByteIndex(b)))).or(job.sections.last());
            let Some(section) = section else { continue };
            // a letter egui would draw (it has a place in the atlas)
            if g.uv_rect.is_nothing() { continue; }
            let font = &section.format.font_id;
            let Some((f, id)) = faces.find(&font.family, g.chr) else { continue };
            let Some(range) = letters.get(faces, f, id) else { continue };
            let tweak = &faces.faces[f].tweak;
            let y_off = (font.size * tweak.scale * tweak.y_offset_factor + tweak.y_offset).round_ui();
            // its colour as egui's tessellator works it out
            let mut colour = row.visuals.mesh.vertices.get(g.first_vertex as usize).map_or(section.format.color, |v| v.color);
            if let Some(c) = t.override_text_color { colour = c; } else if colour == Color32::PLACEHOLDER { colour = t.fallback_color; }
            if t.opacity_factor < 1.0 { colour = colour.gamma_multiply(t.opacity_factor); }
            let at = row_pos + rot * (g.pos.to_vec2() + vec2(0.0, y_off));
            let px = (at - screen.min) * ppp;
            out.push((range, Instance {
                place: [px.x, px.y, font.size * tweak.scale * ppp, t.angle],
                screen: [(screen.width() * ppp).round(), (screen.height() * ppp).round()],
                color: colour.to_array(),
            }));
        }
    }
}

/// A text's backgrounds, strike-throughs and underline: what egui's tessellator draws besides the letters.
fn decorations(t: &TextShape, ppp: f32) -> Vec<Shape> {
    let mut out = vec![];
    let rot = Rot2::from_angle(t.angle);
    let base = galley_pos(t, ppp);
    for row in &t.galley.rows {
        let v = &row.visuals;
        let row_pos = base + rot * row.pos.to_vec2();
        let glyphs = v.glyph_vertex_range.clone();
        let mut mesh = Mesh::default();
        for tri in v.mesh.indices.chunks_exact(3) {
            if tri.iter().any(|i| glyphs.contains(&(*i as usize))) { continue; }
            for &i in tri {
                let src = v.mesh.vertices[i as usize];
                let mut colour = src.color;
                if colour == Color32::PLACEHOLDER { colour = t.fallback_color; }
                if t.opacity_factor < 1.0 { colour = colour.gamma_multiply(t.opacity_factor); }
                mesh.indices.push(mesh.vertices.len() as u32);
                mesh.vertices.push(Vertex { pos: row_pos + rot * src.pos.to_vec2(), uv: WHITE_UV, color: colour });
            }
        }
        if !mesh.is_empty() { out.push(Shape::mesh(mesh)); }
        if t.underline != egui::Stroke::NONE {
            let r = Rect::from_min_size(row_pos, row.size);
            out.push(Shape::line_segment([r.left_bottom(), r.right_bottom()], t.underline));
        }
    }
    out
}
