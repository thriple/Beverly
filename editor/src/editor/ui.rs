//! The 2D parts of the editor: the user's framed layout (frame.rs, docs/ui.svg) in the Motion kit's style: the
//! menus and tabs along the top, the Assets card, the three bottom panels with their drawers, the parts pop-up
//! (Tab), the status island, the speed strip, hints, and the unsaved-changes question.
use crate::level::{self, Shape, Wire};
use crate::settings::{self, Setting};
use crate::update::{self, Status};
use super::theme::{self, ACCENT, INK, LINE, MUTED, PANEL, R_CARD, TEXT, Tone, mono, semi, white};
use super::view::{Mode, SPEED_LABELS, View};
use super::{actions, frame, mcp, Editor, LevelMode, Pending};
use bevy::prelude::*;
use bevy_egui::egui::{self, Align2, Color32, CornerRadius, Id, Rect as ERect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use bevy_egui::{EguiContexts, egui::emath::TSTransform};

/// A section of the parts pop-up. Only Shapes work in this version; the rest show what's coming.
struct Section { name: &'static str, color: Color32, items: &'static [(&'static str, &'static str)], ready: bool }

const SCRIPT_SECTIONS: [Section; 5] = [
    Section { name: "Triggers", color: theme::CAT_TRIGGERS, items: &[("◎", "Proximity"), ("⬚", "Volume"), ("☝", "Interact"), ("▶", "On start"), ("✖", "Killed")], ready: false },
    Section {
        name: "Actions",
        color: theme::CAT_ACTIONS,
        items: &[("✸", "Explosion"), ("≋", "Shake"), ("⁘", "Debris"), ("⌧", "Break"), ("♪", "Sound"), ("⁂", "Particles"), ("☀", "Light FX"), ("❝", "Dialog"), ("Ⅱ", "Pause"), ("▦", "Animatic")],
        ready: false,
    },
    Section { name: "Flow", color: theme::CAT_FLOW, items: &[("◷", "Timer"), ("⇉", "Relay"), ("☰", "Sequence"), ("#", "Counter"), ("⚄", "Random")], ready: false },
    Section { name: "Logic", color: theme::CAT_LOGIC, items: &[("∧", "AND"), ("∨", "OR"), ("⊽", "NOR"), ("⊻", "XOR"), ("¬", "NOT")], ready: false },
    Section { name: "Game", color: theme::CAT_GAME, items: &[("⚑", "Spawner"), ("✚", "Save point"), ("☺", "Player start"), ("⇥", "Level exit")], ready: false },
];

pub fn ui(
    mut contexts: EguiContexts,
    mut ed: ResMut<Editor>,
    mut view: ResMut<View>,
    time: Res<Time<Real>>,
    mut exit: MessageWriter<AppExit>,
    mut init: Local<bool>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    if !*init {
        // fonts take effect from the next frame, so draw nothing until then
        theme::setup(&ctx);
        *init = true;
        return Ok(());
    }
    // test mode: no editor panels, just the status island
    if ed.testing.is_some() {
        let r = ctx.viewport_rect();
        view.rect = Rect::new(r.left(), r.top(), r.right(), r.bottom());
        view.outline.clear();
        ed.island.draw(&ctx, pos2(r.center().x, r.min.y + 14.0), "Testing", &["`"]);
        return Ok(());
    }
    // the frame: the view set into it, the menus and tabs along the top, the panels along the bottom
    let lay = frame::layout(ctx.viewport_rect());
    // (a pixel bigger than the frame's hole, so no seam shows between them)
    let v = lay.view.expand(1.5);
    view.rect = Rect::new(v.left(), v.top(), v.right(), v.bottom());
    view.outline = lay.outline.clone();
    paint_frame(&ctx, &lay);
    top(&ctx, &mut ed, &lay, &mut exit);
    assets_card(&ctx, &mut ed, &lay);
    bottom(&ctx, &mut ed, &mut view, &lay, &mut exit);

    overlays(&ctx, &mut ed, &view, &lay, time.elapsed_secs_f64());
    if ed.ask.as_ref().is_some_and(|(id, _)| ed.level.get(*id).is_some()) { ask_card(&ctx, &mut ed, &view); } else { theme::spring_reset(&ctx, Id::new("ask")); }
    if ed.parts_open { parts_popup(&ctx, &mut ed); } else { theme::spring_reset(&ctx, Id::new("parts-pop")); }
    if let Some(what) = ed.pending { unsaved_modal(&ctx, &mut ed, what, &mut exit); }
    if ed.about_open { about_modal(&ctx, &mut ed); }
    if ed.updates_open { updates_modal(&ctx, &mut ed); }
    if ed.features_open { features_modal(&ctx, &mut ed); }
    if ed.mcp_open { mcp_modal(&ctx, &mut ed); }
    Ok(())
}

fn small_caps(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).font(mono(10.0)).color(MUTED).extra_letter_spacing(1.2));
}

/// Translucent black: the cards and pop-ups over the view (the design's panels).
const GLASS: Color32 = Color32::from_black_alpha(166);
/// The design's buttons and keycaps.
pub(super) const CHIP: Color32 = Color32::from_rgb(0x17, 0x17, 0x17);
pub(super) const CHIP_LINE: Color32 = Color32::from_rgb(0x2e, 0x2e, 0x2e);

use frame::{Fit, LEFT, RIGHT};
const S: Fit = Fit::Stretch;

/// Text at a point of the design (its left end on the baseline, as SVG places text).
fn svg_text(p: &egui::Painter, m: &frame::Map, at: egui::Pos2, fx: Fit, text: &str, size: f32, col: Color32) -> ERect {
    let g = p.layout_no_wrap(text.to_owned(), theme::roboto(m.len(size)), col);
    let pos = m.pos(at, fx, S) - vec2(0.0, g.size().y * 0.78);
    let r = ERect::from_min_size(pos, g.size());
    p.galley(pos, g, col);
    r
}

/// A part of the design drawn in an area over the view (so the view ignores the pointer there).
fn art_area<R>(ctx: &egui::Context, id: &str, bounds: ERect, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Area::new(Id::new(id)).order(egui::Order::Middle).fixed_pos(bounds.min).constrain(false).show(ctx, |ui| {
        ui.set_min_size(bounds.size());
        ui.set_max_size(bounds.size());
        add(ui)
    }).inner
}

fn draw(ui: &Ui, id: &str, m: &frame::Map, fx: Fit, fy: Fit) { frame::paint(ui.painter(), frame::art().part(id), |p| m.pos(p, fx, fy), |c| c); }

/// The frame around the view and the band behind the tabs (behind everything).
fn paint_frame(ctx: &egui::Context, lay: &frame::Layout) {
    let p = ctx.layer_painter(egui::LayerId::background());
    let f = frame::art().part("frame");
    // the design is drawn on white: its frame's shading is over opaque white (nothing of the view shows through)
    let m = lay.m;
    frame::paint(&p, f, |q| m.pos(q, S, S), |_| Color32::WHITE);
    frame::paint(&p, f, |q| m.pos(q, S, S), |c| c);
    frame::paint(&p, frame::art().part("tab-band"), |q| m.pos(q, S, S), |c| c);
}

/// A button of the design's kind (dark, outlined, rounded) on a box of the design, its text's left end at `tx`
/// (None: centred).
#[allow(clippy::too_many_arguments)]
fn svg_box_button(ui: &mut Ui, m: &frame::Map, fx: Fit, b: ERect, tx: Option<f32>, label: &str, size: f32, enabled: bool) -> egui::Response {
    let r = m.rect(b, fx, S);
    let resp = ui.interact(r, ui.id().with(("svg-button", label)), if enabled { Sense::click() } else { Sense::hover() });
    let hot = theme::spring(ui.ctx(), resp.id.with("hot"), if resp.hovered() && enabled { 1.0 } else { 0.0 }, theme::SNAPPY).clamp(0.0, 1.0);
    let p = ui.painter();
    let round = CornerRadius::same(m.len(5.0) as u8);
    p.rect_filled(r, round, theme::lerp_color(CHIP, Color32::from_rgb(0x3a, 0x3a, 0x3a), hot));
    p.rect_stroke(r, round, Stroke::new(m.len(0.5).max(1.0), CHIP_LINE), StrokeKind::Inside);
    let col = if enabled { TEXT } else { white(90) };
    match tx {
        Some(x) => { svg_text(p, m, pos2(x, b.bottom() - 4.2), fx, label, size, col); }
        None => { p.text(r.center(), Align2::CENTER_CENTER, label, theme::roboto(m.len(size)), col); }
    }
    resp
}

/// A design button sized to its label, its left end at `x`; returns it and where the next one starts.
fn svg_button(ui: &mut Ui, m: &frame::Map, fx: Fit, x: f32, (y0, y1): (f32, f32), label: &str, size: f32) -> (egui::Response, f32) {
    let w = ui.painter().layout_no_wrap(label.to_owned(), theme::roboto(m.len(size)), TEXT).size().x / m.s + 9.2;
    (svg_box_button(ui, m, fx, ERect::from_min_max(pos2(x, y0), pos2(x + w, y1)), None, label, size, true), x + w + 4.8)
}

/// The top: the left pill (logo and menus), the tabs, the right pill (Parts, Undo, Redo, Snap).
fn top(ctx: &egui::Context, ed: &mut Editor, lay: &frame::Layout, exit: &mut MessageWriter<AppExit>) {
    let m = lay.m;
    art_area(ctx, "left-pill", lay.left_pill, |ui| {
        draw(ui, "pill-left", &m, LEFT, S);
        draw(ui, "logo", &m, LEFT, S);
        let menus: [(&str, &[&str]); 5] = [
            ("File", &["file.new", "file.open", "", "file.save", "file.save_as", "file.export", "file.show_folder", "", "file.quit"]),
            ("Edit", &["edit.undo", "edit.redo", "", "edit.duplicate", "edit.delete", "edit.make_unique", "edit.deselect", "", "edit.snap"]),
            ("View", &["view.parts", "view.frame", "view.look_through"]),
            ("Play", &["play.test", "", "file.export"]),
            ("Help", &["help.about", "help.updates", "help.features", "help.mcp"]),
        ];
        let mut x = 120.8;
        for (name, ids) in menus {
            let (resp, next) = svg_button(ui, &m, LEFT, x, (68.9, 84.5), name, 10.0);
            x = next;
            egui::Popup::menu(&resp).show(|ui| {
                ui.set_min_width(230.0);
                for id in ids {
                    if id.is_empty() { ui.separator(); } else { menu_action(ui, ed, id, exit); }
                }
                if name == "Play" { ui.label(egui::RichText::new("` tests from where the view looks").color(MUTED).size(11.0)); }
            });
        }
        // the project, and a dot while it has unsaved changes
        let room = lay.left_pill.right() - m.x(x + 4.0, LEFT) - m.len(26.0);
        if room > 20.0 {
            let g = ui.painter().layout(ed.name.clone(), theme::roboto(m.len(10.0)), MUTED, f32::INFINITY);
            let at = m.pos(pos2(x + 4.0, 80.3), LEFT, S) - vec2(0.0, g.size().y * 0.78);
            let clip = ERect::from_min_size(at, vec2(room, g.size().y));
            ui.painter().with_clip_rect(clip).galley(at, g.clone(), MUTED);
            if ed.dirty() { ui.painter().circle_filled(pos2(at.x + g.size().x.min(room) + m.len(5.0), at.y + g.size().y * 0.55), m.len(2.6), ACCENT); }
        }
    });
    art_area(ctx, "right-pill", lay.right_pill, |ui| {
        draw(ui, "pill-right", &m, RIGHT, S);
        let parts = svg_box_button(ui, &m, RIGHT, ERect::from_min_max(pos2(987.8, 69.3), pos2(1020.2, 84.9)), Some(991.9), "Parts", 10.0, true);
        if parts.on_hover_ui(|ui| theme::hint(ui, "Open the parts pop-up", &["Tab"])).clicked() { let _ = actions::run(ed, None, "view.parts", exit); }
        // Undo and Redo: the design's boxes, and its arrow (mirrored for Redo) before the word
        for (k, (x0, x1, tx, label, id, keys)) in [(1025.8, 1067.1, 1039.1, "Undo", "edit.undo", ["Ctrl", "Z"]), (1072.4, 1113.8, 1086.4, "Redo", "edit.redo", ["Ctrl", "Y"])].into_iter().enumerate() {
            let on = if k == 0 { !ed.undo.is_empty() } else { !ed.redo.is_empty() };
            let b = svg_box_button(ui, &m, RIGHT, ERect::from_min_max(pos2(x0, 69.3), pos2(x1, 84.9)), Some(tx), label, 10.0, on);
            let icon = frame::art().part("undo-icon");
            let c = icon.bounds.center();
            let mirror = |p: egui::Pos2| if k == 0 { p } else { pos2(2.0 * c.x - p.x + (1079.0 - 1032.4), p.y) };
            frame::paint(ui.painter(), icon, |p| m.pos(mirror(p), RIGHT, S), |c| if on { c } else { white(90) });
            if b.on_hover_ui(|ui| theme::hint(ui, if k == 0 { "Undo" } else { "Redo" }, &keys)).clicked() { let _ = actions::run(ed, None, id, exit); }
        }
        svg_text(ui.painter(), &m, pos2(1121.0, 79.4), RIGHT, if ed.snap { "Snap 0.25 m · 15°" } else { "Snap off" }, 7.8, TEXT);
        // the design's toggle: green with a black knob when on
        let tr = m.rect(ERect::from_min_max(pos2(1185.1, 71.0), pos2(1205.6, 82.6)), RIGHT, S);
        let resp = ui.interact(tr.expand(3.0), Id::new("snap-toggle"), Sense::click()).on_hover_text("Snap: moves in 0.25 m steps, turns in 15° steps (hold Ctrl to drag freely)");
        if resp.clicked() { ed.snap = !ed.snap; }
        paint_toggle(ui, tr, ed.snap, resp.id);
    });
    tabs(ctx, ed, lay);
}

