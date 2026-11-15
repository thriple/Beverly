//! The Motion kit's look, ported to egui: colours, corner radii, springs and the small kit components the
//! editor uses (keycap, status island, segmented control, toggle, hold button). Values come from the kit's
//! `kit.css` and `motion.ts` (kit/motion-kit.zip); keep them in step with it.
use bevy_egui::egui::{
    self, Align2, Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Id, Rect, Response,
    Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};
use std::sync::Arc;

// ---- colours (kit.css --kit-*) ----
pub const ACCENT: Color32 = Color32::from_rgb(0xf5, 0xdc, 0x4a);
pub const DANGER: Color32 = Color32::from_rgb(0xff, 0x5e, 0x3a);
#[allow(dead_code)]
pub const OK: Color32 = Color32::from_rgb(0x7b, 0xe0, 0x8a);
pub const INK: Color32 = Color32::from_rgb(0x0a, 0x08, 0x09);
pub const TEXT: Color32 = Color32::from_rgb(0xf2, 0xf0, 0xf3);
pub const fn white(a: u8) -> Color32 { Color32::from_rgba_unmultiplied_const(255, 255, 255, a) }
pub const PANEL: Color32 = Color32::from_rgba_unmultiplied_const(12, 9, 10, 240); // rgb(12 9 10 / .94)
pub const LINE: Color32 = white(36); // white 14%
pub const MUTED: Color32 = white(140);

// Part categories (the parts pop-up's section bars, wires, part markers in the world).
pub const CAT_SHAPES: Color32 = Color32::from_rgb(0x8a, 0x93, 0xa6);
pub const CAT_TRIGGERS: Color32 = Color32::from_rgb(0xf0, 0x8c, 0x42);
pub const CAT_ACTIONS: Color32 = Color32::from_rgb(0x5b, 0x9b, 0xf0);
pub const CAT_FLOW: Color32 = Color32::from_rgb(0x48, 0xc0, 0x8c);
pub const CAT_LOGIC: Color32 = Color32::from_rgb(0xa7, 0x7c, 0xe6);
pub const CAT_GAME: Color32 = Color32::from_rgb(0xe0, 0x68, 0x8f);
pub const CAT_LIGHTS: Color32 = Color32::from_rgb(0xf2, 0xc1, 0x4e);
pub const CAT_CAMERAS: Color32 = Color32::from_rgb(0x5e, 0xc8, 0xd8);

// Widget axes.
/// Texture handles and the face a texture would land on (never the move widget's colours).
pub const VIOLET: Color32 = Color32::from_rgb(0xb4, 0x8c, 0xff);
pub const AXIS_X: Color32 = Color32::from_rgb(0xf0, 0x6a, 0x60);
pub const AXIS_Y: Color32 = Color32::from_rgb(0x5c, 0xc2, 0x5c);
pub const AXIS_Z: Color32 = Color32::from_rgb(0x6a, 0x93, 0xf5);

/// The kit's asymmetric corners: rounded-[14px_14px_14px_4px] (bottom-left small).
pub const R_CARD: CornerRadius = CornerRadius { nw: 14, ne: 14, se: 14, sw: 4 };
pub const R_CTRL: CornerRadius = CornerRadius::same(10);

pub fn mono(size: f32) -> FontId { FontId::new(size, FontFamily::Monospace) }
pub fn semi(size: f32) -> FontId { FontId::new(size, FontFamily::Name("semi".into())) }
pub fn body(size: f32) -> FontId { FontId::new(size, FontFamily::Proportional) }

