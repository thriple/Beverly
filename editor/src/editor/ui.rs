//! The 2D parts of the editor: the user's framed layout (frame.rs, docs/ui.svg) in the Motion kit's style: the
//! menus and tabs along the top, the Assets card, the three bottom panels with their drawers, the parts pop-up
//! (Tab), the status island, the speed strip, hints, and the unsaved-changes question.
use crate::level::{self, Shape, Wire};
use crate::settings::{self, Setting};
use crate::update::{self, Status};
use super::theme::{self, highlight, INK, LINE, MUTED, PANEL, R_CARD, TEXT, TEXT_2, Tip, Tone, mono, semi, white};
use super::view::{Mode, SPEED_LABELS, View};
use super::{actions, audio, frame, mcp, Editor, LevelMode, Pending};

#[path = "audio_ui.rs"]
mod audio_ui;
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
    mut window: Single<&mut Window, With<bevy::window::PrimaryWindow>>,
    cam: Single<(&Camera, &GlobalTransform), With<super::view::MainCamera>>,
    sounds: Res<crate::sound::Sounds>,
    perf: Res<crate::perf::Perf>,
    mut baked: MessageReader<crate::bake::BakeDone>,
    mut tinted: Local<usize>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    // the open tab's colours (the frame, highlight and accent)
    theme::set_tab(ed.tab);
    if *tinted != ed.tab { theme::retint(&ctx); *tinted = ed.tab; }
    if !*init {
        // fonts take effect from the next frame, so draw nothing until then
        theme::setup(&ctx);
        *init = true;
        return Ok(());
    }
    for d in baked.read() {
        match &d.error {
            // thumbnails come and go quietly
            None if d.kind == "thumb" => {}
            None => ed.island.show("✓", format!("Baked {}", d.label), "kept in the bake cache", Tone::Accent),
            Some(e) => ed.island.show("!", format!("Couldn't bake {}", d.label), e.clone(), Tone::Danger),
        }
    }
    // test mode: no editor panels, just the status island
    if ed.testing.is_some() {
        let r = ctx.viewport_rect();
        view.rect = Rect::new(r.left(), r.top(), r.right(), r.bottom());
        view.outline.clear();
        ed.island.draw(&ctx, r.min + vec2(14.0, 14.0), "play", "Testing · ` to stop");
        return Ok(());
    }
    // the frame: the view set into it as one outline, its notches as wide as what they hold
    let lay = frame::layout(ctx.viewport_rect(), topics_width(&ctx, &ed), bottom_width(&ctx, &ed, &view));
    view.rect = Rect::new(lay.view.left(), lay.view.top(), lay.view.right(), lay.view.bottom());
    view.outline = lay.outline.clone();
    let edge = if look().frame_text == INK { Color32::from_rgba_unmultiplied(14, 14, 16, 56) } else { white(51) };
    frame::paint_frame(&ctx.layer_painter(egui::LayerId::background()), &lay, frame::Look { fill: look().frame, alpha: look().frame_alpha, edge });
    // the window's own edge
    let we = if look().frame_text == INK { Color32::from_rgba_unmultiplied(14, 14, 16, 89) } else { white(36) };
    ctx.layer_painter(egui::LayerId::background()).rect_stroke(lay.window, CornerRadius::ZERO, Stroke::new(1.0, we), StrokeKind::Inside);
    top_bar(&ctx, &mut ed, &lay, &mut window, &mut exit);
    topic_chips(&ctx, &mut ed, &lay);
    bottom_notch(&ctx, &mut ed, &view, &lay, &mut exit);
    dock(&ctx, &mut ed, &mut view, &lay, &sounds, &mut exit);
    if ed.tab == audio::AUDIO { audio_ui::stage_bar(&ctx, &mut ed, &lay, &sounds); }
    let max = ed.maximized;
    window_edges(&ctx, &lay, &mut window, max);

    overlays(&ctx, &mut ed, &view, &perf, time.elapsed_secs_f64());
    if ed.looking.is_none() && ed.tab == audio::LEVEL { selection_tag(&ctx, &ed, &view, (cam.0, cam.1)); }
    if ed.tab == audio::AUDIO { stage_tag(&ctx, &ed, &view, (cam.0, cam.1)); }
    if ed.tab == audio::LEVEL { stretch_button(&ctx, &mut ed, &view, (cam.0, cam.1)); }
    eyedropper(&ctx, &ed, &view, (cam.0, cam.1));
    if ed.ask.as_ref().is_some_and(|(id, _)| ed.level.get(*id).is_some()) { ask_card(&ctx, &mut ed, &view); } else { theme::spring_reset(&ctx, Id::new("ask")); }
    if ed.parts_open { parts_popup(&ctx, &mut ed); } else { theme::spring_reset(&ctx, Id::new("parts-pop")); }
    if let Some(what) = ed.pending { unsaved_modal(&ctx, &mut ed, what, &mut exit); }
    if ed.about_open { about_modal(&ctx, &mut ed); }
    if ed.updates_open { updates_modal(&ctx, &mut ed); }
    if ed.features_open { features_modal(&ctx, &mut ed); }
    if ed.mcp_open { mcp_modal(&ctx, &mut ed); }
    theme::focus_ring(&ctx);
    Ok(())
}

fn small_caps(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).font(semi(11.5)).color(MUTED));
}

/// The open tab's colours.
fn look() -> theme::TabTheme { theme::tab() }

fn text_w(ctx: &egui::Context, text: &str, font: egui::FontId) -> f32 { ctx.fonts_mut(|f| f.layout_no_wrap(text.to_owned(), font, TEXT).size().x) }

/// The frame's text colour at 80% (the tabs and topics that aren't chosen).
fn frame_muted() -> Color32 { look().frame_text.gamma_multiply(0.8) }

/// Hovering a button on the frame: a soft wash that springs in.
fn hover_wash(ui: &Ui, r: ERect, resp: &egui::Response, round: u8) {
    let t = theme::spring(ui.ctx(), resp.id.with("hot"), if resp.hovered() { 1.0 } else { 0.0 }, theme::SNAPPY).clamp(0.0, 1.0);
    if t > 0.0 { ui.painter().rect_filled(r, CornerRadius::same(round), white((36.0 * t) as u8)); }
}

/// A Level topic: its chip in the top notch (name, icon) and what it shows, or None and what it comes with.
const TOPICS: [(&str, &str, Option<LevelMode>, &str); 7] = [
    ("All", "grid", Some(LevelMode::Build), ""),
    ("Physics", "feel", Some(LevelMode::Physics), ""),
    ("Lighting", "bulb", Some(LevelMode::Lighting), ""),
    ("Cameras", "camera", Some(LevelMode::Cameras), ""),
    ("Water", "drop", None, "Coming soon: water comes with the Ocean"),
    ("Foliage", "leaf", None, "Coming soon: foliage comes with foliage painting"),
    ("World", "globe", None, "Coming soon: World settings come next"),
];

fn chip_w(ctx: &egui::Context, name: &str, on: bool) -> f32 { 11.0 + 14.0 + 6.0 + text_w(ctx, name, if on { semi(12.0) } else { body(12.0) }) + 11.0 }

/// The open tab's topics: (name, icon, chosen, built, when it comes if not).
fn topic_list(ed: &Editor) -> Vec<(&'static str, &'static str, bool, bool, &'static str)> {
    match ed.tab {
        audio::AUDIO => audio::TOPICS.iter().enumerate().map(|(i, (n, icon, later))| (*n, *icon, i == 0, later.is_empty(), *later)).collect(),
        _ => TOPICS.iter().map(|(n, icon, m, later)| (*n, *icon, *m == Some(ed.level_mode), m.is_some(), *later)).collect(),
    }
}

/// The top notch's width: its topic chips, and room for its shoulders.
fn topics_width(ctx: &egui::Context, ed: &Editor) -> f32 {
    topic_list(ed).iter().map(|(n, _, on, _, _)| chip_w(ctx, n, *on) + 2.0).sum::<f32>() - 2.0 + 96.0
}

/// The tab's topics as chips in the top notch: the chosen one is a pill in the frame's text colour; those not built
/// yet are faint and say when they come.
fn topic_chips(ctx: &egui::Context, ed: &mut Editor, lay: &frame::Layout) {
    let n = lay.top_notch;
    let topics = topic_list(ed);
    let w = topics.iter().map(|(name, _, on, _, _)| chip_w(ctx, name, *on) + 2.0).sum::<f32>() - 2.0;
    let row = ERect::from_min_size(pos2(n.center().x - w / 2.0, lay.view.top() + 5.0), vec2(w, 24.0));
    egui::Area::new(Id::new("topics")).order(egui::Order::Middle).fixed_pos(row.min).constrain(false).show(ctx, |ui| {
        ui.set_min_size(row.size());
        let mut x = row.left();
        for (i, &(name, icon, on, built, later)) in topics.iter().enumerate() {
            let r = ERect::from_min_size(pos2(x, row.top()), vec2(chip_w(ctx, name, on), 24.0));
            x = r.right() + 2.0;
            let resp = ui.interact(r, Id::new(("topic", i)), if built { Sense::click() } else { Sense::hover() });
            let p = ui.painter();
            let fade = if built { 1.0 } else { 0.55 };
            if on {
                let t = theme::spring_from(ctx, Id::new(("topic-on", i)), 0.0, 1.0, theme::SNAPPY).clamp(0.0, 1.0);
                p.rect_filled(r.shrink(2.0 * (1.0 - t)), CornerRadius::same(12), look().frame_text);
            } else {
                theme::spring_reset(ctx, Id::new(("topic-on", i)));
                if built { hover_wash(ui, r, &resp, 12); }
            }
            let col = if on { look().frame } else if resp.hovered() && built { look().frame_text } else { frame_muted().gamma_multiply(fade) };
            frame::paint_icon(p, icon, pos2(r.left() + 11.0 + 7.0, r.center().y), 14.0, col);
            p.text(pos2(r.left() + 11.0 + 14.0 + 6.0, r.center().y), Align2::LEFT_CENTER, name, if on { semi(12.0) } else { body(12.0) }, col);
            if !built {
                resp.tip(later);
            } else if resp.clicked() && ed.tab == audio::LEVEL {
                if let Some(m) = TOPICS[i].2 { ed.level_mode = m; }
            }
        }
    });
}