/// The design's toggle (Snap, Auto Keyframe): a green track and a black knob when on; the knob springs across.
pub(super) fn paint_toggle(ui: &Ui, r: ERect, on: bool, id: Id) {
    let t = theme::spring(ui.ctx(), id.with("t"), if on { 1.0 } else { 0.0 }, theme::SNAPPY);
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same((r.height() / 2.0) as u8), theme::lerp_color(white(40), theme::GREEN, t.clamp(0.0, 1.0)));
    let k = r.height() * 0.41;
    let x = egui::lerp(r.left() + r.height() / 2.0..=r.right() - r.height() / 2.0, t);
    p.circle_filled(pos2(x, r.center().y), k, if on { Color32::BLACK } else { TEXT });
}

/// A top tab: its name and where the design puts it (its text's left end), its menu (the design's box, where its
/// first row starts, the row pitch, the text's left end; then each entry with its Level topic, or None and when it
/// comes), or when the tab comes.
struct Tab { name: &'static str, x: f32, menu: Option<(&'static str, f32, f32, f32)>, items: &'static [(&'static str, Option<LevelMode>, &'static str)], later: &'static str }

const TABS: [Tab; 8] = [
    Tab {
        name: "Level",
        x: 379.6,
        menu: Some(("menu-level", 100.5, 20.04, 376.9)),
        items: &[
            ("Physics", Some(LevelMode::Physics), ""),
            ("Water", None, "Water comes with the Ocean"),
            ("Foliage", None, "Foliage comes with foliage painting"),
            ("Cameras", Some(LevelMode::Cameras), ""),
            ("Lighting", Some(LevelMode::Lighting), ""),
        ],
        later: "",
    },
    Tab { name: "Characters", x: 433.4, menu: None, items: &[], later: "Characters come with character files" },
    Tab { name: "Vehicles", x: 508.9, menu: None, items: &[], later: "Vehicles come after characters" },
    Tab { name: "UI/HUD", x: 580.3, menu: None, items: &[], later: "The HUD comes with the HUD editor" },
    Tab { name: "Audio", x: 653.9, menu: None, items: &[], later: "Sounds come with sound parts" },
    Tab {
        name: "Effects",
        x: 719.5,
        menu: Some(("menu-effects", 96.9, 20.78, 717.9)),
        items: &[("Plumes", None, "Plumes come with the Effects tab"), ("Particles", None, "Particles come with particle effects"), ("Fire", None, "Fire comes with visual effects"), ("Post Processing", None, "Post-processing comes later")],
        later: "Effects come with the Effects tab",
    },
    Tab { name: "Screens", x: 785.9, menu: None, items: &[], later: "Screens come with screens and menus" },
    Tab { name: "Animatics", x: 849.4, menu: None, items: &[], later: "Animatics come with the timeline" },
];

/// The tabs, at the design's places: Level (selected: the design's white box) works, with its menu of topics; the
/// rest say when they come. A tab's menu opens under it while the pointer is on the tab or the menu.
fn tabs(ctx: &egui::Context, ed: &mut Editor, lay: &frame::Layout) {
    let m = lay.m;
    let id = Id::new("tabs");
    let band = frame::bounds("tab-band", &m, S, S);
    let mut open: Option<usize> = ctx.data(|d| d.get_temp(id.with("open")));
    let menu_rect: Option<ERect> = ctx.data(|d| d.get_temp(id.with("menu")));
    let rects = art_area(ctx, "tabs", band, |ui| {
        let p = ui.painter().clone();
        let mut rects = vec![];
        for (i, t) in TABS.iter().enumerate() {
            let w = p.layout_no_wrap(t.name.into(), theme::roboto(m.len(12.0)), TEXT).size().x / m.s;
            // the design's selected box: 20.6 either side of its text, the band's height
            let tr = m.rect(ERect::from_min_max(pos2(t.x - 20.6, 44.7), pos2(t.x + w + 20.6, 74.2)), S, S);
            let resp = ui.interact(tr, id.with(i), Sense::click());
            let on = i == 0;
            let hot = theme::spring(ctx, id.with(("hot", i)), if resp.hovered() && !on { 1.0 } else { 0.0 }, theme::SNAPPY).clamp(0.0, 1.0);
            if on { p.rect_filled(tr, CornerRadius::ZERO, Color32::WHITE); } else if hot > 0.0 { p.rect_filled(tr.shrink2(vec2(m.len(8.0), 0.0)), CornerRadius::ZERO, white((hot * 30.0) as u8)); }
            let col = if on { Color32::BLACK } else if t.later.is_empty() { TEXT } else { white(170 + (hot * 85.0) as u8) };
            svg_text(&p, &m, pos2(t.x, 63.6), S, t.name, 12.0, col);
            if resp.hovered() && t.menu.is_some() { open = Some(i); }
            if resp.clicked() {
                if t.later.is_empty() {
                    ed.level_mode = LevelMode::Build;
                    open = None;
                } else {
                    ed.island.show("…", t.later.to_string(), "Levels are what this version builds", Tone::Plain);
                }
            }
            if !t.later.is_empty() { resp.on_hover_text(t.later); }
            rects.push(tr);
        }
        rects
    });
    // keep the menu while the pointer is on its tab or on it
    if let (Some(i), Some(at)) = (open, ctx.pointer_hover_pos()) {
        if !(rects[i].contains(at) || menu_rect.is_some_and(|r| r.union(rects[i]).expand(6.0).contains(at))) { open = None; }
    }
    if ctx.pointer_hover_pos().is_none() { open = None; }
    ctx.data_mut(|d| match open { Some(i) => { d.insert_temp(id.with("open"), i); } None => { d.remove::<usize>(id.with("open")); } });
    let Some((i, (part, first, pitch, tx))) = open.and_then(|i| TABS[i].menu.map(|mm| (i, mm))) else {
        theme::spring_reset(ctx, id.with("menu-in"));
        ctx.data_mut(|d| d.remove::<ERect>(id.with("menu")));
        return;
    };
    // the design's menu box, fading and growing in
    let t = theme::spring_from(ctx, id.with("menu-in"), 0.0, 1.0, theme::SNAPPY).clamp(0.0, 1.0);
    let r = frame::bounds(part, &m, S, S);
    egui::Area::new(id.with("menu-area")).order(egui::Order::Foreground).fixed_pos(r.min).constrain(false).show(ctx, |ui| {
        ui.set_min_size(r.size());
        ui.multiply_opacity(t);
        // fading and growing in from its top
        let grow = egui::lerp(0.96..=1.0, t);
        frame::paint(ui.painter(), frame::art().part(part), |p| r.center_top() + (m.pos(p, S, S) - r.center_top()) * grow, |c| c.gamma_multiply(t));
        let x0 = frame::art().part(part).bounds;
        for (k, (label, mode, later)) in TABS[i].items.iter().enumerate() {
            let y = first + pitch * k as f32;
            let row = m.rect(ERect::from_min_max(pos2(x0.left(), y), pos2(x0.right(), y + 16.7)), S, S);
            let resp = ui.interact(row, id.with(("item", i, k)), if mode.is_some() { Sense::click() } else { Sense::hover() });
            let hov = resp.hovered() && mode.is_some();
            // the design's highlight: a white row
            if hov { ui.painter().rect_filled(row, CornerRadius::ZERO, Color32::WHITE); }
            let col = if hov { Color32::BLACK } else if mode.is_some() { TEXT } else { white(110) };
            svg_text(ui.painter(), &m, pos2(tx, y + 11.9), S, label, 12.0, col);
            if i == 0 && Some(ed.level_mode) == *mode { ui.painter().circle_filled(row.right_center() - vec2(m.len(7.0), 0.0), m.len(2.2), if hov { Color32::BLACK } else { ACCENT }); }
            if resp.clicked() {
                if let Some(mm) = mode {
                    ed.level_mode = *mm;
                    ed.drawers[0] = true;
                }
            }
            if !later.is_empty() { resp.on_hover_text(*later); }
        }
    });
    ctx.data_mut(|d| d.insert_temp(id.with("menu"), r));
}

/// A menu item for a named action (its name and keys from the list), greyed when it can't run; runs it when clicked.
fn menu_action(ui: &mut Ui, ed: &mut Editor, id: &str, exit: &mut MessageWriter<AppExit>) {
    let a = actions::get(id).expect("a listed action");
    let why = actions::why_not(ed, id);
    let r = ui.add_enabled(why.is_none(), egui::Button::new(a.name).shortcut_text(a.keys).frame(false));
    let r = match why { Some(w) => r.on_disabled_hover_text(w), None => r };
    if r.clicked() {
        let _ = actions::run(ed, None, id, exit);
        ui.close();
    }
}

pub(super) fn open_folder(dir: &std::path::Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let cmd = "explorer";
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let cmd = "xdg-open";
    std::process::Command::new(cmd).arg(dir).spawn().map(|_| ())
}

/// How far a drawer slides up to open (design units): from below the window until its bottom meets its panel's.
const DRAWER_RISE: f32 = 1106.3 - 756.7;
/// A drawer stretches between these heights of the design when dragged taller (its sides are straight there).
const DRAWER_STRETCH: (f32, f32) = (850.0, 1050.0);

