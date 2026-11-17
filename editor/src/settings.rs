//! Settings panels from data. A building block describes each of its settings once, as an attribute on the field:
//!
//! ```ignore
//! #[derive(Reflect)]
//! #[reflect(Default)]
//! pub struct Mover {
//!     #[reflect(@Setting::number("Moves by", "m", 0.05, -1e5, 1e5).section("MOVES"))]
//!     pub to: [f32; 3],
//!     #[reflect(@Setting::feel("Speed", ("Slow", "Quick"), 20.0, 0.2, "s").log().section("MOVES"))]
//!     pub secs: f32,
//! }
//! ```
//!
//! `section` draws a section of any such value as a panel in the Motion kit style: word sliders (the numbers and
//! the slider's range ends under More), numbers, toggles, colours, choices, optional parts and lists. `describe`
//! lists the same settings with their values, for the MCP server (and later the timeline's keyable values), and
//! `apply` sets them from the same JSON.
//! Fields without a `Setting` are not shown. Range ends changed under More are kept per file in a `Ranges` map.
use crate::editor::theme::{self, Tip, HIGHLIGHT, LINE, MUTED, TEXT};
use bevy::reflect::{
    enums::{DynamicEnum, DynamicVariant}, std_traits::ReflectDefault, structs::Struct, tuple::DynamicTuple, PartialReflect,
    Reflect, ReflectMut, ReflectRef, TypeInfo, TypeRegistry,
};
use bevy_egui::egui::{self, vec2, Color32, CornerRadius, Sense, Stroke, Ui};
use std::collections::BTreeMap;

/// How one field shows in its panel.
#[derive(Reflect, Clone, Debug, PartialEq)]
pub struct Setting {
    pub label: &'static str,
    pub kind: Kind,
    /// Which section (a column of the panel) it goes in.
    pub section: &'static str,
    pub unit: &'static str,
    /// Shown when hovering.
    pub help: &'static str,
    /// The panel leaves the label out (its section title says it); descriptions still give it.
    pub bare: bool,
}