/// The top bar: the BEVERLY menu, the tabs, and Parts, Undo, Redo, Snap and the window's own buttons. Dragging the
/// bar moves the window; double-clicking it maximises.
fn top_bar(ctx: &egui::Context, ed: &mut Editor, lay: &frame::Layout, window: &mut Window, exit: &mut MessageWriter<AppExit>) {
    let bar = ERect::from_min_size(lay.window.min, vec2(lay.window.width(), frame::TOP));
    egui::Area::new(Id::new("top-bar")).order(egui::Order::Middle).fixed_pos(bar.min).constrain(false).show(ctx, |ui| {
        ui.set_min_size(bar.size());
        let move_it = ui.interact(bar, Id::new("bar-move"), Sense::CLICK | Sense::DRAG);
        if move_it.double_clicked() { window.set_maximized(!ed.maximized); ed.maximized = !ed.maximized; } else if move_it.drag_started() { window.start_drag_move(); }
        let fg = look().frame_text;
        let cy = bar.center().y;

        // the BEVERLY menu: the wordmark and its star, a chevron; it holds File, Edit, View, Play and Help
        let mark = ui.painter().layout_no_wrap("BEVERLY".into(), theme::wordmark(20.0), fg);
        let br = ERect::from_min_size(pos2(bar.left() + 10.0, cy - 16.0), vec2(10.0 + mark.size().x + 2.0 + 9.0 + 8.0 + 14.0 + 10.0, 32.0));
        let resp = ui.interact(br, Id::new("main-menu"), Sense::click()).tip("File, Edit, View, Play, Help");
        hover_wash(ui, br, &resp, 7);
        let p = ui.painter();
        let mt = pos2(br.left() + 10.0, cy - mark.size().y / 2.0);
        p.galley(mt, mark.clone(), fg);
        // the star: the wordmark's, in the highlight, at the top of its letters
        let sc = pos2(mt.x + mark.size().x + 2.0 + 4.5, mt.y + 0.22 * mark.size().y + 2.5);
        let star: Vec<_> = [(5.0, 0.0), (6.2, 3.6), (10.0, 3.6), (6.9, 5.8), (8.1, 9.5), (5.0, 7.2), (1.9, 9.5), (3.1, 5.8), (0.0, 3.6), (3.8, 3.6)].iter().map(|(x, y)| sc + vec2(x - 5.0, y - 5.0) * 0.9).collect();
        frame::paint_polygon(p, &star, look().highlight);
        frame::paint_icon(p, "down", pos2(br.right() - 10.0 - 7.0, cy), 14.0, fg);
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(200.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(egui::RichText::new(if ed.dirty() { format!("{} · not saved", ed.name) } else { ed.name.clone() }).font(semi(12.5)).color(TEXT_2));
            ui.separator();
            let menus: [(&str, &[&str]); 5] = [
                ("File", &["file.new", "file.open", "", "file.save", "file.save_as", "file.export", "file.show_folder", "file.clear_bakes", "", "file.quit"]),
                ("Edit", &["edit.undo", "edit.redo", "", "edit.duplicate", "edit.delete", "edit.make_unique", "edit.deselect", "", "edit.snap"]),
                ("View", &["view.parts", "view.frame", "view.look_through", "", "view.stats"]),
                ("Play", &["play.test", "", "file.export"]),
                ("Help", &["help.about", "help.updates", "help.features", "help.mcp"]),
            ];
            for (name, ids) in menus {
                ui.menu_button(name, |ui| {
                    ui.set_min_width(230.0);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for id in ids {
                        if id.is_empty() { ui.separator(); } else { menu_action(ui, ed, id, exit); }
                    }
                    if name == "Play" { ui.label(egui::RichText::new("` tests from where the view looks").color(MUTED).size(11.0)); }
                });
            }
        });

        // the tabs, centred: Level and Audio are built; the rest say when they come
        let tab_w = |name: &str, on: bool| 11.0 + 15.0 + 6.0 + text_w(ctx, name, if on { semi(13.0) } else { body(13.0) }) + 11.0;
        let total = theme::TABS.iter().enumerate().map(|(i, t)| tab_w(t.name, i == ed.tab) + 2.0).sum::<f32>() - 2.0;
        let mut x = bar.center().x - total / 2.0;
        for (i, t) in theme::TABS.iter().enumerate() {
            let on = i == ed.tab;
            let r = ERect::from_min_size(pos2(x, cy - 15.0), vec2(tab_w(t.name, on), 30.0));
            x = r.right() + 2.0;
            let resp = ui.interact(r, Id::new(("tab", i)), Sense::click());
            if on { ui.painter().rect_filled(r, CornerRadius::same(8), fg); } else { hover_wash(ui, r, &resp, 8); }
            let col = if on { look().frame } else if resp.hovered() { fg } else { frame_muted() };
            frame::paint_icon(ui.painter(), t.icon, pos2(r.left() + 11.0 + 7.5, cy), 15.0, col);
            ui.painter().text(pos2(r.left() + 11.0 + 15.0 + 6.0, cy), Align2::LEFT_CENTER, t.name, if on { semi(13.0) } else { body(13.0) }, col);
            if audio::built(i) {
                let id = if i == audio::AUDIO { "tab.audio" } else { "tab.level" };
                let resp = resp.tip_keys(format!("The {} tab", t.name), &["Ctrl", if i == audio::AUDIO { "5" } else { "1" }]);
                if resp.clicked() && !on {
                    if let Err(e) = actions::run(ed, None, id, exit) { ed.island.show("!", e, "", Tone::Plain); }
                }
            } else {
                let later = TAB_LATER[i];
                if resp.clicked() { ed.island.show("…", later, "Levels and sounds are what this version builds", Tone::Plain); }
                resp.tip(format!("Not built yet: {}", later.to_lowercase()));
            }
        }

        // the right: the window's buttons at the edge, then a divider, Snap, Redo, Undo and Parts
        let mut right = bar.right();
        for (k, (icon, label)) in [("x", "Close"), ("square", "Maximise"), ("minus", "Minimise")].into_iter().enumerate() {
            let r = ERect::from_min_max(pos2(right - 44.0, bar.top()), pos2(right, bar.bottom()));
            right = r.left();
            let resp = ui.interact(r, Id::new(("window", k)), Sense::click()).tip(label);
            let hot = theme::spring(ctx, resp.id.with("hot"), if resp.hovered() { 1.0 } else { 0.0 }, theme::SNAPPY).clamp(0.0, 1.0);
            let wash = if k == 0 { Color32::from_rgb(0xE8, 0x11, 0x23).gamma_multiply(hot) } else { white((36.0 * hot) as u8) };
            if hot > 0.0 { ui.painter().rect_filled(r, CornerRadius::ZERO, wash); }
            frame::paint_icon(ui.painter(), icon, r.center(), 14.0, if k == 0 { theme::lerp_color(fg, Color32::WHITE, hot) } else { fg });
            if resp.clicked() {
                match k {
                    0 => { let _ = actions::run(ed, None, "file.quit", exit); }
                    1 => { ed.maximized = !ed.maximized; window.set_maximized(ed.maximized); }
                    _ => window.set_minimized(true),
                }
            }
        }
        right -= 4.0;
        ui.painter().rect_filled(ERect::from_center_size(pos2(right - 1.0, cy), vec2(1.0, 20.0)), CornerRadius::ZERO, fg.gamma_multiply(0.2));
        right -= 2.0 + 4.0 + 4.0;
        // Snap: the grid icon and a toggle (frame colours); the steps in its tooltip
        let sr = ERect::from_min_max(pos2(right - (9.0 + 16.0 + 7.0 + 30.0 + 9.0), cy - 14.0), pos2(right, cy + 14.0));
        right = sr.left() - 4.0;
        let resp = ui.interact(sr, Id::new("snap"), Sense::click()).tip_keys(if ed.snap { "Snap: 0.25 m steps, 15° turns (hold Ctrl to drag freely)" } else { "Snap is off" }, &["S"]);
        hover_wash(ui, sr, &resp, 7);
        if resp.clicked() { ed.snap = !ed.snap; }
        frame::paint_icon(ui.painter(), "grid", pos2(sr.left() + 9.0 + 8.0, cy), 16.0, fg);
        let tr = ERect::from_min_size(pos2(sr.left() + 9.0 + 16.0 + 7.0, cy - 8.0), vec2(30.0, 16.0));
        let t = theme::spring(ctx, Id::new("snap-knob"), if ed.snap { 1.0 } else { 0.0 }, theme::SNAPPY);
        let c = t.clamp(0.0, 1.0);
        ui.painter().rect_filled(tr, CornerRadius::same(8), theme::lerp_color(theme::TOGGLE_OFF, fg, c));
        ui.painter().circle_filled(pos2(egui::lerp(tr.left() + 8.0..=tr.right() - 8.0, t), cy), 6.0, theme::lerp_color(Color32::from_rgb(0x9A, 0x9A, 0xA0), look().frame, c));
        // Redo and Undo
        for (icon, label, id, keys, can) in [("redo", "Redo", "edit.redo", ["Ctrl", "Y"], !ed.redo.is_empty()), ("undo", "Undo", "edit.undo", ["Ctrl", "Z"], !ed.undo.is_empty())] {
            let r = ERect::from_min_max(pos2(right - 31.0, cy - 14.0), pos2(right, cy + 14.0));
            right = r.left() - 4.0;
            let resp = ui.interact(r, Id::new(id), if can { Sense::click() } else { Sense::hover() }).tip_keys(if can { label.to_string() } else { format!("Nothing to {}", label.to_lowercase()) }, &keys);
            if can { hover_wash(ui, r, &resp, 7); }
            frame::paint_icon(ui.painter(), icon, r.center(), 17.0, if can { fg } else { fg.gamma_multiply(0.4) });
            if resp.clicked() { let _ = actions::run(ed, None, id, exit); }
        }
        // Parts: the bar's one main button (the Level's)
        if ed.tab != audio::LEVEL { return; }
        let kw = theme::frame_kbd_size(ctx, "Tab");
        let pw = 10.0 + 15.0 + 7.0 + text_w(ctx, "Parts", semi(13.0)) + 7.0 + kw.x + 6.0;
        let r = ERect::from_min_max(pos2(right - pw, cy - 15.0), pos2(right, cy + 15.0));
        let resp = ui.interact(r, Id::new("parts-button"), Sense::click()).tip_keys("Open the parts pop-up", &["Tab"]);
        let hot = theme::spring(ctx, resp.id.with("hot"), if resp.hovered() { 1.0 } else { 0.0 }, theme::SNAPPY).clamp(0.0, 1.0);
        let p = ui.painter();
        p.rect_filled(r, CornerRadius::same(8), theme::lerp_color(look().highlight, Color32::WHITE, hot * 0.18));
        frame::paint_icon(p, "cube", pos2(r.left() + 10.0 + 7.5, cy), 15.0, look().highlight_text);
        p.text(pos2(r.left() + 10.0 + 15.0 + 7.0, cy), Align2::LEFT_CENTER, "Parts", semi(13.0), look().highlight_text);
        theme::paint_frame_kbd(p, ERect::from_min_size(pos2(r.right() - 6.0 - kw.x, cy - kw.y / 2.0), kw), "Tab", look().highlight_text, look().highlight);
        if resp.clicked() { let _ = actions::run(ed, None, "view.parts", exit); }
    });
}

/// When each tab that isn't built yet comes.
const TAB_LATER: [&str; 8] = [
    "",
    "Characters come with character files",
    "Vehicles come after characters",
    "The HUD comes with the HUD editor",
    "",
    "Effects come with the Effects tab",
    "Screens come with screens and menus",
    "Animatics come with the timeline",
];

/// The window's edges: drag them to resize it (it has no system border).
fn window_edges(ctx: &egui::Context, lay: &frame::Layout, window: &mut Window, maximized: bool) {
    if maximized { return; }
    use bevy::math::CompassOctant as C;
    let w = lay.window;
    let e = 5.0;
    let zones = [
        (ERect::from_min_max(w.min, pos2(w.left() + e * 2.0, w.top() + e * 2.0)), C::NorthWest, egui::CursorIcon::ResizeNorthWest),
        (ERect::from_min_max(pos2(w.right() - e * 2.0, w.top()), pos2(w.right(), w.top() + e * 2.0)), C::NorthEast, egui::CursorIcon::ResizeNorthEast),
        (ERect::from_min_max(pos2(w.left(), w.bottom() - e * 2.0), pos2(w.left() + e * 2.0, w.bottom())), C::SouthWest, egui::CursorIcon::ResizeSouthWest),
        (ERect::from_min_max(w.max - vec2(e * 2.0, e * 2.0), w.max), C::SouthEast, egui::CursorIcon::ResizeSouthEast),
        (ERect::from_min_max(w.min, pos2(w.right(), w.top() + e)), C::North, egui::CursorIcon::ResizeNorth),
        (ERect::from_min_max(pos2(w.left(), w.bottom() - e), w.max), C::South, egui::CursorIcon::ResizeSouth),
        (ERect::from_min_max(w.min, pos2(w.left() + e, w.bottom())), C::West, egui::CursorIcon::ResizeWest),
        (ERect::from_min_max(pos2(w.right() - e, w.top()), w.max), C::East, egui::CursorIcon::ResizeEast),
    ];
    egui::Area::new(Id::new("window-edges")).order(egui::Order::Foreground).fixed_pos(w.min).constrain(false).show(ctx, |ui| {
        for (k, (r, dir, cursor)) in zones.into_iter().enumerate() {
            let resp = ui.interact(r, Id::new(("edge", k)), Sense::DRAG).on_hover_cursor(cursor);
            if resp.drag_started() { window.start_drag_resize(dir); }
        }
    });
}

/// What the mouse does in the view (the bottom notch's hints).
fn hints(mode: Mode, brush: bool, pen: bool, tab: usize) -> &'static [(&'static str, &'static [&'static str])] {
    if pen { return &[("Point", &["Click"]), ("Curve", &["Drag"]), ("Break handle", &["Alt"]), ("Finish", &["Enter"]), ("Done", &["Esc"])]; }
    if brush { return &[("Size", &["[", "]"]), ("Hardness", &["Shift", "[", "]"]), ("Erase", &["Alt"]), ("Done", &["Esc"])]; }
    match mode {
        Mode::Editor if tab == audio::AUDIO => &[("Orbit", &["Middle-drag"]), ("Pan", &["Space"]), ("Zoom", &["Wheel"]), ("Play", &["P"])],
        Mode::Editor => &[("Orbit", &["Middle-drag"]), ("Pan", &["Space"]), ("Zoom", &["Wheel"]), ("Fly", &["Caps Lock"])],
        Mode::Builder => &[("Fly", &["W", "A", "S", "D"]), ("Speed", &["Mouse 4", "Mouse 5"]), ("Cursor", &["Alt"]), ("Edit", &["Caps Lock"])],
    }
}

