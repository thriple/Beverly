//! The editor's frame: the top bar and its menus, the topic chips, the bottom notch with its hints, the window edges and the dock.
use super::*;

/// A Level topic: its chip in the top notch (name, icon) and what it shows, or None and what it comes with.
pub(super) const TOPICS: [(&str, &str, Option<LevelMode>, &str); 7] = [
    ("All", "grid", Some(LevelMode::Build), ""),
    ("Physics", "feel", Some(LevelMode::Physics), ""),
    ("Lighting", "bulb", Some(LevelMode::Lighting), ""),
    ("Cameras", "camera", Some(LevelMode::Cameras), ""),
    ("Water", "drop", None, "Coming soon: water comes with the Ocean"),
    ("Foliage", "leaf", Some(LevelMode::Foliage), ""),
    ("World", "globe", None, "Coming soon: World settings come next"),
];

pub(super) fn chip_w(ctx: &egui::Context, name: &str, on: bool) -> f32 { 11.0 + 14.0 + 6.0 + text_w(ctx, name, if on { semi(12.0) } else { body(12.0) }) + 11.0 }

/// The open tab's topics: (name, icon, chosen, built, when it comes if not).
pub(super) fn topic_list(ed: &Editor) -> Vec<(&'static str, &'static str, bool, bool, &'static str)> {
    match ed.tab {
        audio::AUDIO => audio::TOPICS.iter().enumerate().map(|(i, (n, icon, later))| (*n, *icon, i == 0, later.is_empty(), *later)).collect(),
        _ => TOPICS.iter().map(|(n, icon, m, later)| (*n, *icon, *m == Some(ed.level_mode), m.is_some(), *later)).collect(),
    }
}

/// The top notch's width: its topic chips, and room for its shoulders.
pub(super) fn topics_width(ctx: &egui::Context, ed: &Editor) -> f32 {
    topic_list(ed).iter().map(|(n, _, on, _, _)| chip_w(ctx, n, *on) + 2.0).sum::<f32>() - 2.0 + 96.0
}