#[derive(Reflect, Clone, Debug, PartialEq)]
pub enum Kind {
    /// A word slider. Fields with the same `group` share one slider; `lo` is the value at the left word and `hi`
    /// at the right one (`hi` may be smaller), spaced evenly or, with `log`, by ratio.
    Feel { group: &'static str, ends: (&'static str, &'static str), lo: f32, hi: f32, log: bool },
    /// A number (or x, y, z for three of them) dragged or typed.
    Number { speed: f32, min: f32, max: f32 },
    Toggle,
    Colour,
    Text,
    /// A rotation (quaternion) shown as a turn in degrees, with Stand upright when tilted.
    Turn,
    /// An optional part (`Option<T>`): the words when off and on; turning it on starts from T's default.
    Optional { off: &'static str, on: &'static str },
    /// One of an enum's variants (labelled by a `Label` on each variant, else its name).
    Choice,
    /// A list (`Vec<T>`): each item's settings, with Remove and + Add.
    List,
    /// A set of an enum's unit variants (`Vec<E>`, labelled as for Choice): a chip for each, on or off.
    Tags,
}

/// A variant's name in a Choice.
#[derive(Reflect, Clone, Debug)]
pub struct Label(pub &'static str);

impl Setting {
    fn new(label: &'static str, kind: Kind) -> Self { Setting { label, kind, section: "", unit: "", help: "", bare: false } }
    pub fn feel(label: &'static str, ends: (&'static str, &'static str), lo: f32, hi: f32, unit: &'static str) -> Self {
        Setting { unit, ..Self::new(label, Kind::Feel { group: label, ends, lo, hi, log: false }) }
    }
    pub fn number(label: &'static str, unit: &'static str, speed: f32, min: f32, max: f32) -> Self {
        Setting { unit, ..Self::new(label, Kind::Number { speed, min, max }) }
    }
    pub fn toggle(label: &'static str) -> Self { Self::new(label, Kind::Toggle) }
    pub fn colour(label: &'static str) -> Self { Self::new(label, Kind::Colour) }
    pub fn text(label: &'static str) -> Self { Self::new(label, Kind::Text) }
    pub fn turn(label: &'static str) -> Self { Self::new(label, Kind::Turn) }
    pub fn optional(label: &'static str, off: &'static str, on: &'static str) -> Self { Self::new(label, Kind::Optional { off, on }) }
    pub fn choice(label: &'static str) -> Self { Self::new(label, Kind::Choice) }
    pub fn list(label: &'static str) -> Self { Self::new(label, Kind::List) }
    pub fn tags(label: &'static str) -> Self { Self::new(label, Kind::Tags) }
    pub fn section(mut self, s: &'static str) -> Self { self.section = s; self }
    pub fn help(mut self, h: &'static str) -> Self { self.help = h; self }
    pub fn bare(mut self) -> Self { self.bare = true; self }
    /// Space a Feel slider's values by ratio (for ranges like 0.2 s to 20 s).
    pub fn log(mut self) -> Self { if let Kind::Feel { log, .. } = &mut self.kind { *log = true; } self }
    /// Put a Feel field on another field's slider (by that slider's label).
    pub fn group(mut self, g: &'static str) -> Self { if let Kind::Feel { group, .. } = &mut self.kind { *group = g; } self }
}

/// Slider range ends changed under More, by "Type.field": (value at the left word, value at the right word).
pub type Ranges = BTreeMap<String, (f32, f32)>;

/// Where a Feel field's slider sits (0..1) for value `v` between `lo` and `hi`.
pub fn feel_t(v: f32, lo: f32, hi: f32, log: bool) -> f32 {
    let t = if log && lo > 0.0 && hi > 0.0 && v > 0.0 { (v / lo).ln() / (hi / lo).ln() } else { (v - lo) / (hi - lo) };
    if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 }
}

/// The value at slider position `t` (0..1).
pub fn feel_value(t: f32, lo: f32, hi: f32, log: bool) -> f32 {
    if log && lo > 0.0 && hi > 0.0 { lo * (hi / lo).powf(t) } else { lo + (hi - lo) * t }
}

/// Types whose defaults the panels need (for Optional and List), registered with their fields' types.
fn registry() -> &'static TypeRegistry {
    static R: std::sync::OnceLock<TypeRegistry> = std::sync::OnceLock::new();
    R.get_or_init(|| {
        let mut r = TypeRegistry::default();
        r.register::<crate::level::Part>();
        r
    })
}

fn default_of(id: std::any::TypeId) -> Option<Box<dyn Reflect>> {
    registry().get_type_data::<ReflectDefault>(id).map(|d| d.default())
}

/// Per-call changes to the descriptions, by field name (e.g. a model's Size shown as Scale ×).
pub struct Ctx<'a> {
    pub replace: &'a [(&'static str, Setting)],
    pub ranges: &'a mut Ranges,
    /// Set when a range end changed (it lives outside the value).
    pub ranges_changed: bool,
}

/// The sections of `value`'s settings, in field order.
pub fn sections(value: &dyn PartialReflect) -> Vec<&'static str> {
    let mut out = vec![];
    for (_, s) in fields(value) {
        if !out.contains(&s.section) { out.push(s.section); }
    }
    out
}

/// A struct's described fields: (field name, its setting).
fn fields(value: &dyn PartialReflect) -> Vec<(&'static str, Setting)> {
    let Some(TypeInfo::Struct(info)) = value.get_represented_type_info() else { return vec![] };
    info.iter().filter_map(|f| f.get_attribute::<Setting>().map(|s| (f.name(), s.clone()))).collect()
}

/// Draw the settings of `value` (a struct) that are in `section`. Returns true if anything changed.
pub fn section(ui: &mut Ui, value: &mut dyn PartialReflect, section: &str, ctx: &mut Ctx) -> bool {
    let ty = value.reflect_short_type_path().to_string();
    let mut list: Vec<(&'static str, Setting)> = fields(value).into_iter().filter(|(_, s)| s.section == section).collect();
    for (name, s) in &mut list {
        if let Some((_, r)) = ctx.replace.iter().find(|(n, _)| n == name) { *s = r.clone(); }
    }
    let ReflectMut::Struct(st) = value.reflect_mut() else { return false };
    let mut changed = false;
    let mut i = 0;
    let mut feels_done: Vec<&'static str> = vec![];
    while i < list.len() {
        let (name, s) = list[i].clone();
        match &s.kind {
            // small ones share a row
            Kind::Toggle | Kind::Colour => {
                ui.horizontal(|ui| {
                    while i < list.len() && matches!(list[i].1.kind, Kind::Toggle | Kind::Colour) {
                        let (n, s) = &list[i];
                        if let Some(f) = st.field_mut(n) { changed |= small(ui, f, s); }
                        ui.add_space(10.0);
                        i += 1;
                    }
                });
                continue;
            }
            Kind::Feel { group, ends, .. } => {
                if !feels_done.contains(group) {
                    feels_done.push(group);
                    let members: Vec<(&'static str, Setting)> = list.iter().filter(|(_, m)| matches!(&m.kind, Kind::Feel { group: g, .. } if g == group)).cloned().collect();
                    changed |= feel(ui, st, &ty, group, *ends, &members, ctx);
                }
            }
            _ => {
                if let Some(f) = st.field_mut(name) { changed |= one(ui, f, &s, &format!("{ty}.{name}"), ctx); }
            }
        }
        i += 1;
    }
    changed
}

fn tip(r: egui::Response, s: &Setting) -> egui::Response { if s.help.is_empty() { r } else { r.tip(s.help) } }

fn small(ui: &mut Ui, f: &mut dyn PartialReflect, s: &Setting) -> bool {
    ui.label(egui::RichText::new(s.label).color(MUTED));
    match s.kind {
        Kind::Toggle => f.try_downcast_mut::<bool>().is_some_and(|b| tip(theme::toggle(ui, b), s).changed()),
        // on a copy: the button nudges the numbers through its colour conversions even when nothing is picked
        _ => f.try_downcast_mut::<[f32; 3]>().is_some_and(|c| {
            let mut copy = *c;
            let picked = tip(ui.color_edit_button_rgb(&mut copy), s).changed();
            if picked { *c = copy; }
            picked
        }),
    }
}

/// One setting that isn't a slider or a small one.
fn one(ui: &mut Ui, f: &mut dyn PartialReflect, s: &Setting, key: &str, ctx: &mut Ctx) -> bool {
    match &s.kind {
        Kind::Number { speed, min, max } => {
            if let Some(v) = f.try_downcast_mut::<f32>() {
                return ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(s.label).color(MUTED));
                    tip(ui.add(egui::DragValue::new(v).speed(*speed).range(*min..=*max).max_decimals(2).suffix(unit(s))), s).changed()
                }).inner;
            }
            if let Some(v) = f.try_downcast_mut::<[f32; 3]>() {
                return ui.horizontal(|ui| {
                    if !s.bare { ui.label(egui::RichText::new(s.label).color(MUTED)); }
                    let mut c = false;
                    for (i, col) in [theme::AXIS_X, theme::AXIS_Y, theme::AXIS_Z].iter().enumerate() {
                        ui.label(egui::RichText::new(["X", "Y", "Z"][i]).font(theme::mono(11.0)).color(*col));
                        c |= ui.add(egui::DragValue::new(&mut v[i]).speed(*speed).range(*min..=*max).max_decimals(2).suffix(unit(s))).changed();
                    }
                    c
                }).inner;
            }
            false
        }
        Kind::Text => f.try_downcast_mut::<String>().is_some_and(|t| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(s.label).color(MUTED));
                tip(ui.add(egui::TextEdit::singleline(t).desired_width(150.0)), s).changed()
            }).inner
        }),
        Kind::Turn => f.try_downcast_mut::<[f32; 4]>().is_some_and(|r| turn(ui, r, s)),
        Kind::Choice => choice(ui, f, s, key),
        Kind::Optional { off, on } => optional(ui, f, s, off, on, key, ctx),
        Kind::List => list(ui, f, s, key, ctx),
        Kind::Tags => tags(ui, f, s),
        Kind::Toggle | Kind::Colour => small(ui, f, s),
        Kind::Feel { .. } => false,
    }
}

fn unit(s: &Setting) -> String { if s.unit.is_empty() { String::new() } else if s.unit == "×" || s.unit == "°" { s.unit.into() } else { format!(" {}", s.unit) } }

fn turn(ui: &mut Ui, r: &mut [f32; 4], s: &Setting) -> bool {
    use bevy::math::{EulerRot, Quat};
    let (yaw, pitch, roll) = Quat::from_array(*r).to_euler(EulerRot::YXZ);
    let mut deg = yaw.to_degrees();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(s.label).color(MUTED));
        let mut c = false;
        if tip(ui.add(egui::DragValue::new(&mut deg).speed(1.0).suffix("°")), s).changed() {
            *r = Quat::from_euler(EulerRot::YXZ, deg.to_radians(), pitch, roll).to_array();
            c = true;
        }
        if (pitch.abs() > 1e-3 || roll.abs() > 1e-3) && ui.small_button("Stand upright").clicked() {
            *r = Quat::from_rotation_y(yaw).to_array();
            c = true;
        }
        c
    }).inner
}

