//! The 2D parts of the editor, in the Motion kit's style: top bar with the File menu, the collapsible Assets
//! panel, the bottom properties panel, the parts pop-up (Tab), the status island, the speed strip, hints, and the
//! unsaved-changes question.
use crate::level::{self, Shape, Wire};
use crate::settings::{self, Setting};
use crate::update::{self, Status};
use super::theme::{self, ACCENT, INK, LINE, MUTED, PANEL, R_CARD, TEXT, Tone, mono, semi, white};
use super::view::{Mode, SPEED_LABELS, View};
use super::{actions, mcp, Editor, Pending};
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
        items: &[("✸", "Explosion"), ("≋", "Shake"), ("⁘", "Debris"), ("⌧", "Break"), ("♪", "Sound"), ("⁂", "Particles"), ("☀", "Light FX"), ("❝", "Dialog"), ("◉", "Camera"), ("Ⅱ", "Pause"), ("▦", "Animatic")],
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
        ed.island.draw(&ctx, pos2(r.center().x, r.min.y + 14.0), "Testing", &["`"]);
        return Ok(());
    }
    let mut root = Ui::new(ctx.clone(), "root".into(), egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(ctx.viewport_rect()));

    top_bar(&mut root, &mut ed, &mut exit);
    assets_panel(&mut root, &mut ed);
    bottom_panel(&mut root, &mut ed);
    let r = root.available_rect_before_wrap();
    view.rect = Rect::new(r.left(), r.top(), r.right(), r.bottom());

    overlays(&ctx, &mut ed, &view, time.elapsed_secs_f64());
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

fn top_bar(root: &mut Ui, ed: &mut Editor, exit: &mut MessageWriter<AppExit>) {
    let frame = egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(12, 0)).stroke(Stroke::new(1.0, LINE));
    egui::Panel::top("top").exact_size(46.0).frame(frame).show(root, |ui| {
        ui.horizontal_centered(|ui| {
            ui.label(egui::RichText::new("Beverly").font(semi(16.0)).color(TEXT));
            ui.add_space(6.0);
            ui.menu_button("File", |ui| {
                ui.set_min_width(230.0);
                for id in ["file.new", "file.open", "", "file.save", "file.save_as", "file.export", "file.show_folder", "", "file.quit"] {
                    if id.is_empty() { ui.separator(); } else { menu_action(ui, ed, id, exit); }
                }
            });
            ui.menu_button("Help", |ui| {
                ui.set_min_width(200.0);
                for id in ["help.about", "help.updates", "help.features", "help.mcp"] { menu_action(ui, ed, id, exit); }
            });
            ui.add_space(6.0);
            ui.label(egui::RichText::new(&ed.name).color(MUTED));
            if ed.dirty() {
                let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 3.5, ACCENT);
            }
            ui.add_space(14.0);
            let mut tab = 0usize;
            if theme::segmented(ui, "mode", &["Level", "Screens", "Animatics"], &mut tab, false) {
                let when = if tab == 1 { "v0.3" } else { "v0.4" };
                ed.island.show("…", format!("{} comes in {when}", ["Level", "Screens", "Animatics"][tab]), "Levels are what this version builds", Tone::Plain);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let snap_label = if ed.snap { "Snap 0.25 m · 15°" } else { "Snap off" };
                let mut s = ed.snap;
                if theme::toggle(ui, &mut s).changed() { ed.snap = s; }
                ui.label(egui::RichText::new(snap_label).color(MUTED));
                ui.add_space(10.0);
                if ui.add_enabled(!ed.redo.is_empty(), egui::Button::new("↷ Redo")).on_hover_ui(|ui| theme::hint(ui, "Redo", &["Ctrl", "Y"])).clicked() { let _ = actions::run(ed, None, "edit.redo", exit); }
                if ui.add_enabled(!ed.undo.is_empty(), egui::Button::new("↶ Undo")).on_hover_ui(|ui| theme::hint(ui, "Undo", &["Ctrl", "Z"])).clicked() { let _ = actions::run(ed, None, "edit.undo", exit); }
                ui.add_space(10.0);
                if ui.button("Parts").on_hover_ui(|ui| theme::hint(ui, "Open the parts pop-up", &["Tab"])).clicked() { let _ = actions::run(ed, None, "view.parts", exit); }
            });
        });
    });
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

