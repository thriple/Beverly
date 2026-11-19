//! The Audio tab's panels: the dock's list of sounds, the open sound in Inspect (its recordings drawn as waveforms,
//! the recordings it plays on particular surfaces, its settings), and the stage's bar over the view.
use super::*;
use crate::level::{Ground, SoundSet};
use crate::sound::{Sounds, Wave};

/// The Sounds view: every sound file, found by name; click one to open it (the stage plays it).
pub(super) fn list(ui: &mut Ui, ed: &mut Editor) {
    let key = Id::new("sounds-find");
    let mut find: String = ui.data(|d| d.get_temp(key)).unwrap_or_default();
    if theme::text_input(ui, &mut find, "Find a sound", Some("search"), ui.available_width()).changed() { ui.data_mut(|d| d.insert_temp(key, find.clone())); }
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    if resp.hovered() { ui.painter().rect_filled(r, CornerRadius::same(7), theme::FIELD); }
    frame::paint_icon(ui.painter(), "plus", pos2(r.left() + 10.0 + 7.0, r.center().y), 14.0, theme::accent());
    ui.painter().text(pos2(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, "New sound", body(12.5), theme::accent());
    if resp.tip("A sound file: add recordings, then set its volume, pitch and range").clicked() {
        ed.sound_open = Some(ed.level.new_sound("Sound", level::Sound::default()));
        ed.dock_view = 0;
    }
    let q = find.trim().to_lowercase();
    let rows: Vec<(String, usize, usize, bool)> = ed.level.sounds.iter()
        .filter(|(p, _)| q.is_empty() || sound_name(p).to_lowercase().contains(&q))
        .map(|(p, s)| (p.clone(), s.all_clips().count(), ed.level.sound_users(p), ed.level.plays(p)))
        .collect();
    if rows.is_empty() {
        ui.label(egui::RichText::new(if q.is_empty() { "No sounds yet: drop recordings on the window, or make a new one" } else { "No sound by that name" }).color(MUTED));
    }
    // the ones the level plays first, then the rest
    for (head, used) in [("In the level", true), ("Not used yet", false)] {
        let rows: Vec<&(String, usize, usize, bool)> = rows.iter().filter(|r| r.3 == used).collect();
        if rows.is_empty() { continue; }
        ui.add_space(4.0);
        ui.label(egui::RichText::new(head).size(11.5).color(MUTED));
    for (path, clips, users, _) in rows.into_iter().cloned() {
        let on = ed.sound_open.as_deref() == Some(path.as_str());
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
        if on { ui.painter().rect_filled(r, CornerRadius::same(7), theme::SELECTED_ROW); } else if resp.hovered() { ui.painter().rect_filled(r, CornerRadius::same(7), theme::FIELD); }
        frame::paint_icon(ui.painter(), "wave", pos2(r.left() + 10.0 + 7.0, r.center().y), 14.0, theme::CAT_ACTIONS);
        ui.painter().text(pos2(r.left() + 32.0, r.center().y), Align2::LEFT_CENTER, sound_name(&path), if on { semi(12.5) } else { body(12.5) }, if on { Color32::WHITE } else { TEXT });
        let n = |n: usize, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
        let (note, c) = if clips == 0 { ("No recordings".to_string(), theme::DANGER_TEXT) } else { (n(clips, "recording"), MUTED) };
        ui.painter().text(pos2(r.right() - 10.0, r.center().y), Align2::RIGHT_CENTER, note, body(11.5), c);
        if resp.tip(format!("Played by {}", n(users, "Sound part"))).clicked() { ed.sound_open = Some(path); }
    }
    }
}

/// Inspect: the open sound. Every Sound part playing it, and everything that plays it later, changes with it.
pub(super) fn inspect(ui: &mut Ui, ed: &mut Editor, sounds: &Sounds) {
    let Some(path) = ed.sound_open.clone().filter(|p| ed.level.sounds.contains_key(p)) else {
        identity(ui, theme::CAT_ACTIONS, "wave", Err("No sound open"), "Pick one in Sounds, or make one", &mut []);
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Drop .wav, .ogg, .mp3 or .flac files on the window to bring recordings in.").color(MUTED));
        if theme::main_button(ui, "New sound").clicked() { ed.sound_open = Some(ed.level.new_sound("Sound", level::Sound::default())); }
        return;
    };
    let mut m = ed.level.sounds[&path].clone();
    let before = m.clone();
    let mut ranges = ed.level.ranges.clone();
    let users = ed.level.sound_users(&path);
    let name = sound_name(&path);
    let recordings: Vec<String> = ed.assets.iter().find(|(f, _)| *f == "sounds").map(|(_, t)| t.iter().filter(|t| level::is_recording(t)).map(|t| format!("assets/sounds/{t}")).collect()).unwrap_or_default();
    let n = |n: usize, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
    let (mut copy, mut play) = (false, false);
    let sub = format!("Sound · {} · on {}", n(m.all_clips().count(), "recording"), n(users, "part"));
    // Play it once only when it has something to play
    let mut buttons: Vec<(&str, &str, &[&str], &mut bool)> = vec![];
    if m.all_clips().next().is_some() { buttons.push(("play", "Play it once, as the game would pick it", &[], &mut play)); }
    buttons.push(("copy", "Copy: a new sound like this one, to change on its own", &[], &mut copy));
    identity(ui, theme::CAT_ACTIONS, "wave", Err(&name), &sub, &mut buttons);
    if play { ed.preview_sound = Some((path.clone(), ed.stage.ground.filter(|_| ed.stage.walker))); }
    column(ui, 0.0, "RECORDINGS", |ui| {
        clip_list(ui, &mut m.clips, &recordings, sounds, "all-recordings");
        if m.clips.is_empty() { ui.label(egui::RichText::new("Add a recording for it to play").color(MUTED).size(11.5)); }
    });
    column(ui, 0.0, "ON SURFACES", |ui| {
        ui.label(egui::RichText::new("On these surfaces it plays these instead (footsteps on grass, metal, water)").color(MUTED).size(11.5));
        let mut gone = None;
        for (i, set) in m.sets.iter_mut().enumerate() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(set.ground.label()).font(semi(12.5)).color(TEXT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if trash(ui, &format!("Plays its usual recordings on {} again", set.ground.label().to_lowercase())) { gone = Some(i); }
                });
            });
            clip_list(ui, &mut set.clips, &recordings, sounds, set.ground.label());
        }
        if let Some(i) = gone { m.sets.remove(i); }
        let free: Vec<Ground> = Ground::ALL.into_iter().filter(|g| !m.sets.iter().any(|s| s.ground == *g)).collect();
        if !free.is_empty() {
            let mut add = None;
            egui::ComboBox::from_id_salt("add surface").selected_text("+ Add a surface").width(ui.available_width()).show_ui(ui, |ui| {
                for g in &free { if ui.selectable_label(false, g.label()).clicked() { add = Some(*g); } }
            });
            if let Some(g) = add { m.sets.push(SoundSet { ground: g, clips: vec![] }); }
        }
    });
    let mut ctx = settings::Ctx { replace: &[], ranges: &mut ranges, ranges_changed: false, palette: vec![] };
    column(ui, 0.0, level::SOUND, |ui| { settings::section(ui, &mut m, level::SOUND, &mut ctx); });
    column(ui, 0.0, level::REACH, |ui| { settings::section(ui, &mut m, level::REACH, &mut ctx); });
    group(ui, "ADVANCED", Some(1), |ui| { settings::section(ui, &mut m, level::SOUND_MORE, &mut ctx); });
    let ranges_changed = ctx.ranges_changed;
    if m != before { ed.level.sounds.insert(path.clone(), m.clone()); }
    if ranges_changed { ed.level.ranges = ranges; }
    if copy {
        let new = ed.level.new_sound(&format!("{name} copy"), m);
        ed.island.show("♪", "Copied the sound", sound_name(&new), theme::Tone::Accent);
        ed.sound_open = Some(new);
    }
}