/// The bottom notch's contents, laid out from `x` along the line through `cy` (or only measured): saved or not, a
/// divider, then the hints. Returns its width.
fn notch_contents(ctx: &egui::Context, p: Option<(&egui::Painter, f32, f32)>, saved: bool, hints: &[(&str, &[&str])]) -> f32 {
    let fg = look().frame_text;
    let mut x = 0.0;
    let state = if saved { "Saved" } else { "Not saved" };
    if let Some((p, x0, cy)) = p {
        if saved { frame::paint_icon(p, "check", pos2(x0 + 7.0, cy), 14.0, fg); } else { p.circle_filled(pos2(x0 + 7.0, cy), 4.0, look().highlight); }
        p.text(pos2(x0 + 20.0, cy), Align2::LEFT_CENTER, state, semi(12.0), fg);
    }
    x += 20.0 + text_w(ctx, state, semi(12.0)) + 14.0;
    if let Some((p, x0, cy)) = p { p.rect_filled(ERect::from_center_size(pos2(x0 + x, cy), vec2(1.0, 16.0)), CornerRadius::ZERO, frame_muted().gamma_multiply(0.35)); }
    x += 1.0 + 14.0;
    for (i, (label, keys)) in hints.iter().enumerate() {
        if i > 0 { x += 16.0; }
        if let Some((p, x0, cy)) = p { p.text(pos2(x0 + x, cy), Align2::LEFT_CENTER, *label, body(12.0), frame_muted()); }
        x += text_w(ctx, label, body(12.0)) + 6.0;
        for (k, key) in keys.iter().enumerate() {
            if k > 0 { x += 4.0; }
            let s = theme::frame_kbd_size(ctx, key);
            if let Some((p, x0, cy)) = p { theme::paint_frame_kbd(p, ERect::from_min_size(pos2(x0 + x, cy - s.y / 2.0), s), key, fg, look().frame); }
            x += s.x;
        }
    }
    x
}

/// The bottom notch: Saved (click to save) and the hints.
fn bottom_notch(ctx: &egui::Context, ed: &mut Editor, view: &View, lay: &frame::Layout, exit: &mut MessageWriter<AppExit>) {
    let saved = !ed.dirty();
    let w = notch_contents(ctx, None, saved, hints(view.mode, ed.brush_on, ed.pen_on, ed.tab));
    let n = lay.bottom_notch;
    let cy = n.top() + 4.0 + 13.0;
    let x0 = n.center().x - w / 2.0;
    egui::Area::new(Id::new("bottom-notch")).order(egui::Order::Middle).fixed_pos(n.min).constrain(false).show(ctx, |ui| {
        ui.set_min_size(n.size());
        notch_contents(ctx, Some((ui.painter(), x0, cy)), saved, hints(view.mode, ed.brush_on, ed.pen_on, ed.tab));
        let sr = ERect::from_min_size(pos2(x0 - 4.0, cy - 12.0), vec2(20.0 + text_w(ctx, if saved { "Saved" } else { "Not saved" }, semi(12.0)) + 8.0, 24.0));
        let resp = ui.interact(sr, Id::new("save-state"), if saved { Sense::hover() } else { Sense::click() }).tip_keys(if saved { "Everything is saved" } else { "Save" }, &["Ctrl", "S"]);
        if resp.clicked() { let _ = actions::run(ed, None, "file.save", exit); }
    });
}

/// The notch's width for these contents (shoulders and a little air).
fn bottom_width(ctx: &egui::Context, ed: &Editor, view: &View) -> f32 {
    notch_contents(ctx, None, !ed.dirty(), hints(view.mode, ed.brush_on, ed.pen_on, ed.tab)) + 2.0 * frame::SHOULDER + 16.0
}

/// The dock's three views: (name, icon); the middle one is the tab's list.
const DOCK: [(&str, &str); 3] = [("Inspect", "sliders"), ("Scene", "layers"), ("Assets", "folder")];
const AUDIO_DOCK: [(&str, &str); 3] = [("Inspect", "sliders"), ("Sounds", "wave"), ("Assets", "folder")];
fn dock_views(ed: &Editor) -> [(&'static str, &'static str); 3] { if ed.tab == audio::AUDIO { AUDIO_DOCK } else { DOCK } }
const DOCK_W: f32 = 352.0;

/// The dock on the view's right (glass): a switch of Inspect · Scene · Assets, and the chosen one's contents; it
/// folds to a rail of icons. It moves as the Motion kit's dock does: the chosen pill slides, the contents slide in
/// from the side you went, and folding springs its width.
fn dock(ctx: &egui::Context, ed: &mut Editor, view: &mut View, lay: &frame::Layout, sounds: &crate::sound::Sounds, exit: &mut MessageWriter<AppExit>) {
    let id = Id::new("dock");
    let v = lay.view;
    let w = theme::spring(ctx, id.with("w"), if ed.dock_open { DOCK_W } else { 44.0 }, theme::SPRING);
    let open = w > 120.0;
    let rail_h = 6.0 + 4.0 * 32.0 + 3.0 * 4.0 + 6.0;
    let h = egui::lerp(rail_h..=v.height() - 24.0, ((w - 44.0) / (DOCK_W - 44.0)).clamp(0.0, 1.0));
    let r = ERect::from_min_size(pos2(v.right() - 12.0 - w, v.top() + 12.0), vec2(w, h));
    egui::Area::new(id).order(egui::Order::Middle).fixed_pos(r.min).constrain(false).show(ctx, |ui| {
        ui.set_min_size(r.size());
        ui.set_max_size(r.size());
        let p = ui.painter();
        let round = CornerRadius::same(if open { 16 } else { 14 });
        p.rect_filled(r, round, theme::PANEL);
        p.rect_stroke(r, round, Stroke::new(1.0, theme::EDGE), StrokeKind::Inside);
        // keep the pointer off the view behind it
        ui.interact(r, id.with("bg"), Sense::click());
        if !open {
            // the rail: open, then each view
            let mut y = r.top() + 6.0;
            let views = dock_views(ed);
            for (k, (icon, label)) in [("left", "Open the dock"), (views[0].1, views[0].0), (views[1].1, views[1].0), (views[2].1, views[2].0)].into_iter().enumerate() {
                let b = ERect::from_center_size(pos2(r.center().x, y + 16.0), vec2(32.0, 32.0));
                y += 36.0;
                let resp = ui.interact(b, id.with(("rail", k)), Sense::click()).tip(label);
                let hot = theme::spring(ctx, resp.id.with("hot"), if resp.hovered() { 1.0 } else { 0.0 }, theme::SNAPPY).clamp(0.0, 1.0);
                if hot > 0.0 { ui.painter().rect_filled(b, CornerRadius::same(8), theme::FIELD_HOVER.gamma_multiply(hot)); }
                frame::paint_icon(ui.painter(), icon, b.center(), 16.0, Color32::from_rgb(0xD2, 0xD2, 0xD6));
                if resp.clicked() {
                    ed.dock_open = true;
                    if k > 0 { ed.dock_view = k - 1; }
                }
            }
            return;
        }
        // the switch: a pill (the highlight) that slides to the chosen view
        let head = ERect::from_min_max(r.min + vec2(10.0, 10.0), pos2(r.right() - 10.0 - 28.0 - 6.0, r.top() + 10.0 + 34.0));
        ui.painter().rect_filled(head, CornerRadius::same(10), white(13));
        let seg = (head.width() - 6.0) / 3.0;
        let at = theme::spring(ctx, id.with("pill"), ed.dock_view as f32, theme::SNAPPY);
        let pill = ERect::from_min_size(pos2(head.left() + 3.0 + at * seg, head.top() + 3.0), vec2(seg, 28.0));
        ui.painter().rect_filled(pill, CornerRadius::same(7), look().highlight);
        let last: usize = ctx.data(|d| d.get_temp(id.with("last")).unwrap_or(ed.dock_view));
        for (k, (label, icon)) in dock_views(ed).iter().enumerate() {
            let b = ERect::from_min_size(pos2(head.left() + 3.0 + k as f32 * seg, head.top() + 3.0), vec2(seg, 28.0));
            let resp = ui.interact(b, id.with(("seg", k)), Sense::click());
            let on = (at - k as f32).abs() < 0.5;
            let col = if on { look().highlight_text } else if resp.hovered() { TEXT } else { TEXT_2 };
            let font = if on { semi(12.5) } else { body(12.5) };
            let tw = text_w(ctx, label, font.clone());
            let x = b.center().x - (13.0 + 6.0 + tw) / 2.0;
            frame::paint_icon(ui.painter(), icon, pos2(x + 6.5, b.center().y), 13.0, col);
            ui.painter().text(pos2(x + 19.0, b.center().y), Align2::LEFT_CENTER, *label, font, col);
            if resp.clicked() { ed.dock_view = k; }
        }
        let fold = ERect::from_min_size(pos2(r.right() - 10.0 - 28.0, head.center().y - 14.0), vec2(28.0, 28.0));
        let resp = ui.interact(fold, id.with("fold"), Sense::click()).tip("Fold the dock to a rail");
        let hot = theme::spring(ctx, resp.id.with("hot"), if resp.hovered() { 1.0 } else { 0.0 }, theme::SNAPPY).clamp(0.0, 1.0);
        if hot > 0.0 { ui.painter().rect_filled(fold, CornerRadius::same(7), theme::FIELD_HOVER.gamma_multiply(hot)); }
        frame::paint_icon(ui.painter(), "right", fold.center(), 16.0, Color32::from_rgb(0xD2, 0xD2, 0xD6));
        if resp.clicked() { ed.dock_open = false; }

        // the contents, sliding in from the side you went (48 px) and fading in
        if last != ed.dock_view {
            ctx.data_mut(|d| { d.insert_temp(id.with("last"), ed.dock_view); d.insert_temp(id.with("dir"), if ed.dock_view > last { 1.0f32 } else { -1.0 }); });
        }
        let dir: f32 = ctx.data(|d| d.get_temp(id.with("dir")).unwrap_or(1.0));
        let t = theme::spring_from(ctx, id.with(("in", ed.dock_view)), 0.0, 1.0, theme::SNAPPY);
        for k in 0..3 { if k != ed.dock_view { theme::spring_reset(ctx, id.with(("in", k))); } }
        let body_r = ERect::from_min_max(pos2(r.left() + 1.0, head.bottom() + 8.0), r.max - vec2(1.0, 1.0));
        let inner = body_r.shrink2(vec2(17.0, 0.0)).translate(vec2(dir * 48.0 * (1.0 - t), 0.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)), |ui| {
            ui.set_clip_rect(body_r);
            ui.multiply_opacity(t.clamp(0.0, 1.0));
            egui::ScrollArea::vertical().id_salt(("dock", ed.dock_view)).auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                match ed.dock_view {
                    0 if ed.tab == audio::AUDIO => audio_ui::inspect(ui, ed, sounds),
                    1 if ed.tab == audio::AUDIO => audio_ui::list(ui, ed),
                    0 if ed.brush_on => brush_panel(ui, ed),
                    0 if ed.pen_on => pen_panel(ui, ed),
                    0 => {
                        if ed.selected.is_some() || ed.material_open.is_some() { inspector(ui, ed, view, exit); } else { mode_panel(ui, ed); }
                    }
                    1 => scene_panel(ui, ed),
                    _ => {
                        drop_hint(ui, ed.hovering_files);
                        asset_tree(ui, ed);
                    }
                }
                ui.add_space(12.0);
            });
        });
    });
}

/// The Assets view's drop hint (the whole window takes dropped files); it lights up while files are dragged over.
fn drop_hint(ui: &mut Ui, hot: bool) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), Sense::hover());
    let t = theme::spring(ui.ctx(), Id::new("drop-hot"), if hot { 1.0 } else { 0.0 }, theme::SNAPPY).clamp(0.0, 1.0);
    let p = ui.painter();
    if t > 0.0 { p.rect_filled(r, CornerRadius::same(12), look().highlight.gamma_multiply(0.12 * t)); }
    // a dashed rounded outline
    let rr = r.shrink(0.75);
    let rad = 12.0;
    let mut pts = vec![];
    for (c, a0) in [(pos2(rr.right() - rad, rr.top() + rad), -90.0f32), (pos2(rr.right() - rad, rr.bottom() - rad), 0.0), (pos2(rr.left() + rad, rr.bottom() - rad), 90.0), (pos2(rr.left() + rad, rr.top() + rad), 180.0)] {
        for i in 0..=8 {
            let a = (a0 + 90.0 * i as f32 / 8.0).to_radians();
            pts.push(c + vec2(a.cos(), a.sin()) * rad);
        }
    }
    pts.push(pts[0]);
    p.extend(egui::Shape::dashed_line(&pts, Stroke::new(1.5, look().highlight.gamma_multiply(0.6 + 0.4 * t)), 6.0, 4.0));
    p.text(r.center(), Align2::CENTER_CENTER, if hot { "Drop to import" } else { "Drop files anywhere" }, semi(13.0), Color32::from_rgb(0xE6, 0xE6, 0xE8));
    ui.add_space(4.0);
}

