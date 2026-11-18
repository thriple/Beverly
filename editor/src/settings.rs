//! Settings panels from data. A building block describes each of its settings once, as an attribute on the field:
//!
//! ```ignore
//! #[derive(Reflect)]
//! #[reflect(Default)]
//! pub struct Mover {
//!     #[reflect(@Setting::number("Moves by", "m", 0.05, -1e5, 1e5).section("MOVES"))]
//!     pub to: [f32; 3],
//!     #[reflect(@Setting::feel("Speed", &["Slow", "Steady", "Brisk", "Quick"], 20.0, 0.2, "s").log().section("MOVES"))]
//!     pub secs: f32,
//! }
//! ```
//!
//! `section` draws a section of any such value as a panel in the Motion kit style: word sliders (the numbers and
//! the slider's range ends under More), numbers, toggles, colours, choices, optional parts and lists. `describe`
//! lists the same settings with their values, for the MCP server (and later the timeline's keyable values), and
//! `apply` sets them from the same JSON.
//! Fields without a `Setting` are not shown. Range ends changed under More are kept per file in a `Ranges` map.
use crate::editor::frame;
use crate::editor::theme::{self, Tip, accent, MUTED, TEXT, TEXT_2};
use bevy::reflect::{
    enums::{DynamicEnum, DynamicVariant}, std_traits::ReflectDefault, structs::Struct, tuple::DynamicTuple, PartialReflect,
    Reflect, ReflectMut, ReflectRef, TypeInfo, TypeRegistry,
};
use bevy_egui::egui::{self, pos2, vec2, Color32, CornerRadius, Sense, Stroke, Ui};
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
    /// An icon before its label (a name in the icon set), or none.
    pub icon: &'static str,
}

