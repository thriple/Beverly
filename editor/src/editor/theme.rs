//! The editor's look (the user's redesign, docs/ui/HANDOFF.md and docs/ui/tokens) with the Motion kit's movement
//! (kit/motion-kit.zip: its springs from `motion.ts`), ported to egui: colours, type, the tab colours, springs and
//! the small components the editor uses (keycap, status island, segmented control, toggle, hold button).
use bevy_egui::egui::{
    self, Align2, Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Id, Rect, Response,
    Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};
use std::sync::Arc;

// ---- colours (docs/ui/tokens/theme.rs, copied as tokens.rs) ----
pub use super::tokens::{
    AXIS_X, AXIS_Y, AXIS_Z, CAT_ACTIONS, CAT_CAMERAS, CAT_FLOW, CAT_GAME, CAT_LIGHTS, CAT_LOGIC, CAT_SHAPES, CAT_TRIGGERS,
    DANGER, DANGER_TEXT, FIELD, FIELD_HOVER, INK, LINE, POPOVER, TABS, TEXT, TEXT_2, TOGGLE_OFF, TRACK, TabTheme,
};
pub use super::tokens::PANEL_GLASS as PANEL;
pub use super::tokens::TEXT_3 as MUTED;

const fn rgb(c: u32) -> Color32 { Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8) }

/// The tab that is open (an index into `TABS`): the frame, highlight and accent are its colours.
static TAB: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub fn set_tab(i: usize) { TAB.store(i.min(TABS.len() - 1), std::sync::atomic::Ordering::Relaxed); }
pub fn tab() -> TabTheme { TABS[TAB.load(std::sync::atomic::Ordering::Relaxed)] }
/// The open tab's highlight (selected, where you are) and accent (values, what you can change).
pub fn highlight() -> Color32 { tab().highlight }
pub fn accent() -> Color32 { tab().accent }
/// egui's own colours that come from the tab (text selection, links), after the tab changes.
pub fn retint(ctx: &Context) {
    ctx.all_styles_mut(|s| {
        s.visuals.selection.bg_fill = accent().gamma_multiply(0.45);
        s.visuals.selection.stroke = Stroke::new(1.5, accent());
        s.visuals.hyperlink_color = accent();
    });
}
/// Saved (the green check).
pub const OK: Color32 = CAT_FLOW;
pub const fn white(a: u8) -> Color32 { Color32::from_rgba_unmultiplied_const(255, 255, 255, a) }
pub const SELECTED_ROW: Color32 = rgb(0x2A2A2F);
/// A glass panel's 1 px edge.
pub const EDGE: Color32 = white(26);

/// Texture handles and the face a texture would land on (never the move widget's colours).
pub const VIOLET: Color32 = Color32::from_rgb(0xb4, 0x8c, 0xff);

/// Panels (16) and controls (7).
pub const R_CARD: CornerRadius = CornerRadius::same(16);
pub const R_CTRL: CornerRadius = CornerRadius::same(7);

pub fn mono(size: f32) -> FontId { FontId::new(size, FontFamily::Monospace) }
/// Roboto Medium (500).
pub fn semi(size: f32) -> FontId { FontId::new(size, FontFamily::Name("medium".into())) }
/// Roboto Bold (700).
pub fn bold(size: f32) -> FontId { FontId::new(size, FontFamily::Name("bold".into())) }
pub fn body(size: f32) -> FontId { FontId::new(size, FontFamily::Proportional) }
/// Roboto Condensed Bold: the BEVERLY wordmark.
pub fn wordmark(size: f32) -> FontId { FontId::new(size, FontFamily::Name("wordmark".into())) }