/// A menu item for a named action (its name and keys from the list), greyed when it can't run; runs it when clicked.
fn menu_action(ui: &mut Ui, ed: &mut Editor, id: &str, exit: &mut MessageWriter<AppExit>) {
    let a = actions::get(id).expect("a listed action");
    let why = actions::why_not(ed, id);
    let r = ui.add_enabled(why.is_none(), egui::Button::new(a.name).shortcut_text(a.keys).frame(false));
    let r = match why { Some(w) => r.tip(w), None => r };
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

/// The project's assets by folder (a row each, with how many it holds; click to open): models to place, materials
/// to change or drag, textures to drag onto faces.
fn asset_tree(ui: &mut Ui, ed: &mut Editor) {
    let mut assets = ed.assets.clone();
    // materials as the editor holds them (new and changed ones are saved with the project)
    if let Some(m) = assets.iter_mut().find(|(f, _)| *f == "materials") {
        m.1 = ed.level.materials.keys().map(|k| k.trim_start_matches("assets/materials/").to_string()).collect();
    }
    // sounds likewise, then the recordings on disk
    if let Some(m) = assets.iter_mut().find(|(f, _)| *f == "sounds") {
        let recordings: Vec<String> = m.1.iter().filter(|f| level::is_recording(f)).cloned().collect();
        m.1 = ed.level.sounds.keys().map(|k| k.trim_start_matches("assets/sounds/").to_string()).chain(recordings).collect();
    }
    // the search: every word must be in a file's name (later: its tags and pips); folders with no match drop out and the
    // others open
    search_field(ui, &mut ed.assets_query, "Search assets");
    let words: Vec<String> = ed.assets_query.split_whitespace().map(str::to_lowercase).collect();
    let searching = !words.is_empty();
    for (_, files) in assets.iter_mut() {
        files.retain(|f| { let n = f.to_lowercase(); words.iter().all(|w| n.contains(w.as_str())) });
    }
    if searching && assets.iter().all(|(_, f)| f.is_empty()) {
        ui.add_space(14.0);
        ui.vertical_centered(|ui| ui.label(egui::RichText::new(format!("No assets match “{}”", ed.assets_query.trim())).color(MUTED)));
    }
    let row = |ui: &mut Ui, indent: f32, sense: Sense| -> (ERect, egui::Response) {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), sense);
        if resp.hovered() { ui.painter().rect_filled(r, CornerRadius::same(7), theme::FIELD); }
        (r.with_min_x(r.left() + indent), resp)
    };
    for (folder, files) in &assets {
        if searching && files.is_empty() { continue; }
        let id = Id::new(("asset-folder", *folder));
        let open: bool = searching || ui.data(|d| d.get_temp(id)).unwrap_or(*folder == "textures");
        let (r, resp) = row(ui, 10.0, Sense::click());
        frame::paint_icon(ui.painter(), if open { "down" } else { "right" }, pos2(r.left() + 5.0, r.center().y), 12.0, if open { TEXT } else { MUTED });
        ui.painter().text(pos2(r.left() + 18.0, r.center().y), Align2::LEFT_CENTER, format!("{}{}", folder[..1].to_uppercase(), &folder[1..]), body(12.5), if open { Color32::WHITE } else { Color32::from_rgb(0xDC, 0xDC, 0xE0) });
        ui.painter().text(pos2(r.right() - 10.0, r.center().y), Align2::RIGHT_CENTER, files.len().to_string(), body(12.5), MUTED);
        if resp.clicked() && !searching { ui.data_mut(|d| d.insert_temp(id, !open)); }
        if !open { continue; }
        if *folder == "materials" && !searching {
            let (r, resp) = row(ui, 30.0, Sense::click());
            frame::paint_icon(ui.painter(), "plus", pos2(r.left() + 7.0, r.center().y), 14.0, theme::accent());
            ui.painter().text(pos2(r.left() + 22.0, r.center().y), Align2::LEFT_CENTER, "New material", body(12.5), theme::accent());
            if resp.tip("A plain material: give it a texture, tint, shine or glow").clicked() {
                ed.material_open = Some(ed.level.new_material("Material", level::Material::default()));
            }
        }
        if *folder == "sounds" && !searching {
            let (r, resp) = row(ui, 30.0, Sense::click());
            frame::paint_icon(ui.painter(), "plus", pos2(r.left() + 7.0, r.center().y), 14.0, theme::accent());
            ui.painter().text(pos2(r.left() + 22.0, r.center().y), Align2::LEFT_CENTER, "New sound", body(12.5), theme::accent());
            if resp.tip("A sound file: add recordings, then set its volume, pitch and range (in the Audio tab)").clicked() {
                ed.sound_open = Some(ed.level.new_sound("Sound", level::Sound::default()));
                ed.tab = audio::AUDIO;
                ed.dock_view = 0;
            }
        }
        for f in files {
            let path = format!("assets/{folder}/{f}");
            if *folder == "sounds" {
                // sound files: click to change, drag (or Place) to put one in the level; recordings: click or drag to place
                let file = f.ends_with(".bev");
                let (r, resp) = row(ui, 30.0, Sense::click_and_drag());
                let open = ed.sound_open.as_deref() == Some(path.as_str());
                frame::paint_icon(ui.painter(), if file { "wave" } else { "play" }, pos2(r.left() + 7.0, r.center().y), 14.0, if file { theme::CAT_ACTIONS } else { MUTED });
                ui.painter().text(pos2(r.left() + 22.0, r.center().y), Align2::LEFT_CENTER, f.trim_end_matches(".bev"), body(12.5), if open { theme::CAT_ACTIONS } else { Color32::from_rgb(0xDC, 0xDC, 0xE0) });
                let place = |ed: &mut Editor, sound: String| {
                    ed.island.show("♪", format!("Placing {}", sound_name(&sound)), "Click a surface · Esc cancels", Tone::Accent);
                    ed.placing = Some(Shape::Sound);
                    ed.placing_model = Some(sound);
                };
                if ed.tab == audio::AUDIO {
                    // the Audio tab: open it (a recording gets a sound of its own)
                    let resp = resp.tip(if file { "Click to open it" } else { "A recording: click to open a sound playing it" });
                    if resp.clicked() { ed.sound_open = Some(if file { path } else { ed.level.sound_for_clip(&path) }); ed.dock_view = 0; }
                } else if file {
                    let resp = resp.on_hover_cursor(egui::CursorIcon::Grab).tip("Double-click to open it in the Audio tab · drag into the view to place a Sound part");
                    if resp.double_clicked() { ed.sound_open = Some(path); ed.tab = audio::AUDIO; ed.dock_view = 0; } else if resp.drag_started() { place(ed, path); }
                } else {
                    let resp = resp.on_hover_cursor(egui::CursorIcon::Grab).tip("A recording: click, then click a surface, to place a Sound part playing it");
                    if resp.clicked() || resp.drag_started() { let s = ed.level.sound_for_clip(&path); place(ed, s); }
                }
                continue;
            }
            let model = crate::models::is_model(f);
            let texture = super::texture::is_texture(f);
            let material = *folder == "materials";
            let sense = if model { Sense::click() } else if material || texture { Sense::click_and_drag() } else { Sense::hover() };
            let (r, resp) = row(ui, 30.0, sense);
            let shader = *folder == "shaders";
            let (icon, colour) = if model { ("cube", theme::CAT_SHAPES) } else if material { ("palette", theme::VIOLET) } else if texture { ("grid", theme::VIOLET) } else if shader { ("spark", theme::VIOLET) } else { ("folder", MUTED) };
            let open = material && ed.material_open.as_deref() == Some(path.as_str());
            // a picture on the left: a texture's own, a material's texture (else its tint as a swatch)
            let pic = if texture || model { Some(Some(path.clone())) } else if material { ed.level.materials.get(&path).map(|m| m.texture.clone()) } else { None };
            let mut text_x = 22.0;
            if let Some(tex) = pic {
                let box_ = ERect::from_min_size(pos2(r.left() + 1.0, r.center().y - 12.5), vec2(25.0, 25.0));
                match tex.and_then(|t| ed.thumbs.get(ui.ctx(), &ed.dir, &t, &ed.bakes)) {
                    Some(id) => { ui.painter().image(id, box_, ERect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE); }
                    // a model's picture is still being taken: its icon meanwhile
                    None if model => frame::paint_icon(ui.painter(), "cube", box_.center(), 14.0, theme::CAT_SHAPES),
                    None => {
                        let tint = ed.level.materials.get(&path).map_or([0.4; 3], |m| m.tint);
                        ui.painter().rect_filled(box_, CornerRadius::same(5), Color32::from_rgb((tint[0] * 255.0) as u8, (tint[1] * 255.0) as u8, (tint[2] * 255.0) as u8));
                    }
                }
                if !model { ui.painter().rect_stroke(box_, CornerRadius::same(5), Stroke::new(1.0, white(30)), StrokeKind::Inside); }
                text_x = 35.0;
            } else {
                frame::paint_icon(ui.painter(), icon, pos2(r.left() + 7.0, r.center().y), 14.0, colour);
            }
            ui.painter().text(pos2(r.left() + text_x, r.center().y), Align2::LEFT_CENTER, f.trim_end_matches(".bev"), body(12.5), if open { theme::VIOLET } else { Color32::from_rgb(0xDC, 0xDC, 0xE0) });
            if model {
                // models: click to place
                if resp.tip("Click, then click a surface to place it").clicked() {
                    ed.placing = Some(Shape::Model);
                    ed.placing_model = Some(path);
                    ed.island.show("◆", format!("Placing {f}"), "Click a surface · Shift keeps it upright · Esc cancels", Tone::Accent);
                }
            } else if material {
                // materials: click to change, drag onto a face
                let resp = resp.on_hover_cursor(egui::CursorIcon::Grab).tip(if ed.brush_on { "Click to paint with it" } else { "Click to change it (every surface using it changes) · drag onto a face" });
                if resp.clicked() && ed.brush_on { ed.brush.material = path; ed.dock_view = 0; }
                else if resp.clicked() { ed.material_open = Some(path); } else if resp.drag_started() { ed.texturing = Some(path); }
            } else if shader {
                resp.tip("A shader: choose it on a material");
            } else if texture {
                // textures: drag onto a face (or click, then click a face)
                let resp = resp.on_hover_cursor(egui::CursorIcon::Grab).tip("Drag onto a face of a part (Shift: the whole part)");
                if resp.drag_started() || resp.clicked() { ed.texturing = Some(path); }
            }
        }
    }
}

fn body(size: f32) -> egui::FontId { theme::body(size) }

