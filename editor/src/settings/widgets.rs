//! The controls a settings panel draws: rows, word sliders, choices, tags, optional parts and lists.
use super::*;

/// The label column's width, and how far an icon before a label moves it along.
pub(super) const LABEL_W: f32 = 76.0;
pub(super) const ICON_W: f32 = 21.0;

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
pub(super) fn changed_bar(ui: &mut Ui, r: egui::Rect) -> bool {
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

/// A shader's settings (its header, shader.rs) as rows: the values this material set, else the shader's defaults. A value
/// put back to the default is forgotten. True when something changed.
pub fn shader_settings(ui: &mut Ui, header: &crate::shader::Header, set: &mut BTreeMap<String, Vec<f32>>, palette: &[[f32; 3]]) -> bool {
    use crate::shader::Kind as K;
    let mut changed = false;
    for s in &header.settings {
        let own = set.get(&s.name);
        let mut v = [s.default[0], s.default[1], s.default[2]];
        if let Some(o) = own { for (i, x) in o.iter().take(3).enumerate() { v[i] = *x; } }
        let before = v;
        let differs = v != s.default;
        let mut reset = false;
        match s.kind {
            K::Number { min, max } => { row(ui, "", &s.label, "", differs, &mut reset, |ui| number(ui, &mut v[0], (max - min) / 200.0, min, max, "", None, ui.available_width())); }
            K::Colour => { row(ui, "", &s.label, "", differs, &mut reset, |ui| colour(ui, &mut v, None, &s.name, palette)); }
            K::Toggle => { let mut on = v[0] > 0.5; toggle_row(ui, "", &s.label, &mut on, "", differs, &mut reset); v[0] = on as u8 as f32; }
        }
        if reset { v = s.default; }
        if v != before {
            if v == s.default { set.remove(&s.name); } else { set.insert(s.name.clone(), if s.kind == K::Colour { v.to_vec() } else { vec![v[0]] }); }
            changed = true;
        }
    }
    changed
}

/// One setting that isn't a slider.
#[allow(clippy::too_many_arguments)]
pub(super) fn one(ui: &mut Ui, f: &mut dyn PartialReflect, s: &Setting, key: &str, differs: bool, reset: &mut bool, alpha: Option<&mut f32>, ctx: &mut Ctx) -> bool {
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

pub(super) fn unit(s: &Setting) -> String { if s.unit.is_empty() { String::new() } else if s.unit == "×" || s.unit == "°" { s.unit.into() } else { format!(" {}", s.unit) } }

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

pub(super) fn turn(ui: &mut Ui, r: &mut [f32; 4], s: &Setting) -> bool {
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
pub(super) fn choice(ui: &mut Ui, f: &mut dyn PartialReflect, s: &Setting, key: &str, differs: bool, reset: &mut bool) -> bool {
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
pub(super) fn tag_set(f: &dyn PartialReflect) -> Option<(Vec<(&'static str, &'static str)>, Vec<String>)> {
    let Some(TypeInfo::List(l)) = f.get_represented_type_info() else { return None };
    let Some(TypeInfo::Enum(info)) = l.item_info() else { return None };
    let all = info.iter().map(|v| (v.name(), v.get_attribute::<Label>().map_or(v.name(), |l| l.0))).collect();
    let ReflectRef::List(items) = f.reflect_ref() else { return None };
    let on = items.iter().filter_map(|i| match i.reflect_ref() { ReflectRef::Enum(e) => Some(e.variant_name().to_string()), _ => None }).collect();
    Some((all, on))
}

/// Make a `Vec<E>` hold `names` (variant names), in E's order.
pub(super) fn set_tags(f: &mut dyn PartialReflect, all: &[(&'static str, &'static str)], names: &[&str]) {
    let ReflectMut::List(l) = f.reflect_mut() else { return };
    while l.len() > 0 { l.remove(l.len() - 1); }
    for (n, _) in all.iter().filter(|(n, _)| names.contains(n)) { l.push(Box::new(DynamicEnum::new(*n, DynamicVariant::Unit))); }
}

pub(super) fn tags(ui: &mut Ui, f: &mut dyn PartialReflect, s: &Setting) -> bool {
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
pub(super) fn optional(ui: &mut Ui, f: &mut dyn PartialReflect, s: &Setting, off: &str, on_word: &str, differs: bool, reset: &mut bool, ctx: &mut Ctx) -> bool {
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

pub(super) fn list(ui: &mut Ui, f: &mut dyn PartialReflect, s: &Setting, key: &str, ctx: &mut Ctx) -> bool {
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
pub(super) fn more(ui: &mut Ui, id: egui::Id, open: bool) -> bool {
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
pub(super) fn feel(ui: &mut Ui, st: &mut dyn Struct, ty: &str, group: &str, words: &[&str], members: &[(&'static str, Setting)], default: Option<f32>, ctx: &mut Ctx) -> bool {
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
pub(crate) fn colour(ui: &mut Ui, c: &mut [f32; 3], mut alpha: Option<&mut f32>, key: &str, palette: &[[f32; 3]]) -> bool {
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