fn choice(ui: &mut Ui, f: &mut dyn PartialReflect, s: &Setting, key: &str) -> bool {
    let Some(TypeInfo::Enum(info)) = f.get_represented_type_info() else { return false };
    let ReflectRef::Enum(e) = f.reflect_ref() else { return false };
    let names: Vec<&'static str> = info.variant_names().to_vec();
    let labels: Vec<&str> = info.iter().map(|v| v.get_attribute::<Label>().map_or(v.name(), |l| l.0)).collect();
    let mut i = names.iter().position(|n| *n == e.variant_name()).unwrap_or(0);
    if !s.bare { ui.label(egui::RichText::new(s.label).color(MUTED)); }
    if theme::segmented(ui, key, &labels, &mut i, false) {
        f.apply(&DynamicEnum::new(names[i], DynamicVariant::Unit));
        return true;
    }
    false
}

/// A `Vec<E>`'s variants (E's unit variants): (name, label), and the names it holds.
fn tag_set(f: &dyn PartialReflect) -> Option<(Vec<(&'static str, &'static str)>, Vec<String>)> {
    let Some(TypeInfo::List(l)) = f.get_represented_type_info() else { return None };
    let Some(TypeInfo::Enum(info)) = l.item_info() else { return None };
    let all = info.iter().map(|v| (v.name(), v.get_attribute::<Label>().map_or(v.name(), |l| l.0))).collect();
    let ReflectRef::List(items) = f.reflect_ref() else { return None };
    let on = items.iter().filter_map(|i| match i.reflect_ref() { ReflectRef::Enum(e) => Some(e.variant_name().to_string()), _ => None }).collect();
    Some((all, on))
}