/// The tab's topics as chips in the top notch: the chosen one is a pill in the frame's text colour; those not built
/// yet are faint and say when they come.
pub(super) fn topic_chips(ctx: &egui::Context, ed: &mut Editor, lay: &frame::Layout) {
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
pub(super) fn top_bar(ctx: &egui::Context, ed: &mut Editor, lay: &frame::Layout, window: &mut Window, exit: &mut MessageWriter<AppExit>) {
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
                ("Play", &["play.test", "game.controls", "", "file.export"]),
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
pub(super) const TAB_LATER: [&str; 8] = [
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
pub(super) fn window_edges(ctx: &egui::Context, lay: &frame::Layout, window: &mut Window, maximized: bool) {
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
pub(super) fn hints(mode: Mode, brush: bool, pen: bool, bucket: bool, tab: usize) -> &'static [(&'static str, &'static [&'static str])] {
    if pen { return &[("Point", &["Click"]), ("Curve", &["Drag"]), ("Break handle", &["Alt"]), ("Finish", &["Enter"]), ("Done", &["Esc"])]; }
    if bucket { return &[("Fill", &["Click"]), ("Erase", &["Alt", "Click"]), ("Done", &["Esc"])]; }
    if brush { return &[("Size", &["[", "]"]), ("Hardness", &["Shift", "[", "]"]), ("Erase", &["Alt"]), ("Done", &["Esc"])]; }
    match mode {
        Mode::Editor if tab == audio::AUDIO => &[("Orbit", &["Middle-drag"]), ("Pan", &["Space"]), ("Zoom", &["Wheel"]), ("Play", &["P"])],
        Mode::Editor => &[("Orbit", &["Middle-drag"]), ("Pan", &["Space"]), ("Zoom", &["Wheel"]), ("Fly", &["Caps Lock"])],
        Mode::Builder => &[("Fly", &["W", "A", "S", "D"]), ("Speed", &["Mouse 4", "Mouse 5"]), ("Cursor", &["Alt"]), ("Edit", &["Caps Lock"])],
    }
}

/// The bottom notch's contents, laid out from `x` along the line through `cy` (or only measured): saved or not, a
/// divider, then the hints. Returns its width.
pub(super) fn notch_contents(ctx: &egui::Context, p: Option<(&egui::Painter, f32, f32)>, saved: bool, hints: &[(&str, &[&str])]) -> f32 {
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
pub(super) fn bottom_notch(ctx: &egui::Context, ed: &mut Editor, view: &View, lay: &frame::Layout, exit: &mut MessageWriter<AppExit>) {
    let saved = !ed.dirty();
    let w = notch_contents(ctx, None, saved, hints(view.mode, ed.brush_on, ed.pen_on, ed.bucket_on, ed.tab));
    let n = lay.bottom_notch;
    let cy = n.top() + 4.0 + 13.0;
    let x0 = n.center().x - w / 2.0;
    egui::Area::new(Id::new("bottom-notch")).order(egui::Order::Middle).fixed_pos(n.min).constrain(false).show(ctx, |ui| {
        ui.set_min_size(n.size());
        notch_contents(ctx, Some((ui.painter(), x0, cy)), saved, hints(view.mode, ed.brush_on, ed.pen_on, ed.bucket_on, ed.tab));
        let sr = ERect::from_min_size(pos2(x0 - 4.0, cy - 12.0), vec2(20.0 + text_w(ctx, if saved { "Saved" } else { "Not saved" }, semi(12.0)) + 8.0, 24.0));
        let resp = ui.interact(sr, Id::new("save-state"), if saved { Sense::hover() } else { Sense::click() }).tip_keys(if saved { "Everything is saved" } else { "Save" }, &["Ctrl", "S"]);
        if resp.clicked() { let _ = actions::run(ed, None, "file.save", exit); }
    });
}

/// The notch's width for these contents (shoulders and a little air).
pub(super) fn bottom_width(ctx: &egui::Context, ed: &Editor, view: &View) -> f32 {
    notch_contents(ctx, None, !ed.dirty(), hints(view.mode, ed.brush_on, ed.pen_on, ed.bucket_on, ed.tab)) + 2.0 * frame::SHOULDER + 16.0
}

/// The dock's three views: (name, icon); the middle one is the tab's list.
pub(super) const DOCK: [(&str, &str); 3] = [("Inspect", "sliders"), ("Scene", "layers"), ("Assets", "folder")];
pub(super) const AUDIO_DOCK: [(&str, &str); 3] = [("Inspect", "sliders"), ("Sounds", "wave"), ("Assets", "folder")];
pub(super) fn dock_views(ed: &Editor) -> [(&'static str, &'static str); 3] { if ed.tab == audio::AUDIO { AUDIO_DOCK } else { DOCK } }
pub(super) const DOCK_W: f32 = 352.0;

/// The dock on the view's right (glass): a switch of Inspect · Scene · Assets, and the chosen one's contents; it
/// folds to a rail of icons. It moves as the Motion kit's dock does: the chosen pill slides, the contents slide in
/// from the side you went, and folding springs its width.
pub(super) fn dock(ctx: &egui::Context, ed: &mut Editor, view: &mut View, lay: &frame::Layout, sounds: &crate::sound::Sounds, exit: &mut MessageWriter<AppExit>) {
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
                    0 if foliage_open(ed) && ed.material_open.is_some() => inspector(ui, ed, view, exit),
                    0 if foliage_open(ed) => foliage_panel(ui, ed),
                    0 if ed.brush_on => brush_panel(ui, ed),
                    0 if ed.pen_on => pen_panel(ui, ed),
                    0 if ed.bucket_on => bucket_panel(ui, ed),
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

/// A menu item for a named action (its name and keys from the list), greyed when it can't run; runs it when clicked.
pub(super) fn menu_action(ui: &mut Ui, ed: &mut Editor, id: &str, exit: &mut MessageWriter<AppExit>) {
    let a = actions::get(id).expect("a listed action");
    let why = actions::why_not(ed, id);
    let r = ui.add_enabled(why.is_none(), egui::Button::new(a.name).shortcut_text(a.keys).frame(false));
    let r = match why { Some(w) => r.tip(w), None => r };
    if r.clicked() {
        let _ = actions::run(ed, None, id, exit);
        ui.close();
    }
}