/// Recordings, each with its waveform, length and ✕, then a menu to add another from the project's.
fn clip_list(ui: &mut Ui, clips: &mut Vec<String>, recordings: &[String], sounds: &Sounds, id: &str) {
    let mut remove = None;
    for (i, c) in clips.iter().enumerate() {
        let wave = sounds.wave(c);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(c.rsplit('/').next().unwrap_or(c)).color(TEXT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if trash(ui, "Take this recording out") { remove = Some(i); }
                if let Some(w) = wave { ui.label(egui::RichText::new(format!("{:.1} s", w.secs)).font(mono(11.0)).color(MUTED)); }
            });
        });
        waveform(ui, wave);
    }
    if let Some(i) = remove { clips.remove(i); }
    let mut add: Option<String> = None;
    egui::ComboBox::from_id_salt(("add recording", id)).selected_text("+ Add a recording").width(ui.available_width()).show_ui(ui, |ui| {
        for r in recordings.iter().filter(|r| !clips.contains(r)) {
            if ui.selectable_label(false, r.rsplit('/').next().unwrap_or(r)).clicked() { add = Some(r.clone()); }
        }
        if recordings.is_empty() { ui.label(egui::RichText::new("Drop recordings on the window first").color(MUTED)); }
    });
    if let Some(r) = add { clips.push(r); }
}