/// A search field (the dock's): a drawn magnifier and a line of text.
fn search_field(ui: &mut Ui, text: &mut String, hint: &str) {
    let focused = ui.memory(|m| m.focused()).is_some_and(|f| f == ui.next_auto_id());
    egui::Frame::new()
        .fill(white(10))
        .stroke(Stroke::new(1.0, if focused { Color32::from_rgba_unmultiplied(0xf5, 0xdc, 0x4a, 150) } else { LINE }))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (ir, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                let c = ir.center() - vec2(1.0, 1.0);
                ui.painter().circle_stroke(c, 4.5, Stroke::new(1.4, MUTED));
                ui.painter().line_segment([c + vec2(3.3, 3.3), c + vec2(6.5, 6.5)], Stroke::new(1.6, MUTED));
                ui.add(egui::TextEdit::singleline(text).hint_text(hint).frame(egui::Frame::NONE).font(body(13.0)).desired_width(ui.available_width()));
            });
        });
    ui.add_space(4.0);
}

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
        let (_, icon, colour) = kind(p.shape);
        let file = p.model.as_deref().and_then(|m| m.rsplit('/').next()).map(|f| format!(" · {f}")).unwrap_or_default();
        // instances: the parts with the same contents (shape, colour, textures, Visible, Collision)
        let n = ed.level.instances(id).len();
        let inst = if p.unique { " · Unique".to_string() } else if ed.level.instanced(&p) && n > 0 { format!(" · {} alike", n + 1) } else { String::new() };
        let sub = format!("{} · {}{file}{inst}", kind(p.shape).0.trim_end_matches('s'), p.shape.label());
        let (mut dup, mut del) = (false, false);
        identity(ui, colour, icon, Ok(&mut p.name), &sub, &mut [("copy", "Duplicate", &["Ctrl", "D"], &mut dup), ("trash", "Delete", &["Del"], &mut del)]);
        if dup { ed.duplicate_selected(); }
        if del { delete = true; }
        // the rest is drawn from the part's setting descriptions (level.rs, settings.rs)
        let model = p.shape == Shape::Model;
        let scale = [("size", Setting::number("Scale", "×", 0.01, 0.001, 100000.0).section(level::LOOK).bare())];
        let mut ctx = settings::Ctx { replace: if model { &scale } else { &[] }, ranges: &mut ranges, ranges_changed: false, palette: project_palette(&ed.level) };
        for sec in settings::sections(&p) {
            if sec == level::PART || (sec == level::MOVES && p.shape == Shape::PlayerStart) { continue; }
            // the player start, lights and cameras have no surface to tag
            if sec == level::TAGS { continue; }
            // a light or a camera: its settings in place of size and look (the cone for spots only)
            if sec == level::LOOK && p.shape.is_marker() { continue; }
            if sec == level::CAMERA {
                let orbit = p.shape == Shape::OrbitCamera;
                if let Some(c) = p.camera.as_mut() {
                    column(ui, 250.0, "CAMERA", |ui| {
                        settings::section(ui, c, level::CAMERA, &mut ctx);
                        if ui.button("Look through").tip_keys("Fly it to aim it", &["L"]).clicked() { look = true; }
                    });
                    if orbit { column(ui, 260.0, "ORBIT", |ui| { settings::section(ui, c, level::ORBIT, &mut ctx); }); }
                }
                continue;
            }
            if sec == level::SPEAKER {
                let files: Vec<String> = ed.level.sounds.keys().cloned().collect();
                if let Some(s) = p.sound.as_mut() {
                    column(ui, 260.0, "SOUND", |ui| {
                        let shown = if s.sound.is_empty() { "None yet".to_string() } else { sound_name(&s.sound) };
                        egui::ComboBox::from_id_salt("part sound").selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
                            for f in &files { ui.selectable_value(&mut s.sound, f.clone(), sound_name(f)); }
                        }).response.tip("The sound file it plays (Assets > Sounds)");
                        settings::section(ui, s, level::SPEAKER, &mut ctx);
                        if !s.sound.is_empty() && ui.button("Edit in the Audio tab").tip("Its recordings, volume, pitch and range (every part playing it changes)").clicked() {
                            ed.sound_open = Some(s.sound.clone());
                            ed.tab = audio::AUDIO;
                            ed.dock_view = 0;
                        }
                    });
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
        if !p.surfaces.is_empty() {
            column(ui, 250.0, "TEXTURES", |ui| {
                let names = ed.geo.shapes[&p.shape].face_names();
                let mut remove = None;
                for (i, s) in p.surfaces.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        let face = s.face.map_or("All over".to_string(), |f| names.get(f as usize).cloned().unwrap_or_default());
                        let mat = std::path::Path::new(&s.material).file_stem().map(|m| m.to_string_lossy().into_owned()).unwrap_or_default();
                        ui.label(egui::RichText::new(format!("{face} · {mat}")).color(TEXT));
                        if ui.small_button("✕").tip("Take this texture off").clicked() { remove = Some(i); }
                    });
                    for (label, v, help) in [("Size", &mut s.tile, "How big one copy of the texture is (across, down)"), ("Slide", &mut s.offset, "How far the texture is slid along the surface")] {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(label).color(MUTED)).tip(help);
                            let lo = if label == "Size" { 0.05 } else { -1000.0 };
                            for x in v.iter_mut() { ui.add(egui::DragValue::new(x).speed(0.01).range(lo..=1000.0).max_decimals(2).suffix(" m")); }
                        });
                    }
                }
                if let Some(i) = remove { p.surfaces.remove(i); }
            });
        }
        // what it plays when things happen to it
        let events = p.events();
        if !events.is_empty() {
            let files: Vec<String> = ed.level.sounds.keys().cloned().collect();
            column(ui, 0.0, "SOUNDS", |ui| {
                for e in events {
                    let mut chosen = p.sound_on(e).unwrap_or_default().to_string();
                    if sound_row(ui, e, &mut chosen, &files, &format!("What it plays when it's {}", event_words(e))) {
                        p.sounds.retain(|h| h.on != e);
                        if !chosen.is_empty() { p.sounds.push(level::Hook { on: e.into(), sound: chosen }); }
                    }
                }
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
                        if ui.small_button(if wiring { "click a part…" } else { "+ wire" }).tip("Then click the part it goes to").clicked() {
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
                        if ui.small_button("✕").tip("Remove this wire").clicked() { unwire = Some(i); }
                    });
                }
                if !p.inputs().is_empty() && !ed.level.wires.iter().any(|w| w.to == id) {
                    ui.horizontal(|ui| {
                        for i in p.inputs() { port(ui, &format!("▸ {i}"), theme::CAT_ACTIONS); }
                    });
                }
            });
        }
        // rarely used: Unique, and its surface's tags (the player start, lights and cameras have no surface to tag)
        let tags = !(p.shape == Shape::PlayerStart || p.shape.is_marker());
        group(ui, "ADVANCED", Some(1 + 2 * tags as usize), |ui| {
            ui.horizontal(|ui| {
                let help = if p.unique {
                    "Its contents are its own"
                } else if !ed.level.instanced(&p) {
                    "Lights, cameras, moving and wired parts are always individual"
                } else {
                    "Parts with the same shape, colour and textures are instances; changing one asks first"
                };
                ui.label(egui::RichText::new("Unique").color(TEXT_2)).tip(help);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    theme::toggle(ui, &mut p.unique).tip("On: changing its look never changes the parts just like it");
                });
            });
            if tags {
                settings::section(ui, &mut p, level::TAGS, &mut ctx);
                ground_row(ui, "part sounds like", &mut p.sounds_like, "What its surface sounds like underfoot: steps play their recordings for it");
            }
        });
        ranges_changed = ctx.ranges_changed;
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
    let mut ctx = settings::Ctx { replace: &[], ranges: &mut ranges, ranges_changed: false, palette: project_palette(&ed.level) };
    // the level (or its topic) at the top, as a part is when one is selected
    let parts = ed.level.parts.len();
    let (name, icon, colour) = match ed.level_mode {
        LevelMode::Build => (ed.level.name.clone(), "mountain", highlight()),
        LevelMode::Lighting => ("Lighting".into(), "bulb", theme::CAT_LIGHTS),
        LevelMode::Physics => ("Physics".into(), "feel", theme::CAT_FLOW),
        LevelMode::Cameras => ("Cameras".into(), "camera", theme::CAT_CAMERAS),
    };
    let sub = if ed.level_mode == LevelMode::Build { format!("Level · {parts} part{}", if parts == 1 { "" } else { "s" }) } else { format!("Level · {}", ed.level.name) };
    identity(ui, colour, icon, Err(&name), &sub, &mut []);
    match ed.level_mode {
        LevelMode::Build => {
            ui.label(egui::RichText::new("Click a part in the view to inspect it, or press Tab to place one.").color(TEXT_2));
            ui.add_space(8.0);
            column(ui, 0.0, "SHORTCUTS", |ui| {
                theme::hint(ui, "Parts", &["Tab"]);
                theme::hint(ui, "Fly / edit", &["Caps Lock"]);
                theme::hint(ui, "Test", &["`"]);
                theme::hint(ui, "Save", &["Ctrl", "S"]);
            });
            // the walker's sounds (until characters have their own)
            let files: Vec<String> = ed.level.sounds.keys().cloned().collect();
            let mut w = ed.level.walker.clone();
            column(ui, 0.0, "WALKER SOUNDS", |ui| {
                for e in level::WalkerSounds::EVENTS {
                    let help = match e { "Step" => "Each step (its recordings for the surface underfoot)", "Jump" => "Leaving the ground", _ => "Coming down from a jump or a fall" };
                    sound_row(ui, e, w.get_mut(e).expect("one of its events"), &files, help);
                }
            });
            if w != ed.level.walker { ed.level.walker = w; }
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
                    if theme::main_button(ui, if ed.nav.is_some() { "Build again" } else { "Build navigation" }).tip("Work out where characters of this size can walk, across floors, stairs and ramps").clicked() { build = true; }
                    if ed.nav.is_some() {
                        theme::toggle(ui, &mut ed.nav_shown).tip("Draw the walkable area over the level");
                        ui.label(egui::RichText::new("Show").color(MUTED));
                    }
                });
                if ed.finder.is_some() {
                    let on = ed.path_try.is_some();
                    if ui.add(egui::Button::selectable(on, "Try a path")).tip("Click a start, then an end, in the view: the way a character would walk").clicked() {
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
                    if ui.add(egui::Button::selectable(on, egui::RichText::new(name).color(if on { highlight() } else { TEXT })).frame(false)).clicked() { ed.selected = Some(id); ed.material_open = None; ed.sound_open = None; }
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

/// The pen's panel (in Inspect while the pen is out): the curve being edited, and what it paints along itself.
fn pen_panel(ui: &mut Ui, ed: &mut Editor) {
    let cur = ed.pen.curve.and_then(|id| ed.level.curve(id)).cloned();
    let sub = match &cur {
        Some(c) => format!("Pen · {} point{}{}", c.points.len(), if c.points.len() == 1 { "" } else { "s" }, if c.closed { " · closed" } else { "" }),
        None => "Click a surface to start a curve".to_string(),
    };
    let mut name = cur.as_ref().map_or_else(|| "Pen".to_string(), |c| c.name.clone());
    let named = cur.is_some();
    identity(ui, theme::VIOLET, "route", if named { Ok(&mut name) } else { Err("Pen") }, &sub, &mut []);
    if let Some(c) = &cur {
        let mut strip = c.strip.clone();
        let mut ranges = ed.level.ranges.clone();
        let mut ctx = settings::Ctx { replace: &[], ranges: &mut ranges, ranges_changed: false, palette: vec![] };
        let file = |p: &str| std::path::Path::new(p).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        column(ui, 0.0, crate::curve::STRIP, |ui| {
            let mut on = strip.is_some();
            let mut none = false;
            if settings::toggle_row(ui, "", "Paint a strip", &mut on, "Paints a material along the line as a brush would, kept in step with the curve", false, &mut none) {
                strip = on.then(|| crate::curve::Strip { material: if ed.brush.material.is_empty() { ed.level.materials.keys().next().cloned().unwrap_or_default() } else { ed.brush.material.clone() }, ..Default::default() });
            }
            if let Some(st) = strip.as_mut() {
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(vec2(76.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_min_width(76.0); ui.label(egui::RichText::new("Paints").color(TEXT_2)); });
                    let shown = if st.material.is_empty() { "No material".to_string() } else { file(&st.material) };
                    egui::ComboBox::from_id_salt("strip material").selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
                        for m in ed.level.materials.keys() { ui.selectable_value(&mut st.material, m.clone(), file(m)); }
                        if ed.level.materials.is_empty() { ui.label(egui::RichText::new("No materials yet: + New material in Assets").color(MUTED)); }
                    });
                });
                ui.add_space(8.0);
                settings::section(ui, st, crate::curve::STRIP, &mut ctx);
            }
        });
        if ctx.ranges_changed { ed.level.ranges = ranges; }
        if let Some(cm) = ed.level.curve_mut(c.id) {
            if strip != c.strip { cm.strip = strip; }
            if name != c.name && !name.is_empty() { cm.name = name; }
        }
    }
    column(ui, 0.0, "SHORTCUTS", |ui| {
        theme::hint(ui, "Corner point", &["Click"]);
        theme::hint(ui, "Smooth point", &["Drag"]);
        theme::hint(ui, "Break a handle", &["Alt", "Drag"]);
        theme::hint(ui, "Close the curve", &["Click", "First point"]);
        theme::hint(ui, "Finish", &["Enter"]);
        theme::hint(ui, "Delete a point or the curve", &["Del"]);
        theme::hint(ui, "Put it away", &["Esc"]);
    });
    if theme::main_button(ui, "Done").clicked() { ed.pen_on = false; ed.pen.reset(); }
}

/// The brush's panel (in Inspect while the brush is out): what it paints and how.
fn brush_panel(ui: &mut Ui, ed: &mut Editor) {
    let name = |p: &str| std::path::Path::new(p).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let sub = if ed.brush.material.is_empty() { "Pick a material to paint with".to_string() } else { format!("Brush · painting {}", name(&ed.brush.material)) };
    identity(ui, theme::VIOLET, "palette", Err("Brush"), &sub, &mut []);
    let mut b = ed.brush.clone();
    let mut ranges = ed.level.ranges.clone();
    let mut ctx = settings::Ctx { replace: &[], ranges: &mut ranges, ranges_changed: false, palette: vec![] };
    column(ui, 0.0, "BRUSH", |ui| {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(vec2(76.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_min_width(76.0); ui.label(egui::RichText::new("Paints").color(TEXT_2)); });
            let shown = if b.material.is_empty() { "No material".to_string() } else { name(&b.material) };
            egui::ComboBox::from_id_salt("brush material").selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
                for m in ed.level.materials.keys() { ui.selectable_value(&mut b.material, m.clone(), name(m)); }
                if ed.level.materials.is_empty() { ui.label(egui::RichText::new("No materials yet: + New material in Assets").color(MUTED)); }
            });
        });
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(vec2(76.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_min_width(76.0); ui.label(egui::RichText::new("Preset").color(TEXT_2)).tip("Four brushes to switch between (1 to 4); each remembers how you left it"); });
            let mut k = b.preset;
            if theme::choices(ui, "brush-preset", &super::brush::PRESETS, &mut k) { b.pick(k); }
        });
        ui.add_space(8.0);
        settings::section(ui, &mut b, super::brush::BRUSH, &mut ctx);
    });
    column(ui, 0.0, level::RULES, |ui| {
        settings::section(ui, &mut b.rules, level::RULES, &mut ctx);
        ui.add_space(8.0);
        // only between two heights in the world
        let mut band = b.rules.band.is_some();
        let mut none = false;
        if settings::toggle_row(ui, "", "Only between heights", &mut band, "Paints only between two heights in the world, wherever the part goes", false, &mut none) {
            b.rules.band = band.then(|| { let y = ed.painting.over.map_or(0.0, |o| o.1.y); [y - 1.0, y + 1.0] });
        }
        if let Some([lo, hi]) = b.rules.band.as_mut() {
            settings::row(ui, "", "From", "The lowest it paints (metres)", false, &mut none, |ui| settings::number(ui, lo, 0.05, -1e6, 1e6, " m", None, ui.available_width()));
            settings::row(ui, "", "To", "The highest it paints (metres)", false, &mut none, |ui| settings::number(ui, hi, 0.05, -1e6, 1e6, " m", None, ui.available_width()));
        }
        // only on parts with a surface tag
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(vec2(76.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_min_width(76.0); ui.label(egui::RichText::new("Only on").color(TEXT_2)).tip("Paints only parts with this surface tag (their Tags, in Advanced)"); });
            let label = |t: Option<level::Tag>| t.map_or("Any surface".to_string(), |t| format!("{t:?}").chars().enumerate().flat_map(|(i, c)| if i > 0 && c.is_uppercase() { vec![' ', c.to_ascii_lowercase()] } else { vec![c] }).collect());
            egui::ComboBox::from_id_salt("brush only on").selected_text(label(b.rules.on)).width(ui.available_width()).show_ui(ui, |ui| {
                ui.selectable_value(&mut b.rules.on, None, label(None));
                for t in level::Tag::ALL { ui.selectable_value(&mut b.rules.on, Some(t), label(Some(t))); }
            });
        });
    });
    column(ui, 0.0, "SHORTCUTS", |ui| {
        theme::hint(ui, "Presets S to XL", &["1", "2", "3", "4"]);
        theme::hint(ui, "Smaller / larger", &["[", "]"]);
        theme::hint(ui, "Softer / harder", &["Shift", "[", "]"]);
        theme::hint(ui, "Size it in the view", &["Alt", "Right-drag"]);
        theme::hint(ui, "Erase", &["Alt"]);
        theme::hint(ui, "Put it away", &["Esc"]);
    });
    if theme::main_button(ui, "Done").clicked() { ed.brush_on = false; }
    if ctx.ranges_changed { ed.level.ranges = ranges; }
    if b != ed.brush { b.keep(); ed.brush = b; }
}