/// The three panels along the bottom (the design's): the Level topic's settings, the inspector, the scene list.
/// Each has a drawer (the design's, below the window) that slides up over the view: click the grip above the panel
/// to open or close it, drag the grip for its height.
fn bottom(ctx: &egui::Context, ed: &mut Editor, view: &mut View, lay: &frame::Layout, exit: &mut MessageWriter<AppExit>) {
    let m = lay.m;
    let fy = frame::BOTTOM;
    for i in 0..3 {
        let panel = lay.panels[i];
        let drawer_id = ["drawer-mode", "drawer-inspector", "drawer-scene"][i];
        let d = frame::art().part(drawer_id);
        // how much taller than drawn (design units), and how far open
        // at most up to just under the pills
        let most = d.bounds.top() - DRAWER_RISE - 100.0;
        let extra = ed.drawer_h[i].clamp(0.0, most.max(0.0));
        let t = theme::spring(ctx, Id::new(("drawer", i)), if ed.drawers[i] { 1.0 } else { 0.0 }, theme::SPRING);
        let lift = |p: egui::Pos2| {
            let k = 1.0 - ((p.y - DRAWER_STRETCH.0) / (DRAWER_STRETCH.1 - DRAWER_STRETCH.0)).clamp(0.0, 1.0);
            pos2(p.x, p.y - DRAWER_RISE * t - extra * k * t)
        };
        let top = m.y(lift(pos2(0.0, d.bounds.top())).y, fy);
        let drawer = ERect::from_min_max(pos2(m.x(d.bounds.left(), S), top), pos2(m.x(d.bounds.right(), S), m.y(lift(pos2(0.0, d.bounds.bottom())).y, fy)));
        let open = t > 0.02 && drawer.top() < panel.top();
        let area = if open { drawer.union(panel) } else { panel };
        // the grip: the design's handle, centred in the gap above the panel; inside the drawer's top once it rises
        let grip_at = pos2(panel.center().x, (panel.top() - m.len(4.6)).min(drawer.top() + m.len(8.0)));
        art_area(ctx, ["panel-mode", "panel-inspector", "panel-scene"][i], area.union(ERect::from_center_size(grip_at, vec2(m.len(110.0), m.len(10.0)))), |ui| {
            draw(ui, ["panel-mode", "panel-inspector", "panel-scene"][i], &m, S, fy);
            if t > 0.0 { frame::paint(ui.painter(), d, |p| m.pos(lift(p), S, fy), |c| c); }
            let g = frame::art().part("grip");
            let gc = g.bounds.center();
            let shift = grip_at - m.pos(gc, Fit::Pin(gc.x), Fit::Pin(gc.y));
            let gr = ui.interact(ERect::from_center_size(grip_at, vec2(m.len(105.0), m.len(9.0))), Id::new(("grip", i)), Sense::click_and_drag())
                .on_hover_cursor(egui::CursorIcon::ResizeVertical)
                .on_hover_text(if ed.drawers[i] { "Click to close · drag for its height" } else { "Click to open · drag for its height" });
            let hot = gr.hovered() || gr.dragged();
            frame::paint(ui.painter(), g, |p| m.pos(p, Fit::Pin(gc.x), Fit::Pin(gc.y)) + shift, |c| if hot { theme::lerp_color(c, white(255), 0.35) } else { c });
            if gr.clicked() { ed.drawers[i] = !ed.drawers[i]; }
            if gr.drag_started() && !ed.drawers[i] { ed.drawer_h[i] = 0.0; }
            if gr.dragged() {
                ed.drawers[i] = true;
                ed.drawer_h[i] = (extra - gr.drag_delta().y / m.s).clamp(0.0, most.max(0.0));
            }
            // its contents: in the drawer while open, else in the panel
            let r = if open { drawer } else { panel };
            let pad = if open { vec2(m.len(22.0), m.len(20.0)) } else { vec2(m.len(14.0), m.len(11.0)) };
            let inner = ERect::from_min_max(r.min + pad, r.max - vec2(pad.x, m.len(12.0)));
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)), |ui| {
                ui.set_clip_rect(inner.expand2(vec2(6.0, 2.0)));
                egui::ScrollArea::vertical().id_salt(("panel", i)).auto_shrink([false, false]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                    match i {
                        0 => mode_panel(ui, ed),
                        1 => inspector(ui, ed, view, exit),
                        _ => scene_panel(ui, ed),
                    }
                    ui.add_space(6.0);
                });
            });
        });
    }
}

/// The Assets card over the view's right side (the design's), with its DROP FILES header, and the project's assets.
fn assets_card(ctx: &egui::Context, ed: &mut Editor, lay: &frame::Layout) {
    let m = lay.m;
    if !ed.assets_open {
        // folded away: a small button where the card's top was
        let r = ERect::from_min_size(pos2(lay.assets.right() - 92.0, lay.assets.top()), vec2(92.0, 26.0));
        art_area(ctx, "assets", r, |ui| {
            ui.painter().rect_filled(r, CornerRadius::same(13), GLASS);
            if ui.interact(r, Id::new("assets-show"), Sense::click()).on_hover_text("Show the Assets panel").clicked() { ed.assets_open = true; }
            ui.painter().text(r.center(), Align2::CENTER_CENTER, "⇤  ASSETS", mono(10.0), TEXT);
        });
        return;
    }
    art_area(ctx, "assets", lay.assets, |ui| {
        draw(ui, "assets-card", &m, RIGHT, S);
        // the design's drop header (its chevrons and lettering); lit up while files are dragged over the window
        let hot = ed.hovering_files;
        frame::paint(ui.painter(), frame::art().part("drop-files"), |p| m.pos(p, RIGHT, S), |c| if hot { Color32::from_rgba_unmultiplied(ACCENT.r(), ACCENT.g(), ACCENT.b(), c.a()) } else { c });
        let help = if hot { "Drop to import" } else { "Drag textures, models and sounds here to import them" };
        ui.interact(lay.drop, Id::new("drop-files"), Sense::hover()).on_hover_text(help);
        let inner = ERect::from_min_max(pos2(lay.drop.left(), lay.drop.bottom() + m.len(6.0)), lay.assets.max - vec2(m.len(8.0), m.len(10.0)));
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)), |ui| {
            ui.horizontal(|ui| {
                small_caps(ui, "ASSETS");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("⇥").on_hover_ui(|ui| theme::hint(ui, "Fold the Assets panel away", &[])).clicked() { ed.assets_open = false; }
                });
            });
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| { asset_tree(ui, ed); });
        });
    });
}

/// The hints along the view's bottom (the design's strip, its labels and keycaps): what the mouse does here.
fn hint_strip(ctx: &egui::Context, lay: &frame::Layout, hints: &[(&str, &[&str])], editor_mode: bool) {
    let m = lay.m;
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::new("hint-strip")));
    // lay out in design units: label, keycaps, gaps as drawn
    let tw = |t: &str, size: f32| p.layout_no_wrap(t.to_owned(), theme::roboto(m.len(size)), TEXT).size().x / m.s;
    let mut items: Vec<(f32, &str, bool)> = vec![];
    let mut x = 0.0;
    for (label, keys) in hints {
        items.push((x, label, false));
        x += tw(label, 7.46) + 3.0;
        for k in *keys {
            items.push((x, k, true));
            x += tw(k, 6.58) + 6.4 + 3.0;
        }
        x += 7.0;
    }
    let w = x - 7.0;
    let c = 635.05;
    let (x0, y0, y1) = (c - w / 2.0, 639.0, 662.0);
    if editor_mode { frame::paint(&p, frame::art().part("view-hints"), |q| m.pos(q, S, frame::BOTTOM), |c| c); } else {
        let r = m.rect(ERect::from_min_max(pos2(x0 - 12.0, y0), pos2(x0 + w + 12.0, y1)), S, frame::BOTTOM);
        p.rect_filled(r, CornerRadius::same((r.height() / 2.0) as u8), GLASS);
    }
    for (dx, t, key) in items {
        let at = x0 + dx;
        if key {
            let kr = m.rect(ERect::from_min_max(pos2(at, 644.8), pos2(at + tw(t, 6.58) + 6.4, 656.2)), S, frame::BOTTOM);
            p.rect_filled(kr, CornerRadius::same(m.len(3.1) as u8), CHIP);
            p.rect_stroke(kr, CornerRadius::same(m.len(3.1) as u8), Stroke::new(m.len(0.53).max(1.0), CHIP_LINE), StrokeKind::Inside);
            p.text(kr.center(), Align2::CENTER_CENTER, t, theme::roboto(m.len(6.58)), TEXT);
        } else {
            let g = p.layout_no_wrap(t.to_owned(), theme::roboto(m.len(7.46)), white(200));
            let pos = pos2(m.x(at, S), m.y(652.8, frame::BOTTOM) - g.size().y * 0.78);
            p.galley(pos, g, white(200));
        }
    }
}

/// The project's assets by folder: models to place, materials to change or drag, textures to drag onto faces.
fn asset_tree(ui: &mut Ui, ed: &mut Editor) {
    let mut assets = ed.assets.clone();
    // materials as the editor holds them (new and changed ones are saved with the project)
    if let Some(m) = assets.iter_mut().find(|(f, _)| *f == "materials") {
        m.1 = ed.level.materials.keys().map(|k| k.trim_start_matches("assets/materials/").to_string()).collect();
    }
    for (folder, files) in &assets {
        let title = format!("{}{}  ({})", folder[..1].to_uppercase(), &folder[1..], files.len());
        egui::CollapsingHeader::new(title).id_salt(folder).default_open(*folder == "textures").show(ui, |ui| {
            if files.is_empty() {
                ui.label(egui::RichText::new("empty").color(MUTED).size(11.0));
            }
            if *folder == "materials" && ui.small_button("+ New material").on_hover_text("A plain material: give it a texture, tint, shine or glow").clicked() {
                ed.material_open = Some(ed.level.new_material("Material", level::Material::default()));
            }
            for f in files {
                let path = format!("assets/{folder}/{f}");
                if crate::models::is_model(f) {
                    // models: click to place
                    let r = ui.add(egui::Button::new(egui::RichText::new(format!("◆  {f}")).size(12.0)).frame(false)).on_hover_text("Click, then click a surface to place it");
                    if r.clicked() {
                        ed.placing = Some(Shape::Model);
                        ed.placing_model = Some(path);
                        ed.island.show("◆", format!("Placing {f}"), "Click a surface · Shift keeps it upright · Esc cancels", Tone::Accent);
                    }
                } else if *folder == "materials" {
                    // materials: click to change, drag onto a face
                    let open = ed.material_open.as_deref() == Some(path.as_str());
                    let r = ui.add(egui::Label::new(egui::RichText::new(format!("◈  {}", f.trim_end_matches(".bev"))).size(12.0).color(if open { theme::VIOLET } else { TEXT })).selectable(false).sense(Sense::click_and_drag()))
                        .on_hover_cursor(egui::CursorIcon::Grab)
                        .on_hover_text("Click to change it (every surface using it changes) · drag onto a face");
                    if r.clicked() { ed.material_open = Some(path); } else if r.drag_started() { ed.texturing = Some(path); }
                } else if super::texture::is_texture(f) {
                    // textures: drag onto a face (or click, then click a face)
                    let r = ui.add(egui::Label::new(egui::RichText::new(format!("▦  {f}")).size(12.0)).selectable(false).sense(Sense::click_and_drag()))
                        .on_hover_cursor(egui::CursorIcon::Grab)
                        .on_hover_text("Drag onto a face of a part (Shift: the whole part)");
                    if r.drag_started() || r.clicked() { ed.texturing = Some(path); }
                } else {
                    ui.label(egui::RichText::new(f).size(12.0));
                }
            }
        });
    }
}

fn body(size: f32) -> egui::FontId { theme::body(size) }