pub fn setup(ctx: &Context) {
    let mut f = FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| { f.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes))); };
    add("roboto", include_bytes!("../../assets/fonts/Roboto-Regular.ttf"));
    add("roboto-medium", include_bytes!("../../assets/fonts/Roboto-Medium.ttf"));
    add("roboto-bold", include_bytes!("../../assets/fonts/Roboto-Bold.ttf"));
    add("roboto-condensed", include_bytes!("../../assets/fonts/RobotoCondensed-Bold.ttf"));
    add("roboto-mono", include_bytes!("../../assets/fonts/RobotoMono-Regular.ttf"));
    add("dejavu", include_bytes!("../../assets/fonts/DejaVuSans.ttf"));
    // Roboto for text, DejaVu Sans for the symbols Roboto lacks
    f.families.get_mut(&FontFamily::Proportional).unwrap().splice(0..0, ["roboto".to_string(), "dejavu".to_string()]);
    f.families.get_mut(&FontFamily::Monospace).unwrap().splice(0..0, ["roboto-mono".to_string(), "dejavu".to_string()]);
    for (family, font) in [("medium", "roboto-medium"), ("bold", "roboto-bold"), ("wordmark", "roboto-condensed")] {
        let mut fam = vec![font.to_string()];
        fam.extend(f.families[&FontFamily::Proportional].iter().cloned());
        f.families.insert(FontFamily::Name(family.into()), fam);
    }
    super::text::set_fonts(&f);
    ctx.set_fonts(f);

    ctx.all_styles_mut(|s| {
        s.text_styles.insert(egui::TextStyle::Body, body(12.5));
        s.text_styles.insert(egui::TextStyle::Button, body(12.5));
        s.text_styles.insert(egui::TextStyle::Small, body(11.0));
        s.text_styles.insert(egui::TextStyle::Monospace, mono(12.0));
        s.text_styles.insert(egui::TextStyle::Heading, bold(16.0));
        s.spacing.item_spacing = vec2(8.0, 8.0);
        s.spacing.button_padding = vec2(10.0, 5.0);
        s.spacing.interact_size.y = 28.0;
        let v = &mut s.visuals;
        *v = egui::Visuals::dark();
        v.override_text_color = Some(TEXT);
        v.panel_fill = PANEL;
        v.window_fill = POPOVER;
        v.extreme_bg_color = FIELD;
        v.faint_bg_color = white(8);
        v.window_stroke = Stroke::new(1.0, EDGE);
        v.window_corner_radius = CornerRadius::same(12);
        v.menu_corner_radius = CornerRadius::same(10);
        v.window_shadow = egui::Shadow { offset: [0, 10], blur: 34, spread: 0, color: Color32::from_black_alpha(140) };
        v.popup_shadow = egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(120) };
        v.selection.bg_fill = accent().gamma_multiply(0.45);
        v.selection.stroke = Stroke::new(1.5, accent());
        v.hyperlink_color = accent();
        for (w, fill) in [
            (&mut v.widgets.noninteractive, Color32::TRANSPARENT),
            (&mut v.widgets.inactive, FIELD),
            (&mut v.widgets.hovered, FIELD_HOVER),
            (&mut v.widgets.active, SELECTED_ROW),
            (&mut v.widgets.open, FIELD_HOVER),
        ] {
            w.corner_radius = R_CTRL;
            w.bg_fill = fill;
            w.weak_bg_fill = fill;
            w.bg_stroke = Stroke::NONE;
            w.fg_stroke = Stroke::new(1.0, TEXT);
            w.expansion = 0.0;
        }
        // buttons (a second action) are a step lighter than fields
        v.widgets.inactive.weak_bg_fill = SELECTED_ROW;
        v.widgets.hovered.weak_bg_fill = TRACK;
        v.widgets.active.weak_bg_fill = TOGGLE_OFF;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, white(18));
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_2);
    });
}

// ---- springs (motion.ts) ----
#[derive(Clone, Copy)]
pub struct Spring { pub k: f32, pub c: f32 }
pub const SPRING: Spring = Spring { k: 420.0, c: 32.0 }; // containers resizing
pub const SNAPPY: Spring = Spring { k: 520.0, c: 34.0 }; // menus, tooltips, pills
#[allow(dead_code)]
pub const SOFT: Spring = Spring { k: 400.0, c: 30.0 }; // toasts, snackbars

#[derive(Clone, Copy, Default)]
struct SpringState { x: f32, v: f32 }

/// A value that springs toward `target` (mass 1, like motion's springs: slight overshoot). Starts at `target`.
pub fn spring(ctx: &Context, id: Id, target: f32, s: Spring) -> f32 { spring_from(ctx, id, target, target, s) }

/// Like [`spring`], but a value seen for the first time starts at `start` (for things appearing).
pub fn spring_from(ctx: &Context, id: Id, start: f32, target: f32, s: Spring) -> f32 {
    let dt = ctx.input(|i| i.stable_dt).clamp(0.0, 1.0 / 30.0);
    let (x, moving) = ctx.data_mut(|d| {
        let st = d.get_temp_mut_or_insert_with(id, || SpringState { x: start, v: 0.0 });
        let n = 4;
        let h = dt / n as f32;
        for _ in 0..n {
            let a = -s.k * (st.x - target) - s.c * st.v;
            st.v += a * h;
            st.x += st.v * h;
        }
        let scale = target.abs().max(1.0);
        if (st.x - target).abs() < 1e-3 * scale && st.v.abs() < 1e-2 * scale {
            st.x = target;
            st.v = 0.0;
        }
        (st.x, st.v != 0.0)
    });
    if moving { ctx.request_repaint(); }
    x
}