#[derive(Reflect, Clone, Debug, PartialEq)]
pub enum Kind {
    /// A word slider. Fields with the same `group` share one slider; its words are stops spaced evenly along it,
    /// `lo` the value at the first and `hi` at the last (`hi` may be smaller), spaced evenly or, with `log`, by ratio.
    Feel { group: &'static str, #[reflect(ignore)] words: &'static [&'static str], lo: f32, hi: f32, log: bool },
    /// A number (or x, y, z for three of them) dragged or typed.
    Number { speed: f32, min: f32, max: f32 },
    Toggle,
    /// A colour (`[f32; 3]`, sRGB 0..1); `alpha` names an f32 field (0..1) that the picker's opacity strip sets.
    Colour { alpha: &'static str },
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
    fn new(label: &'static str, kind: Kind) -> Self { Setting { label, kind, section: "", unit: "", help: "", bare: false, icon: "" } }
    pub fn feel(label: &'static str, words: &'static [&'static str], lo: f32, hi: f32, unit: &'static str) -> Self {
        Setting { unit, ..Self::new(label, Kind::Feel { group: label, words, lo, hi, log: false }) }
    }
    pub fn number(label: &'static str, unit: &'static str, speed: f32, min: f32, max: f32) -> Self {
        Setting { unit, ..Self::new(label, Kind::Number { speed, min, max }) }
    }
    pub fn toggle(label: &'static str) -> Self { Self::new(label, Kind::Toggle) }
    pub fn colour(label: &'static str) -> Self { Self::new(label, Kind::Colour { alpha: "" }) }
    pub fn text(label: &'static str) -> Self { Self::new(label, Kind::Text) }
    pub fn turn(label: &'static str) -> Self { Self::new(label, Kind::Turn) }
    pub fn optional(label: &'static str, off: &'static str, on: &'static str) -> Self { Self::new(label, Kind::Optional { off, on }) }
    pub fn choice(label: &'static str) -> Self { Self::new(label, Kind::Choice) }
    pub fn list(label: &'static str) -> Self { Self::new(label, Kind::List) }
    pub fn tags(label: &'static str) -> Self { Self::new(label, Kind::Tags) }
    pub fn section(mut self, s: &'static str) -> Self { self.section = s; self }
    pub fn help(mut self, h: &'static str) -> Self { self.help = h; self }
    pub fn bare(mut self) -> Self { self.bare = true; self }
    pub fn icon(mut self, i: &'static str) -> Self { self.icon = i; self }
    /// Give a colour's picker an opacity strip for another field (an f32 from 0 to 1).
    pub fn alpha(mut self, field: &'static str) -> Self { if let Kind::Colour { alpha } = &mut self.kind { *alpha = field; } self }
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
        r.register::<crate::level::Lighting>();
        r.register::<crate::level::Navigation>();
        r.register::<crate::level::Material>();
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
    /// The project's colours, offered first by the colour picker.
    pub palette: Vec<[f32; 3]>,
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
    // the type's defaults, where it has them: a setting changed from its default is marked (and can be put back)
    let defaults = value.get_represented_type_info().and_then(|i| default_of(i.type_id()));
    let ReflectMut::Struct(st) = value.reflect_mut() else { return false };
    let mut changed = false;
    let mut feels_done: Vec<&'static str> = vec![];
    for (name, s) in list.iter().cloned() {
        let dflt = defaults.as_ref().and_then(|d| match d.reflect_ref() { ReflectRef::Struct(d) => d.field(name), _ => None });
        match &s.kind {
            Kind::Feel { group, words, .. } => {
                if !feels_done.contains(group) {
                    feels_done.push(group);
                    let members: Vec<(&'static str, Setting)> = list.iter().filter(|(_, m)| matches!(&m.kind, Kind::Feel { group: g, .. } if g == group)).cloned().collect();
                    let first = defaults.as_ref().and_then(|d| match d.reflect_ref() { ReflectRef::Struct(d) => d.field(members[0].0).and_then(|f| f.try_downcast_ref::<f32>()).copied(), _ => None });
                    changed |= feel(ui, st, &ty, group, words, &members, first, ctx);
                }
            }
            _ => {
                // a colour's opacity field, worked on as a copy (it is another field of the same struct)
                let alpha = match s.kind { Kind::Colour { alpha } if !alpha.is_empty() => Some(alpha), _ => None };
                let mut a = alpha.and_then(|n| st.field(n)).and_then(|f| f.try_downcast_ref::<f32>()).copied();
                let a0 = a;
                if let Some(f) = st.field_mut(name) {
                    let differs = dflt.is_some_and(|d| !matches!(&s.kind, Kind::Number { .. } | Kind::Turn | Kind::Text) && !d.reflect_partial_eq(f).unwrap_or(true));
                    let mut reset = false;
                    changed |= one(ui, f, &s, &format!("{ty}.{name}"), differs, &mut reset, a.as_mut(), ctx);
                    if reset { if let Some(d) = dflt { f.apply(d); changed = true; } }
                }
                if a != a0 {
                    if let (Some(n), Some(v)) = (alpha, a) { if let Some(x) = st.field_mut(n).and_then(|f| f.try_downcast_mut::<f32>()) { *x = v; changed = true; } }
                }
            }
        }
    }
    changed
}

fn tip(r: egui::Response, s: &Setting) -> egui::Response { if s.help.is_empty() { r } else { r.tip(s.help) } }

/// The label column's width, and how far an icon before a label moves it along.
const LABEL_W: f32 = 76.0;
const ICON_W: f32 = 21.0;

/// A settings row: the label in its column, then the control. A setting changed from its default has a bold, white
/// label and the accent bar at its left: click the bar to put it back.
pub(crate) fn row<R>(ui: &mut Ui, icon: &str, label: &str, help: &str, differs: bool, reset: &mut bool, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal_top(|ui| {
        let colour = if differs { Color32::WHITE } else { TEXT_2 };
        let lead = if icon.is_empty() { 0.0 } else { ICON_W };
        let g = ui.painter().layout(label.to_owned(), if differs { theme::semi(12.5) } else { theme::body(12.5) }, colour, LABEL_W - lead);
        let (r, resp) = ui.allocate_exact_size(vec2(LABEL_W, g.size().y.max(28.0)), Sense::hover());
        if !icon.is_empty() { frame::paint_icon(ui.painter(), icon, pos2(r.left() + 7.0, r.top() + 14.0), 14.0, colour); }
        ui.painter().galley(pos2(r.left() + lead, r.top() + (28.0 - g.size().y).max(0.0) / 2.0), g, TEXT);
        if !help.is_empty() { resp.tip(help); }
        if differs { *reset |= changed_bar(ui, r); }
        add(ui)
    }).inner
}

/// The accent bar left of a changed setting's row; true when clicked (put it back).
fn changed_bar(ui: &mut Ui, r: egui::Rect) -> bool {
    let bar = egui::Rect::from_min_max(pos2(r.left() - 18.0, r.top() + 5.0), pos2(r.left() - 15.0, r.top() + 23.0));
    ui.painter().rect_filled(bar, CornerRadius { nw: 0, sw: 0, ne: 2, se: 2 }, accent());
    ui.interact(bar.expand2(vec2(3.0, 2.0)), ui.id().with(("changed", r.min.x as i32, r.min.y as i32)), Sense::click()).tip("Changed from default. Click the bar to put it back.").clicked()
}

/// A toggle's row: its label across, the toggle at the right.
pub(crate) fn toggle_row(ui: &mut Ui, icon: &str, label: &str, on: &mut bool, help: &str, differs: bool, reset: &mut bool) -> bool {
    ui.horizontal(|ui| {
        ui.set_min_height(28.0);
        let r = ui.max_rect();
        if differs { *reset |= changed_bar(ui, egui::Rect::from_min_size(pos2(r.left(), ui.cursor().top()), vec2(1.0, 28.0))); }
        if !icon.is_empty() {
            let (b, _) = ui.allocate_exact_size(vec2(ICON_W - ui.spacing().item_spacing.x, 28.0), Sense::hover());
            frame::paint_icon(ui.painter(), icon, pos2(b.left() + 7.0, b.center().y), 14.0, if differs { Color32::WHITE } else { TEXT_2 });
        }
        ui.label(egui::RichText::new(label).font(if differs { theme::semi(12.5) } else { theme::body(12.5) }).color(if differs { Color32::WHITE } else { Color32::from_rgb(0xE6, 0xE6, 0xE8) }));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = theme::toggle(ui, on);
            (if help.is_empty() { r } else { r.tip(help) }).changed()
        }).inner
    }).inner
}

/// One setting that isn't a slider.
#[allow(clippy::too_many_arguments)]
fn one(ui: &mut Ui, f: &mut dyn PartialReflect, s: &Setting, key: &str, differs: bool, reset: &mut bool, alpha: Option<&mut f32>, ctx: &mut Ctx) -> bool {
    match &s.kind {
        Kind::Number { speed, min, max } => {
            if let Some(v) = f.try_downcast_mut::<f32>() {
                return row(ui, s.icon, s.label, s.help, false, reset, |ui| number(ui, v, *speed, *min, *max, &unit(s), None, ui.available_width()).changed());
            }
            if let Some(v) = f.try_downcast_mut::<[f32; 3]>() {
                return row(ui, s.icon, s.label, s.help, false, reset, |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let w = (ui.available_width() - 12.0) / 3.0;
                    let mut c = false;
                    for (i, x) in v.iter_mut().enumerate() { c |= number(ui, x, *speed, *min, *max, "", Some(i), w).tip(format!("{} {}, in {}", s.label, ["across (X)", "up (Y)", "along (Z)"][i], if s.unit.is_empty() { "times" } else { s.unit })).changed(); }
                    c
                });
            }
            false
        }
        Kind::Text => f.try_downcast_mut::<String>().is_some_and(|t| row(ui, s.icon, s.label, s.help, false, reset, |ui| theme::text_input(ui, t, "", None, ui.available_width()).changed())),
        Kind::Turn => f.try_downcast_mut::<[f32; 4]>().is_some_and(|r| turn(ui, r, s)),
        Kind::Choice => choice(ui, f, s, key, differs, reset),
        Kind::Optional { off, on } => optional(ui, f, s, off, on, differs, reset, ctx),
        Kind::List => list(ui, f, s, key, ctx),
        Kind::Tags => tags(ui, f, s),
        Kind::Toggle => f.try_downcast_mut::<bool>().is_some_and(|b| toggle_row(ui, s.icon, s.label, b, s.help, differs, reset)),
        // on a copy: the picker nudges the numbers through its colour conversions even when nothing is picked
        Kind::Colour { .. } => f.try_downcast_mut::<[f32; 3]>().is_some_and(|c| {
            let mut copy = *c;
            let picked = row(ui, s.icon, s.label, s.help, differs, reset, |ui| colour(ui, &mut copy, alpha, key, &ctx.palette));
            if picked { *c = copy; }
            picked
        }),
        Kind::Feel { .. } => false,
    }
}

fn unit(s: &Setting) -> String { if s.unit.is_empty() { String::new() } else if s.unit == "×" || s.unit == "°" { s.unit.into() } else { format!(" {}", s.unit) } }

/// A number field: on placement's, the axis letter in its colour first. Drag sideways to scrub, click to type.
pub(crate) fn number(ui: &mut Ui, v: &mut f32, speed: f32, min: f32, max: f32, suffix: &str, axis: Option<usize>, w: f32) -> egui::Response {
    let h = if axis.is_some() { 26.0 } else { 28.0 };
    let (r, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let round = if axis.is_some() { 6 } else { 7 };
    let inner = egui::Rect::from_min_max(pos2(r.left() + if axis.is_some() { 18.0 + 7.0 } else { 10.0 }, r.top()), pos2(r.right() - 4.0, r.bottom()));
    // the field goes under the value: keep its place before the value is drawn
    let at = ui.painter().add(egui::Shape::Noop);
    // the value in a child of the row (it takes no room of its own)
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let vis = &mut child.style_mut().visuals.widgets;
    for w in [&mut vis.inactive, &mut vis.hovered, &mut vis.active] { w.bg_fill = Color32::TRANSPARENT; w.weak_bg_fill = Color32::TRANSPARENT; w.bg_stroke = Stroke::NONE; }
    child.style_mut().override_font_id = Some(theme::mono(12.0));
    let resp = child.add(egui::DragValue::new(v).speed(speed as f64).range(min..=max).max_decimals(2).suffix(suffix));
    let hot = resp.hovered() || resp.dragged();
    let p = ui.painter();
    p.set(at, egui::Shape::rect_filled(r, CornerRadius::same(round), if hot { theme::FIELD_HOVER } else { theme::FIELD }));
    if let Some(i) = axis {
        let chip = egui::Rect::from_min_size(r.min, vec2(18.0, h));
        p.rect_filled(chip, CornerRadius { nw: round, sw: round, ne: 0, se: 0 }, [theme::AXIS_X, theme::AXIS_Y, theme::AXIS_Z][i]);
        p.text(chip.center(), egui::Align2::CENTER_CENTER, ["X", "Y", "Z"][i], theme::bold(10.0), theme::INK);
    }
    if resp.has_focus() {
        theme::own_focus(ui.ctx(), resp.id);
        p.rect_stroke(r, CornerRadius::same(round), Stroke::new(1.5, accent()), egui::StrokeKind::Inside);
    } else if hot {
        crate::editor::frame_icon(p, "scrub", pos2(r.right() - 11.0, r.center().y), 12.0, accent());
    }
    resp
}

fn turn(ui: &mut Ui, r: &mut [f32; 4], s: &Setting) -> bool {
    use bevy::math::{EulerRot, Quat};
    let (yaw, pitch, roll) = Quat::from_array(*r).to_euler(EulerRot::YXZ);
    let mut deg = yaw.to_degrees();
    let mut reset = false;
    row(ui, s.icon, s.label, s.help, false, &mut reset, |ui| {
        let mut c = false;
        let w = (ui.available_width() - 12.0) / 3.0;
        if number(ui, &mut deg, 1.0, -100000.0, 100000.0, "°", Some(1), w).changed() {
            *r = Quat::from_euler(EulerRot::YXZ, deg.to_radians(), pitch, roll).to_array();
            c = true;
        }
        if (pitch.abs() > 1e-3 || roll.abs() > 1e-3) && theme::quiet_button(ui, "Stand upright").clicked() {
            *r = Quat::from_rotation_y(yaw).to_array();
            c = true;
        }
        c
    })
}

/// A choice: two to four as a segmented row, more as a dropdown.
fn choice(ui: &mut Ui, f: &mut dyn PartialReflect, s: &Setting, key: &str, differs: bool, reset: &mut bool) -> bool {
    let Some(TypeInfo::Enum(info)) = f.get_represented_type_info() else { return false };
    let ReflectRef::Enum(e) = f.reflect_ref() else { return false };
    let names: Vec<&'static str> = info.variant_names().to_vec();
    let labels: Vec<&str> = info.iter().map(|v| v.get_attribute::<Label>().map_or(v.name(), |l| l.0)).collect();
    let mut i = names.iter().position(|n| *n == e.variant_name()).unwrap_or(0);
    let picked = row(ui, s.icon, s.label, s.help, differs, reset, |ui| {
        if labels.len() <= 4 { return theme::choices(ui, key, &labels, &mut i); }
        let before = i;
        egui::ComboBox::from_id_salt(key).selected_text(labels[i]).width(ui.available_width()).show_ui(ui, |ui| {
            for (k, l) in labels.iter().enumerate() { ui.selectable_value(&mut i, k, *l); }
        });
        i != before
    });
    if picked {
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
    if !s.bare { ui.label(egui::RichText::new(s.label).color(TEXT_2)); }
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

/// Something that may or may not be there (a part that moves): a toggle row saying which, then its settings.
#[allow(clippy::too_many_arguments)]
fn optional(ui: &mut Ui, f: &mut dyn PartialReflect, s: &Setting, off: &str, on_word: &str, differs: bool, reset: &mut bool, ctx: &mut Ctx) -> bool {
    let Some(TypeInfo::Enum(info)) = f.get_represented_type_info() else { return false };
    let ReflectMut::Enum(e) = f.reflect_mut() else { return false };
    let mut on = e.variant_name() == "Some";
    if toggle_row(ui, s.icon, if on { on_word } else { off }, &mut on, s.help, differs, reset) {
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
    let mut changed = false;
    if let Some(inner) = e.field_at_mut(0) {
        for sec in sections(inner) { changed |= section(ui, inner, sec, ctx); }
    }
    changed
}

fn list(ui: &mut Ui, f: &mut dyn PartialReflect, s: &Setting, key: &str, ctx: &mut Ctx) -> bool {
    let item_ty = match f.get_represented_type_info() { Some(TypeInfo::List(l)) => l.item_ty().id(), _ => return false };
    let ReflectMut::List(l) = f.reflect_mut() else { return false };
    let mut changed = false;
    let mut remove = None;
    ui.label(egui::RichText::new(s.label).color(TEXT_2));
    for i in 0..l.len() {
        let Some(item) = l.get_mut(i) else { continue };
        ui.push_id((key, i), |ui| {
            egui::Frame::new().fill(theme::FIELD).corner_radius(CornerRadius::same(10)).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                for sec in sections(item) { changed |= section(ui, item, sec, ctx); }
                if theme::quiet_button(ui, "Remove").clicked() { remove = Some(i); }
            });
        });
    }
    if let Some(i) = remove { l.remove(i); changed = true; }
    if theme::quiet_button(ui, "+ Add").clicked() {
        if let Some(d) = default_of(item_ty) { l.push(d.into_partial_reflect()); changed = true; }
    }
    changed
}

/// "More" at the right of the row under a slider, its caret turning down while its numbers are open. True when clicked.
fn more(ui: &mut Ui, id: egui::Id, open: bool) -> bool {
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
    let g = ui.painter().layout_no_wrap("More".to_owned(), theme::semi(12.0), accent());
    let w = g.size().x + 4.0 + 12.0;
    let hit = egui::Rect::from_min_max(pos2(row.right() - w, row.top()), row.max);
    let resp = ui.interact(hit, id.with("more"), Sense::click());
    let c = if resp.hovered() { theme::lerp_color(accent(), Color32::WHITE, 0.3) } else { accent() };
    let turn = theme::spring(ui.ctx(), id.with("turn"), if open { 1.0 } else { 0.0 }, theme::SNAPPY);
    ui.painter().galley(pos2(hit.left(), row.center().y - g.size().y / 2.0), g, c);
    frame::paint_icon_turned(ui.painter(), "right", pos2(row.right() - 6.0, row.center().y), 12.0, c, turn * std::f32::consts::FRAC_PI_2);
    resp.tip(if open { "Fewer: hide its numbers" } else { "Its numbers, and what the slider's ends mean" }).clicked()
}

/// One word slider for every Feel field in `group`, with their numbers and range ends under More numbers.
#[allow(clippy::too_many_arguments)]
fn feel(ui: &mut Ui, st: &mut dyn Struct, ty: &str, group: &str, words: &[&str], members: &[(&'static str, Setting)], default: Option<f32>, ctx: &mut Ctx) -> bool {
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
    let differs = default.is_some_and(|d| (d - v0).abs() > 1e-4 * d.abs().max(1.0));
    let mut reset = false;
    if row(ui, s0.icon, group, s0.help, differs, &mut reset, |ui| word_slider(ui, &mut t, words).changed()) {
        for (n, s) in members {
            let (lo, hi, log) = range(ctx, n, s);
            if let Some(v) = st.field_mut(n).and_then(|f| f.try_downcast_mut::<f32>()) { *v = feel_value(t, lo, hi, log); }
        }
        changed = true;
    }
    if reset {
        if let Some(d) = default { if let Some(v) = st.field_mut(n0).and_then(|f| f.try_downcast_mut::<f32>()) { *v = d; changed = true; } }
    }
    let id = ui.id().with(("more", ty, group));
    let mut open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    if more(ui, id, open) {
        open = !open;
        ui.data_mut(|d| d.insert_temp(id, open));
    }
    if open {
        let mut none = false;
        for (n, s) in members {
            let (lo, hi, log) = range(ctx, n, s);
            let u = unit(s);
            if let Some(v) = st.field_mut(n).and_then(|f| f.try_downcast_mut::<f32>()) {
                changed |= row(ui, "", s.label, "", false, &mut none, |ui| number(ui, v, 0.01, f32::MIN, f32::MAX, &u, None, ui.available_width()).changed());
            }
            let (mut a, mut b) = (lo, hi);
            let (ca, cb) = row(ui, "", "Ends", "What the slider's two ends mean: the value at its first word and at its last", false, &mut none, |ui| {
                let w = (ui.available_width() - 6.0) / 2.0;
                let ca = number(ui, &mut a, 0.01, f32::MIN, f32::MAX, &u, None, w).tip(format!("{} end", words[0])).changed();
                let cb = number(ui, &mut b, 0.01, f32::MIN, f32::MAX, &u, None, w).tip(format!("{} end", words[words.len() - 1])).changed();
                (ca, cb)
            });
            if (ca || cb) && a != b && (!log || (a > 0.0 && b > 0.0)) {
                ctx.ranges.insert(format!("{ty}.{n}"), (a, b));
                ctx.ranges_changed = true;
            }
        }
    }
    changed
}

/// The word slider: a 4 px track filling with the accent, a tick at each word (its stops, spaced evenly), a white
/// thumb with an accent halo, the words under the track (the nearest one white). Dragging thickens it and the word
/// rides above the thumb.
pub fn word_slider(ui: &mut Ui, t: &mut f32, words: &[&str]) -> egui::Response {
    let w = ui.available_width();
    let (rect, mut resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click_and_drag());
    let track = egui::Rect::from_center_size(pos2(rect.center().x, rect.top() + 11.0), vec2(rect.width() - 12.0, 4.0));
    if let Some(p) = resp.interact_pointer_pos() {
        let nt = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
        if nt != *t { *t = nt; resp.mark_changed(); }
    }
    if resp.has_focus() {
        let d = ui.input(|i| (i.key_pressed(egui::Key::ArrowRight) as i32 - i.key_pressed(egui::Key::ArrowLeft) as i32) as f32 * 0.05);
        if d != 0.0 { *t = (*t + d).clamp(0.0, 1.0); resp.mark_changed(); }
    }
    let shown = theme::spring(ui.ctx(), resp.id.with("t"), *t, if resp.dragged() { theme::SNAPPY } else { theme::SPRING }).clamp(0.0, 1.0);
    let held = theme::spring(ui.ctx(), resp.id.with("held"), if resp.dragged() { 1.0 } else { 0.0 }, theme::SNAPPY).clamp(0.0, 1.2);
    let p = ui.painter();
    let th = 4.0 + 2.0 * held.min(1.0);
    let tr = egui::Rect::from_center_size(track.center(), vec2(track.width(), th));
    p.rect_filled(tr, CornerRadius::same(3), theme::TRACK);
    let x = tr.left() + tr.width() * shown;
    p.rect_filled(egui::Rect::from_min_max(tr.min, pos2(x, tr.max.y)), CornerRadius::same(3), accent());
    let last = (words.len().max(2) - 1) as f32;
    for k in 0..words.len() {
        let tx = tr.left() + tr.width() * k as f32 / last;
        if (tx - x).abs() > 9.0 { p.rect_filled(egui::Rect::from_center_size(pos2(tx, tr.center().y), vec2(2.0, 8.0)), CornerRadius::same(1), Color32::from_rgb(0x4A, 0x4A, 0x52)); }
    }
    let r = 8.0 + 2.0 * held;
    let c = pos2(x, tr.center().y);
    p.circle_filled(c, r + 3.0, accent().gamma_multiply(0.3));
    p.circle_filled(c, r, Color32::WHITE);
    if resp.has_focus() { theme::own_focus(ui.ctx(), resp.id); p.circle_stroke(c, r + 4.0, Stroke::new(2.0, accent())); }
    let k_near = (*t * last).round() as usize;
    let near = words.get(k_near).copied().unwrap_or("");
    let y = rect.bottom() - 8.0;
    for (k, word) in words.iter().enumerate() {
        // the ends sit inside the slider's edges, the rest under their ticks
        let (at, align) = match k {
            0 => (rect.left() + 6.0, egui::Align2::LEFT_CENTER),
            _ if k + 1 == words.len() => (rect.right() - 6.0, egui::Align2::RIGHT_CENTER),
            _ => (tr.left() + tr.width() * k as f32 / last, egui::Align2::CENTER_CENTER),
        };
        // the middle words are smaller (user, 2026-11-17): the ends carry the meaning
        let on = k == k_near;
        let size = if k == 0 || k + 1 == words.len() { 11.0 } else { 9.5 };
        p.text(pos2(at, y), align, *word, if on { theme::semi(size) } else { theme::body(size) }, if on { TEXT } else { MUTED });
    }
    if held > 0.01 {
        let g = p.layout_no_wrap(near.to_owned(), theme::semi(11.5), Color32::WHITE);
        let pill = egui::Rect::from_center_size(pos2(x, c.y - r - 16.0), vec2(g.size().x + 18.0, 20.0));
        let a = held.min(1.0);
        p.rect_filled(pill, CornerRadius::same(10), accent().gamma_multiply(a));
        p.galley(pill.center() - g.size() / 2.0, g, Color32::WHITE.gamma_multiply(a));
    }
    resp
}

/// A colour: its swatch and a friendly name; click for the picker (saturation and brightness, hue, hex), with the
/// project's colours first so a game keeps one palette.
fn colour(ui: &mut Ui, c: &mut [f32; 3], mut alpha: Option<&mut f32>, key: &str, palette: &[[f32; 3]]) -> bool {
    let col = |c: [f32; 3]| Color32::from_rgb((c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8);
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same(7), if resp.hovered() { theme::FIELD_HOVER } else { theme::FIELD });
    let sw = egui::Rect::from_center_size(pos2(r.left() + 6.0 + 9.0, r.center().y), vec2(18.0, 18.0));
    p.rect_filled(sw, CornerRadius::same(5), col(*c));
    p.rect_stroke(sw, CornerRadius::same(5), Stroke::new(1.0, Color32::from_white_alpha(51)), egui::StrokeKind::Inside);
    p.text(pos2(sw.right() + 8.0, r.center().y), egui::Align2::LEFT_CENTER, theme::colour_name(*c), theme::body(12.5), TEXT);
    crate::editor::frame_icon(p, "down", pos2(r.right() - 14.0, r.center().y), 12.0, MUTED);
    let mut changed = false;
    let pid = ui.id().with(("colour", key));
    let ctx = ui.ctx().clone();
    let dropping = ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new(EYEDROPPER))) == Some(pid);
    // while the eyedropper is out, clicks in the view pick instead of closing the picker
    let close = if dropping { egui::PopupCloseBehavior::IgnoreClicks } else { egui::PopupCloseBehavior::CloseOnClickOutside };
    egui::Popup::menu(&resp).id(pid).close_behavior(close).show(|ui| {
        ui.set_width(280.0);
        if let Some((from, picked)) = ctx.data(|d| d.get_temp::<(egui::Id, [f32; 3])>(egui::Id::new(EYEDROPPED))) {
            if from == pid { *c = picked; changed = true; ctx.data_mut(|d| d.remove::<(egui::Id, [f32; 3])>(egui::Id::new(EYEDROPPED))); }
        }
        ui.horizontal(|ui| {
            let (sw, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
            ui.painter().rect_filled(sw, CornerRadius::same(8), col(*c));
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(egui::RichText::new(theme::colour_name(*c)).font(theme::semi(12.5)));
                ui.label(egui::RichText::new("Named automatically").size(11.0).color(MUTED));
            });
        });
        changed |= picker(ui, c);
        if let Some(a) = alpha.as_deref_mut() { changed |= opacity_strip(ui, a, col(*c)); }
        let c32 = col(*c);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            // the eyedropper: the colour of a part in the view (I)
            let (b, bresp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
            ui.painter().rect_filled(b, CornerRadius::same(7), if dropping { accent() } else if bresp.hovered() { theme::FIELD_HOVER } else { rgb3(0x26262B) });
            crate::editor::frame_icon(ui.painter(), "pipette", b.center(), 16.0, if dropping { theme::INK } else { TEXT });
            let key_i = ui.memory(|m| m.focused().is_none()) && ui.input(|i| i.key_pressed(egui::Key::I));
            if bresp.tip_keys("Pick a colour from a part in the view", &["I"]).clicked() || key_i {
                ctx.data_mut(|d| if dropping { d.remove::<egui::Id>(egui::Id::new(EYEDROPPER)); } else { d.insert_temp(egui::Id::new(EYEDROPPER), pid); });
            }
            let mut hex = format!("{:02X}{:02X}{:02X}", c32.r(), c32.g(), c32.b());
            ui.label(egui::RichText::new("Hex").size(12.0).color(MUTED));
            let w = ui.available_width() - if alpha.is_some() { 76.0 } else { 0.0 };
            if theme::text_input(ui, &mut hex, "", None, w).changed() {
                if let Ok(v) = u32::from_str_radix(hex.trim_start_matches('#'), 16) {
                    if hex.trim_start_matches('#').len() == 6 { *c = [(v >> 16) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0]; changed = true; }
                }
            }
            if let Some(a) = alpha.as_deref_mut() {
                let mut pct = (*a * 100.0).round();
                if number(ui, &mut pct, 1.0, 0.0, 100.0, "%", None, 70.0).tip("Opacity").changed() { *a = pct / 100.0; changed = true; }
            }
        });
        if !palette.is_empty() {
            ui.label(egui::RichText::new("In this project").size(11.5).color(MUTED));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                for q in palette {
                    let (sw, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                    ui.painter().rect_filled(sw, CornerRadius::same(6), col(*q));
                    if col(*q) == col(*c) { ui.painter().rect_stroke(sw.expand(2.0), CornerRadius::same(7), Stroke::new(1.5, accent()), egui::StrokeKind::Inside); }
                    if resp.tip(theme::colour_name(*q)).clicked() { *c = *q; changed = true; }
                }
            });
        }
    });
    // the picker closed: put the eyedropper away
    if dropping && !egui::Popup::is_id_open(&ctx, pid) { ctx.data_mut(|d| d.remove::<egui::Id>(egui::Id::new(EYEDROPPER))); }
    changed
}

/// Temp data: the colour picker (its popup id) whose eyedropper is out; the editor's view answers it.
pub const EYEDROPPER: &str = "eyedropper";
/// Temp data: (the picker's popup id, the colour picked in the view).
pub const EYEDROPPED: &str = "eyedropped";

fn rgb3(x: u32) -> Color32 { Color32::from_rgb((x >> 16) as u8, (x >> 8) as u8, x as u8) }

/// The opacity strip: the colour from clear to solid over a checker, with a thumb; drag to set `a` (0..1).
fn opacity_strip(ui: &mut Ui, a: &mut f32, c: Color32) -> bool {
    let (strip, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 12.0), Sense::click_and_drag());
    let p = ui.painter().with_clip_rect(strip);
    p.rect_filled(strip, CornerRadius::same(6), rgb3(0x26262B));
    let n = (strip.width() / 6.0).ceil() as i32;
    for i in 0..n {
        for j in 0..2 {
            if (i + j) % 2 == 0 { continue; }
            let q = egui::Rect::from_min_size(pos2(strip.left() + i as f32 * 6.0, strip.top() + j as f32 * 6.0), vec2(6.0, 6.0));
            p.rect_filled(q.intersect(strip.shrink2(vec2(3.0, 0.0))), CornerRadius::ZERO, rgb3(0x3A3A40));
        }
    }
    let mut mesh = egui::Mesh::default();
    let clear = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 0);
    for (x, k) in [(strip.left(), clear), (strip.right(), c)] {
        mesh.colored_vertex(pos2(x, strip.top()), k);
        mesh.colored_vertex(pos2(x, strip.bottom()), k);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    p.add(mesh);
    let before = *a;
    if let Some(q) = resp.interact_pointer_pos() { *a = ((q.x - strip.left()) / strip.width()).clamp(0.0, 1.0); }
    let at = pos2(strip.left() + strip.width() * *a, strip.center().y);
    ui.painter().circle_filled(at, 8.0, c);
    ui.painter().circle_stroke(at, 8.0, Stroke::new(2.5, Color32::WHITE));
    resp.tip("Opacity: clear at the left, solid at the right");
    *a != before
}

/// The picker: a square of saturation (across) and brightness (up), and a strip of hues under it.
fn picker(ui: &mut Ui, c: &mut [f32; 3]) -> bool {
    use bevy::color::{Color, Hsva};
    let id = ui.id().with("hsv");
    let rgb = |h: [f32; 3]| { let s = Color::from(Hsva::new(h[0], h[1], h[2], 1.0)).to_srgba(); [s.red, s.green, s.blue] };
    let col = |c: [f32; 3]| Color32::from_rgb((c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8);
    // the hue is kept while the colour is grey (where any hue gives it)
    let mut hsv: [f32; 3] = ui.data(|d| d.get_temp(id)).filter(|h| col(rgb(*h)) == col(*c)).unwrap_or_else(|| {
        let h = Hsva::from(Color::srgb(c[0], c[1], c[2]));
        [h.hue, h.saturation, h.value]
    });
    let before = hsv;
    let w = ui.available_width();
    let (sq, resp) = ui.allocate_exact_size(vec2(w, 150.0), Sense::click_and_drag());
    let n = 16;
    let mut mesh = egui::Mesh::default();
    for j in 0..=n {
        for i in 0..=n {
            let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
            mesh.colored_vertex(pos2(sq.left() + sq.width() * u, sq.top() + sq.height() * v), col(rgb([hsv[0], u, 1.0 - v])));
            if i < n && j < n {
                let k = (j * (n + 1) + i) as u32;
                mesh.add_triangle(k, k + 1, k + n as u32 + 1);
                mesh.add_triangle(k + 1, k + n as u32 + 2, k + n as u32 + 1);
            }
        }
    }
    ui.painter().add(mesh);
    if let Some(p) = resp.interact_pointer_pos() {
        hsv[1] = ((p.x - sq.left()) / sq.width()).clamp(0.0, 1.0);
        hsv[2] = 1.0 - ((p.y - sq.top()) / sq.height()).clamp(0.0, 1.0);
    }
    let at = pos2(sq.left() + sq.width() * hsv[1], sq.top() + sq.height() * (1.0 - hsv[2]));
    ui.painter().circle_stroke(at, 8.0, Stroke::new(2.0, Color32::WHITE));
    ui.painter().circle_stroke(at, 9.5, Stroke::new(1.0, Color32::from_black_alpha(90)));
    // the hues
    let (strip, resp) = ui.allocate_exact_size(vec2(w, 12.0), Sense::click_and_drag());
    let mut mesh = egui::Mesh::default();
    for k in 0..=12 {
        let u = k as f32 / 12.0;
        let x = strip.left() + strip.width() * u;
        let c = col(rgb([360.0 * u, 1.0, 1.0]));
        mesh.colored_vertex(pos2(x, strip.top()), c);
        mesh.colored_vertex(pos2(x, strip.bottom()), c);
        if k < 12 { let q = 2 * k as u32; mesh.add_triangle(q, q + 1, q + 2); mesh.add_triangle(q + 1, q + 3, q + 2); }
    }
    ui.painter().add(mesh);
    if let Some(p) = resp.interact_pointer_pos() { hsv[0] = 360.0 * ((p.x - strip.left()) / strip.width()).clamp(0.0, 0.9999); }
    let hx = pos2(strip.left() + strip.width() * hsv[0] / 360.0, strip.center().y);
    ui.painter().circle_filled(hx, 8.0, col(rgb([hsv[0], 1.0, 1.0])));
    ui.painter().circle_stroke(hx, 8.0, Stroke::new(2.0, Color32::WHITE));
    ui.data_mut(|d| d.insert_temp(id, hsv));
    if hsv != before { *c = rgb(hsv); return true; }
    false
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
            Kind::Feel { group, words, lo, hi, log } => { d["slider"] = json!({ "group": group, "words": words, "values": [num(*lo), num(*hi)], "by_ratio": log }); "feel" }
            Kind::Number { min, max, .. } => { d["min"] = num(*min); d["max"] = num(*max); "number" }
            Kind::Turn => {
                if let Some(r) = f.try_downcast_ref::<[f32; 4]>() {
                    let (yaw, pitch, roll) = bevy::math::Quat::from_array(*r).to_euler(bevy::math::EulerRot::YXZ);
                    d["degrees"] = num((yaw.to_degrees() * 100.0).round() / 100.0);
                    d["upright"] = json!(pitch.abs() < 1e-3 && roll.abs() < 1e-3);
                }
                "turn"
            }
            Kind::Toggle => "toggle", Kind::Colour { .. } => "colour", Kind::Text => "text",
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
        if let (Kind::Colour { .. }, Some(h)) = (&s.kind, v.as_str()) {
            let h = h.trim_start_matches('#');
            let c = (h.len() == 6).then(|| u32::from_str_radix(h, 16).ok()).flatten().ok_or("expected \"#rrggbb\"")?;
            *x = [(c >> 16) as f32 / 255.0, ((c >> 8) & 255) as f32 / 255.0, (c & 255) as f32 / 255.0];
        } else {
            let a = v.as_array().filter(|a| a.len() == 3).ok_or_else(|| format!("expected [x, y, z], not {v}"))?;
            for i in 0..3 { x[i] = num(&a[i])?; }
            if matches!(s.kind, Kind::Colour { .. }) { for c in x.iter_mut() { *c = c.clamp(0.0, 1.0); } }
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
            visible: true, collision: Collision::Solid, model: None, mover: Some(Mover { to: [0.0, 3.0, 0.0], secs: 2.0 }), surfaces: vec![], unique: false, tags: vec![], loose: None, light: None, camera: None, sound: None, sounds: vec![], sounds_like: None, strokes: vec![] }
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
        assert_eq!(sections(&p), vec!["PART", "POSITION  ·  ROTATION", "SIZE  ·  LOOK", "TAGS", "LIGHT", "CAMERA", "PLAYS", "MOVES"]);
        let d = describe(&p);
        let f = |name: &str| d.as_array().unwrap().iter().find(|x| x["field"] == name).unwrap().clone();
        assert_eq!(f("collision")["value"], "Solid");
        assert_eq!(f("size")["kind"], "number");
        assert_eq!(f("mover")["value"].as_array().unwrap().iter().find(|x| x["field"] == "secs").unwrap()["slider"]["words"][3], "Quick");
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