/// The inspector (bottom middle): the selected part's settings, or the open material's.
fn inspector(ui: &mut Ui, ed: &mut Editor, view: &mut View, exit: &mut MessageWriter<AppExit>) {
    if let Some(path) = ed.material_open.clone().filter(|p| ed.level.materials.contains_key(p)) {
        material_panel(ui, ed, &path);
        return;
    }
    let Some(id) = ed.selected.filter(|id| ed.level.get(*id).is_some()) else {
        small_caps(ui, "INSPECTOR");
        ui.label(egui::RichText::new("Nothing selected: click a part to see its settings").color(MUTED));
        theme::hint(ui, "Parts", &["Tab"]);
        return;
    };
    let mut p = ed.level.get(id).unwrap().clone();
    let before = p.clone();
    let mut delete = false;
    let mut look = false;
    let mut unwire: Option<usize> = None;
    let mut ranges = ed.level.ranges.clone();
    let mut ranges_changed = false;
    { ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
        column(ui, 190.0, "PART", |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter().rect_filled(r, CornerRadius::same(2), theme::CAT_SHAPES);
                ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(150.0).font(semi(13.0)));
            });
            let file = p.model.as_deref().and_then(|m| m.rsplit('/').next()).map(|f| format!(" · {f}")).unwrap_or_default();
            ui.label(egui::RichText::new(format!("{} {}{file}", p.shape.glyph(), p.shape.label())).color(MUTED));
            // instances: the parts with the same contents (shape, colour, textures, Visible, Collision)
            let n = ed.level.instances(id).len();
            ui.horizontal(|ui| {
                theme::toggle(ui, &mut p.unique).on_hover_text("On: changing its look never changes the parts just like it");
                let (text, help) = if p.unique {
                    ("Unique", "Its contents are its own")
                } else if !ed.level.instanced(&p) {
                    ("Its own", "Lights, cameras, moving and wired parts are always individual")
                } else if n > 0 {
                    ("", "Parts with the same shape, colour and textures; changing one asks first")
                } else {
                    ("No instances", "Parts with the same shape, colour and textures would be its instances")
                };
                let text = if text.is_empty() { format!("Instance · {} alike", n + 1) } else { text.to_string() };
                ui.label(egui::RichText::new(text).color(MUTED)).on_hover_text(help);
            });
            ui.horizontal(|ui| {
                if ui.button("Duplicate").on_hover_ui(|ui| theme::hint(ui, "Duplicate", &["Ctrl", "D"])).clicked() { ed.duplicate_selected(); }
                if ui.button("Delete").on_hover_ui(|ui| theme::hint(ui, "Delete", &["Del"])).clicked() { delete = true; }
            });
        });
        // the rest is drawn from the part's setting descriptions (level.rs, settings.rs)
        let model = p.shape == Shape::Model;
        let scale = [("size", Setting::number("Scale", "×", 0.01, 0.001, 100000.0).section(level::LOOK).bare())];
        let mut ctx = settings::Ctx { replace: if model { &scale } else { &[] }, ranges: &mut ranges, ranges_changed: false };
        for sec in settings::sections(&p) {
            if sec == level::PART || (sec == level::MOVES && p.shape == Shape::PlayerStart) { continue; }
            // the player start, lights and cameras have no surface to tag
            if sec == level::TAGS && (p.shape == Shape::PlayerStart || p.shape.is_light() || p.shape.is_camera()) { continue; }
            // a light or a camera: its settings in place of size and look (the cone for spots only)
            if sec == level::LOOK && (p.shape.is_light() || p.shape.is_camera()) { continue; }
            if sec == level::CAMERA {
                let orbit = p.shape == Shape::OrbitCamera;
                if let Some(c) = p.camera.as_mut() {
                    column(ui, 250.0, "CAMERA", |ui| {
                        settings::section(ui, c, level::CAMERA, &mut ctx);
                        if ui.button("Look through").on_hover_ui(|ui| theme::hint(ui, "Fly it to aim it", &["L"])).clicked() { look = true; }
                    });
                    if orbit { column(ui, 260.0, "ORBIT", |ui| { settings::section(ui, c, level::ORBIT, &mut ctx); }); }
                }
                continue;
            }
            if sec == level::LIGHT {
                let spot = p.shape == Shape::SpotLight;
                if let Some(l) = p.light.as_mut() {
                    column(ui, 260.0, "LIGHT", |ui| { settings::section(ui, l, level::LIGHT, &mut ctx); });
                    if spot { column(ui, 250.0, "CONE", |ui| { settings::section(ui, l, level::CONE, &mut ctx); }); }
                }
                continue;
            }
            let title = if model && sec == level::LOOK { "SCALE  ·  LOOK" } else { sec };
            column(ui, if sec == level::LOOK { 260.0 } else { 250.0 }, title, |ui| { settings::section(ui, &mut p, sec, &mut ctx); });
        }
        ranges_changed = ctx.ranges_changed;
        if !p.surfaces.is_empty() {
            column(ui, 250.0, "TEXTURES", |ui| {
                let names = ed.geo.shapes[&p.shape].face_names();
                let mut remove = None;
                for (i, s) in p.surfaces.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        let face = s.face.map_or("All over".to_string(), |f| names.get(f as usize).cloned().unwrap_or_default());
                        let mat = std::path::Path::new(&s.material).file_stem().map(|m| m.to_string_lossy().into_owned()).unwrap_or_default();
                        ui.label(egui::RichText::new(format!("{face} · {mat}")).color(TEXT));
                        if ui.small_button("✕").on_hover_text("Take this texture off").clicked() { remove = Some(i); }
                    });
                    for (label, v, help) in [("Size", &mut s.tile, "How big one copy of the texture is (across, down)"), ("Slide", &mut s.offset, "How far the texture is slid along the surface")] {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(label).color(MUTED)).on_hover_text(help);
                            let lo = if label == "Size" { 0.05 } else { -1000.0 };
                            for x in v.iter_mut() { ui.add(egui::DragValue::new(x).speed(0.01).range(lo..=1000.0).max_decimals(2).suffix(" m")); }
                        });
                    }
                }
                if let Some(i) = remove { p.surfaces.remove(i); }
            });
        }
        let outs = p.outputs();
        let has_wires = ed.level.wires.iter().any(|w| w.from == id || w.to == id);
        if !outs.is_empty() || !p.inputs().is_empty() || has_wires {
            column(ui, 260.0, "WIRES", |ui| {
                let name_of = |pid: u64| ed.level.get(pid).map(|q| q.name.clone()).unwrap_or_default();
                for out in &outs {
                    ui.horizontal(|ui| {
                        port(ui, &format!("{out} ▸"), theme::CAT_TRIGGERS);
                        let wiring = ed.wiring == Some((id, out));
                        if ui.small_button(if wiring { "click a part…" } else { "+ wire" }).on_hover_text("Then click the part it goes to").clicked() {
                            ed.wiring = if wiring { None } else { Some((id, out)) };
                        }
                    });
                }
                for (i, w) in ed.level.wires.iter().enumerate() {
                    let text = if w.from == id {
                        format!("{} → {} · {}", w.out, name_of(w.to), w.input)
                    } else if w.to == id {
                        format!("{} ← {} · {}", w.input, name_of(w.from), w.out)
                    } else {
                        continue;
                    };
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(text).color(MUTED));
                        if ui.small_button("✕").on_hover_text("Remove this wire").clicked() { unwire = Some(i); }
                    });
                }
                if !p.inputs().is_empty() && !ed.level.wires.iter().any(|w| w.to == id) {
                    ui.horizontal(|ui| {
                        for i in p.inputs() { port(ui, &format!("▸ {i}"), theme::CAT_ACTIONS); }
                    });
                }
            });
        }
    });
    }
    if let Some(i) = unwire { ed.level.wires.remove(i); }
    if ranges_changed { ed.level.ranges = ranges; }
    if look { let _ = actions::run(ed, Some(view), "view.look_through", exit); }
    if delete {
        ed.delete_selected();
    } else if p != before {
        if p.mover.is_none() {
            // its inputs are gone: so are the wires into them
            ed.level.wires.retain(|w| w.to != id || p.inputs().contains(&w.input.as_str()));
            let outs = p.outputs();
            ed.level.wires.retain(|w| w.from != id || outs.contains(&w.out.as_str()));
        }
        if let Some(m) = ed.level.get_mut(id) { *m = p; }
    }
}

/// The bottom-left panel: what the Level tab's topic (its menu) sets, or the level and its topics.
fn mode_panel(ui: &mut Ui, ed: &mut Editor) {
    let mut l = ed.level.lighting.clone();
    let mut n = ed.level.navigation.clone();
    let mut build = false;
    let mut ranges = ed.level.ranges.clone();
    let mut ctx = settings::Ctx { replace: &[], ranges: &mut ranges, ranges_changed: false };
    match ed.level_mode {
        LevelMode::Build => {
            column(ui, 0.0, "LEVEL", |ui| {
                ui.label(egui::RichText::new(&ed.level.name).font(semi(13.0)).color(TEXT));
                ui.horizontal_wrapped(|ui| {
                    for (label, m) in [("Lighting", LevelMode::Lighting), ("Physics", LevelMode::Physics), ("Cameras", LevelMode::Cameras)] {
                        if ui.button(label).on_hover_text("Also in the Level tab's menu").clicked() { ed.level_mode = m; }
                    }
                });
                theme::hint(ui, "Parts", &["Tab"]);
                theme::hint(ui, "Fly / edit", &["Caps Lock"]);
                theme::hint(ui, "Test", &["`"]);
                theme::hint(ui, "Save", &["Ctrl", "S"]);
            });
        }
        LevelMode::Lighting => {
            column(ui, 0.0, "LEVEL · LIGHTING", |ui| { settings::section(ui, &mut l, level::LIGHTING, &mut ctx); });
            // Interior: no sun
            if !l.interior { column(ui, 0.0, level::SUN, |ui| { settings::section(ui, &mut l, level::SUN, &mut ctx); }); }
        }
        LevelMode::Physics => {
            // where enemies can walk: who for, Build, and how it stands
            column(ui, 0.0, "LEVEL · PHYSICS · NAVIGATION", |ui| {
                settings::section(ui, &mut n, level::NAVIGATION, &mut ctx);
                ui.add_space(2.0);
                let (status, colour) = match &ed.nav {
                    None => ("Not built yet".to_string(), MUTED),
                    Some(_) if ed.nav_out_of_date() => ("Out of date: the level changed".to_string(), theme::DANGER),
                    Some((m, _)) => {
                        let k = crate::nav::islands(m).len();
                        (format!("{:.0} m² · {k} area{}", crate::nav::area(m), if k == 1 { "" } else { "s" }), theme::OK)
                    }
                };
                ui.label(egui::RichText::new(status).color(colour));
                ui.horizontal(|ui| {
                    if ui.button(if ed.nav.is_some() { "Build again" } else { "Build navigation" }).on_hover_text("Work out where characters of this size can walk, across floors, stairs and ramps").clicked() { build = true; }
                    if ed.nav.is_some() {
                        theme::toggle(ui, &mut ed.nav_shown).on_hover_text("Draw the walkable area over the level");
                        ui.label(egui::RichText::new("Show").color(MUTED));
                    }
                });
                if ed.finder.is_some() {
                    let on = ed.path_try.is_some();
                    if ui.add(egui::Button::selectable(on, "Try a path")).on_hover_text("Click a start, then an end, in the view: the way a character would walk").clicked() {
                        ed.path_try = if on { None } else { Some(Default::default()) };
                    }
                    if on { theme::hint(ui, "Stop", &["Esc"]); }
                }
            });
        }
        LevelMode::Cameras => {
            column(ui, 0.0, "LEVEL · CAMERAS", |ui| {
                let cams: Vec<(u64, String)> = ed.level.parts.iter().filter(|p| p.shape.is_camera()).map(|p| (p.id, format!("{}  {}", p.shape.glyph(), p.name))).collect();
                if cams.is_empty() { ui.label(egui::RichText::new("No cameras yet: the Cameras section of the parts pop-up (Tab)").color(MUTED)); }
                for (id, name) in cams {
                    let on = ed.selected == Some(id);
                    if ui.add(egui::Button::selectable(on, egui::RichText::new(name).color(if on { ACCENT } else { TEXT })).frame(false)).clicked() { ed.selected = Some(id); ed.material_open = None; }
                }
                theme::hint(ui, "Look through the selected one", &["L"]);
            });
        }
    }
    if ctx.ranges_changed { ed.level.ranges = ranges; }
    if l != ed.level.lighting { ed.level.lighting = l; }
    if n != ed.level.navigation { ed.level.navigation = n; }
    if build { ed.build_navigation(); }
}

/// The bottom-right panel: the level's parts (those of the Level tab's topic).
fn scene_panel(ui: &mut Ui, ed: &mut Editor) {
    let (title, shown): (&str, fn(&level::Part) -> bool) = match ed.level_mode {
        LevelMode::Build => ("SCENE", |_| true),
        LevelMode::Lighting => ("SCENE · LIGHTS", |p| p.shape.is_light()),
        LevelMode::Cameras => ("SCENE · CAMERAS", |p| p.shape.is_camera()),
        LevelMode::Physics => ("SCENE · SOLID PARTS", |p| crate::physics::kind(p).is_some()),
    };
    small_caps(ui, title);
    if ed.level.parts.is_empty() { ui.label(egui::RichText::new("Nothing placed yet. Press Tab.").color(MUTED).size(12.0)); }
    let mut pick = None;
    let mut flip = None;
    for p in ed.level.parts.iter().filter(|p| shown(p)) {
        ui.horizontal(|ui| {
            let on = ed.selected == Some(p.id);
            let eye = if p.visible { "◉" } else { "○" };
            if ui.add(egui::Button::new(egui::RichText::new(eye).size(11.0)).frame(false)).on_hover_text(if p.visible { "Hide (keep as collider)" } else { "Show" }).clicked() { flip = Some(p.id); }
            let txt = egui::RichText::new(format!("{}  {}", p.shape.glyph(), p.name)).size(12.5).color(if on { ACCENT } else { TEXT });
            if ui.add(egui::Button::selectable(on, txt).frame(false)).clicked() { pick = Some(p.id); }
        });
    }
    if let Some(id) = pick { ed.selected = Some(id); ed.material_open = None; }
    if let Some(id) = flip {
        if let Some(p) = ed.level.get_mut(id) { p.visible = !p.visible; }
    }
}