/// Forget a spring, so the next [`spring_from`] starts over (call when the thing it animates closes).
pub fn spring_reset(ctx: &Context, id: Id) { ctx.data_mut(|d| d.remove::<SpringState>(id)); }

// ---- Keycaps (HANDOFF.md, "Keycaps") ----
pub fn kbd_size(ui: &Ui, key: &str) -> egui::Vec2 {
    let g = ui.painter().layout_no_wrap(key.to_owned(), semi(10.5), TEXT);
    vec2((g.size().x + 10.0).max(17.0), 17.0)
}

/// A keycap on a panel: dark, with a 2 px bottom edge.
pub fn paint_kbd(p: &egui::Painter, rect: Rect, key: &str, color: Color32) {
    let r = CornerRadius::same(4);
    p.rect_filled(rect, r, rgb(0x3A3A40));
    p.rect_filled(Rect::from_min_max(rect.min + vec2(1.0, 1.0), rect.max - vec2(1.0, 2.0)), CornerRadius::same(3), rgb(0x26262A));
    p.text(rect.center() - vec2(0.0, 0.5), Align2::CENTER_CENTER, key, semi(10.5), if color == TEXT { rgb(0xE6E6E8) } else { color });
}

/// A keycap on the frame: solid in the frame's text colour, with the frame's colour as its text.
pub fn paint_frame_kbd(p: &egui::Painter, rect: Rect, key: &str, fill: Color32, text: Color32) {
    p.rect_filled(rect, CornerRadius::same(5), fill);
    p.text(rect.center(), Align2::CENTER_CENTER, key, semi(10.5), text);
}

pub fn frame_kbd_size(ctx: &Context, key: &str) -> egui::Vec2 {
    vec2(ctx.fonts_mut(|f| f.layout_no_wrap(key.to_owned(), semi(10.5), TEXT).size().x) + 12.0, 18.0)
}

/// A keycap, laid out inline: `kbd(ui, "Ctrl")`.
pub fn kbd(ui: &mut Ui, key: &str) -> Response {
    let (rect, r) = ui.allocate_exact_size(kbd_size(ui, key), Sense::hover());
    paint_kbd(ui.painter(), rect, key, TEXT);
    r
}

/// Label followed by keycaps, the kit's way of showing shortcuts.
pub fn hint(ui: &mut Ui, label: &str, keys: &[&str]) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(egui::RichText::new(label).color(MUTED).size(12.0));
        for k in keys { kbd(ui, k); }
    });
}

// ---- Segmented control ----
/// 2-4 options; the highlight's pill slides to the chosen one (SNAPPY). Returns true when the choice changed.
pub fn segmented(ui: &mut Ui, id: &str, options: &[&str], selected: &mut usize, numbered: bool) -> bool {
    let id = ui.id().with(id);
    let font = body(12.0);
    let pad = 10.0;
    let widths: Vec<f32> = options
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let w = ui.painter().layout_no_wrap(o.to_string(), semi(12.0), TEXT).size().x;
            let n = if numbered { ui.painter().layout_no_wrap(format!("{}", i + 1), mono(11.0), TEXT).size().x + 4.0 } else { 0.0 };
            w + n + pad * 2.0
        })
        .collect();
    let h = 28.0;
    let total: f32 = widths.iter().sum::<f32>() + 4.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total, h), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(8), FIELD);
    let mut x = rect.left() + 2.0;
    let mut starts = Vec::new();
    for w in &widths {
        starts.push(x);
        x += w;
    }
    let tx = spring(ui.ctx(), id.with("x"), starts[*selected], SNAPPY);
    let tw = spring(ui.ctx(), id.with("w"), widths[*selected], SNAPPY);
    let ind = Rect::from_min_size(pos2(tx, rect.top() + 2.0), vec2(tw, h - 4.0));
    p.rect_filled(ind, CornerRadius::same(6), highlight());
    let mut changed = false;
    for (i, o) in options.iter().enumerate() {
        let r = Rect::from_min_size(pos2(starts[i], rect.top()), vec2(widths[i], h));
        let resp = ui.interact(r, id.with(i), Sense::click());
        if resp.clicked() && *selected != i {
            *selected = i;
            changed = true;
        }
        let on = ind.intersects(r.shrink2(vec2(widths[i] * 0.45, 0.0)));
        if !on && resp.hovered() { p.rect_filled(r.shrink(2.0), CornerRadius::same(6), white(13)); }
        let col = if on { tab().highlight_text } else if resp.hovered() { TEXT } else { TEXT_2 };
        let f = if on { semi(12.0) } else { font.clone() };
        let mut cx = r.left() + pad;
        if numbered {
            let g = ui.painter().layout_no_wrap(format!("{}", i + 1), mono(11.0), col);
            ui.painter().galley(pos2(cx, r.center().y - g.size().y / 2.0), g.clone(), col);
            cx += g.size().x + 4.0;
        }
        ui.painter().text(pos2(cx, r.center().y), Align2::LEFT_CENTER, o, f, col);
    }
    changed
}

