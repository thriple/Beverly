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

mod colour;
mod json;
mod widgets;
pub use colour::*;
pub use json::*;
pub use widgets::*;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{Collision, Mover, Part, Shape};

    fn part() -> Part {
        Part { id: 1, name: "Pad".into(), shape: Shape::Box, pos: [0.0; 3], rot: [0.0, 0.0, 0.0, 1.0], size: [1.0; 3], color: [0.5; 3],
            visible: true, collision: Collision::Solid, model: None, mover: Some(Mover { to: [0.0, 3.0, 0.0], secs: 2.0, ..Default::default() }), surfaces: vec![], unique: false, tags: vec![], loose: None, light: None, camera: None, sound: None, sounds: vec![], sounds_like: None, strokes: vec![], parent: None }
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
        assert_eq!(t["choices"].as_array().unwrap().len(), 8);
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
        assert_eq!(p.mover, Some(Mover { to: [0.0, 3.0, 0.0], secs: 0.5, ..Default::default() }));
        assert!(!p.visible);
        assert!(apply(&mut p, &serde_json::json!({"Moves": false})).is_ok() && p.mover.is_none());
        assert!(apply(&mut p, &serde_json::json!({"Wobble": 1})).unwrap_err().contains("Colour"));
        assert!(apply(&mut p, &serde_json::json!({"Collision": "Sticky"})).unwrap_err().contains("Walk-through"));
        let d = describe(&p);
        assert_eq!(d.as_array().unwrap().iter().find(|x| x["field"] == "rot").unwrap()["degrees"], 90.0);
    }
}