fn assets_panel(root: &mut Ui, ed: &mut Editor) {
    let open = ed.assets_open;
    let frame = egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::same(if open { 12 } else { 4 })).stroke(Stroke::new(1.0, LINE));
    egui::Panel::right("assets").exact_size(if open { 240.0 } else { 34.0 }).resizable(false).frame(frame).show(root, |ui| {
        if !open {
            ui.vertical_centered(|ui| {
                if ui.button("⇤").on_hover_ui(|ui| theme::hint(ui, "Show the Assets panel", &[])).clicked() { ed.assets_open = true; }
                let g = ui.painter().layout_no_wrap("ASSETS".into(), mono(10.0), MUTED);
                let at = pos2(ui.max_rect().center().x - g.size().y / 2.0, ui.min_rect().bottom() + 70.0);
                ui.painter().add(egui::epaint::TextShape::new(at, g, MUTED).with_angle(std::f32::consts::FRAC_PI_2));
            });
            return;
        }
        ui.horizontal(|ui| {
            small_caps(ui, "ASSETS");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⇥").on_hover_ui(|ui| theme::hint(ui, "Collapse the Assets panel", &[])).clicked() { ed.assets_open = false; }
            });
        });
        // drop target
        let h = if ed.hovering_files { 110.0 } else { 54.0 };
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
        let hot = ed.hovering_files;
        let p = ui.painter();
        p.rect_filled(r, R_CARD, if hot { Color32::from_rgba_unmultiplied(0xf5, 0xdc, 0x4a, 30) } else { white(6) });
        dashed_rect(p, r, if hot { ACCENT } else { white(50) });
        let msg = if hot { "Drop to import" } else { "Drag files here to import" };
        p.text(r.center() - vec2(0.0, 7.0), Align2::CENTER_CENTER, msg, semi(12.0), if hot { ACCENT } else { TEXT });
        p.text(r.center() + vec2(0.0, 10.0), Align2::CENTER_CENTER, "textures · models · sounds", body(11.0), MUTED);
        ui.add_space(4.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).max_height(ui.available_height() * 0.5).show(ui, |ui| {
            let assets = ed.assets.clone();
            for (folder, files) in &assets {
                let title = format!("{}{}  ({})", folder[..1].to_uppercase(), &folder[1..], files.len());
                egui::CollapsingHeader::new(title).id_salt(folder).default_open(*folder == "textures").show(ui, |ui| {
                    if files.is_empty() {
                        ui.label(egui::RichText::new("empty").color(MUTED).size(11.0));
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
                        } else {
                            ui.label(egui::RichText::new(f).size(12.0));
                        }
                    }
                });
            }
        });
        ui.add_space(6.0);
        small_caps(ui, "SCENE");
        egui::ScrollArea::vertical().id_salt("scene").auto_shrink([false, false]).show(ui, |ui| {
            if ed.level.parts.is_empty() {
                ui.label(egui::RichText::new("Nothing placed yet. Press Tab.").color(MUTED).size(12.0));
            }
            let mut pick = None;
            let mut flip = None;
            for p in &ed.level.parts {
                ui.horizontal(|ui| {
                    let on = ed.selected == Some(p.id);
                    let eye = if p.visible { "◉" } else { "○" };
                    if ui.add(egui::Button::new(egui::RichText::new(eye).size(11.0)).frame(false)).on_hover_text(if p.visible { "Hide (keep as collider)" } else { "Show" }).clicked() {
                        flip = Some(p.id);
                    }
                    let txt = egui::RichText::new(format!("{}  {}", p.shape.glyph(), p.name)).size(12.5).color(if on { ACCENT } else { TEXT });
                    if ui.add(egui::Button::selectable(on, txt).frame(false)).clicked() { pick = Some(p.id); }
                });
            }
            if let Some(id) = pick { ed.selected = Some(id); }
            if let Some(id) = flip {
                if let Some(p) = ed.level.get_mut(id) { p.visible = !p.visible; }
            }
        });
    });
}

fn body(size: f32) -> egui::FontId { theme::body(size) }

fn dashed_rect(p: &egui::Painter, r: ERect, color: Color32) {
    let r = r.shrink(1.0);
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    for w in pts.windows(2) {
        p.extend(egui::Shape::dashed_line(&[w[0], w[1]], Stroke::new(1.2, color), 6.0, 4.0));
    }
}