/// Make a `Vec<E>` hold `names` (variant names), in E's order.
fn set_tags(f: &mut dyn PartialReflect, all: &[(&'static str, &'static str)], names: &[&str]) {
    let ReflectMut::List(l) = f.reflect_mut() else { return };
    while l.len() > 0 { l.remove(l.len() - 1); }
    for (n, _) in all.iter().filter(|(n, _)| names.contains(n)) { l.push(Box::new(DynamicEnum::new(*n, DynamicVariant::Unit))); }
}

fn tags(ui: &mut Ui, f: &mut dyn PartialReflect, s: &Setting) -> bool {
    let Some((all, on)) = tag_set(f) else { return false };
    if !s.bare { ui.label(egui::RichText::new(s.label).color(MUTED)); }
    let mut flip = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for (n, label) in &all {
            let mut is = on.iter().any(|o| o == n);
            if tip(theme::chip(ui, label, &mut is), s).changed() { flip = Some(*n); }
        }
    });
    let Some(flip) = flip else { return false };
    let mut names: Vec<&str> = on.iter().map(String::as_str).filter(|n| *n != flip).collect();
    if !on.iter().any(|o| o == flip) { names.push(flip); }
    set_tags(f, &all, &names);
    true
}

fn optional(ui: &mut Ui, f: &mut dyn PartialReflect, s: &Setting, off: &str, on_word: &str, key: &str, ctx: &mut Ctx) -> bool {
    let Some(TypeInfo::Enum(info)) = f.get_represented_type_info() else { return false };
    let ReflectMut::Enum(e) = f.reflect_mut() else { return false };
    let mut on = e.variant_name() == "Some";
    let mut changed = false;
    ui.horizontal(|ui| {
        if tip(theme::toggle(ui, &mut on), s).changed() { changed = true; }
        ui.label(egui::RichText::new(if on { on_word } else { off }).color(MUTED));
    });
    if changed {
        let mut next = DynamicEnum::new("None", DynamicVariant::Unit);
        if on {
            let inner = info.variant("Some").and_then(|v| v.as_tuple_variant().ok()).and_then(|t| t.field_at(0)).and_then(|f| default_of(f.type_id()));
            let Some(inner) = inner else { return false };
            let mut t = DynamicTuple::default();
            t.insert_boxed(inner.into_partial_reflect());
            next = DynamicEnum::new("Some", DynamicVariant::Tuple(t));
        }
        e.apply(&next);
        return true;
    }
    if let Some(inner) = e.field_at_mut(0) {
        for sec in sections(inner) { changed |= section(ui, inner, sec, ctx); }
    }
    let _ = key;
    changed
}

fn list(ui: &mut Ui, f: &mut dyn PartialReflect, s: &Setting, key: &str, ctx: &mut Ctx) -> bool {
    let item_ty = match f.get_represented_type_info() { Some(TypeInfo::List(l)) => l.item_ty().id(), _ => return false };
    let ReflectMut::List(l) = f.reflect_mut() else { return false };
    let mut changed = false;
    let mut remove = None;
    ui.label(egui::RichText::new(s.label).color(MUTED));
    for i in 0..l.len() {
        let Some(item) = l.get_mut(i) else { continue };
        ui.push_id((key, i), |ui| {
            egui::Frame::new().stroke(Stroke::new(1.0, LINE)).corner_radius(CornerRadius { nw: 10, ne: 10, se: 10, sw: 3 }).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
                for sec in sections(item) { changed |= section(ui, item, sec, ctx); }
                if ui.small_button("Remove").clicked() { remove = Some(i); }
            });
        });
    }
    if let Some(i) = remove { l.remove(i); changed = true; }
    if ui.small_button("+ Add").clicked() {
        if let Some(d) = default_of(item_ty) { l.push(d.into_partial_reflect()); changed = true; }
    }
    changed
}