/// A row choosing a sound file (or none) for `label`; true when it changed.
fn sound_row(ui: &mut Ui, label: &str, chosen: &mut String, files: &[String], help: &str) -> bool {
    let before = chosen.clone();
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(vec2(76.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_min_width(76.0); ui.label(egui::RichText::new(label).color(TEXT_2)).tip(help); });
        let shown = if chosen.is_empty() { "None".to_string() } else { sound_name(chosen) };
        egui::ComboBox::from_id_salt(("sound row", label, ui.id())).selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
            ui.selectable_value(chosen, String::new(), "None");
            for f in files { ui.selectable_value(chosen, f.clone(), sound_name(f)); }
            if files.is_empty() { ui.label(egui::RichText::new("No sounds yet (Audio tab)").color(MUTED)); }
        });
    });
    *chosen != before
}

/// A row choosing what a surface sounds like underfoot.
fn ground_row(ui: &mut Ui, id: &str, g: &mut Option<level::Ground>, help: &str) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(vec2(76.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| { ui.set_min_width(76.0); ui.label(egui::RichText::new("Sounds like").color(TEXT_2)).tip(help); });
        egui::ComboBox::from_id_salt(id).selected_text(g.map_or("Nothing special", |g| g.label())).width(ui.available_width()).show_ui(ui, |ui| {
            ui.selectable_value(g, None, "Nothing special");
            for x in level::Ground::ALL { ui.selectable_value(g, Some(x), x.label()); }
        });
    });
}

/// An event in words: "pressed", "hit", "starts moving".
fn event_words(e: &str) -> String {
    match e { "Starts" => "starting to move".into(), "Hit" => "knocked into something".into(), _ => e.to_lowercase() }
}

/// The dock's list: the level's parts (those of the Level topic), found by name, in sections by kind, each with its
/// icon in its category colour; hidden parts and too many lights say so.
fn scene_panel(ui: &mut Ui, ed: &mut Editor) {
    let shown: fn(&level::Part) -> bool = match ed.level_mode {
        LevelMode::Build => |_| true,
        LevelMode::Lighting => |p| p.shape.is_light(),
        LevelMode::Cameras => |p| p.shape.is_camera(),
        LevelMode::Physics => |p| crate::physics::kind(p).is_some(),
    };
    // the search field
    let qid = Id::new("scene-find");
    let mut q: String = ui.data(|d| d.get_temp(qid)).unwrap_or_default();
    theme::text_input(ui, &mut q, "Find in scene", Some("search"), ui.available_width());
    ui.data_mut(|d| d.insert_temp(qid, q.clone()));
    let q = q.to_lowercase();
    if ed.level.parts.is_empty() { ui.label(egui::RichText::new("Nothing placed yet. Press Tab.").color(MUTED).size(12.5)); }
    let over: Vec<u64> = { let w = super::light::budget(&ed.level); if w.len() > super::light::BUDGET { w } else { vec![] } };
    let mut pick = None;
    let mut flip = None;
    for sec in ["Shapes", "Models", "Triggers", "Game", "Lights", "Cameras"] {
        let rows: Vec<&level::Part> = ed.level.parts.iter().filter(|p| shown(p) && kind(p.shape).0 == sec && (q.is_empty() || p.name.to_lowercase().contains(&q))).collect();
        if rows.is_empty() { continue; }
        ui.add_space(4.0);
        ui.label(egui::RichText::new(sec).size(11.5).color(MUTED));
        for p in rows {
            let (_, icon, colour) = kind(p.shape);
            let on = ed.selected == Some(p.id);
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
            let painter = ui.painter();
            if on { painter.rect_filled(r, CornerRadius::same(7), theme::SELECTED_ROW); } else if resp.hovered() { painter.rect_filled(r, CornerRadius::same(7), theme::FIELD); }
            frame::paint_icon(painter, icon, pos2(r.left() + 10.0 + 7.0, r.center().y), 14.0, if p.visible { colour } else { colour.gamma_multiply(0.45) });
            painter.text(pos2(r.left() + 34.0, r.center().y), Align2::LEFT_CENTER, &p.name, body(12.5), if on { Color32::WHITE } else if p.visible { Color32::from_rgb(0xDC, 0xDC, 0xE0) } else { MUTED });
            // on the right: a warning, or Hidden; the eye shows or hides it (hidden: a collider only)
            let eye = ERect::from_center_size(pos2(r.right() - 16.0, r.center().y), vec2(24.0, 24.0));
            let er = ui.interact(eye, Id::new(("eye", p.id)), Sense::click()).tip(if p.visible { "Hide it (it stays a collider)" } else { "Show it" });
            if resp.hovered() || er.hovered() || !p.visible {
                frame::paint_icon(ui.painter(), "eye", eye.center(), 14.0, if er.hovered() { TEXT } else { MUTED });
            }
            let note = if over.contains(&p.id) { Some(("Too many lights", theme::DANGER_TEXT)) } else if !p.visible { Some(("Hidden", MUTED)) } else { None };
            if let Some((n, c)) = note { ui.painter().text(pos2(eye.left() - 6.0, r.center().y), Align2::RIGHT_CENTER, n, body(11.5), c); }
            if er.clicked() { flip = Some(p.id); } else if resp.clicked() { pick = Some(p.id); }
        }
    }
    if let Some(id) = pick { ed.selected = Some(id); ed.material_open = None; ed.sound_open = None; }
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
    let ranges_changed;
    let (surfaces, parts) = ed.level.material_users(path);
    let name = path.trim_start_matches("assets/materials/").trim_end_matches(".bev").to_string();
    let textures: Vec<String> = ed.assets.iter().find(|(f, _)| *f == "textures").map(|(_, t)| t.iter().filter(|t| super::texture::is_texture(t)).cloned().collect()).unwrap_or_default();
    let (mut copy, mut close) = (false, false);
    let n = |n: usize, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
    identity(ui, theme::VIOLET, "palette", Err(&name), &format!("Material · on {} of {}", n(surfaces, "surface"), n(parts, "part")), &mut []);
    ui.horizontal(|ui| {
        if ui.button("Copy").tip("A new material like this one, to change on its own").clicked() { copy = true; }
        if ui.button("Close").tip_keys("Back to the selected part", &["Esc"]).clicked() { close = true; }
    });
    ui.add_space(8.0);
    column(ui, 0.0, "TEXTURE", |ui| {
        let shown = m.texture.as_deref().and_then(|t| t.rsplit('/').next()).unwrap_or("None (plain)").to_string();
        egui::ComboBox::from_id_salt("material texture").selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
            ui.selectable_value(&mut m.texture, None, "None (plain)");
            for t in &textures { ui.selectable_value(&mut m.texture, Some(format!("assets/textures/{t}")), t); }
        });
        ui.label(egui::RichText::new("Drag the material from Assets onto faces").color(MUTED).size(11.0));
    });
    let mut ctx = settings::Ctx { replace: &[], ranges: &mut ranges, ranges_changed: false, palette: project_palette(&ed.level) };
    column(ui, 0.0, "SHADER", |ui| {
        let files = crate::shader::list(&ed.dir);
        let shown = m.shader.as_deref().and_then(|s| s.rsplit('/').next()).unwrap_or("None (standard)").to_string();
        egui::ComboBox::from_id_salt("material shader").selected_text(shown).width(ui.available_width()).show_ui(ui, |ui| {
            if ui.selectable_value(&mut m.shader, None, "None (standard)").clicked() { m.shader_values.clear(); }
            for f in &files { if ui.selectable_value(&mut m.shader, Some(f.clone()), f.trim_start_matches("assets/shaders/")).clicked() { m.shader_values.clear(); } }
        });
        if let Some(path) = m.shader.clone() {
            match ed.shader_info.get(&path) {
                Some((Some(h), _)) => { settings::shader_settings(ui, h, &mut m.shader_values, &ctx.palette); }
                Some((None, None)) | None => { ui.label(egui::RichText::new("Reading the shader…").color(MUTED).size(11.0)); }
                Some((None, Some(_))) => {}
            }
            if let Some((_, Some(e))) = ed.shader_info.get(&path) { ui.label(egui::RichText::new(e).color(theme::DANGER_TEXT).size(11.0)); }
        } else {
            ui.label(egui::RichText::new("A shader draws the material its own way: write one in assets/shaders").color(MUTED).size(11.0));
        }
    });
    column(ui, 0.0, "LOOK", |ui| {
        settings::section(ui, &mut m, level::MATERIAL, &mut ctx);
        ground_row(ui, "material sounds like", &mut m.sounds_like, "What it sounds like underfoot, wherever it's laid (else what the part says)");
    });
    ranges_changed = ctx.ranges_changed;
    if m != before { ed.level.materials.insert(path.to_string(), m.clone()); }
    if ranges_changed { ed.level.ranges = ranges; }
    if copy {
        let new = ed.level.new_material(&format!("{name} copy"), m);
        ed.island.show("◈", "Copied the material", new.trim_start_matches("assets/materials/").to_string(), theme::Tone::Accent);
        ed.material_open = Some(new);
    }
    if close || ui.input(|i| i.key_pressed(egui::Key::Escape)) { ed.material_open = None; }
}

/// A sound file's name: "assets/sounds/birds.bev" is "birds".
fn sound_name(path: &str) -> String { std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default() }

fn port(ui: &mut Ui, text: &str, color: Color32) {
    egui::Frame::new().fill(color.gamma_multiply(0.22)).stroke(Stroke::new(1.0, color.gamma_multiply(0.7))).corner_radius(CornerRadius { nw: 8, ne: 8, se: 8, sw: 3 }).inner_margin(egui::Margin::symmetric(7, 2)).show(ui, |ui| {
        ui.label(egui::RichText::new(text).font(semi(12.0)).color(TEXT));
    });
}

/// A group of settings in the dock: a line across, its heading (an icon in its colour, its name, a chevron to
/// fold it) and its contents. `title` is a settings section (or one of the panels' own); its look comes from
/// [`group_look`].
fn column(ui: &mut Ui, _w: f32, title: &str, add: impl FnOnce(&mut Ui)) { group(ui, title, None, add); }