/// "Change all 12, or make this one Unique?" after a change to a part's contents that its instances didn't get.
fn ask_card(ctx: &egui::Context, ed: &mut Editor, view: &View) {
    let Some((id, them)) = ed.ask.clone() else { return };
    let Some(name) = ed.level.get(id).map(|p| p.name.clone()) else { return };
    let key = Id::new("ask");
    // fade and scale in (SNAPPY), above the hints at the bottom of the view
    let t = theme::spring_from(ctx, key, 0.0, 1.0, theme::SNAPPY);
    let at = pos2(view.rect.center().x, view.rect.max.y - 58.0);
    let (mut all, mut unique) = (false, false);
    let resp = egui::Area::new(key).order(egui::Order::Foreground).pivot(Align2::CENTER_BOTTOM).fixed_pos(at).show(ctx, |ui| {
        ui.multiply_opacity(t.clamp(0.0, 1.0));
        egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, white(24))).corner_radius(R_CARD).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
            ui.label(egui::RichText::new(format!("{name} has {} like it", them.len())).font(semi(14.0)));
            ui.label(egui::RichText::new("Its instances share what is inside the box: shape, colour, textures, Visible, Collision").color(MUTED).size(12.0));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let button = |ui: &mut Ui, label: String, key: &str| {
                    let r = ui.add(egui::Button::new(egui::RichText::new(label).font(semi(13.0))).min_size(vec2(0.0, 28.0)));
                    theme::kbd(ui, key);
                    r.clicked()
                };
                all = button(ui, format!("Change all {}", them.len() + 1), "Enter");
                ui.add_space(10.0);
                unique = button(ui, "Make this one Unique".into(), "U");
                ui.add_space(10.0);
                theme::hint(ui, "Undo", &["Ctrl", "Z"]);
            });
        });
    });
    let s = 0.94 + 0.06 * t;
    let c = resp.response.rect.center_bottom();
    ctx.set_transform_layer(resp.response.layer_id, TSTransform::new(c.to_vec2() * (1.0 - s), s));
    if all { ed.settle(true); }
    if unique { ed.settle(false); }
}

/// A port chip: an output ("Pressed ▸") or an input ("▸ Go") in its category colour.
/// A material's settings: every surface using it changes with it.
fn material_panel(ui: &mut Ui, ed: &mut Editor, path: &str) {
    let mut m = ed.level.materials[path].clone();
    let before = m.clone();
    let mut ranges = ed.level.ranges.clone();
    let mut ranges_changed = false;
    let (surfaces, parts) = ed.level.material_users(path);
    let name = path.trim_start_matches("assets/materials/").trim_end_matches(".bev").to_string();
    let textures: Vec<String> = ed.assets.iter().find(|(f, _)| *f == "textures").map(|(_, t)| t.iter().filter(|t| super::texture::is_texture(t)).cloned().collect()).unwrap_or_default();
    let (mut copy, mut close) = (false, false);
    egui::ScrollArea::horizontal().show(ui, |ui| { ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
        column(ui, 220.0, "MATERIAL", |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter().rect_filled(r, CornerRadius::same(2), theme::VIOLET);
                ui.label(egui::RichText::new(&name).font(semi(13.0)));
            });
            let s = |n: usize, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
            ui.label(egui::RichText::new(format!("Used by {} on {}", s(surfaces, "surface"), s(parts, "part"))).color(MUTED));
            ui.horizontal(|ui| {
                if ui.button("Copy").on_hover_text("A new material like this one, to change on its own").clicked() { copy = true; }
                if ui.button("Close").on_hover_ui(|ui| theme::hint(ui, "Back to the selected part", &["Esc"])).clicked() { close = true; }
            });
        });
        column(ui, 230.0, "TEXTURE", |ui| {
            let shown = m.texture.as_deref().and_then(|t| t.rsplit('/').next()).unwrap_or("None (plain)").to_string();
            egui::ComboBox::from_id_salt("material texture").selected_text(shown).width(200.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut m.texture, None, "None (plain)");
                for t in &textures { ui.selectable_value(&mut m.texture, Some(format!("assets/textures/{t}")), t); }
            });
            ui.label(egui::RichText::new("Drag the material from Assets onto faces").color(MUTED).size(11.0));
        });
        let mut ctx = settings::Ctx { replace: &[], ranges: &mut ranges, ranges_changed: false };
        column(ui, 300.0, "LOOK", |ui| { settings::section(ui, &mut m, level::MATERIAL, &mut ctx); });
        ranges_changed = ctx.ranges_changed;
    }); });
    if m != before { ed.level.materials.insert(path.to_string(), m.clone()); }
    if ranges_changed { ed.level.ranges = ranges; }
    if copy {
        let new = ed.level.new_material(&format!("{name} copy"), m);
        ed.island.show("◈", "Copied the material", new.trim_start_matches("assets/materials/").to_string(), theme::Tone::Accent);
        ed.material_open = Some(new);
    }
    if close || ui.input(|i| i.key_pressed(egui::Key::Escape)) { ed.material_open = None; }
}

fn port(ui: &mut Ui, text: &str, color: Color32) {
    egui::Frame::new().fill(color.gamma_multiply(0.22)).stroke(Stroke::new(1.0, color.gamma_multiply(0.7))).corner_radius(CornerRadius { nw: 8, ne: 8, se: 8, sw: 3 }).inner_margin(egui::Margin::symmetric(7, 2)).show(ui, |ui| {
        ui.label(egui::RichText::new(text).font(semi(12.0)).color(TEXT));
    });
}

/// A titled group of settings: side by side in a row, or stacked (full width, a line under each) down a panel.
fn column(ui: &mut Ui, w: f32, title: &str, add: impl FnOnce(&mut Ui)) {
    if !ui.layout().is_horizontal() {
        small_caps(ui, title);
        add(ui);
        ui.add_space(4.0);
        let (x0, x1, y) = (ui.max_rect().left(), ui.max_rect().right(), ui.cursor().top());
        ui.painter().line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.0, LINE));
        ui.add_space(8.0);
        return;
    }
    ui.allocate_ui_with_layout(vec2(w, ui.available_height()), egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(w);
        small_caps(ui, title);
        add(ui);
    });
    let x = ui.cursor().left();
    let r = ui.max_rect();
    ui.painter().line_segment([pos2(x, r.top() + 4.0), pos2(x, r.bottom() - 4.0)], Stroke::new(1.0, LINE));
    ui.add_space(14.0);
}

fn overlays(ctx: &egui::Context, ed: &mut Editor, view: &View, lay: &frame::Layout, now: f64) {
    let r = view.rect;
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, Id::new("hud")));
    let c = pos2(r.center().x, r.center().y);
    match view.mode {
        Mode::Builder => {
            let s = Stroke::new(1.5, white(220));
            painter.line_segment([c - vec2(6.0, 0.0), c + vec2(6.0, 0.0)], s);
            painter.line_segment([c - vec2(0.0, 6.0), c + vec2(0.0, 6.0)], s);
            painter.circle_stroke(c, 12.0, Stroke::new(1.0, white(120)));
        }
        Mode::Editor => {
            painter.circle_stroke(c, 4.0, Stroke::new(1.0, white(70)));
        }
    }

    // looking through a camera: its picture, the rest dimmed
    if let Some(name) = ed.looking.as_ref().and_then(|l| ed.level.get(l.id)).map(|p| p.name.clone()) {
        let f = super::camera::frame(r);
        let dim = Color32::from_black_alpha(170);
        for band in [ERect::from_min_max(pos2(r.min.x, r.min.y), pos2(r.max.x, f.min.y)), ERect::from_min_max(pos2(r.min.x, f.max.y), pos2(r.max.x, r.max.y)),
            ERect::from_min_max(pos2(r.min.x, f.min.y), pos2(f.min.x, f.max.y)), ERect::from_min_max(pos2(f.max.x, f.min.y), pos2(r.max.x, f.max.y))] {
            if band.width() > 0.5 && band.height() > 0.5 { painter.rect_filled(band, CornerRadius::ZERO, dim); }
        }
        painter.rect_stroke(ERect::from_min_max(pos2(f.min.x, f.min.y), pos2(f.max.x, f.max.y)), CornerRadius::ZERO, Stroke::new(1.0, theme::CAT_CAMERAS), StrokeKind::Inside);
        theme::float_label(ctx, "looking", pos2(f.min.x + 14.0, f.min.y + 14.0), &format!("Looking through {name} · fly to aim it"), theme::CAT_CAMERAS, &["Esc"]);
    }

    // status island
    let rest = match view.mode {
        Mode::Editor => "Editor movement".to_string(),
        Mode::Builder => format!("Builder · fly {}", speed_text(view.speed)),
    };
    // under the tabs
    ed.island.draw(ctx, pos2(r.center().x, lay.m.y(92.0, frame::Fit::Stretch)), &rest, &["Caps Lock"]);

    // speed strip (Builder movement, or for 2 s after a change)
    let recent = now - view.speed_changed < 2.0;
    if view.mode == Mode::Builder || recent {
        egui::Area::new(Id::new("speeds")).order(egui::Order::Foreground).fixed_pos(pos2(r.min.x + 14.0, lay.panels[0].top() - 56.0)).interactable(false).show(ctx, |ui| {
            egui::Frame::new().fill(INK).corner_radius(R_CARD).stroke(Stroke::new(1.0, white(20))).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("FLY").font(mono(10.0)).color(MUTED));
                    let mut s = view.speed;
                    theme::segmented(ui, "speed", &SPEED_LABELS, &mut s, false);
                    theme::kbd(ui, "[");
                    theme::kbd(ui, "]");
                });
            });
        });
    }

    // what the mouse does here
    let hints: &[(&str, &[&str])] = match view.mode {
        Mode::Editor => &[("Orbit", &["Middle-drag"]), ("Pan", &["Space"]), ("Zoom", &["Wheel"]), ("Parts", &["Tab"]), ("Fly", &["Caps Lock"])],
        Mode::Builder => &[("Fly", &["W", "A", "S", "D"]), ("Speed", &["Mouse 4", "Mouse 5"]), ("Cursor", &["Alt"]), ("Parts", &["Tab"]), ("Edit", &["Caps Lock"])],
    };
    if ed.placing.is_none() && !ed.parts_open && ed.wiring.is_none() && ed.looking.is_none() && !ed.drawers[1] {
        hint_strip(ctx, lay, hints, view.mode == Mode::Editor);
    } else if let Some(shape) = ed.placing {
        theme::float_label(ctx, "placing", pos2(r.center().x - 170.0, r.max.y - 48.0), &format!("Placing {} · click a surface", if shape == Shape::Model { ed.placing_model.as_deref().and_then(|m| m.rsplit('/').next()).unwrap_or("model").to_string() } else { shape.label().to_string() }), ACCENT, &["Shift", "Esc"]);
    }

    if let Some((from, out)) = ed.wiring {
        let name = ed.level.get(from).map(|p| p.name.clone()).unwrap_or_default();
        theme::float_label(ctx, "wiring", pos2(r.center().x - 170.0, r.max.y - 48.0), &format!("Wire {name} · {out}: click the part it starts"), theme::CAT_TRIGGERS, &["Esc"]);
    }
    if let Some((from, out, to, at)) = ed.wire_pick {
        wire_menu(ctx, ed, from, out, to, pos2(at.x, at.y));
    }

    light_budget(ctx, ed, pos2(lay.left_pill.left(), lay.left_pill.bottom() + 10.0));

    // a texture or material being dragged out: a grabbing hand and its name under the pointer, all the way
    if let Some(tex) = ed.texturing.as_deref() {
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        if view.pointer().is_none() {
            if let Some(at) = ctx.pointer_hover_pos() {
                let name = tex.rsplit('/').next().unwrap_or_default().trim_end_matches(".bev");
                theme::float_label(ctx, "widget", at + vec2(18.0, 14.0), &format!("▦ {name}"), theme::VIOLET, &[]);
            }
        }
    }

    if let Some((at, text, color)) = ed.label.clone() {
        let keys: &[&str] = if ed.drag.is_some() || ed.texturing.is_some() { &[] } else { &["drag"] };
        theme::float_label(ctx, "widget", pos2(at.x, at.y), &text, color, keys);
    }
}

/// The light budget meter (under the menus, while the level has shadowed lights): the most shadowed lights reaching one
/// place, against the budget. Clicking it selects them in turn.
fn light_budget(ctx: &egui::Context, ed: &mut Editor, at: egui::Pos2) {
    let id = Id::new("light-budget");
    let worst = super::light::budget(&ed.level);
    if worst.is_empty() { theme::spring_reset(ctx, id.with("fade")); return; }
    let (n, max) = (worst.len(), super::light::BUDGET);
    let col = if n > max { theme::DANGER } else if n == max { ACCENT } else { theme::OK };
    let t = theme::spring_from(ctx, id.with("fade"), 0.0, 1.0, theme::SNAPPY);
    let names: Vec<String> = worst.iter().filter_map(|i| ed.level.get(*i)).map(|p| p.name.clone()).collect();
    let help = format!(
        "Shadowed lights reaching the same place: {} of the {max} that stay smooth.\n{}\n{}Click to select them in turn.",
        n, names.join(", "), if n > max { "Turn Shadows off on some, or make their reach smaller.\n" } else { "" }
    );
    let res = egui::Area::new(id).order(egui::Order::Foreground).pivot(egui::Align2::LEFT_TOP).fixed_pos(at).show(ctx, |ui| {
        ui.multiply_opacity(t.clamp(0.0, 1.0));
        let card = egui::Frame::new().fill(INK).corner_radius(R_CARD).stroke(Stroke::new(1.0, if n > max { col } else { white(20) })).inner_margin(egui::Margin::symmetric(10, 7)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("LIGHT BUDGET").font(mono(10.0)).color(MUTED));
                let (bar, _) = ui.allocate_exact_size(vec2(max as f32 * 14.0, 10.0), Sense::hover());
                for i in 0..max {
                    let cell = ERect::from_min_size(pos2(bar.min.x + i as f32 * 14.0, bar.min.y), vec2(11.0, 10.0));
                    ui.painter().rect_filled(cell, CornerRadius::same(2), if i < n { col } else { white(24) });
                }
                ui.label(egui::RichText::new(format!("{n} / {max}")).font(semi(12.0)).color(if n > max { col } else { TEXT }));
            });
        });
        ui.interact(card.response.rect, id.with("click"), Sense::click()).on_hover_text(help)
    }).inner;
    if res.clicked() {
        let next = ed.selected.and_then(|s| worst.iter().position(|i| *i == s)).map_or(0, |k| (k + 1) % n);
        ed.selected = Some(worst[next]);
        ed.material_open = None;
    }
}