/// One word slider for every Feel field in `group`, with their numbers and range ends under More.
fn feel(ui: &mut Ui, st: &mut dyn Struct, ty: &str, group: &str, ends: (&str, &str), members: &[(&'static str, Setting)], ctx: &mut Ctx) -> bool {
    let range = |ctx: &Ctx, name: &str, s: &Setting| -> (f32, f32, bool) {
        let Kind::Feel { lo, hi, log, .. } = s.kind else { return (0.0, 1.0, false) };
        let (lo, hi) = ctx.ranges.get(&format!("{ty}.{name}")).copied().unwrap_or((lo, hi));
        (lo, hi, log)
    };
    let mut changed = false;
    let (n0, s0) = &members[0];
    let Some(v0) = st.field(n0).and_then(|f| f.try_downcast_ref::<f32>()).copied() else { return false };
    let (lo, hi, log) = range(ctx, n0, s0);
    let mut t = feel_t(v0, lo, hi, log);
    ui.label(egui::RichText::new(group).font(theme::semi(12.0)).color(TEXT));
    if tip(word_slider(ui, &mut t, ends), s0).changed() {
        for (n, s) in members {
            let (lo, hi, log) = range(ctx, n, s);
            if let Some(v) = st.field_mut(n).and_then(|f| f.try_downcast_mut::<f32>()) { *v = feel_value(t, lo, hi, log); }
        }
        changed = true;
    }
    let id = ui.id().with(("more", ty, group));
    let mut open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    if ui.add(egui::Label::new(egui::RichText::new(if open { "▾ More" } else { "▸ More" }).size(11.0).color(MUTED)).sense(Sense::click())).clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_temp(id, open));
    }
    if open {
        for (n, s) in members {
            let (lo, hi, log) = range(ctx, n, s);
            let u = unit(s);
            if let Some(v) = st.field_mut(n).and_then(|f| f.try_downcast_mut::<f32>()) {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(s.label).color(MUTED));
                    changed |= ui.add(egui::DragValue::new(v).speed(0.01).max_decimals(3).suffix(u.clone())).changed();
                });
            }
            ui.horizontal(|ui| {
                let (mut a, mut b) = (lo, hi);
                ui.label(egui::RichText::new(format!("{} end", ends.0)).size(11.0).color(MUTED));
                let ca = ui.add(egui::DragValue::new(&mut a).speed(0.01).max_decimals(3).suffix(u.clone())).changed();
                ui.label(egui::RichText::new(format!("{} end", ends.1)).size(11.0).color(MUTED));
                let cb = ui.add(egui::DragValue::new(&mut b).speed(0.01).max_decimals(3).suffix(u.clone())).changed();
                if (ca || cb) && a != b && (!log || (a > 0.0 && b > 0.0)) {
                    ctx.ranges.insert(format!("{ty}.{n}"), (a, b));
                    ctx.ranges_changed = true;
                }
            });
        }
    }
    changed
}

/// The kit's word slider: a track that fills from the left, a round handle, the two words under its ends.
pub fn word_slider(ui: &mut Ui, t: &mut f32, ends: (&str, &str)) -> egui::Response {
    let w = ui.available_width().min(240.0);
    let (rect, mut resp) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click_and_drag());
    let track = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 8.0), vec2(rect.width() - 14.0, 6.0));
    if let Some(p) = resp.interact_pointer_pos() {
        let nt = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
        if nt != *t { *t = nt; resp.mark_changed(); }
    }
    if resp.has_focus() {
        let d = ui.input(|i| (i.key_pressed(egui::Key::ArrowRight) as i32 - i.key_pressed(egui::Key::ArrowLeft) as i32) as f32 * 0.05);
        if d != 0.0 { *t = (*t + d).clamp(0.0, 1.0); resp.mark_changed(); }
    }
    let shown = theme::spring(ui.ctx(), resp.id.with("t"), *t, if resp.dragged() { theme::SNAPPY } else { theme::SPRING });
    let p = ui.painter();
    p.rect_filled(track, CornerRadius::same(3), Color32::from_white_alpha(28));
    let x = track.left() + track.width() * shown.clamp(0.0, 1.0);
    p.rect_filled(egui::Rect::from_min_max(track.min, egui::pos2(x, track.max.y)), CornerRadius::same(3), HIGHLIGHT);
    let hot = resp.hovered() || resp.dragged();
    p.circle_filled(egui::pos2(x, track.center().y), if hot { 7.5 } else { 6.5 }, TEXT);
    let font = theme::body(11.0);
    let y = rect.bottom() - 7.0;
    p.text(egui::pos2(rect.left(), y), egui::Align2::LEFT_CENTER, ends.0, font.clone(), if *t <= 0.5 { TEXT } else { MUTED });
    p.text(egui::pos2(rect.right(), y), egui::Align2::RIGHT_CENTER, ends.1, font, if *t > 0.5 { TEXT } else { MUTED });
    resp
}

/// An f32 as JSON without float noise (0.05, not 0.05000000074505806).
pub fn num(x: f32) -> serde_json::Value { x.to_string().parse::<f64>().map_or(serde_json::Value::Null, serde_json::Value::from) }

pub fn nums(x: &[f32]) -> serde_json::Value { serde_json::Value::Array(x.iter().map(|v| num(*v)).collect()) }