fn bottom_panel(root: &mut Ui, ed: &mut Editor) {
    let frame = egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(16, 12)).stroke(Stroke::new(1.0, LINE));
    egui::Panel::bottom("props").min_size(168.0).max_size(420.0).resizable(false).frame(frame).show(root, |ui| {
        let Some(id) = ed.selected.filter(|id| ed.level.get(*id).is_some()) else {
            ui.vertical_centered(|ui| {
                ui.add_space(34.0);
                ui.label(egui::RichText::new("Select a part to see its settings").font(semi(14.0)).color(MUTED));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let w = 560.0;
                    ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                    theme::hint(ui, "Parts", &["Tab"]);
                    ui.add_space(10.0);
                    theme::hint(ui, "Fly / edit", &["Caps Lock"]);
                    ui.add_space(10.0);
                    theme::hint(ui, "Undo", &["Ctrl", "Z"]);
                    ui.add_space(10.0);
                    theme::hint(ui, "Save", &["Ctrl", "S"]);
                });
            });
            return;
        };
        let mut p = ed.level.get(id).unwrap().clone();
        let before = p.clone();
        let mut delete = false;
        let mut unwire: Option<usize> = None;
        let mut ranges = ed.level.ranges.clone();
        let mut ranges_changed = false;
        egui::ScrollArea::horizontal().show(ui, |ui| { ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
            column(ui, 190.0, "PART", |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                    ui.painter().rect_filled(r, CornerRadius::same(2), theme::CAT_SHAPES);
                    ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(150.0).font(semi(13.0)));
                });
                let file = p.model.as_deref().and_then(|m| m.rsplit('/').next()).map(|f| format!(" · {f}")).unwrap_or_default();
                ui.label(egui::RichText::new(format!("{} {}{file}", p.shape.glyph(), p.shape.label())).color(MUTED));
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
                let title = if model && sec == level::LOOK { "SCALE  ·  LOOK" } else { sec };
                column(ui, if sec == level::LOOK { 260.0 } else { 250.0 }, title, |ui| { settings::section(ui, &mut p, sec, &mut ctx); });
            }
            ranges_changed = ctx.ranges_changed;
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
            ui.add_space(12.0); // room for the scroll bar, which floats over the bottom
        });
        if let Some(i) = unwire { ed.level.wires.remove(i); }
        if ranges_changed { ed.level.ranges = ranges; }
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
    });
}

/// A port chip: an output ("Pressed ▸") or an input ("▸ Go") in its category colour.
fn port(ui: &mut Ui, text: &str, color: Color32) {
    egui::Frame::new().fill(color.gamma_multiply(0.22)).stroke(Stroke::new(1.0, color.gamma_multiply(0.7))).corner_radius(CornerRadius { nw: 8, ne: 8, se: 8, sw: 3 }).inner_margin(egui::Margin::symmetric(7, 2)).show(ui, |ui| {
        ui.label(egui::RichText::new(text).font(semi(12.0)).color(TEXT));
    });
}

fn column(ui: &mut Ui, w: f32, title: &str, add: impl FnOnce(&mut Ui)) {
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

fn overlays(ctx: &egui::Context, ed: &mut Editor, view: &View, now: f64) {
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

    // status island
    let rest = match view.mode {
        Mode::Editor => "Editor movement".to_string(),
        Mode::Builder => format!("Builder · fly {}", speed_text(view.speed)),
    };
    ed.island.draw(ctx, pos2(r.center().x, r.min.y + 12.0), &rest, &["Caps Lock"]);

    // speed strip (Builder movement, or for 2 s after a change)
    let recent = now - view.speed_changed < 2.0;
    if view.mode == Mode::Builder || recent {
        egui::Area::new(Id::new("speeds")).order(egui::Order::Foreground).fixed_pos(pos2(r.min.x + 14.0, r.max.y - 46.0)).interactable(false).show(ctx, |ui| {
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
    if ed.placing.is_none() && !ed.parts_open && ed.wiring.is_none() {
        let id = Id::new("hint-bar");
        let w_est = 120.0 * hints.len() as f32;
        let at = pos2(r.center().x - w_est / 2.0, r.max.y - 44.0);
        egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(pos2(at.x.max(r.min.x + 260.0), at.y)).interactable(false).show(ctx, |ui| {
            egui::Frame::new().fill(Color32::from_rgba_unmultiplied(10, 8, 9, 200)).corner_radius(R_CARD).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (label, keys) in hints {
                        theme::hint(ui, label, keys);
                        ui.add_space(6.0);
                    }
                });
            });
        });
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

    if let Some((at, text, color)) = ed.label.clone() {
        let keys: &[&str] = if ed.drag.is_some() { &[] } else { &["drag"] };
        theme::float_label(ctx, "widget", pos2(at.x, at.y), &text, color, keys);
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
    let mut sections: Vec<(&str, Color32, Vec<Item>, bool)> = vec![("Shapes", theme::CAT_SHAPES, shapes, true)];
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
        ui.add(egui::Label::new(egui::RichText::new("Lets an AI assistant on this computer, such as Claude Code, read your project, look at parts and their settings, and run Beverly's actions. Everything it changes can be undone.").color(MUTED)).wrap());
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
        Shape::Model | Shape::PlayerStart | Shape::Interact => {}
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