/// After clicking the part a wire goes to: pick which of its inputs.
fn wire_menu(ctx: &egui::Context, ed: &mut Editor, from: u64, out: &'static str, to: u64, at: egui::Pos2) {
    let id = Id::new("wire-menu").with(to);
    let t = theme::spring_from(ctx, id, 0.0, 1.0, theme::SNAPPY);
    let Some(target) = ed.level.get(to).cloned() else {
        ed.wire_pick = None;
        return;
    };
    let mut pick: Option<&str> = None;
    let resp = egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(at + vec2(8.0, 8.0 * (1.0 - t))).show(ctx, |ui| {
        ui.multiply_opacity(t.clamp(0.0, 1.0));
        egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, white(24))).corner_radius(R_CARD).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
            ui.label(egui::RichText::new(format!("{out} starts {} …", target.name)).font(semi(13.0)));
            ui.add_space(4.0);
            for i in target.inputs() {
                if ui.add(egui::Button::new(egui::RichText::new(format!("▸ {i}")).font(semi(13.0))).min_size(vec2(160.0, 26.0))).clicked() { pick = Some(i); }
            }
            ui.add_space(2.0);
            theme::hint(ui, "Cancel", &["Esc"]);
        });
    });
    // (not in its first frames: the press that opened it)
    let outside = t > 0.3 && ctx.input(|i| i.pointer.any_pressed()) && !resp.response.contains_pointer();
    if let Some(i) = pick {
        ed.level.wires.push(Wire { from, out: out.into(), to, input: i.into() });
        let name = ed.level.get(from).map(|p| p.name.clone()).unwrap_or_default();
        ed.island.show("⟿", format!("Wired {name} → {}", target.name), format!("{out} → {i} · ` to test"), Tone::Accent);
        ed.wire_pick = None;
        theme::spring_reset(ctx, id);
    } else if outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        ed.wire_pick = None;
        theme::spring_reset(ctx, id);
    }
}

fn speed_text(i: usize) -> String {
    let s = super::view::SPEEDS[i];
    if s < 1.0 { format!("{}×", SPEED_LABELS[i]) } else { format!("{s}×") }
}

struct Item { glyph: &'static str, label: &'static str, shape: Option<Shape> }

/// The parts pop-up (Tab): a search field over three columns of colour-coded sections, each a grid of squares.
/// Pops in like the kit's palette (scale .92 → 1, rising 8 px, fading in; sections follow in a short stagger).
/// Arrow keys move through the shapes, Enter picks, Tab or Esc closes.
fn parts_popup(ctx: &egui::Context, ed: &mut Editor) {
    let id = Id::new("parts-pop");
    let screen = ctx.viewport_rect();
    let t = theme::spring_from(ctx, id, 0.0, 1.0, theme::SNAPPY);
    let shown = t.clamp(0.0, 1.0);

    // scrim: click outside to close
    egui::Area::new(id.with("scrim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
        let r = ui.allocate_rect(screen, Sense::click());
        ui.painter().rect_filled(screen, CornerRadius::ZERO, Color32::from_black_alpha((160.0 * shown) as u8));
        if r.clicked() { ed.parts_open = false; }
    });

    let q = ed.parts_query.trim().to_lowercase();
    let matches = |label: &str| q.is_empty() || label.to_lowercase().contains(&q);
    let shapes: Vec<Item> = Shape::ALL.iter().filter(|s| matches(s.label())).map(|s| Item { glyph: s.glyph(), label: s.label(), shape: Some(*s) }).collect();
    let lights: Vec<Item> = Shape::LIGHTS.iter().filter(|s| matches(s.label())).map(|s| Item { glyph: s.glyph(), label: s.label(), shape: Some(*s) }).collect();
    // (under the Cameras heading, "Fixed" says "Fixed camera")
    let cameras: Vec<Item> = Shape::CAMERAS.iter().filter(|s| matches(s.label())).map(|s| Item { glyph: s.glyph(), label: s.label().trim_end_matches(" camera"), shape: Some(*s) }).collect();
    let mut sections: Vec<(&str, Color32, Vec<Item>, bool)> =
        vec![("Shapes", theme::CAT_SHAPES, shapes, true), ("Lights", theme::CAT_LIGHTS, lights, true), ("Cameras", theme::CAT_CAMERAS, cameras, true)];
    for sec in &SCRIPT_SECTIONS {
        let shape_of = |l: &str| match l {
            "Player start" => Some(Shape::PlayerStart),
            "Interact" => Some(Shape::Interact),
            _ => None,
        };
        sections.push((sec.name, sec.color, sec.items.iter().filter(|(_, l)| matches(l)).map(|(g, l)| Item { glyph: g, label: l, shape: shape_of(l) }).collect(), sec.ready));
    }

    // keyboard: arrows move through the shapes (the parts that work in this version)
    let n = sections[0].2.len();
    let (left, right, up, down, enter, tab, esc) = ctx.input(|i| {
        use egui::Key::*;
        (i.key_pressed(ArrowLeft), i.key_pressed(ArrowRight), i.key_pressed(ArrowUp), i.key_pressed(ArrowDown), i.key_pressed(Enter), i.key_pressed(Tab), i.key_pressed(Escape))
    });
    let mut cursor = ed.parts_cursor.min(n.saturating_sub(1));
    if n > 0 {
        if right { cursor = (cursor + 1) % n; }
        if left { cursor = (cursor + n - 1) % n; }
        if down && cursor + 4 < n { cursor += 4; }
        if up && cursor >= 4 { cursor -= 4; }
    }

    // POP_IN
    let scale = 0.92 + 0.08 * t;
    let center = screen.center();
    let layer = egui::LayerId::new(egui::Order::Foreground, id);
    ctx.set_transform_layer(layer, TSTransform::new(center.to_vec2() * (1.0 - scale) + vec2(0.0, 8.0 * (1.0 - t)), scale));

    let mut pick: Option<Shape> = None;
    let mut soon: Option<&str> = None;
    let mut info: Option<(String, String, Color32)> = None;
    let w = 720.0;
    let gap = 16.0;
    let col_w = (w - 2.0 * gap) / 3.0;
    egui::Area::new(id).order(egui::Order::Foreground).pivot(Align2::CENTER_CENTER).fixed_pos(center).show(ctx, |ui| {
        ui.multiply_opacity(shown);
        let frame = egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, white(24))).corner_radius(R_CARD).inner_margin(egui::Margin::same(20)).shadow(ctx.global_style().visuals.window_shadow);
        frame.show(ui, |ui| {
            ui.set_width(w);
            // title row
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Parts").font(semi(18.0)));
                ui.add_space(4.0);
                ui.label(egui::RichText::new("pick one, then click a surface").color(MUTED));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("close").color(MUTED));
                    theme::kbd(ui, "Esc");
                    theme::kbd(ui, "Tab");
                });
            });
            ui.add_space(10.0);
            // search field, kit style
            let focused = ui.memory(|m| m.focused()).is_some();
            egui::Frame::new()
                .fill(white(10))
                .stroke(Stroke::new(1.0, if focused { Color32::from_rgba_unmultiplied(0xf5, 0xdc, 0x4a, 150) } else { LINE }))
                .corner_radius(CornerRadius { nw: 12, ne: 12, se: 12, sw: 4 })
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // magnifier, drawn (no font has a good one)
                        let (ir, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                        let c = ir.center() - vec2(1.5, 1.5);
                        ui.painter().circle_stroke(c, 5.5, Stroke::new(1.6, MUTED));
                        ui.painter().line_segment([c + vec2(4.0, 4.0), c + vec2(8.0, 8.0)], Stroke::new(1.8, MUTED));
                        let te = ui.add(
                            egui::TextEdit::singleline(&mut ed.parts_query)
                                .hint_text("Search parts")
                                .frame(egui::Frame::NONE)
                                .font(body(15.0))
                                .desired_width(ui.available_width()),
                        );
                        if !te.has_focus() { te.request_focus(); }
                        if te.changed() { cursor = 0; }
                    });
                });
            ui.add_space(16.0);

            if sections.iter().all(|s| s.2.is_empty()) {
                ui.vertical_centered(|ui| {
                    ui.add_space(30.0);
                    ui.label(egui::RichText::new(format!("No parts match “{}”", ed.parts_query.trim())).font(semi(14.0)).color(MUTED));
                    ui.add_space(30.0);
                });
            } else {
                // three columns, each a stack of sections: no gaps where one section is taller than its neighbour
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for col in [[0usize, 3], [1, 4], [2, 5]] {
                        ui.vertical(|ui| {
                            ui.set_width(col_w);
                            for &si in &col {
                                let (name, color, items, ready) = &sections[si];
                                if items.is_empty() { continue; }
                                let a = ((t - si as f32 * 0.05) / 0.75).clamp(0.0, 1.0);
                                ui.scope(|ui| {
                                    ui.multiply_opacity(a);
                                    section_bar(ui, col_w, name, *color, *ready, items.len());
                                    ui.add_space(6.0);
                                    let sq = ((col_w - 3.0 * 8.0) / 4.0).floor();
                                    for (ri, row) in items.chunks(4).enumerate() {
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 8.0;
                                            for (ci, it) in row.iter().enumerate() {
                                                let k = ri * 4 + ci;
                                                let selected = si == 0 && k == cursor;
                                                let (clicked, hovered) = part_square(ui, sq, it.glyph, it.shape.filter(|s| Shape::ALL.contains(s)), it.label, *color, *ready || it.shape.is_some(), selected);
                                                if hovered || (selected && info.is_none()) {
                                                    let desc = match it.shape {
                                                        Some(s) => s.description(),
                                                        None => "Coming with script parts in v0.2".to_string(),
                                                    };
                                                    if hovered || info.is_none() { info = Some((it.label.to_string(), desc, *color)); }
                                                }
                                                if clicked {
                                                    match it.shape {
                                                        Some(s) => pick = Some(s),
                                                        None => soon = Some(it.label),
                                                    }
                                                }
                                            }
                                        });
                                        ui.add_space(2.0);
                                    }
                                });
                                ui.add_space(14.0);
                            }
                        });
                    }
                });
            }

            // footer: what the highlighted part is, and the keys
            let (r, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
            ui.painter().rect_filled(r, CornerRadius::ZERO, LINE);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if let Some((label, desc, color)) = &info {
                    let (d, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                    ui.painter().rect_filled(d, CornerRadius::same(2), *color);
                    ui.label(egui::RichText::new(label).font(semi(13.0)));
                    ui.label(egui::RichText::new(desc).color(MUTED));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("place").color(MUTED));
                    theme::kbd(ui, "Enter");
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("move").color(MUTED));
                    for k in ["→", "↓", "↑", "←"] { theme::kbd(ui, k); }
                });
            });
        });
    });

    ed.parts_cursor = cursor;
    if enter && pick.is_none() {
        if let Some(Item { shape: Some(s), .. }) = sections[0].2.get(cursor) { pick = Some(*s); }
    }
    if let Some(s) = pick {
        ed.placing = Some(s);
        ed.placing_model = None;
        ed.parts_open = false;
        ed.island.show(s.glyph(), format!("Placing {}", s.label()), "Click a surface · Shift keeps it upright · Esc cancels", Tone::Accent);
    } else if let Some(l) = soon {
        ed.island.show("…", format!("{l} arrives with script parts"), "v0.2: triggers, actions, flow, logic", Tone::Plain);
    }
    if (tab && !ed.parts_fresh) || esc { ed.parts_open = false; }
    ed.parts_fresh = false;
}