pub fn setup(ctx: &Context) {
    let mut f = FontDefinitions::default();
    f.font_data.insert("inter".into(), Arc::new(FontData::from_static(include_bytes!("../../assets/fonts/Inter-Regular.otf"))));
    f.font_data.insert("inter-semi".into(), Arc::new(FontData::from_static(include_bytes!("../../assets/fonts/Inter-SemiBold.otf"))));
    f.font_data.insert("dejavu".into(), Arc::new(FontData::from_static(include_bytes!("../../assets/fonts/DejaVuSans.ttf"))));
    f.font_data.insert("jbmono".into(), Arc::new(FontData::from_static(include_bytes!("../../assets/fonts/JetBrainsMono-Medium.ttf"))));
    // Inter for text, DejaVu Sans for the symbols Inter lacks
    f.families.get_mut(&FontFamily::Proportional).unwrap().splice(0..0, ["inter".to_string(), "dejavu".to_string()]);
    f.families.get_mut(&FontFamily::Monospace).unwrap().splice(0..0, ["jbmono".to_string(), "dejavu".to_string()]);
    let mut semi_fam = vec!["inter-semi".to_string()];
    semi_fam.extend(f.families[&FontFamily::Proportional].iter().cloned());
    f.families.insert(FontFamily::Name("semi".into()), semi_fam);
    ctx.set_fonts(f);

    ctx.all_styles_mut(|s| {
        s.text_styles.insert(egui::TextStyle::Body, body(13.0));
        s.text_styles.insert(egui::TextStyle::Button, body(13.0));
        s.text_styles.insert(egui::TextStyle::Small, body(11.0));
        s.text_styles.insert(egui::TextStyle::Monospace, mono(12.0));
        s.text_styles.insert(egui::TextStyle::Heading, semi(15.0));
        s.spacing.item_spacing = vec2(8.0, 6.0);
        s.spacing.button_padding = vec2(10.0, 5.0);
        s.spacing.interact_size.y = 26.0;
        let v = &mut s.visuals;
        *v = egui::Visuals::dark();
        v.override_text_color = Some(TEXT);
        v.panel_fill = PANEL;
        v.window_fill = PANEL;
        v.extreme_bg_color = INK;
        v.faint_bg_color = white(8);
        v.window_stroke = Stroke::new(1.0, LINE);
        v.window_corner_radius = R_CARD;
        v.menu_corner_radius = R_CARD;
        v.window_shadow = egui::Shadow { offset: [0, 10], blur: 34, spread: 0, color: Color32::from_black_alpha(140) };
        v.popup_shadow = egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(120) };
        v.selection.bg_fill = Color32::from_rgba_unmultiplied(0xf5, 0xdc, 0x4a, 70);
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.hyperlink_color = ACCENT;
        for (w, fill) in [
            (&mut v.widgets.noninteractive, Color32::TRANSPARENT),
            (&mut v.widgets.inactive, white(15)),
            (&mut v.widgets.hovered, white(26)),
            (&mut v.widgets.active, white(36)),
            (&mut v.widgets.open, white(26)),
        ] {
            w.corner_radius = R_CTRL;
            w.bg_fill = fill;
            w.weak_bg_fill = fill;
            w.bg_stroke = Stroke::new(1.0, LINE);
            w.fg_stroke = Stroke::new(1.0, TEXT);
            w.expansion = 0.0;
        }
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, white(70));
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

// ---- Kbd (kit .kc) ----
pub fn kbd_size(ui: &Ui, key: &str) -> egui::Vec2 {
    let g = ui.painter().layout_no_wrap(key.to_owned(), semi(10.0), TEXT);
    vec2((g.size().x + 10.0).max(20.0), 20.0)
}

pub fn paint_kbd(p: &egui::Painter, rect: Rect, key: &str, color: Color32) {
    p.rect_filled(rect, CornerRadius::same(5), white(15));
    p.rect_stroke(rect, CornerRadius::same(5), Stroke::new(1.0, white(56)), StrokeKind::Inside);
    p.line_segment([pos2(rect.left() + 3.0, rect.bottom() - 1.0), pos2(rect.right() - 3.0, rect.bottom() - 1.0)], Stroke::new(1.0, white(56)));
    p.text(rect.center() - vec2(0.0, 0.5), Align2::CENTER_CENTER, key, semi(10.0), color);
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
/// Options with a sliding white indicator (SNAPPY). Returns true when the choice changed.
pub fn segmented(ui: &mut Ui, id: &str, options: &[&str], selected: &mut usize, numbered: bool) -> bool {
    let id = ui.id().with(id);
    let font = semi(12.0);
    let pad = 10.0;
    let widths: Vec<f32> = options
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let w = ui.painter().layout_no_wrap(o.to_string(), font.clone(), TEXT).size().x;
            let n = if numbered { ui.painter().layout_no_wrap(format!("{}", i + 1), mono(9.0), TEXT).size().x + 4.0 } else { 0.0 };
            w + n + pad * 2.0
        })
        .collect();
    let h = 28.0;
    let total: f32 = widths.iter().sum::<f32>() + 6.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total, h), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, R_CTRL, white(12));
    p.rect_stroke(rect, R_CTRL, Stroke::new(1.0, LINE), StrokeKind::Inside);
    let mut x = rect.left() + 3.0;
    let mut starts = Vec::new();
    for w in &widths {
        starts.push(x);
        x += w;
    }
    let tx = spring(ui.ctx(), id.with("x"), starts[*selected], SNAPPY);
    let tw = spring(ui.ctx(), id.with("w"), widths[*selected], SNAPPY);
    let ind = Rect::from_min_size(pos2(tx, rect.top() + 3.0), vec2(tw, h - 6.0));
    p.rect_filled(ind, CornerRadius::same(8), TEXT);
    let mut changed = false;
    for (i, o) in options.iter().enumerate() {
        let r = Rect::from_min_size(pos2(starts[i], rect.top()), vec2(widths[i], h));
        let resp = ui.interact(r, id.with(i), Sense::click());
        if resp.clicked() && *selected != i {
            *selected = i;
            changed = true;
        }
        let on = ind.intersects(r.shrink2(vec2(widths[i] * 0.45, 0.0)));
        let col = if on { INK } else if resp.hovered() { TEXT } else { white(190) };
        let mut cx = r.left() + pad;
        if numbered {
            let g = ui.painter().layout_no_wrap(format!("{}", i + 1), mono(9.0), if on { INK } else { MUTED });
            ui.painter().galley(pos2(cx, r.center().y - g.size().y / 2.0), g.clone(), col);
            cx += g.size().x + 4.0;
        }
        ui.painter().text(pos2(cx, r.center().y), Align2::LEFT_CENTER, o, font.clone(), col);
    }
    changed
}