// ---- Toggle (30 x 16: on is the accent with an ink knob) ----
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut r) = ui.allocate_exact_size(vec2(30.0, 16.0), Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    paint_toggle(ui, rect, *on, r.id);
    r
}

/// A toggle's track and knob in `rect` (30 x 16); the knob springs across.
pub fn paint_toggle(ui: &Ui, rect: Rect, on: bool, id: Id) {
    let t = spring(ui.ctx(), id.with("t"), if on { 1.0 } else { 0.0 }, SNAPPY);
    let p = ui.painter();
    let c = t.clamp(0.0, 1.0);
    p.rect_filled(rect, CornerRadius::same(8), lerp_color(TOGGLE_OFF, accent(), c));
    let x = egui::lerp(rect.left() + 8.0..=rect.right() - 8.0, t);
    p.circle_filled(pos2(x, rect.center().y), 6.0, lerp_color(rgb(0x9A9AA0), INK, c));
}

/// A chip that is on or off (a tag): a pill that fills with the highlight when on (SNAPPY).
pub fn chip(ui: &mut Ui, label: &str, on: &mut bool) -> Response {
    let w = ui.painter().layout_no_wrap(label.to_owned(), semi(12.0), TEXT).size().x;
    let (rect, mut r) = ui.allocate_exact_size(vec2(w + 22.0, 24.0), Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    let t = spring(ui.ctx(), r.id.with("t"), if *on { 1.0 } else { 0.0 }, SNAPPY).clamp(0.0, 1.0);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(12), lerp_color(if r.hovered() { FIELD_HOVER } else { FIELD }, highlight(), t));
    p.text(rect.center(), Align2::CENTER_CENTER, label, if *on { semi(12.0) } else { body(12.0) }, lerp_color(TEXT_2, INK, t));
    r
}

/// A choice of two to four in a row: equal parts across the width, the highlight's pill sliding to the chosen one.
pub fn choices(ui: &mut Ui, id: &str, options: &[&str], selected: &mut usize) -> bool {
    let id = ui.id().with(id);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(8), FIELD);
    let w = (rect.width() - 4.0) / options.len() as f32;
    let at = spring(ui.ctx(), id.with("x"), *selected as f32, SNAPPY);
    p.rect_filled(Rect::from_min_size(pos2(rect.left() + 2.0 + at * w, rect.top() + 2.0), vec2(w, 24.0)), CornerRadius::same(6), highlight());
    let mut changed = false;
    for (i, o) in options.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + 2.0 + i as f32 * w, rect.top() + 2.0), vec2(w, 24.0));
        let resp = ui.interact(r, id.with(i), Sense::click());
        let on = (at - i as f32).abs() < 0.5;
        if !on && resp.hovered() { ui.painter().rect_filled(r, CornerRadius::same(6), white(13)); }
        ui.painter().text(r.center(), Align2::CENTER_CENTER, *o, if on { semi(12.0) } else { body(12.0) }, if on { tab().highlight_text } else if resp.hovered() { TEXT } else { TEXT_2 });
        if resp.clicked() && *selected != i { *selected = i; changed = true; }
    }
    changed
}

/// A quiet button: the accent's text alone (More numbers, Stand upright).
pub fn quiet_button(ui: &mut Ui, label: &str) -> Response {
    let g = ui.painter().layout_no_wrap(label.to_owned(), semi(12.0), accent());
    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 4.0, 24.0), Sense::click());
    let c = if resp.hovered() { lerp_color(accent(), Color32::WHITE, 0.3) } else { accent() };
    ui.painter().galley(pos2(r.left() + 2.0, r.center().y - g.size().y / 2.0), g, c);
    resp
}