/// Every described setting of `value` with its current value: for the MCP server and anything else that needs to
/// know what can be set.
pub fn describe(value: &dyn PartialReflect) -> serde_json::Value {
    use serde_json::{json, Value};
    let ReflectRef::Struct(st) = value.reflect_ref() else { return Value::Null };
    let mut out = vec![];
    for (name, s) in fields(value) {
        let Some(f) = st.field(name) else { continue };
        let mut d_tags = None;
        let v: Value = if let Some(x) = f.try_downcast_ref::<f32>() { num(*x) }
            else if let Some(x) = f.try_downcast_ref::<bool>() { json!(x) }
            else if let Some(x) = f.try_downcast_ref::<String>() { json!(x) }
            else if let Some(x) = f.try_downcast_ref::<[f32; 3]>() { nums(x) }
            else if let Some(x) = f.try_downcast_ref::<[f32; 4]>() { nums(x) }
            else if let ReflectRef::Enum(e) = f.reflect_ref() {
                match (e.variant_name(), e.field_at(0)) {
                    ("Some", Some(inner)) => describe(inner),
                    ("None", None) => Value::Null,
                    (n, _) => json!(n),
                }
            } else if let (Kind::Tags, Some((all, on))) = (&s.kind, tag_set(f)) {
                d_tags = Some(all.iter().map(|(_, l)| *l).collect::<Vec<_>>());
                json!(all.iter().filter(|(n, _)| on.iter().any(|o| o == n)).map(|(_, l)| *l).collect::<Vec<_>>())
            } else if let ReflectRef::List(l) = f.reflect_ref() { Value::Array(l.iter().map(describe).collect()) }
            else { Value::Null };
        let mut d = json!({ "field": name, "label": s.label, "section": s.section, "value": v });
        let kind = match &s.kind {
            Kind::Feel { group, ends, lo, hi, log } => { d["slider"] = json!({ "group": group, "ends": [ends.0, ends.1], "values": [num(*lo), num(*hi)], "by_ratio": log }); "feel" }
            Kind::Number { min, max, .. } => { d["min"] = num(*min); d["max"] = num(*max); "number" }
            Kind::Turn => {
                if let Some(r) = f.try_downcast_ref::<[f32; 4]>() {
                    let (yaw, pitch, roll) = bevy::math::Quat::from_array(*r).to_euler(bevy::math::EulerRot::YXZ);
                    d["degrees"] = num((yaw.to_degrees() * 100.0).round() / 100.0);
                    d["upright"] = json!(pitch.abs() < 1e-3 && roll.abs() < 1e-3);
                }
                "turn"
            }
            Kind::Toggle => "toggle", Kind::Colour => "colour", Kind::Text => "text",
            Kind::Optional { .. } => "optional", Kind::Choice => "choice", Kind::List => "list",
            Kind::Tags => { d["choices"] = json!(d_tags); "tags" }
        };
        d["kind"] = json!(kind);
        if !s.unit.is_empty() { d["unit"] = json!(s.unit); }
        if !s.help.is_empty() { d["help"] = json!(s.help); }
        out.push(d);
    }
    Value::Array(out)
}

/// Set settings of `value` from JSON, as `describe` gives them: an object keyed by field or label (any case).
/// Numbers are kept in their range; a turn is degrees about the vertical (standing upright) or a quaternion; a
/// colour is [r, g, b] from 0 to 1 or "#rrggbb"; a choice is its name; an optional part is true, false or an object
/// of its own settings (turned on as needed). Returns the fields changed; on an error `value` may be half changed.
pub fn apply(value: &mut dyn PartialReflect, changes: &serde_json::Value) -> Result<Vec<&'static str>, String> {
    let serde_json::Value::Object(m) = changes else { return Err("Settings go in an object, such as {\"Colour\": [1, 0, 0]}".into()) };
    let list = fields(value);
    let ReflectMut::Struct(st) = value.reflect_mut() else { return Err("This has no settings".into()) };
    let mut done = vec![];
    for (key, v) in m {
        let k = key.to_lowercase();
        let Some((name, s)) = list.iter().find(|(n, s)| n.to_lowercase() == k || s.label.to_lowercase() == k) else {
            let names: Vec<&str> = list.iter().map(|(_, s)| s.label).collect();
            return Err(format!("No setting called \"{key}\" (there are: {})", names.join(", ")));
        };
        let f = st.field_mut(name).ok_or("missing field")?;
        apply_one(f, s, v).map_err(|e| format!("{}: {e}", s.label))?;
        done.push(*name);
    }
    Ok(done)
}