/// A warning chip: the danger ring and text, with the alert icon.
fn warning(ui: &mut Ui, text: &str, tip: &str) {
    let g = ui.painter().layout_no_wrap(text.to_owned(), body(12.0), theme::DANGER_TEXT);
    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 38.0, 28.0), Sense::hover());
    let p = ui.painter();
    p.rect_stroke(r, CornerRadius::same(14), Stroke::new(1.0, theme::DANGER.gamma_multiply(0.7)), StrokeKind::Inside);
    frame::paint_icon(p, "alert", pos2(r.left() + 12.0 + 7.0, r.center().y), 14.0, theme::DANGER);
    p.galley(pos2(r.left() + 32.0, r.center().y - g.size().y / 2.0), g, theme::DANGER_TEXT);
    resp.tip(tip);
}

/// A small trash button (24 points): takes something out of the list it's in.
fn trash(ui: &mut Ui, tip: &str) -> bool {
    let (b, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    if resp.hovered() { ui.painter().rect_filled(b, CornerRadius::same(6), theme::FIELD_HOVER); }
    frame::paint_icon(ui.painter(), "trash", b.center(), 14.0, if resp.hovered() { TEXT } else { TEXT_2 });
    resp.tip(tip).clicked()
}

/// A recording's outline: a bar per slice, from its lowest to its highest, in the accent, across the panel.
fn waveform(ui: &mut Ui, wave: Option<&Wave>) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same(6), theme::FIELD);
    let mid = r.center().y;
    let Some(w) = wave else {
        p.hline(r.left() + 8.0..=r.right() - 8.0, mid, Stroke::new(1.0, MUTED));
        return;
    };
    let inner = r.shrink2(vec2(6.0, 3.0));
    // scaled to its own loudest, so a quiet recording's shape shows too
    let loud = w.bars.iter().map(|[lo, hi]| hi.abs().max(lo.abs())).fold(0.0f32, f32::max).max(1e-4);
    let half = inner.height() / 2.0 / loud;
    let step = inner.width() / w.bars.len() as f32;
    let col = theme::accent();
    for (i, [lo, hi]) in w.bars.iter().enumerate() {
        let x = inner.left() + i as f32 * step;
        let (top, bottom) = (mid - hi.clamp(-1.0, 1.0) * half, mid - lo.clamp(-1.0, 1.0) * half);
        let bar = ERect::from_min_max(pos2(x, top.min(mid - 0.5)), pos2(x + (step - 0.6).max(0.6), bottom.max(mid + 0.5)));
        p.rect_filled(bar, CornerRadius::ZERO, col);
    }
}

/// The stage's bar over the view: what the sound plays from (the speaker, or the walker's steps on a surface) and
/// Play / Stop.
pub(super) fn stage_bar(ctx: &egui::Context, ed: &mut Editor, lay: &frame::Layout, sounds: &Sounds) {
    let at = pos2(lay.view.left() + 14.0, lay.view.top() + 54.0);
    egui::Area::new(Id::new("stage-bar")).order(egui::Order::Middle).fixed_pos(at).show(ctx, |ui| {
        egui::Frame::new().fill(theme::PANEL).stroke(Stroke::new(1.0, theme::EDGE)).corner_radius(CornerRadius::same(12)).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let Some(path) = ed.sound_open.clone().filter(|p| ed.level.sounds.contains_key(p)) else {
                    ui.label(egui::RichText::new("Open a sound in Sounds to hear it here").color(TEXT_2));
                    return;
                };
                ui.label(egui::RichText::new("On").color(TEXT_2));
                let mut k = ed.stage.walker as usize;
                ui.allocate_ui(vec2(170.0, 28.0), |ui| {
                    if theme::choices(ui, "stage-on", &["Speaker", "Walker"], &mut k) { ed.stage.walker = k == 1; }
                });
                if ed.stage.walker {
                    let shown = ed.stage.ground.map_or("Any surface", |g| g.label());
                    egui::ComboBox::from_id_salt("stage ground").selected_text(shown).width(120.0).show_ui(ui, |ui| {
                        ui.selectable_value(&mut ed.stage.ground, None, "Any surface");
                        for g in Ground::ALL { ui.selectable_value(&mut ed.stage.ground, Some(g), g.label()); }
                    }).response.tip("What the walker walks on: a sound plays its recordings for that surface");
                }
                let (icon, label) = if ed.stage.playing { ("square", "Stop") } else { ("play", "Play") };
                let help = if ed.stage.walker { "The walker walks round, a step each time" } else if ed.level.sounds[&path].looped { "It loops from the speaker" } else { "It plays from the speaker again and again" };
                if theme::main_button_keyed(ui, icon, label, "P").tip(help).clicked() { ed.stage.playing = !ed.stage.playing; }
                if !sounds.on() { warning(ui, "No sound device", "Beverly found nothing to play sound on: plug in headphones or speakers, or pick one in Windows' sound settings"); }
            });
        });
    });
}