/// A panel's one main button: the highlight, with ink text.
pub fn main_button(ui: &mut Ui, label: &str) -> Response {
    let g = ui.painter().layout_no_wrap(label.to_owned(), semi(12.5), INK);
    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 24.0, 30.0), Sense::click());
    let hot = spring(ui.ctx(), resp.id.with("hot"), if resp.hovered() { 1.0 } else { 0.0 }, SNAPPY).clamp(0.0, 1.0);
    ui.painter().rect_filled(r, CornerRadius::same(8), lerp_color(highlight(), Color32::WHITE, 0.18 * hot));
    ui.painter().galley(r.center() - g.size() / 2.0, g, INK);
    resp
}

/// A text input (30 px, the field's colour; a 1.5 px accent ring while typing; `lead`: an icon first) whose letters
/// spring up as they're typed, as the kit's animated input does.
/// A widget that draws its own focus (fields, inputs, sliders) says so, and `focus_ring` leaves it alone.
pub fn own_focus(ctx: &Context, id: Id) { ctx.data_mut(|d| d.insert_temp(Id::new("own-focus"), id)); }

/// The focus ring (2 px accent, 2 px out) around whatever has keyboard focus, for egui's own buttons, menus and
/// edits; shown once Tab moves the focus, until the mouse is pressed. Call after everything is drawn.
pub fn focus_ring(ctx: &Context) {
    let k = Id::new("focus-by-keys");
    let (tab, press) = ctx.input(|i| (i.key_pressed(egui::Key::Tab), i.pointer.any_pressed()));
    let keys = if press { false } else { tab || ctx.data(|d| d.get_temp::<bool>(k)).unwrap_or(false) };
    ctx.data_mut(|d| d.insert_temp(k, keys));
    let Some(id) = ctx.memory(|m| m.focused()) else { return };
    if !keys || ctx.data(|d| d.get_temp::<Id>(Id::new("own-focus"))) == Some(id) { return; }
    let Some(r) = ctx.read_response(id) else { return };
    ctx.layer_painter(r.layer_id).rect_stroke(r.rect.expand(2.0), CornerRadius::same(9), Stroke::new(2.0, accent()), StrokeKind::Outside);
}

pub fn text_input(ui: &mut Ui, text: &mut String, hint: &str, lead: Option<&str>, width: f32) -> Response {
    let font = body(12.5);
    let left = if lead.is_some() { 32.0 } else { 10.0 };
    let out = egui::TextEdit::singleline(text)
        .hint_text(egui::RichText::new(hint).color(MUTED))
        .font(font.clone())
        .text_color(Color32::TRANSPARENT)
        .desired_width(width)
        .margin(egui::Margin { left: left as i8, right: 10, top: 7, bottom: 7 })
        .show(ui);
    let resp = out.response.response;
    let r = resp.rect;
    if let Some(icon) = lead { super::frame::paint_icon(ui.painter(), icon, pos2(r.left() + 10.0 + 7.0, r.center().y), 14.0, MUTED); }
    if resp.has_focus() { own_focus(ui.ctx(), resp.id); ui.painter().rect_stroke(r, CornerRadius::same(7), Stroke::new(1.5, accent()), StrokeKind::Inside); }
    // each letter keeps an id while it stays, so only the ones typed spring in
    let key = resp.id.with("letters");
    let (before, mut ids, mut next): (String, Vec<u64>, u64) = ui.data(|d| d.get_temp(key)).unwrap_or_default();
    if before != *text {
        let (a, b): (Vec<char>, Vec<char>) = (before.chars().collect(), text.chars().collect());
        let start = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
        let end = a[start..].iter().rev().zip(b[start..].iter().rev()).take_while(|(x, y)| x == y).count();
        let added: Vec<u64> = (0..b.len() - start - end).map(|k| next + k as u64).collect();
        next += added.len() as u64;
        let mut fresh = ids[..start.min(ids.len())].to_vec();
        fresh.extend(added);
        fresh.extend(ids[ids.len().saturating_sub(end)..].iter().copied());
        ids = fresh;
        ui.data_mut(|d| d.insert_temp(key, (text.clone(), ids.clone(), next)));
    }
    let land = Spring { k: 600.0, c: 18.0 };
    let p = ui.painter().with_clip_rect(r.shrink(1.0));
    if let Some(row) = out.galley.rows.first() {
        for (g, id) in row.glyphs.iter().zip(&ids) {
            let t = spring_from(ui.ctx(), key.with(*id), 0.0, 1.0, land);
            let at = out.galley_pos + vec2(g.pos.x, row.pos.y + g.pos.y - g.font_ascent + (1.0 - t) * 0.7 * font.size);
            p.text(at, Align2::LEFT_TOP, g.chr, font.clone(), TEXT.gamma_multiply(t.clamp(0.0, 1.0)));
        }
    }
    resp
}