// ---- Toggle (kit style: accent when on, knob springs) ----
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut r) = ui.allocate_exact_size(vec2(32.0, 18.0), Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    let t = spring(ui.ctx(), r.id.with("t"), if *on { 1.0 } else { 0.0 }, SNAPPY);
    let p = ui.painter();
    let bg = lerp_color(white(30), ACCENT, t.clamp(0.0, 1.0));
    p.rect_filled(rect, CornerRadius::same(9), bg);
    let x = egui::lerp(rect.left() + 9.0..=rect.right() - 9.0, t);
    p.circle_filled(pos2(x, rect.center().y), 7.0, if *on { INK } else { TEXT });
    r
}

/// A chip that is on or off (a tag): a pill that fills with the accent when on (SNAPPY).
pub fn chip(ui: &mut Ui, label: &str, on: &mut bool) -> Response {
    let font = semi(12.0);
    let w = ui.painter().layout_no_wrap(label.to_owned(), font.clone(), TEXT).size().x;
    let (rect, mut r) = ui.allocate_exact_size(vec2(w + 22.0, 24.0), Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    let t = spring(ui.ctx(), r.id.with("t"), if *on { 1.0 } else { 0.0 }, SNAPPY).clamp(0.0, 1.0);
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(12), lerp_color(white(12), ACCENT, t));
    p.rect_stroke(rect, CornerRadius::same(12), Stroke::new(1.0, if r.hovered() { white(90) } else { LINE }), StrokeKind::Inside);
    p.text(rect.center(), Align2::CENTER_CENTER, label, font, lerp_color(TEXT, INK, t));
    r
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
    p.rect_filled(rect, R_CTRL, white(15));
    if t > 0.0 {
        let mut f = rect;
        f.set_width(rect.width() * t);
        p.rect_filled(f, R_CTRL, Color32::from_rgba_unmultiplied(0xff, 0x5e, 0x3a, 150));
    }
    p.rect_stroke(rect, R_CTRL, Stroke::new(1.0, if r.hovered() { white(80) } else { LINE }), StrokeKind::Inside);
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

    /// Draw the island centred at `top` (the top-centre of the viewport). `rest` is the resting content.
    pub fn draw(&mut self, ctx: &Context, top: egui::Pos2, rest: &str, rest_keys: &[&str]) {
        let now = ctx.input(|i| i.time);
        if let Some((_, until)) = &mut self.toast {
            if until.is_nan() { *until = now + 1.8; }
            if now > *until { self.toast = None; }
        }
        let id = Id::new("island");
        let open = self.toast.is_some();
        let rest_w = ctx.fonts_mut(|f| f.layout_no_wrap(rest.to_owned(), semi(12.0), TEXT).size().x)
            + rest_keys.iter().map(|k| ctx.fonts_mut(|f| f.layout_no_wrap(k.to_string(), semi(10.0), TEXT).size().x + 14.0)).sum::<f32>()
            + 38.0;
        let (tw, th) = if let Some((t, _)) = &self.toast {
            let lw = ctx.fonts_mut(|f| f.layout_no_wrap(t.label.clone(), semi(13.0), TEXT).size().x);
            let sw = ctx.fonts_mut(|f| f.layout_no_wrap(t.sub.clone(), mono(11.0), TEXT).size().x);
            ((lw.max(sw) + 74.0).max(rest_w + 40.0), 52.0)
        } else {
            (rest_w, 32.0)
        };
        let w = spring(ctx, id.with("w"), tw, SPRING);
        let h = spring(ctx, id.with("h"), th, SPRING);
        let rect = Rect::from_center_size(pos2(top.x, top.y + h / 2.0), vec2(w, h));
        egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(rect.min).interactable(false).show(ctx, |ui| {
            let p = ui.painter();
            p.rect_filled(rect, CornerRadius::same((h / 2.0).min(26.0) as u8), INK);
            p.rect_stroke(rect, CornerRadius::same((h / 2.0).min(26.0) as u8), Stroke::new(1.0, white(20)), StrokeKind::Inside);
            // content fades in from a slight scale-down (the kit blurs in; egui can't blur, so fade + scale)
            let key = if open { self.serial } else { 0 };
            let a = spring_from(ctx, id.with(("a", key)), 0.0, 1.0, SNAPPY).clamp(0.0, 1.0);
            let fade = |c: Color32| c.gamma_multiply(a);
            if let Some((t, _)) = &self.toast {
                let gc = match t.tone { Tone::Accent => ACCENT, Tone::Danger => DANGER, Tone::Plain => TEXT };
                let gr = Rect::from_center_size(pos2(rect.left() + 28.0, rect.center().y), vec2(30.0, 30.0));
                p.rect_filled(gr, CornerRadius::same(15), fade(white(18)));
                p.text(gr.center(), Align2::CENTER_CENTER, &t.glyph, semi(13.0), fade(gc));
                p.text(pos2(rect.left() + 52.0, rect.center().y - 8.0), Align2::LEFT_CENTER, &t.label, semi(13.0), fade(TEXT));
                p.text(pos2(rect.left() + 52.0, rect.center().y + 9.0), Align2::LEFT_CENTER, &t.sub, mono(11.0), fade(MUTED));
            } else {
                p.circle_filled(pos2(rect.left() + 16.0, rect.center().y), 3.5, fade(ACCENT));
                let g = p.layout_no_wrap(rest.to_owned(), semi(12.0), fade(TEXT));
                let mut x = rect.left() + 26.0;
                p.galley(pos2(x, rect.center().y - g.size().y / 2.0), g.clone(), fade(TEXT));
                x += g.size().x + 8.0;
                for k in rest_keys {
                    let s = vec2(p.layout_no_wrap(k.to_string(), semi(10.0), TEXT).size().x + 10.0, 20.0).max(vec2(20.0, 20.0));
                    let kr = Rect::from_min_size(pos2(x, rect.center().y - 10.0), s);
                    paint_kbd(p, kr, k, fade(TEXT));
                    x += s.x + 4.0;
                }
            }
        });
    }
}

/// A small floating label in the kit's tooltip style, drawn at a screen point (used by the move widget).
pub fn float_label(ctx: &Context, id: &str, at: egui::Pos2, text: &str, color: Color32, keys: &[&str]) {
    egui::Area::new(Id::new(id)).order(egui::Order::Tooltip).fixed_pos(at).interactable(false).show(ctx, |ui| {
        egui::Frame::new().fill(INK).stroke(Stroke::new(1.0, color)).corner_radius(R_CARD).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(egui::RichText::new(text).font(semi(12.0)).color(TEXT));
                for k in keys { kbd(ui, k); }
            });
        });
    });
}