/// A section's colour bar: name on the left, the count (or the version it arrives in) on the right.
fn section_bar(ui: &mut Ui, w: f32, name: &str, color: Color32, ready: bool, count: usize) {
    let (r, _) = ui.allocate_exact_size(vec2(w, 24.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, CornerRadius { nw: 8, ne: 8, se: 8, sw: 3 }, color);
    p.text(r.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, name, semi(12.5), INK);
    let tag = if ready { format!("{count}") } else { "v0.2".into() };
    let g = p.layout_no_wrap(tag, mono(10.0), INK);
    let chip = ERect::from_min_size(pos2(r.right() - g.size().x - 18.0, r.center().y - 8.0), vec2(g.size().x + 10.0, 16.0));
    p.rect_filled(chip, CornerRadius::same(8), Color32::from_black_alpha(40));
    p.galley(pos2(chip.left() + 5.0, chip.center().y - g.size().y / 2.0), g, INK);
}

/// One square: glyph and name. Hover lifts it and lights the border in the section colour; the keyboard
/// highlight is the accent. Returns (clicked, hovered).
fn part_square(ui: &mut Ui, size: f32, glyph: &str, shape: Option<Shape>, label: &str, color: Color32, ready: bool, selected: bool) -> (bool, bool) {
    let (r, resp) = ui.allocate_exact_size(vec2(size, size + 6.0), Sense::click());
    let hot = theme::spring(ui.ctx(), resp.id.with("hover"), if resp.hovered() { 1.0 } else { 0.0 }, theme::SNAPPY);
    let sel = theme::spring(ui.ctx(), resp.id.with("sel"), if selected { 1.0 } else { 0.0 }, theme::SNAPPY);
    let lift = 2.0 * hot.max(sel);
    let r = r.translate(vec2(0.0, -lift));
    let p = ui.painter();
    let radius = CornerRadius { nw: 12, ne: 12, se: 12, sw: 4 };
    p.rect_filled(r, radius, theme::lerp_color(white(9), white(22), hot.max(sel * 0.7).clamp(0.0, 1.0)));
    let stroke = if resp.hovered() {
        Stroke::new(1.5, color)
    } else if selected {
        Stroke::new(1.5, ACCENT)
    } else {
        Stroke::new(1.0, white(22))
    };
    p.rect_stroke(r, radius, stroke, StrokeKind::Inside);
    let a = if ready { 1.0 } else { 0.42 };
    let ic = r.center() - vec2(0.0, 9.0);
    match shape {
        Some(s) => shape_icon(p, ic, 11.0 + hot, s, TEXT.gamma_multiply(a)),
        None => {
            p.text(ic, Align2::CENTER_CENTER, glyph, body(20.0 + 2.0 * hot), TEXT.gamma_multiply(a));
        }
    }
    p.text(pos2(r.center().x, r.bottom() - 13.0), Align2::CENTER_CENTER, label, body(10.5), white(200).gamma_multiply(a));
    let clicked = resp.clicked();
    let hovered = resp.hovered();
    resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    (clicked, hovered)
}

fn unsaved_modal(ctx: &egui::Context, ed: &mut Editor, what: Pending, exit: &mut MessageWriter<AppExit>) {
    let verb = match what {
        Pending::Quit => "quit",
        Pending::New => "start a new project",
        Pending::Open => "open another project",
    };
    let mut choice = None;
    egui::Modal::new(Id::new("unsaved")).frame(egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, LINE)).corner_radius(R_CARD).inner_margin(egui::Margin::same(22))).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.label(egui::RichText::new("Unsaved changes").font(semi(17.0)));
        ui.add_space(4.0);
        ui.label(egui::RichText::new(format!("Save “{}” before you {verb}?", ed.name)).color(MUTED));
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            let save = egui::Button::new(egui::RichText::new("Save").font(semi(13.0)).color(INK)).fill(ACCENT).min_size(vec2(90.0, 32.0));
            if ui.add(save).clicked() { choice = Some(1); }
            if theme::hold_button(ui, "Hold to discard", 1.0) { choice = Some(2); }
            if ui.add(egui::Button::new("Cancel").min_size(vec2(80.0, 32.0))).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) { choice = Some(0); }
        });
    });
    match choice {
        Some(1) => {
            ed.save();
            ed.pending = None;
            if !ed.dirty() { ed.run(what, exit); }
        }
        Some(2) => {
            ed.pending = None;
            ed.saved = ed.level.clone();
            ed.run(what, exit);
        }
        Some(_) => ed.pending = None,
        None => {}
    }
}

/// Help > Features: what's built and what's planned, in build order (editor/features.json, updated with every
/// release).
#[derive(serde::Deserialize)]
struct Roadmap { updated: String, features: Vec<Feature> }

#[derive(serde::Deserialize)]
struct Feature {
    id: String, name: String, area: String, status: String, effort: String, needs: Vec<String>, what: String, chunks: Vec<String>, note: String, order: u32,
    /// When a built feature was finished (YYYY-MM-DD) and the release build it shipped in.
    #[serde(default)]
    built: String,
    #[serde(default)]
    build: u32,
}

/// "2026-11-15" as "15 Nov 2026" (or "15 Nov" when `short`).
fn day(s: &str, short: bool) -> String {
    const M: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let p: Vec<&str> = s.split('-').collect();
    let (Some(y), Some(m), Some(d)) = (p.first(), p.get(1).and_then(|m| m.parse::<usize>().ok()).filter(|m| (1..=12).contains(m)), p.get(2).and_then(|d| d.parse::<u32>().ok())) else { return s.into() };
    if short { format!("{d} {}", M[m - 1]) } else { format!("{d} {} {y}", M[m - 1]) }
}

fn roadmap() -> &'static Roadmap {
    static R: std::sync::OnceLock<Roadmap> = std::sync::OnceLock::new();
    R.get_or_init(|| serde_json::from_str(include_str!("../../features.json")).expect("features.json"))
}

fn features_modal(ctx: &egui::Context, ed: &mut Editor) {
    let r = roadmap();
    let done = |id: &str| r.features.iter().any(|f| f.id == id && f.status == "done");
    let ready = |f: &Feature| f.status == "planned" && f.needs.iter().all(|n| done(n));
    let id = Id::new("features");
    let mut filter = ctx.data(|d| d.get_temp::<usize>(id)).unwrap_or(1);
    let mut open = ctx.data(|d| d.get_temp::<String>(id.with("open"))).unwrap_or_default();
    let mut close = false;
    let h = (ctx.viewport_rect().height() - 280.0).max(240.0);
    egui::Modal::new(id).frame(dialog_frame()).show(ctx, |ui| {
        ui.set_width(720.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Features").font(semi(18.0)).color(TEXT));
            let n_done = r.features.iter().filter(|f| f.status == "done").count();
            ui.label(egui::RichText::new(format!("{n_done} of {} built · updated {} · {}", r.features.len(), r.updated, this_build())).color(MUTED));
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            theme::segmented(ui, "feat-filter", &["All, in build order", "Ready to build", "Built"], &mut filter, false);
            ui.label(egui::RichText::new("S under a session · M one · L and XL in session-sized parts").size(11.0).color(MUTED));
        });
        ui.add_space(8.0);
        egui::ScrollArea::vertical().max_height(h).show(ui, |ui| {
            for f in r.features.iter().filter(|f| match filter { 1 => ready(f), 2 => f.status == "done", _ => true }) {
                let is_open = open == f.id;
                let (dot, word) = match f.status.as_str() {
                    "done" => (theme::OK, format!("built {}", day(&f.built, true))),
                    "hold" => (white(40), "on hold".into()),
                    _ if ready(f) => (ACCENT, "ready".into()),
                    _ => (white(60), "planned".into()),
                };
                let resp = egui::Frame::new().fill(if is_open { white(10) } else { Color32::TRANSPARENT }).corner_radius(CornerRadius { nw: 10, ne: 10, se: 10, sw: 3 }).inner_margin(egui::Margin::symmetric(8, 5)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("{:>2}", f.order)).font(mono(11.0)).color(MUTED));
                        let (d, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                        ui.painter().circle_filled(d.center(), 3.5, dot);
                        ui.label(egui::RichText::new(&f.name).font(semi(13.0)).color(TEXT));
                        ui.label(egui::RichText::new(&f.area).size(11.0).color(MUTED));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(&f.effort).font(mono(11.0)).color(ACCENT));
                            ui.label(egui::RichText::new(word).size(11.0).color(dot));
                        });
                    });
                    if is_open {
                        ui.add(egui::Label::new(egui::RichText::new(&f.what).color(TEXT)).wrap());
                        if f.status == "done" { ui.label(egui::RichText::new(format!("Built {} in build {}", day(&f.built, false), f.build)).size(12.0).color(theme::OK)); }
                        if !f.needs.is_empty() {
                            let names: Vec<String> = f.needs.iter().map(|n| {
                                let g = r.features.iter().find(|g| &g.id == n);
                                format!("{}{}", g.map_or(n.as_str(), |g| g.name.as_str()), if done(n) { " ✓" } else { "" })
                            }).collect();
                            ui.add(egui::Label::new(egui::RichText::new(format!("Needs: {}", names.join(" · "))).size(12.0).color(MUTED)).wrap());
                        }
                        for (i, c) in f.chunks.iter().enumerate() {
                            ui.add(egui::Label::new(egui::RichText::new(format!("Part {}: {c}", i + 1)).size(12.0).color(MUTED)).wrap());
                        }
                        if !f.note.is_empty() { ui.label(egui::RichText::new(&f.note).size(11.0).italics().color(MUTED)); }
                    }
                });
                if resp.response.interact(Sense::click()).clicked() { open = if is_open { String::new() } else { f.id.clone() }; }
            }
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Click a feature for what it needs and its parts.").size(11.0).color(MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                theme::kbd(ui, "Esc");
                if ui.add(egui::Button::new("Close").min_size(vec2(80.0, 32.0))).clicked() { close = true; }
            });
        });
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { close = true; }
    });
    ctx.data_mut(|d| {
        d.insert_temp(id, filter);
        d.insert_temp(id.with("open"), open);
    });
    if close { ed.features_open = false; }
}

fn dialog_frame() -> egui::Frame { egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, LINE)).corner_radius(R_CARD).inner_margin(egui::Margin::same(22)) }

fn this_build() -> String { update::this_build().map_or("a build made by hand".into(), |n| format!("build {n}")) }

fn about_modal(ctx: &egui::Context, ed: &mut Editor) {
    let mut close = false;
    egui::Modal::new(Id::new("about")).frame(dialog_frame()).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.label(egui::RichText::new("Beverly").font(semi(20.0)).color(TEXT));
        ui.add_space(2.0);
        ui.label(egui::RichText::new(format!("This is {}", this_build())).color(ACCENT));
        ui.add_space(10.0);
        ui.add(egui::Label::new(egui::RichText::new("A no-code-first 3D game editor for Bevy: build levels by placing and wiring parts, test them on the spot, and export a Windows game.").color(TEXT)).wrap());
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Bevy 0.20.1 · bevy_egui 0.43.1 · Motion UI kit").color(MUTED));
        ui.hyperlink_to(egui::RichText::new("github.com/thriple/Beverly").color(ACCENT), env!("CARGO_PKG_REPOSITORY"));
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new("Check for updates…").min_size(vec2(150.0, 32.0))).clicked() {
                close = true;
                ed.updates_open = true;
                ed.updater.check();
            }
            if ui.add(egui::Button::new("Close").min_size(vec2(80.0, 32.0))).clicked() { close = true; }
            theme::kbd(ui, "Esc");
        });
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { close = true; }
    });
    if close { ed.about_open = false; }
}