/// How a group looks: its name, icon and colour.
fn group_look(title: &str) -> (String, &'static str, Color32) {
    let (name, icon, colour) = match title {
        level::PLACE => ("Placement", "move", theme::CAT_SHAPES),
        level::LOOK | "LOOK" => ("Look", "drop", theme::CAT_LIGHTS),
        "SCALE  ·  LOOK" => ("Scale and look", "drop", theme::CAT_LIGHTS),
        level::MOVES => ("Moves", "route", theme::CAT_FLOW),
        level::LIGHT => ("Light", "bulb", theme::CAT_LIGHTS),
        level::CONE => ("Cone", "cone", theme::CAT_LIGHTS),
        level::CAMERA => ("Camera", "camera", theme::CAT_CAMERAS),
        level::ORBIT => ("Orbit", "loop", theme::CAT_CAMERAS),
        level::TAGS => ("Tags", "pin", TEXT_2),
        "TEXTURES" => ("Textures", "palette", theme::VIOLET),
        "TEXTURE" => ("Texture", "palette", theme::VIOLET),
        "WIRES" => ("Wires", "wire", theme::CAT_ACTIONS),
        "LEVEL · LIGHTING" => ("Lighting", "bulb", theme::CAT_LIGHTS),
        level::SUN => ("Sun", "spark", theme::CAT_LIGHTS),
        "LEVEL · PHYSICS · NAVIGATION" => ("Navigation", "route", theme::CAT_FLOW),
        "LEVEL · CAMERAS" => ("Cameras", "camera", theme::CAT_CAMERAS),
        "ADVANCED" => ("Advanced", "sliders", MUTED),
        "SOUNDS" => ("Sounds", "wave", theme::CAT_ACTIONS),
        "WALKER SOUNDS" => ("Walker sounds", "person", theme::CAT_GAME),
        "BRUSH" => ("Brush", "palette", theme::VIOLET),
        crate::curve::STRIP => ("Strip", "route", theme::VIOLET),
        level::RULES => ("Where it lands", "pin", theme::VIOLET),
        "RECORDINGS" => ("Recordings", "wave", theme::CAT_ACTIONS),
        "ON SURFACES" => ("On surfaces", "layers", theme::CAT_ACTIONS),
        level::SOUND => ("Sound", "sliders", theme::CAT_ACTIONS),
        level::REACH => ("Reach", "target", theme::accent()),
        "SHORTCUTS" => ("Getting about", "keyboard", TEXT_2),
        other => return (other[..1].to_string() + &other[1..].to_lowercase(), "sliders", TEXT_2),
    };
    (name.to_string(), icon, colour)
}

/// A group (see [`column`]); `count` makes it a folded-away group showing how many settings it holds (Advanced).
fn group(ui: &mut Ui, title: &str, count: Option<usize>, add: impl FnOnce(&mut Ui)) {
    let (name, icon, colour) = group_look(title);
    let id = Id::new(("group", title));
    let shut: bool = ui.data(|d| d.get_temp(id)).unwrap_or(count.is_some());
    let full = ui.max_rect();
    let y = ui.cursor().top();
    ui.painter().hline(full.left() - 17.0..=full.right() + 17.0, y, Stroke::new(1.0, white(18)));
    ui.add_space(16.0);
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 16.0), Sense::click());
    let p = ui.painter();
    frame::paint_icon(p, icon, pos2(r.left() + 8.0, r.center().y), 16.0, colour);
    let g = p.layout_no_wrap(name.clone(), semi(12.5), Color32::from_rgb(0xD2, 0xD2, 0xD6));
    let tx = r.left() + 24.0;
    p.galley(pos2(tx, r.center().y - g.size().y / 2.0), g.clone(), TEXT);
    if let Some(n) = count {
        p.text(pos2(tx + g.size().x + 6.0, r.center().y), Align2::LEFT_CENTER, format!("{n} setting{}", if n == 1 { "" } else { "s" }), body(12.0), MUTED);
    }
    let hot = resp.hovered();
    frame::paint_icon(p, if shut { "right" } else { "down" }, pos2(r.right() - 7.0, r.center().y), 14.0, if hot { TEXT_2 } else { Color32::from_rgb(0x6E, 0x6E, 0x75) });
    if resp.tip(if shut { "Show" } else { "Fold away" }).clicked() { ui.data_mut(|d| d.insert_temp(id, !shut)); }
    if shut {
        ui.add_space(16.0);
        return;
    }
    ui.add_space(14.0);
    add(ui);
    ui.add_space(18.0);
}

/// The identity at the top of the inspector: a tile with its icon in its colour, its name (`name` makes it
/// editable), what it is, and its icon buttons (icon, label, keys, clicked), e.g. Duplicate / Delete.
fn identity(ui: &mut Ui, colour: Color32, icon: &str, name: Result<&mut String, &str>, sub: &str, actions: &mut [(&str, &str, &[&str], &mut bool)]) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        let (tile, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(tile, CornerRadius::same(11), colour.gamma_multiply(0.16));
        p.rect_stroke(tile, CornerRadius::same(11), Stroke::new(1.0, colour.gamma_multiply(0.35)), StrokeKind::Inside);
        frame::paint_icon(p, icon, tile.center(), 20.0, colour);
        let room = ui.available_width() - if actions.is_empty() { 0.0 } else { actions.len() as f32 * 28.0 + 8.0 };
        ui.allocate_ui_with_layout(vec2(room, 40.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            match name {
                Ok(n) => { ui.add(egui::TextEdit::singleline(n).frame(egui::Frame::NONE).margin(vec2(0.0, 0.0)).desired_width(room).font(theme::bold(16.0))).tip("Click to rename"); }
                Err(n) => { ui.label(egui::RichText::new(n).font(theme::bold(16.0))); }
            }
            ui.label(egui::RichText::new(sub).size(12.0).color(Color32::from_rgb(0xA0, 0xA0, 0xA7)));
        });
        if !actions.is_empty() {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (icon, label, keys, flag) in actions.iter_mut() {
                let (b, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
                let hot = theme::spring(ui.ctx(), resp.id.with("hot"), if resp.hovered() { 1.0 } else { 0.0 }, theme::SNAPPY).clamp(0.0, 1.0);
                if hot > 0.0 { ui.painter().rect_filled(b, CornerRadius::same(7), theme::FIELD_HOVER.gamma_multiply(hot)); }
                frame::paint_icon(ui.painter(), *icon, b.center(), 16.0, Color32::from_rgb(0xD2, 0xD2, 0xD6));
                if resp.tip_keys(*label, keys).clicked() { **flag = true; }
            }
        }
    });
    ui.add_space(16.0);
}

/// The colours the level's parts use (the colour picker offers them first), most used first.
fn project_palette(l: &level::Level) -> Vec<[f32; 3]> {
    let mut seen: Vec<([u8; 3], [f32; 3], usize)> = vec![];
    for p in l.parts.iter().filter(|p| !p.shape.is_marker() && p.shape != Shape::PlayerStart) {
        let k = p.color.map(|x| (x * 255.0).round() as u8);
        match seen.iter_mut().find(|s| s.0 == k) { Some(s) => s.2 += 1, None => seen.push((k, p.color, 1)) }
    }
    seen.sort_by_key(|s| std::cmp::Reverse(s.2));
    seen.into_iter().take(12).map(|s| s.1).collect()
}

/// A part's kind: its list section, icon and category colour.
fn kind(shape: Shape) -> (&'static str, &'static str, Color32) {
    match shape {
        s if s.is_light() => ("Lights", "bulb", theme::CAT_LIGHTS),
        s if s.is_camera() => ("Cameras", "camera", theme::CAT_CAMERAS),
        Shape::PlayerStart => ("Game", "person", theme::CAT_GAME),
        Shape::Interact => ("Triggers", "bolt", theme::CAT_TRIGGERS),
        Shape::Sound => ("Actions", "play", theme::CAT_ACTIONS),
        Shape::Model => ("Models", "cube", theme::CAT_SHAPES),
        _ => ("Shapes", "cube", theme::CAT_SHAPES),
    }
}

/// While placing: how the wheel has turned and sized it, when it has.
fn placed_how(ed: &Editor) -> String {
    let mut s = String::new();
    let deg = ed.place_turn.to_degrees().rem_euclid(360.0).round();
    if deg != 0.0 && deg != 360.0 { s += &format!(" · turned {deg}°"); }
    if (ed.place_scale - 1.0).abs() > 1e-3 { s += &format!(" · ×{:.2}", ed.place_scale); }
    s
}

/// The selection tag: a small pill in the highlight with the part's icon and name, at its top left in the view.
/// A colour picker's eyedropper is out: the part under the pointer shows its colour (a light's light, a material's
/// tint on that face, else the part's colour); a click in the view hands it to the picker, Esc puts it away.
fn eyedropper(ctx: &egui::Context, ed: &Editor, view: &View, cam: (&Camera, &GlobalTransform)) {
    let key = Id::new(crate::settings::EYEDROPPER);
    let Some(pid) = ctx.data(|d| d.get_temp::<Id>(key)) else { return };
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { ctx.data_mut(|d| d.remove::<Id>(key)); return; }
    let Some(at) = ctx.pointer_hover_pos() else { return };
    let v = view.rect;
    if !v.contains(Vec2::new(at.x, at.y)) || !frame::inside(&view.outline, at) || ctx.is_pointer_over_egui() { return; }
    ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
    let colour = view.ray(cam.0, cam.1, Vec2::new(at.x, at.y)).and_then(|ray| ed.hit(ray)).and_then(|h| {
        let p = ed.level.get(h.part?)?;
        Some(if let Some(l) = &p.light { l.colour } else if let Some(m) = h.face.and_then(|f| p.surface(f)).and_then(|s| ed.level.materials.get(&s.material)) { m.tint } else { p.color })
    });
    let Some(c) = colour else {
        theme::float_label(ctx, "eyedropper", at + vec2(16.0, 16.0), "Point at a part", MUTED, &["Esc"]);
        return;
    };
    let c32 = Color32::from_rgb((c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8);
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("eyedropper-swatch")));
    p.circle_filled(at, 14.0, c32);
    p.circle_stroke(at, 14.0, Stroke::new(2.5, Color32::WHITE));
    p.circle_stroke(at, 4.0, Stroke::new(1.5, if c[0] + c[1] + c[2] > 1.5 { INK } else { Color32::WHITE }));
    theme::float_label(ctx, "eyedropper", at + vec2(22.0, 16.0), &theme::colour_name(c), c32, &["Click"]);
    if ctx.input(|i| i.pointer.primary_clicked()) {
        ctx.data_mut(|d| { d.remove::<Id>(key); d.insert_temp(Id::new(crate::settings::EYEDROPPED), (pid, c)); });
    }
}

fn selection_tag(ctx: &egui::Context, ed: &Editor, view: &View, cam: (&Camera, &GlobalTransform)) {
    let Some(p) = ed.selected.and_then(|id| ed.level.get(id)) else { return };
    let (lo, hi) = ed.geo.local_box(p);
    let m = crate::world::part_affine(p);
    let pts: Vec<Vec2> = (0..8).filter_map(|i| {
        let c = vec3(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
        view.to_screen(cam.0, cam.1, m.transform_point3(c))
    }).collect();
    if pts.len() < 8 { return; }
    let (x, y) = pts.iter().fold((f32::MAX, f32::MAX), |(x, y), q| (x.min(q.x), y.min(q.y)));
    paint_tag(ctx, view, Vec2::new(x, y), kind(p.shape).1, &p.name, Id::new(("tag-in", p.id)));
}

/// The open sound's tag in the Audio tab, over whatever plays it on the stage (the speaker, or the walker).
fn stage_tag(ctx: &egui::Context, ed: &Editor, view: &View, cam: (&Camera, &GlobalTransform)) {
    let Some(path) = ed.sound_open.as_ref().filter(|p| ed.level.sounds.contains_key(*p)) else { return };
    let top = if ed.stage.walker { audio::walker_at(ed.stage.turn) + Vec3::Y * 1.8 } else { audio::SPEAKER_AT + Vec3::Y * 0.6 };
    let Some(at) = view.to_screen(cam.0, cam.1, top) else { return };
    paint_tag(ctx, view, at - Vec2::new(24.0, 0.0), "wave", &sound_name(path), Id::new(("tag-in", path)));
}

/// The pen's stretch button: a small pill by the selected point with how the line leaving it treats what it meets
/// (Ground, Over everything, Air, Fall); a click goes on to the next.
fn stretch_button(ctx: &egui::Context, ed: &mut Editor, view: &View, cam: (&Camera, &GlobalTransform)) {
    if !ed.pen_on { return; }
    let (Some(id), Some(i)) = (ed.pen.curve, ed.pen.point) else { return };
    let Some(c) = ed.level.curve(id) else { return };
    let Some(point) = c.points.get(i) else { return };
    // the last point of an open curve has no stretch yet, unless the next point is about to go on
    if i + 1 == c.points.len() && !c.closed && !ed.pen.drawing { return; }
    let Some(at) = view.to_screen(cam.0, cam.1, point.pos()) else { return };
    let name = point.stretch.name();
    let w = 12.0 + text_w(ctx, name, theme::bold(12.0)) + 12.0;
    let v = view.rect;
    let pos = pos2((at.x + 16.0).clamp(v.min.x + 8.0, (v.max.x - w - 8.0).max(v.min.x)), (at.y - 38.0).clamp(v.min.y + 48.0, v.max.y - 60.0));
    let mut next = None;
    egui::Area::new(Id::new("stretch-button")).order(egui::Order::Middle).fixed_pos(pos).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
        let fill = if resp.hovered() { highlight() } else { highlight().gamma_multiply(0.85) };
        ui.painter().rect_filled(r, CornerRadius::same(7), fill);
        ui.painter().text(r.center(), Align2::CENTER_CENTER, name, theme::bold(12.0), look().highlight_text);
        let resp = resp.tip_keys("How the line leaving this point treats what it meets: Ground, Over everything, Air or Fall", &["Click"]);
        if resp.clicked() { next = Some(point.stretch.next()); }
    });
    if let Some(s) = next {
        if let Some(p) = ed.level.curve_mut(id).and_then(|c| c.points.get_mut(i)) { p.stretch = s; }
    }
}