fn apply_one(f: &mut dyn PartialReflect, s: &Setting, v: &serde_json::Value) -> Result<(), String> {
    use serde_json::Value;
    let keep = |x: f32| match s.kind {
        Kind::Number { min, max, .. } => x.clamp(min, max),
        Kind::Feel { log: true, .. } => x.max(1e-3),
        _ => x,
    };
    let num = |v: &Value| v.as_f64().map(|x| keep(x as f32)).ok_or_else(|| format!("expected a number, not {v}"));
    if let Some(x) = f.try_downcast_mut::<f32>() {
        *x = num(v)?;
    } else if let Some(x) = f.try_downcast_mut::<bool>() {
        *x = v.as_bool().ok_or_else(|| format!("expected true or false, not {v}"))?;
    } else if let Some(x) = f.try_downcast_mut::<String>() {
        *x = v.as_str().ok_or_else(|| format!("expected text, not {v}"))?.to_string();
    } else if let Some(x) = f.try_downcast_mut::<[f32; 3]>() {
        if let (Kind::Colour, Some(h)) = (&s.kind, v.as_str()) {
            let h = h.trim_start_matches('#');
            let c = (h.len() == 6).then(|| u32::from_str_radix(h, 16).ok()).flatten().ok_or("expected \"#rrggbb\"")?;
            *x = [(c >> 16) as f32 / 255.0, ((c >> 8) & 255) as f32 / 255.0, (c & 255) as f32 / 255.0];
        } else {
            let a = v.as_array().filter(|a| a.len() == 3).ok_or_else(|| format!("expected [x, y, z], not {v}"))?;
            for i in 0..3 { x[i] = num(&a[i])?; }
            if matches!(s.kind, Kind::Colour) { for c in x.iter_mut() { *c = c.clamp(0.0, 1.0); } }
        }
    } else if let Some(x) = f.try_downcast_mut::<[f32; 4]>() {
        use bevy::math::Quat;
        let q = match v {
            Value::Number(d) => Quat::from_rotation_y((d.as_f64().unwrap_or(0.0) as f32).to_radians()),
            Value::Array(a) if a.len() == 4 => {
                let mut q = [0.0; 4];
                for i in 0..4 { q[i] = num(&a[i])?; }
                { let q = Quat::from_array(q); if q.length() < 1e-6 { return Err("that quaternion has no length".into()) } q.normalize() }
            }
            _ => return Err(format!("expected degrees or a quaternion [x, y, z, w], not {v}")),
        };
        *x = q.to_array();
    } else if let Kind::Optional { .. } = s.kind {
        let Some(TypeInfo::Enum(info)) = f.get_represented_type_info() else { return Err("not an optional part".into()) };
        let ReflectMut::Enum(e) = f.reflect_mut() else { return Err("not an optional part".into()) };
        let on = e.variant_name() == "Some";
        match v {
            Value::Bool(false) | Value::Null => { e.apply(&DynamicEnum::new("None", DynamicVariant::Unit)); }
            Value::Bool(true) | Value::Object(_) => {
                if !on {
                    let inner = info.variant("Some").and_then(|v| v.as_tuple_variant().ok()).and_then(|t| t.field_at(0)).and_then(|f| default_of(f.type_id()));
                    let mut t = DynamicTuple::default();
                    t.insert_boxed(inner.ok_or("it has no default")?.into_partial_reflect());
                    e.apply(&DynamicEnum::new("Some", DynamicVariant::Tuple(t)));
                }
                if v.is_object() { apply(e.field_at_mut(0).ok_or("it didn't turn on")?, v)?; }
            }
            _ => return Err(format!("expected true, false or an object of its settings, not {v}")),
        }
    } else if let Kind::Choice = s.kind {
        let Some(TypeInfo::Enum(info)) = f.get_represented_type_info() else { return Err("not a choice".into()) };
        let want = v.as_str().ok_or_else(|| format!("expected a name, not {v}"))?.to_lowercase();
        let pick = info.iter().find(|x| x.name().to_lowercase() == want || x.get_attribute::<Label>().is_some_and(|l| l.0.to_lowercase() == want));
        let names: Vec<&str> = info.iter().map(|x| x.get_attribute::<Label>().map_or(x.name(), |l| l.0)).collect();
        let pick = pick.ok_or_else(|| format!("expected one of {}", names.join(", ")))?;
        f.apply(&DynamicEnum::new(pick.name(), DynamicVariant::Unit));
    } else if let Kind::Tags = s.kind {
        let (all, _) = tag_set(f).ok_or("not a set of tags")?;
        let a = v.as_array().ok_or_else(|| format!("expected a list of names, not {v}"))?;
        let mut names = vec![];
        for x in a {
            let want = x.as_str().ok_or_else(|| format!("expected a name, not {x}"))?.to_lowercase();
            let (n, _) = all.iter().find(|(n, l)| n.to_lowercase() == want || l.to_lowercase() == want)
                .ok_or_else(|| format!("no tag called {x} (there are: {})", all.iter().map(|(_, l)| *l).collect::<Vec<_>>().join(", ")))?;
            names.push(*n);
        }
        set_tags(f, &all, &names);
    } else if let Kind::List = s.kind {
        let item_ty = match f.get_represented_type_info() { Some(TypeInfo::List(l)) => l.item_ty().id(), _ => return Err("not a list".into()) };
        let a = v.as_array().ok_or_else(|| format!("expected a list of objects, not {v}"))?;
        let ReflectMut::List(l) = f.reflect_mut() else { return Err("not a list".into()) };
        while l.len() > 0 { l.remove(l.len() - 1); }
        for item in a {
            let mut d = default_of(item_ty).ok_or("its items have no default")?;
            apply(d.as_partial_reflect_mut(), item)?;
            l.push(d.into_partial_reflect());
        }
    } else {
        return Err("this kind of setting can't be set yet".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{Collision, Mover, Part, Shape};

    fn part() -> Part {
        Part { id: 1, name: "Pad".into(), shape: Shape::Box, pos: [0.0; 3], rot: [0.0, 0.0, 0.0, 1.0], size: [1.0; 3], color: [0.5; 3],
            visible: true, collision: Collision::Solid, model: None, mover: Some(Mover { to: [0.0, 3.0, 0.0], secs: 2.0 }), surfaces: vec![], unique: false, tags: vec![], loose: None, light: None, camera: None }
    }

    #[test]
    fn a_slider_maps_both_ways_even_or_by_ratio_and_backwards() {
        for &(lo, hi, log) in &[(0.0, 10.0, false), (20.0, 0.2, true), (30.0, 2.0, false)] {
            for &t in &[0.0, 0.25, 0.5, 1.0] {
                assert!((feel_t(feel_value(t, lo, hi, log), lo, hi, log) - t).abs() < 1e-4, "{lo}..{hi} log {log} at {t}");
            }
        }
        assert!((feel_value(0.5, 20.0, 0.2, true) - 2.0).abs() < 1e-4);
        assert_eq!(feel_t(99.0, 0.0, 10.0, false), 1.0);
    }

    #[test]
    fn a_part_describes_its_settings_in_sections_with_values() {
        let p = part();
        assert_eq!(sections(&p), vec!["PART", "POSITION  ·  ROTATION", "SIZE  ·  LOOK", "TAGS", "LIGHT", "CAMERA", "MOVES"]);
        let d = describe(&p);
        let f = |name: &str| d.as_array().unwrap().iter().find(|x| x["field"] == name).unwrap().clone();
        assert_eq!(f("collision")["value"], "Solid");
        assert_eq!(f("size")["kind"], "number");
        assert_eq!(f("mover")["value"].as_array().unwrap().iter().find(|x| x["field"] == "secs").unwrap()["slider"]["ends"][1], "Quick");
        assert!(d.as_array().unwrap().iter().all(|x| x["field"] != "id"), "id isn't a setting");
    }

    #[test]
    fn turning_an_optional_part_on_uses_its_default_and_choices_set_variants() {
        let mut p = part();
        p.mover = None;
        let ReflectMut::Struct(st) = p.reflect_mut() else { panic!() };
        let f = st.field_mut("mover").unwrap();
        let Some(TypeInfo::Enum(info)) = f.get_represented_type_info() else { panic!() };
        let inner = info.variant("Some").unwrap().as_tuple_variant().unwrap().field_at(0).unwrap();
        let d = default_of(inner.type_id()).expect("Mover has a default");
        let mut t = DynamicTuple::default();
        t.insert_boxed(d.into_partial_reflect());
        f.apply(&DynamicEnum::new("Some", DynamicVariant::Tuple(t)));
        assert_eq!(p.mover, Some(Mover::default()));
        let ReflectMut::Struct(st) = p.reflect_mut() else { panic!() };
        st.field_mut("collision").unwrap().apply(&DynamicEnum::new("Trigger", DynamicVariant::Unit));
        assert_eq!(p.collision, Collision::Trigger);
    }

    #[test]
    fn tags_are_set_by_label_in_their_own_order_and_described_with_the_choices() {
        use crate::level::Tag;
        let mut p = part();
        assert_eq!(apply(&mut p, &serde_json::json!({"Tags": ["slippery", "Grind rail"]})).unwrap(), vec!["tags"]);
        assert_eq!(p.tags, vec![Tag::GrindRail, Tag::Slippery]);
        let d = describe(&p);
        let t = d.as_array().unwrap().iter().find(|x| x["field"] == "tags").unwrap().clone();
        assert_eq!(t["value"], serde_json::json!(["Grind rail", "Slippery"]));
        assert_eq!(t["kind"], "tags");
        assert_eq!(t["choices"].as_array().unwrap().len(), 6);
        assert!(apply(&mut p, &serde_json::json!({"Tags": ["Bouncy"]})).unwrap_err().contains("Perch spot"));
        apply(&mut p, &serde_json::json!({"Tags": []})).unwrap();
        assert!(p.tags.is_empty());
    }

    #[test]
    fn settings_are_set_from_json_by_label_or_field_in_range() {
        let mut p = part();
        p.mover = None;
        let done = apply(&mut p, &serde_json::json!({
            "colour": "#ff8000", "Turn": 90, "size": [3, 0, 2], "Collision": "walk-through", "Moves": {"Speed": 0.5}, "visible": false,
        })).unwrap();
        assert_eq!(done.len(), 6);
        assert_eq!(p.color, [1.0, 128.0 / 255.0, 0.0]);
        assert!((bevy::math::Quat::from_array(p.rot).to_euler(bevy::math::EulerRot::YXZ).0.to_degrees() - 90.0).abs() < 1e-3);
        assert_eq!(p.size, [3.0, 0.02, 2.0], "kept in range");
        assert_eq!(p.collision, Collision::WalkThrough);
        assert_eq!(p.mover, Some(Mover { to: [0.0, 3.0, 0.0], secs: 0.5 }));
        assert!(!p.visible);
        assert!(apply(&mut p, &serde_json::json!({"Moves": false})).is_ok() && p.mover.is_none());
        assert!(apply(&mut p, &serde_json::json!({"Wobble": 1})).unwrap_err().contains("Colour"));
        assert!(apply(&mut p, &serde_json::json!({"Collision": "Sticky"})).unwrap_err().contains("Walk-through"));
        let d = describe(&p);
        assert_eq!(d.as_array().unwrap().iter().find(|x| x["field"] == "rot").unwrap()["degrees"], 90.0);
    }
}