/// Help > Check for updates: what the check found, and the button that downloads and installs a newer build.
fn updates_modal(ctx: &egui::Context, ed: &mut Editor) {
    let status = ed.updater.status();
    if status.busy() { ctx.request_repaint(); }
    let (mut close, mut install, mut again) = (false, false, false);
    egui::Modal::new(Id::new("updates")).frame(dialog_frame()).show(ctx, |ui| {
        ui.set_width(440.0);
        ui.label(egui::RichText::new("Check for updates").font(semi(17.0)).color(TEXT));
        ui.label(egui::RichText::new(format!("This is {}", this_build())).color(MUTED));
        ui.add_space(12.0);
        let say = |ui: &mut Ui, text: &str| {
            ui.add(egui::Label::new(egui::RichText::new(text).color(TEXT)).wrap());
        };
        let accent = |label: &str| egui::Button::new(egui::RichText::new(label).font(semi(13.0)).color(INK)).fill(ACCENT).min_size(vec2(170.0, 32.0));
        match &status {
            Status::Idle | Status::Checking => say(ui, "Looking for a newer build…"),
            Status::Local => say(ui, "This build was made by hand, so there's nothing to compare it with. Updates are for the builds from the Releases page."),
            Status::Unsupported => say(ui, "Updates are for the Windows build."),
            Status::GameEditor => say(ui, "This editor is part of a game, so it updates with the game's own downloads."),
            Status::UpToDate(n) => say(ui, &format!("Build {n} is the newest.")),
            Status::Available { latest, .. } => {
                say(ui, &format!("Build {latest} is available. Beverly downloads it, checks it, and puts it in place of this one (the editor and the game player). It runs from the next start."));
                ui.add_space(12.0);
                if ui.add(accent("Download and install")).clicked() { install = true; }
            }
            Status::Downloading { latest, done, total } => {
                let mb = |b: u64| b as f64 / 1_048_576.0;
                say(ui, &match total {
                    Some(t) => format!("Downloading build {latest}: {:.1} of {:.1} MB", mb(*done), mb(*t)),
                    None => format!("Downloading build {latest}: {:.1} MB", mb(*done)),
                });
                ui.add_space(10.0);
                let w = ui.available_width();
                let (bar, _) = ui.allocate_exact_size(vec2(w, 8.0), Sense::hover());
                ui.painter().rect_filled(bar, CornerRadius::same(4), white(18));
                if let Some(t) = total.filter(|t| *t > 0) {
                    let f = (*done as f32 / t as f32).clamp(0.0, 1.0);
                    ui.painter().rect_filled(bar.with_max_x(bar.left() + w * f), CornerRadius::same(4), ACCENT);
                }
            }
            Status::Installed(n) => {
                ui.label(egui::RichText::new(format!("Installed build {n}.")).font(semi(14.0)).color(theme::OK));
                say(ui, "Close Beverly and open it again to use it.");
            }
            Status::Failed(why) => {
                say(ui, why);
                ui.add_space(12.0);
                if ui.add(accent("Try again")).clicked() { again = true; }
            }
        }
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Older builds stay on the Releases page.").size(11.0).color(MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                theme::kbd(ui, "Esc");
                // a download carries on with the window closed
                if ui.add(egui::Button::new("Close").min_size(vec2(80.0, 32.0))).clicked() { close = true; }
            });
        });
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { close = true; }
    });
    if close { ed.updates_open = false; }
    if install { ed.updater.install(); }
    if again { ed.updater.check(); }
}

/// Help > MCP Server: let an AI assistant on this computer (such as Claude Code) read the project and run actions.
fn mcp_modal(ctx: &egui::Context, ed: &mut Editor) {
    let (mut close, mut changed) = (false, false);
    egui::Modal::new(Id::new("mcp")).frame(dialog_frame()).show(ctx, |ui| {
        ui.set_width(520.0);
        ui.label(egui::RichText::new("MCP Server").font(semi(17.0)).color(TEXT));
        ui.add(egui::Label::new(egui::RichText::new("Lets an AI assistant on this computer, such as Claude Code, build with you: read the project, place and change parts, wire them, take screenshots, play-test the level and run your tests. Everything it changes can be undone.").color(MUTED)).wrap());
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let mut on = ed.mcp.settings.enabled;
            if theme::toggle(ui, &mut on).changed() {
                ed.mcp.settings.enabled = on;
                changed = true;
            }
            ui.label(egui::RichText::new("Let AI assistants connect").font(semi(13.0)).color(TEXT));
        });
        let (dot, state) = match &ed.mcp.listen {
            mcp::Listen::Off => (white(60), "Off".to_string()),
            mcp::Listen::On(port) => (theme::OK, format!("Listening on port {port}, this computer only")),
            mcp::Listen::Failed(why) => (theme::DANGER, why.clone()),
        };
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 3.5, dot);
            ui.add(egui::Label::new(egui::RichText::new(state).color(MUTED)).wrap());
        });
        ui.add_space(10.0);
        small_caps(ui, "ADDRESS");
        ui.label(egui::RichText::new(ed.mcp.settings.url()).font(mono(12.0)).color(TEXT));
        ui.add_space(6.0);
        small_caps(ui, "TOKEN");
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&ed.mcp.settings.token).font(mono(12.0)).color(TEXT));
            if ui.small_button("New token").on_hover_text("Assistants using the old one can no longer connect").clicked() {
                ed.mcp.settings.token = mcp::new_token();
                changed = true;
            }
        });
        ui.add_space(6.0);
        small_caps(ui, "ADD TO CLAUDE CODE");
        let cmd = ed.mcp.settings.claude_command();
        egui::Frame::new().fill(white(8)).corner_radius(CornerRadius { nw: 10, ne: 10, se: 10, sw: 3 }).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.add(egui::Label::new(egui::RichText::new(&cmd).font(mono(11.0)).color(TEXT)).wrap());
        });
        if ui.button("Copy").clicked() {
            ctx.copy_text(cmd);
            ed.island.show("⧉", "Copied", "Paste it into a terminal", Tone::Plain);
        }
        if !ed.mcp.log.is_empty() {
            ui.add_space(8.0);
            small_caps(ui, "RECENT CALLS");
            for l in &ed.mcp.log { ui.label(egui::RichText::new(l).font(mono(11.0)).color(MUTED)); }
        }
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Off by default. The token stays on this computer.").size(11.0).color(MUTED));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                theme::kbd(ui, "Esc");
                if ui.add(egui::Button::new("Close").min_size(vec2(80.0, 32.0))).clicked() { close = true; }
            });
        });
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { close = true; }
    });
    if changed { ed.mcp.apply(); }
    if close { ed.mcp_open = false; }
}

/// The basic shapes as small line drawings (one stroke weight, all the same size), in a box of half-size `h`.
pub fn shape_icon(p: &egui::Painter, c: egui::Pos2, h: f32, shape: Shape, col: Color32) {
    let st = Stroke::new(1.6, col);
    // a simple oblique projection: x right, y up, z back-and-up
    let pt = |x: f32, y: f32, z: f32| pos2(c.x + (x * 0.8 + z * 0.45) * h, c.y - (y * 0.8 + z * 0.3) * h);
    // edges as separate segments with round dots at the corners (closed lines get mitre spikes at sharp corners)
    let poly = |pts: &[egui::Pos2]| {
        for i in 0..pts.len() {
            p.line_segment([pts[i], pts[(i + 1) % pts.len()]], st);
            p.circle_filled(pts[i], st.width / 2.0, col);
        }
    };
    let line = |a: egui::Pos2, b: egui::Pos2| p.line_segment([a, b], st);
    match shape {
        Shape::Box => {
            poly(&[pt(-1., -1., -1.), pt(1., -1., -1.), pt(1., 1., -1.), pt(-1., 1., -1.)]);
            poly(&[pt(-1., 1., -1.), pt(-1., 1., 1.), pt(1., 1., 1.), pt(1., 1., -1.)]);
            poly(&[pt(1., -1., -1.), pt(1., -1., 1.), pt(1., 1., 1.), pt(1., 1., -1.)]);
        }
        Shape::Plane => {
            poly(&[pt(-1., -0.15, -1.), pt(1., -0.15, -1.), pt(1., 0.15, -1.), pt(-1., 0.15, -1.)]);
            poly(&[pt(-1., 0.15, -1.), pt(-1., 0.15, 1.), pt(1., 0.15, 1.), pt(1., 0.15, -1.)]);
            poly(&[pt(1., -0.15, -1.), pt(1., -0.15, 1.), pt(1., 0.15, 1.), pt(1., 0.15, -1.)]);
        }
        Shape::Ramp => {
            poly(&[pt(-1., -1., -1.), pt(1., -1., -1.), pt(1., 1., -1.)]);
            poly(&[pt(-1., -1., -1.), pt(-1., -1., 1.), pt(1., 1., 1.), pt(1., 1., -1.)]);
            poly(&[pt(1., -1., -1.), pt(1., -1., 1.), pt(1., 1., 1.), pt(1., 1., -1.)]);
        }
        Shape::Stairs => {
            let n = 3;
            let mut prof = vec![pt(-1., -1., -1.), pt(1., -1., -1.), pt(1., 1., -1.)];
            for k in (0..n).rev() {
                let x = -1.0 + 2.0 * k as f32 / n as f32;
                let top = -1.0 + 2.0 * (k + 1) as f32 / n as f32;
                prof.push(pt(x, top, -1.));
                if k > 0 { prof.push(pt(x, top - 2.0 / n as f32, -1.)); }
            }
            poly(&prof);
            for k in 0..n {
                let x = -1.0 + 2.0 * k as f32 / n as f32;
                let top = -1.0 + 2.0 * (k + 1) as f32 / n as f32;
                line(pt(x, top, -1.), pt(x, top, 1.));
                line(pt(x, top, 1.), pt(x + 2.0 / n as f32, top, 1.));
            }
            line(pt(1., 1., -1.), pt(1., 1., 1.));
            line(pt(1., -1., -1.), pt(1., -1., 1.));
            line(pt(1., -1., 1.), pt(1., 1., 1.));
        }
        Shape::Arch => {
            let mut v = vec![pt(-1., -1., 0.), pt(-0.45, -1., 0.), pt(-0.45, -0.1, 0.)];
            for i in 1..12 {
                let a = std::f32::consts::PI * (1.0 - i as f32 / 12.0);
                v.push(pt(a.cos() * 0.45, -0.1 + a.sin() * 0.45, 0.));
            }
            v.extend([pt(0.45, -0.1, 0.), pt(0.45, -1., 0.), pt(1., -1., 0.), pt(1., 1., 0.), pt(-1., 1., 0.)]);
            poly(&v);
            line(pt(-1., 1., 0.), pt(-1., 1., 0.9));
            line(pt(1., 1., 0.), pt(1., 1., 0.9));
            line(pt(-1., 1., 0.9), pt(1., 1., 0.9));
            line(pt(1., -1., 0.), pt(1., -1., 0.9));
            line(pt(1., -1., 0.9), pt(1., 1., 0.9));
        }
        Shape::Cylinder => {
            let rx = 0.75 * h;
            let ry = 0.28 * h;
            let (t, b) = (c.y - 0.75 * h, c.y + 0.75 * h);
            let ell = |cy: f32, full: bool| {
                let n = 28;
                let pts: Vec<egui::Pos2> = (0..=n)
                    .map(|i| {
                        let a = if full { std::f32::consts::TAU } else { std::f32::consts::PI } * i as f32 / n as f32;
                        pos2(c.x + rx * a.cos(), cy + ry * a.sin())
                    })
                    .collect();
                p.add(egui::Shape::line(pts, st));
            };
            ell(t, true);
            ell(b, false);
            line(pos2(c.x - rx, t), pos2(c.x - rx, b));
            line(pos2(c.x + rx, t), pos2(c.x + rx, b));
        }
        Shape::Sphere => {
            p.circle_stroke(c, 0.85 * h, st);
            let n = 28;
            let pts: Vec<egui::Pos2> = (0..=n)
                .map(|i| {
                    let a = std::f32::consts::PI * i as f32 / n as f32;
                    pos2(c.x + 0.85 * h * a.cos(), c.y + 0.28 * h * a.sin())
                })
                .collect();
            p.add(egui::Shape::line(pts, st));
        }
        Shape::Model | Shape::PlayerStart | Shape::Interact | Shape::PointLight | Shape::SpotLight | Shape::FixedCamera | Shape::OrbitCamera | Shape::FollowCamera => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_features_list_reads_and_every_feature_comes_after_what_it_needs() {
        let r = roadmap();
        for f in &r.features {
            assert!(["S", "M", "L", "XL"].contains(&f.effort.as_str()), "{}: effort", f.id);
            assert_eq!(f.chunks.is_empty(), matches!(f.effort.as_str(), "S" | "M"), "{}: L and XL come in parts", f.id);
            for n in &f.needs {
                let g = r.features.iter().find(|g| &g.id == n).unwrap_or_else(|| panic!("{} needs unknown {n}", f.id));
                assert!(g.order < f.order, "{} is ordered before {n}", f.id);
            }
            if f.status == "done" { assert!(f.build > 0 && f.built.len() == 10 && day(&f.built, false) != f.built, "{}: when it was built", f.id); }
        }
        assert_eq!(day("2026-11-05", false), "5 Nov 2026");
        assert_eq!(day("2026-01-15", true), "15 Jan");
    }
}