/// A tag: the small pill in the highlight with an icon and a name, its top left just above `at` (screen points),
/// kept in the view; it springs in.
fn paint_tag(ctx: &egui::Context, view: &View, at: Vec2, icon: &str, name: &str, id: Id) {
    let w = 7.0 + 13.0 + 6.0 + text_w(ctx, name, theme::bold(12.0)) + 9.0;
    let v = view.rect;
    let at = pos2(at.x.clamp(v.min.x + 8.0, (v.max.x - w - 8.0).max(v.min.x)), (at.y - 30.0).clamp(v.min.y + 48.0, v.max.y - 60.0));
    let r = ERect::from_min_size(at, vec2(w, 24.0));
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, Id::new("selection-tag")));
    let t = theme::spring_from(ctx, id, 0.0, 1.0, theme::SNAPPY).clamp(0.0, 1.0);
    painter.rect_filled(r, CornerRadius::same(7), highlight().gamma_multiply(t));
    frame::paint_icon(&painter, icon, pos2(r.left() + 7.0 + 6.5, r.center().y), 13.0, look().highlight_text.gamma_multiply(t));
    painter.text(pos2(r.left() + 7.0 + 13.0 + 6.0, r.center().y), Align2::LEFT_CENTER, name, theme::bold(12.0), look().highlight_text.gamma_multiply(t));
}

fn overlays(ctx: &egui::Context, ed: &mut Editor, view: &View, perf: &crate::perf::Perf, now: f64) {
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

    // the status pills at the view's top left: the island (the movement mode; it springs open for each message),
    // then the light budget
    let rest = match view.mode {
        Mode::Editor => "Editor movement".to_string(),
        Mode::Builder => format!("Builder movement · fly {}", speed_text(view.speed)),
    };
    // while something bakes in the background the island says how far it has got
    let baking = ed.bakes.running();
    let rest = match baking.first() {
        Some((what, done)) => format!("Baking {what} · {:.0}%{}", done * 100.0, if baking.len() > 1 { format!(" · {} more", baking.len() - 1) } else { String::new() }),
        None => rest,
    };
    let island = ed.island.draw(ctx, pos2(r.min.x + 14.0, r.min.y + 14.0), "move", &rest);

    // speed strip (Builder movement, or for 2 s after a change)
    let recent = now - view.speed_changed < 2.0;
    if view.mode == Mode::Builder || recent {
        egui::Area::new(Id::new("speeds")).order(egui::Order::Foreground).fixed_pos(pos2(r.min.x + 14.0, r.max.y - 14.0 - 40.0)).interactable(false).show(ctx, |ui| {
            egui::Frame::new().fill(INK).corner_radius(R_CARD).stroke(Stroke::new(1.0, white(20))).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("FLY").font(mono(10.0)).color(MUTED));
                    let mut s = view.speed;
                    theme::segmented(ui, "speed", &SPEED_LABELS, &mut s, false);
                    theme::kbd(ui, "PgDn");
                    theme::kbd(ui, "PgUp");
                });
            });
        });
    }

    if let Some(shape) = ed.placing {
        theme::float_label(ctx, "placing", pos2(r.center().x - 170.0, r.max.y - 48.0 - 34.0), &format!("Placing {}{} · click a surface", if matches!(shape, Shape::Model | Shape::Sound) { ed.placing_model.as_deref().and_then(|m| m.rsplit('/').next()).map(|m| m.trim_end_matches(".bev").to_string()).unwrap_or_else(|| shape.label().to_string()) } else { shape.label().to_string() }, placed_how(ed)), highlight(), &["Wheel", "Shift", "Esc"]);
    }

    if let Some((from, out)) = ed.wiring {
        let name = ed.level.get(from).map(|p| p.name.clone()).unwrap_or_default();
        theme::float_label(ctx, "wiring", pos2(r.center().x - 170.0, r.max.y - 48.0 - 34.0), &format!("Wire {name} · {out}: click the part it starts"), theme::CAT_TRIGGERS, &["Esc"]);
    }
    if let Some((from, out, to, at)) = ed.wire_pick {
        wire_menu(ctx, ed, from, out, to, pos2(at.x, at.y));
    }

    light_budget(ctx, ed, pos2(island.right() + 8.0, island.top()));
    if ed.stats { stats_pill(ctx, perf, pos2(island.left(), island.bottom() + 8.0)); }

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

/// The performance meter (View > Performance meter), under the island: what the last frames cost, each number green,
/// amber or red against what a 60 fps game can spare (perf.rs). A number the card can't give is left out.
fn stats_pill(ctx: &egui::Context, perf: &crate::perf::Perf, at: egui::Pos2) {
    use crate::perf::Grade;
    let tone = |g: Grade| match g { Grade::Green => theme::OK, Grade::Amber => theme::CAT_LIGHTS, Grade::Red => theme::DANGER_TEXT };
    let big = |n: f64| if n >= 1e6 { format!("{:.1}M", n / 1e6) } else if n >= 1e3 { format!("{:.0}k", n / 1e3) } else { format!("{n:.0}") };
    let mut items: Vec<(&str, String, Option<Grade>)> = vec![("fps", format!("{:.0}", perf.fps), Some(perf.frame_grade())), ("frame", format!("{:.1} ms", perf.frame_ms), Some(perf.frame_grade()))];
    if let Some(ms) = perf.gpu_ms { items.push(("card", format!("{ms:.1} ms"), perf.gpu_grade())); }
    if let Some(t) = perf.triangles { items.push(("triangles", big(t), perf.triangles_grade())); }
    items.push(("meshes", big(perf.meshes as f64), Some(perf.meshes_grade())));
    items.push(("lights", format!("{}", perf.lights), Some(perf.lights_grade())));
    items.push(("entities", big(perf.entities as f64), None));
    if let Some(gb) = perf.ram_gb { items.push(("memory", format!("{gb:.1} GB"), perf.ram_grade())); }
    let worst = items.iter().filter_map(|i| i.2).max_by_key(|g| *g as u8);
    let (lab, val) = (body(11.0), mono(12.0));
    let w = 12.0 + 14.0 + 10.0 + items.iter().map(|(l, v, _)| text_w(ctx, v, val.clone()) + 4.0 + text_w(ctx, l, lab.clone()) + 12.0).sum::<f32>();
    let rect = ERect::from_min_size(at, vec2(w, 28.0));
    egui::Area::new(Id::new("stats")).order(egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(14), Color32::from_rgba_unmultiplied_const(14, 14, 16, 184));
        p.rect_stroke(rect, CornerRadius::same(14), Stroke::new(1.0, if worst == Some(Grade::Red) { theme::DANGER.gamma_multiply(0.7) } else { theme::EDGE }), StrokeKind::Inside);
        let cy = rect.center().y;
        frame::paint_icon(p, "gauge", pos2(rect.left() + 12.0 + 7.0, cy), 14.0, theme::accent());
        let mut x = rect.left() + 12.0 + 14.0 + 10.0;
        for (l, v, g) in &items {
            let r = p.text(pos2(x, cy), Align2::LEFT_CENTER, v, val.clone(), g.map_or(TEXT, tone));
            let r = p.text(pos2(r.right() + 4.0, cy), Align2::LEFT_CENTER, l, lab.clone(), MUTED);
            x = r.right() + 12.0;
        }
        ui.interact(rect, Id::new("stats-tip"), Sense::hover()).tip("The last frames: frames a second and their time, the card's own time and the triangles it drew (shadows too; only where the card can count them), meshes in view, lights, everything in the level, and this program's memory.");
    });
}

/// The light budget meter (under the menus, while the level has shadowed lights): the most shadowed lights reaching one
/// place, against the budget. Clicking it selects them in turn.
fn light_budget(ctx: &egui::Context, ed: &mut Editor, at: egui::Pos2) {
    let id = Id::new("light-budget");
    let worst = super::light::budget(&ed.level);
    if worst.is_empty() { theme::spring_reset(ctx, id.with("fade")); return; }
    let (n, max) = (worst.len(), super::light::BUDGET);
    let col = if n > max { theme::DANGER } else { theme::accent() };
    let t = theme::spring_from(ctx, id.with("fade"), 0.0, 1.0, theme::SNAPPY);
    let names: Vec<String> = worst.iter().filter_map(|i| ed.level.get(*i)).map(|p| p.name.clone()).collect();
    let help = format!(
        "Shadowed lights reaching the same place: {} of the {max} that stay smooth.\n{}\n{}Click to select them in turn.",
        n, names.join(", "), if n > max { "Turn Shadows off on some, or make their reach smaller.\n" } else { "" }
    );
    // a status pill: the bulb in the accent, a cell per shadowed light; a danger ring when over
    let label_w = text_w(ctx, "Lights", body(12.0));
    let count = format!("{n} / {max}");
    let cells = n.max(max);
    let w = 12.0 + 14.0 + 8.0 + label_w + 8.0 + cells as f32 * 10.0 - 2.0 + 8.0 + text_w(ctx, &count, mono(12.0)) + 12.0;
    let rect = ERect::from_min_size(at, vec2(w, 28.0));
    let res = egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
        ui.multiply_opacity(t.clamp(0.0, 1.0));
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(14), Color32::from_rgba_unmultiplied_const(14, 14, 16, 184));
        p.rect_stroke(rect, CornerRadius::same(14), Stroke::new(1.0, if n > max { theme::DANGER.gamma_multiply(0.7) } else { theme::EDGE }), StrokeKind::Inside);
        let cy = rect.center().y;
        frame::paint_icon(p, "bulb", pos2(rect.left() + 12.0 + 7.0, cy), 14.0, if n > max { theme::DANGER } else { theme::accent() });
        let mut x = rect.left() + 12.0 + 14.0 + 8.0;
        p.text(pos2(x, cy), Align2::LEFT_CENTER, "Lights", body(12.0), if n > max { theme::DANGER_TEXT } else { TEXT });
        x += label_w + 8.0;
        for i in 0..cells {
            let c = if i >= max { theme::DANGER.gamma_multiply(0.45) } else if i < n { col } else { white(30) };
            p.rect_filled(ERect::from_min_size(pos2(x, cy - 5.0), vec2(8.0, 10.0)), CornerRadius::same(2), c);
            x += 10.0;
        }
        p.text(pos2(x - 2.0 + 8.0, cy), Align2::LEFT_CENTER, count, mono(12.0), if n > max { theme::DANGER_TEXT } else { TEXT });
        ui.interact(rect, id.with("click"), Sense::click()).tip(help)
    }).inner;
    if res.clicked() {
        let next = ed.selected.and_then(|s| worst.iter().position(|i| *i == s)).map_or(0, |k| (k + 1) % n);
        ed.selected = Some(worst[next]);
        ed.material_open = None;
        ed.sound_open = None;
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
            "Sound" => Some(Shape::Sound),
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
        (ed.place_turn, ed.place_scale) = (0.0, 1.0);
        ed.parts_open = false;
        ed.island.show(s.glyph(), format!("Placing {}", s.label()), "Click a surface · wheel turns, Shift + wheel sizes · Shift keeps it upright · Esc cancels", Tone::Accent);
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
        Stroke::new(1.5, highlight())
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
            let save = egui::Button::new(egui::RichText::new("Save").font(semi(13.0)).color(INK)).fill(highlight()).min_size(vec2(90.0, 32.0));
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
                    _ if ready(f) => (highlight(), "ready".into()),
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
                            ui.label(egui::RichText::new(&f.effort).font(mono(11.0)).color(highlight()));
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
        ui.label(egui::RichText::new(format!("This is {}", this_build())).color(highlight()));
        ui.add_space(10.0);
        ui.add(egui::Label::new(egui::RichText::new("A no-code-first 3D game editor for Bevy: build levels by placing and wiring parts, test them on the spot, and export a Windows game.").color(TEXT)).wrap());
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Bevy 0.20.1 · bevy_egui 0.43.1 · Motion UI kit").color(MUTED));
        ui.hyperlink_to(egui::RichText::new("github.com/thriple/Beverly").color(highlight()), env!("CARGO_PKG_REPOSITORY"));
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
        let accent = |label: &str| egui::Button::new(egui::RichText::new(label).font(semi(13.0)).color(INK)).fill(highlight()).min_size(vec2(170.0, 32.0));
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
                    ui.painter().rect_filled(bar.with_max_x(bar.left() + w * f), CornerRadius::same(4), highlight());
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
            if ui.small_button("New token").tip("Assistants using the old one can no longer connect").clicked() {
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
        Shape::Model | Shape::Sound | Shape::PlayerStart | Shape::Interact | Shape::PointLight | Shape::SpotLight | Shape::FixedCamera | Shape::OrbitCamera | Shape::FollowCamera => {}
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