/// A colour's friendly name ("Warm grey", "Deep blue"), worked out from its hue, saturation and lightness.
pub fn colour_name(c: [f32; 3]) -> String {
    let (r, g, b) = (c[0], c[1], c[2]);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    let d = mx - mn;
    let s = if d < 1e-6 { 0.0 } else { d / (1.0 - (2.0 * l - 1.0).abs()).max(1e-6) };
    let mut h = if d < 1e-6 { 0.0 } else if mx == r { 60.0 * (((g - b) / d) % 6.0) } else if mx == g { 60.0 * ((b - r) / d + 2.0) } else { 60.0 * ((r - g) / d + 4.0) };
    if h < 0.0 { h += 360.0; }
    if s < 0.12 || d < 0.04 {
        let grey = if l > 0.93 { "White" } else if l > 0.7 { "Light grey" } else if l > 0.38 { "Grey" } else if l > 0.12 { "Dark grey" } else { "Black" };
        if d >= 0.02 && (0.12..0.93).contains(&l) { return format!("{} {}", if !(70.0..=300.0).contains(&h) { "Warm" } else { "Cool" }, grey.to_lowercase()); }
        return grey.into();
    }
    let hue = match h as u32 { 0..=14 | 345..=360 => "red", 15..=40 => "orange", 41..=65 => "yellow", 66..=100 => "lime", 101..=150 => "green", 151..=180 => "teal", 181..=200 => "cyan", 201..=240 => "blue", 241..=265 => "indigo", 266..=290 => "violet", 291..=320 => "purple", _ => "pink" };
    let hue = if hue == "orange" && l < 0.4 { "brown" } else { hue };
    let shade = if l > 0.78 { "Pale " } else if l > 0.62 { "Light " } else if l < 0.28 { "Deep " } else if l < 0.4 { "Dark " } else { "" };
    let word = format!("{shade}{hue}");
    let mut ch = word.chars();
    ch.next().map(|f| f.to_uppercase().collect::<String>() + ch.as_str()).unwrap_or_default()
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

// ---- Hold button (press and hold to confirm; a red fill tracks the hold) ----
/// Returns true on the frame the hold completes (`secs` long).
pub fn hold_button(ui: &mut Ui, label: &str, secs: f32) -> bool {
    let w = ui.painter().layout_no_wrap(label.to_owned(), semi(13.0), TEXT).size().x + 46.0;
    let (rect, r) = ui.allocate_exact_size(vec2(w, 32.0), Sense::click_and_drag());
    let id = r.id.with("hold");
    let dt = ui.input(|i| i.stable_dt);
    let held = r.is_pointer_button_down_on();
    let mut t: f32 = ui.data(|d| d.get_temp(id).unwrap_or(0.0));
    let mut done = false;
    if held {
        t += dt / secs;
        if t >= 1.0 {
            done = true;
            t = 0.0;
        }
        ui.ctx().request_repaint();
    } else {
        t = 0.0;
    }
    ui.data_mut(|d| d.insert_temp(id, t));
    let p = ui.painter();
    p.rect_filled(rect, R_CTRL, if r.hovered() { FIELD_HOVER } else { SELECTED_ROW });
    if t > 0.0 {
        let mut f = rect;
        f.set_width(rect.width() * t);
        p.rect_filled(f, R_CTRL, DANGER.gamma_multiply(0.4));
    }
    let c = pos2(rect.left() + 17.0, rect.center().y);
    p.circle_stroke(c, 7.0, Stroke::new(1.5, white(80)));
    if t > 0.0 {
        let pts: Vec<_> = (0..=32)
            .map(|i| {
                let a = -std::f32::consts::FRAC_PI_2 + t * std::f32::consts::TAU * i as f32 / 32.0;
                c + vec2(a.cos(), a.sin()) * 7.0
            })
            .collect();
        p.add(egui::Shape::line(pts, Stroke::new(2.0, DANGER)));
    }
    p.text(pos2(rect.left() + 32.0, rect.center().y), Align2::LEFT_CENTER, label, semi(13.0), TEXT);
    done
}

// ---- Dynamic island (status pill that springs open for each message) ----
#[derive(Clone, Copy, PartialEq)]
pub enum Tone { Plain, Accent, Danger }

#[derive(Clone)]
pub struct Toast { pub glyph: String, pub label: String, pub sub: String, pub tone: Tone }

#[derive(Default, bevy::prelude::Resource)]
pub struct Island { toast: Option<(Toast, f64)>, serial: u64, history: std::collections::VecDeque<String> }

impl Island {
    pub fn show(&mut self, glyph: &str, label: impl Into<String>, sub: impl Into<String>, tone: Tone) {
        self.serial += 1;
        let (label, sub) = (label.into(), sub.into());
        self.history.push_back(if sub.is_empty() { label.clone() } else { format!("{label} · {sub}") });
        if self.history.len() > 40 { self.history.pop_front(); }
        self.toast = Some((Toast { glyph: glyph.into(), label, sub, tone }, f64::NAN));
    }

    /// The messages shown so far (the last 40), oldest first.
    pub fn history(&self) -> &std::collections::VecDeque<String> { &self.history }

    /// Draw the island with its top-left at `at` (the view's top left): resting, a status pill (an icon in the
    /// accent and `rest`); for each message it springs open to show it, then settles back. Returns where it is.
    pub fn draw(&mut self, ctx: &Context, at: egui::Pos2, icon: &str, rest: &str) -> Rect {
        let now = ctx.input(|i| i.time);
        if let Some((_, until)) = &mut self.toast {
            if until.is_nan() { *until = now + 1.8; }
            if now > *until { self.toast = None; }
        }
        let id = Id::new("island");
        let open = self.toast.is_some();
        let text_w = |t: &str, f: FontId| ctx.fonts_mut(|fo| fo.layout_no_wrap(t.to_owned(), f, TEXT).size().x);
        let rest_w = text_w(rest, body(12.0)) + 12.0 + 14.0 + 8.0 + 12.0;
        let (tw, th) = if let Some((t, _)) = &self.toast {
            ((text_w(&t.label, semi(12.5)).max(text_w(&t.sub, body(11.0))) + 8.0 + 28.0 + 8.0 + 16.0).max(rest_w), 44.0)
        } else {
            (rest_w, 28.0)
        };
        let w = spring(ctx, id.with("w"), tw, SPRING);
        let h = spring(ctx, id.with("h"), th, SPRING);
        let rect = Rect::from_min_size(at, vec2(w, h));
        egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(rect.min).interactable(false).show(ctx, |ui| {
            let p = ui.painter();
            let round = CornerRadius::same((h / 2.0) as u8);
            p.rect_filled(rect, round, Color32::from_rgba_unmultiplied_const(14, 14, 16, 184));
            p.rect_stroke(rect, round, Stroke::new(1.0, EDGE), StrokeKind::Inside);
            // content fades in from a slight scale-down (the kit blurs in; egui can't blur, so fade + scale)
            let key = if open { self.serial } else { 0 };
            let a = spring_from(ctx, id.with(("a", key)), 0.0, 1.0, SNAPPY).clamp(0.0, 1.0);
            let fade = |c: Color32| c.gamma_multiply(a);
            let clip = p.with_clip_rect(rect.shrink(1.0));
            if let Some((t, _)) = &self.toast {
                let gc = match t.tone { Tone::Accent => accent(), Tone::Danger => DANGER, Tone::Plain => TEXT };
                let gr = Rect::from_center_size(pos2(rect.left() + 8.0 + 14.0, rect.center().y), vec2(28.0, 28.0));
                clip.circle_filled(gr.center(), 14.0, fade(gc));
                clip.text(gr.center(), Align2::CENTER_CENTER, &t.glyph, semi(13.0), fade(INK));
                let x = gr.right() + 8.0;
                if t.sub.is_empty() {
                    clip.text(pos2(x, rect.center().y), Align2::LEFT_CENTER, &t.label, semi(12.5), fade(TEXT));
                } else {
                    clip.text(pos2(x, rect.center().y - 7.5), Align2::LEFT_CENTER, &t.label, semi(12.5), fade(TEXT));
                    clip.text(pos2(x, rect.center().y + 8.5), Align2::LEFT_CENTER, &t.sub, body(11.0), fade(TEXT_2));
                }
            } else {
                super::frame::paint_icon(&clip, icon, pos2(rect.left() + 12.0 + 7.0, rect.center().y), 14.0, fade(accent()));
                clip.text(pos2(rect.left() + 12.0 + 14.0 + 8.0, rect.center().y), Align2::LEFT_CENTER, rest, body(12.0), fade(TEXT));
            }
        });
        rect
    }
}

/// A small floating label in the tooltip's look, at a screen point (the move widget's readouts, what's being placed).
pub fn float_label(ctx: &Context, id: &str, at: egui::Pos2, text: &str, color: Color32, keys: &[&str]) {
    egui::Area::new(Id::new(id)).order(egui::Order::Tooltip).fixed_pos(at).interactable(false).show(ctx, |ui| {
        egui::Frame::new().fill(POPOVER).stroke(Stroke::new(1.0, color)).corner_radius(CornerRadius::same(8)).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(egui::RichText::new(text).font(semi(12.0)).color(TEXT));
                for k in keys { kbd(ui, k); }
            });
        });
    });
}

// ---- Tooltips (the redesign's light pill; the kit's spring up from 96%) ----
pub trait Tip {
    /// A tooltip while the pointer rests on it.
    fn tip(self, text: impl Into<String>) -> Self;
    /// A tooltip with the shortcut's keycaps after its name ("Duplicate  Ctrl D").
    fn tip_keys(self, text: impl Into<String>, keys: &[&str]) -> Self;
}

impl Tip for Response {
    fn tip(self, text: impl Into<String>) -> Self { self.tip_keys(text, &[]) }
    fn tip_keys(self, text: impl Into<String>, keys: &[&str]) -> Self {
        tooltip(&self.ctx, self.id.with("tip"), self.rect, self.contains_pointer() && !self.is_pointer_button_down_on(), &text.into(), keys);
        self
    }
}

fn tooltip(ctx: &Context, id: Id, over: Rect, hovered: bool, text: &str, keys: &[&str]) {
    let now = ctx.input(|i| i.time);
    if !hovered {
        ctx.data_mut(|d| d.remove::<f64>(id));
        spring_reset(ctx, id);
        return;
    }
    let since = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(id, || now));
    if now - since < 0.35 {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(0.36 - (now - since)));
        return;
    }
    let t = spring_from(ctx, id, 0.0, 1.0, SNAPPY);
    let g = ctx.fonts_mut(|f| f.layout(text.to_owned(), semi(12.0), INK, 280.0));
    let kw: Vec<f32> = keys.iter().map(|k| ctx.fonts_mut(|f| f.layout_no_wrap(k.to_string(), semi(10.5), TEXT).size().x) + 10.0).collect();
    let keys_w = kw.iter().map(|w| w + 4.0).sum::<f32>();
    let size = vec2(10.0 + g.size().x + if keys.is_empty() { 0.0 } else { 8.0 + keys_w - 4.0 } + 10.0, (g.size().y + 12.0).max(28.0));
    let screen = ctx.content_rect();
    // above the thing, or under it when there's no room above
    let above = over.top() - 10.0 - size.y > screen.top() + 4.0;
    let dy = if above { 4.0 } else { -4.0 } * (1.0 - t);
    let x = (over.center().x - size.x / 2.0).clamp(screen.left() + 4.0, (screen.right() - 4.0 - size.x).max(screen.left()));
    let y = if above { over.top() - 10.0 - size.y } else { over.bottom() + 10.0 };
    let rect = Rect::from_min_size(pos2(x, y + dy), size);
    let resp = egui::Area::new(id).order(egui::Order::Tooltip).fixed_pos(rect.min).interactable(false).show(ctx, |ui| {
        ui.multiply_opacity(t.clamp(0.0, 1.0));
        let p = ui.painter();
        p.add(egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(100) }.as_shape(rect, CornerRadius::same(8)));
        p.rect_filled(rect, CornerRadius::same(8), TEXT);
        p.galley(pos2(rect.left() + 10.0, rect.center().y - g.size().y / 2.0), g.clone(), INK);
        let mut kx = rect.left() + 10.0 + g.size().x + 8.0;
        for (k, w) in keys.iter().zip(kw) {
            let kr = Rect::from_min_size(pos2(kx, rect.center().y - 8.5), vec2(w, 17.0));
            p.rect_filled(kr, CornerRadius::same(4), INK);
            p.text(kr.center(), Align2::CENTER_CENTER, *k, semi(10.5), TEXT);
            kx += w + 4.0;
        }
        ui.allocate_rect(rect, Sense::hover());
    });
    let s = 0.96 + 0.04 * t.clamp(0.0, 1.0);
    let c = if above { rect.center_bottom() } else { rect.center_top() };
    ctx.set_transform_layer(resp.response.layer_id, egui::emath::TSTransform::new(c.to_vec2() * (1.0 - s), s));
}
