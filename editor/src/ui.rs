//! The 2D parts of the editor, in the Motion kit's style: top bar with the File menu, the collapsible Assets
//! panel, the bottom properties panel, the parts pop-up (Tab), the status island, the speed strip, hints, and the
//! unsaved-changes question.
use crate::level::{Collision, Shape};
use crate::theme::{self, ACCENT, INK, LINE, MUTED, PANEL, R_CARD, TEXT, Tone, mono, semi, white};
use crate::view::{Mode, SPEED_LABELS, View};
use crate::{Editor, Pending};
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
    let mut root = Ui::new(ctx.clone(), "root".into(), egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(ctx.viewport_rect()));

    top_bar(&mut root, &mut ed, &mut exit);
    assets_panel(&mut root, &mut ed);
    bottom_panel(&mut root, &mut ed);
    let r = root.available_rect_before_wrap();
    view.rect = Rect::new(r.left(), r.top(), r.right(), r.bottom());

    overlays(&ctx, &mut ed, &view, time.elapsed_secs_f64());
    if ed.parts_open { parts_popup(&ctx, &mut ed); } else { theme::spring_reset(&ctx, Id::new("parts-pop")); }
    if let Some(what) = ed.pending { unsaved_modal(&ctx, &mut ed, what, &mut exit); }
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
                let item = |ui: &mut Ui, label: &str, keys: &str| ui.add(egui::Button::new(label).shortcut_text(keys).frame(false)).clicked();
                if item(ui, "New project", "Ctrl+N") {
                    ed.request(Pending::New, exit);
                    ui.close();
                }
                if item(ui, "Open project…", "Ctrl+O") {
                    ed.request(Pending::Open, exit);
                    ui.close();
                }
                ui.separator();
                if item(ui, "Save", "Ctrl+S") {
                    ed.save();
                    ui.close();
                }
                if item(ui, "Save as…", "Ctrl+Shift+S") {
                    ed.save_as();
                    ui.close();
                }
                if item(ui, "Show project folder", "") {
                    let _ = open_folder(&ed.dir);
                    ui.close();
                }
                ui.separator();
                if item(ui, "Quit", "Ctrl+Q") {
                    ed.request(Pending::Quit, exit);
                    ui.close();
                }
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
                if ui.add_enabled(!ed.redo.is_empty(), egui::Button::new("↷ Redo")).on_hover_ui(|ui| theme::hint(ui, "Redo", &["Ctrl", "Y"])).clicked() { ed.redo(); }
                if ui.add_enabled(!ed.undo.is_empty(), egui::Button::new("↶ Undo")).on_hover_ui(|ui| theme::hint(ui, "Undo", &["Ctrl", "Z"])).clicked() { ed.undo(); }
                ui.add_space(10.0);
                if ui.button("Parts").on_hover_ui(|ui| theme::hint(ui, "Open the parts pop-up", &["Tab"])).clicked() {
                    ed.parts_open = true;
                    ed.parts_query.clear();
                }
            });
        });
    });
}

fn open_folder(dir: &std::path::Path) -> std::io::Result<()> {
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
                        ui.label(egui::RichText::new(f).size(12.0));
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
    egui::Panel::bottom("props").exact_size(168.0).resizable(false).frame(frame).show(root, |ui| {
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
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
            column(ui, 230.0, "PART", |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                    ui.painter().rect_filled(r, CornerRadius::same(2), theme::CAT_SHAPES);
                    ui.add(egui::TextEdit::singleline(&mut p.name).desired_width(180.0).font(semi(13.0)));
                });
                ui.label(egui::RichText::new(format!("{} {}", p.shape.glyph(), p.shape.label())).color(MUTED));
                ui.horizontal(|ui| {
                    if ui.button("Duplicate").on_hover_ui(|ui| theme::hint(ui, "Duplicate", &["Ctrl", "D"])).clicked() { ed.duplicate_selected(); }
                    if ui.button("Delete").on_hover_ui(|ui| theme::hint(ui, "Delete", &["Del"])).clicked() { delete = true; }
                });
            });
            column(ui, 250.0, "POSITION  ·  ROTATION", |ui| {
                xyz(ui, &mut p.pos, 0.05, -100000.0, " m");
                let q = Quat::from_array(p.rot);
                let (yaw, pitch, roll) = q.to_euler(EulerRot::YXZ);
                let mut deg = yaw.to_degrees();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Turn").color(MUTED));
                    if ui.add(egui::DragValue::new(&mut deg).speed(1.0).suffix("°")).changed() {
                        p.rot = Quat::from_euler(EulerRot::YXZ, deg.to_radians(), pitch, roll).to_array();
                    }
                    if (pitch.abs() > 1e-3 || roll.abs() > 1e-3) && ui.small_button("Stand upright").clicked() {
                        p.rot = Quat::from_rotation_y(yaw).to_array();
                    }
                });
            });
            column(ui, 250.0, "SIZE", |ui| {
                xyz(ui, &mut p.size, 0.02, 0.02, " m");
                ui.label(egui::RichText::new("Width · height · depth").color(MUTED).size(11.0));
            });
            column(ui, 330.0, "LOOK  ·  COLLISION", |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Colour").color(MUTED));
                    ui.color_edit_button_rgb(&mut p.color);
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new("Visible").color(MUTED));
                    theme::toggle(ui, &mut p.visible).on_hover_text("Off: an invisible collider (shown hatched in the editor only)");
                });
                let mut c = Collision::ALL.iter().position(|x| *x == p.collision).unwrap_or(0);
                let labels: Vec<&str> = Collision::ALL.iter().map(|c| c.label()).collect();
                if theme::segmented(ui, "collision", &labels, &mut c, false) { p.collision = Collision::ALL[c]; }
            });
        });
        if delete {
            ed.delete_selected();
        } else if p != before {
            if let Some(m) = ed.level.get_mut(id) { *m = p; }
        }
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

fn xyz(ui: &mut Ui, v: &mut [f32; 3], speed: f64, min: f32, suffix: &str) {
    ui.horizontal(|ui| {
        for (i, c) in [theme::AXIS_X, theme::AXIS_Y, theme::AXIS_Z].iter().enumerate() {
            ui.label(egui::RichText::new(["X", "Y", "Z"][i]).font(mono(11.0)).color(*c));
            ui.add(egui::DragValue::new(&mut v[i]).speed(speed).range(min..=100000.0).max_decimals(2).suffix(suffix));
        }
    });
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
    if ed.placing.is_none() {
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
    } else {
        let shape = ed.placing.unwrap();
        theme::float_label(ctx, "placing", pos2(r.center().x - 170.0, r.max.y - 48.0), &format!("Placing {} · click a surface", shape.label()), ACCENT, &["Shift", "Esc"]);
    }

    if let Some((at, text, color)) = ed.label.clone() {
        let keys: &[&str] = if ed.drag.is_some() { &[] } else { &["drag"] };
        theme::float_label(ctx, "widget", pos2(at.x, at.y), &text, color, keys);
    }
}

fn speed_text(i: usize) -> String {
    let s = crate::view::SPEEDS[i];
    if s < 1.0 { format!("{}×", SPEED_LABELS[i]) } else { format!("{s}×") }
}

fn parts_popup(ctx: &egui::Context, ed: &mut Editor) {
    let id = Id::new("parts-pop");
    let screen = ctx.viewport_rect();
    // scrim: click outside to close
    egui::Area::new(id.with("scrim")).order(egui::Order::Middle).fixed_pos(screen.min).show(ctx, |ui| {
        let r = ui.allocate_rect(screen, Sense::click());
        ui.painter().rect_filled(screen, CornerRadius::ZERO, Color32::from_black_alpha(150));
        if r.clicked() { ed.parts_open = false; }
    });
    // POP_IN: scale .92 → 1, y 8 → 0, fade in (SNAPPY)
    let t = theme::spring_from(ctx, id, 0.0, 1.0, theme::SNAPPY);
    let scale = 0.92 + 0.08 * t;
    let w = 660.0;
    let center = screen.center();
    let layer = egui::LayerId::new(egui::Order::Foreground, id);
    ctx.set_transform_layer(layer, TSTransform::new((center.to_vec2()) * (1.0 - scale) + vec2(0.0, 8.0 * (1.0 - t)), scale));
    let mut pick: Option<Shape> = None;
    let mut soon: Option<&str> = None;
    let (tab, esc, enter) = ctx.input(|i| (i.key_pressed(egui::Key::Tab), i.key_pressed(egui::Key::Escape), i.key_pressed(egui::Key::Enter)));
    egui::Area::new(id).order(egui::Order::Foreground).pivot(Align2::CENTER_CENTER).fixed_pos(center).show(ctx, |ui| {
        ui.multiply_opacity(t.clamp(0.0, 1.0));
        egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, LINE)).corner_radius(R_CARD).inner_margin(egui::Margin::same(18)).shadow(ctx.global_style().visuals.window_shadow).show(ui, |ui| {
            ui.set_width(w);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Parts").font(semi(16.0)));
                ui.add_space(8.0);
                let te = ui.add(egui::TextEdit::singleline(&mut ed.parts_query).hint_text("Type to search…").desired_width(360.0));
                if !te.has_focus() && ed.parts_query.is_empty() { te.request_focus(); }
                ui.add_space(8.0);
                theme::kbd(ui, "Tab");
                ui.label(egui::RichText::new("or").color(MUTED));
                theme::kbd(ui, "Esc");
                ui.label(egui::RichText::new("to close").color(MUTED));
            });
            ui.add_space(10.0);
            let q = ed.parts_query.to_lowercase();
            let matches = |label: &str| q.is_empty() || label.to_lowercase().contains(&q);
            let shapes: Vec<(&str, &str, Option<Shape>)> = Shape::ALL.iter().filter(|s| matches(s.label())).map(|s| (s.glyph(), s.label(), Some(*s))).collect();
            if enter {
                if let Some((_, _, Some(s))) = shapes.first() { pick = Some(*s); }
            }
            let col_w = (w - 24.0) / 3.0;
            egui::Grid::new("sections").num_columns(3).spacing(vec2(12.0, 14.0)).show(ui, |ui| {
                let mut n = 0;
                let all: Vec<(&str, Color32, Vec<(&str, &str, Option<Shape>)>, bool)> = std::iter::once(("Shapes", theme::CAT_SHAPES, shapes.clone(), true))
                    .chain(SCRIPT_SECTIONS.iter().map(|s| (s.name, s.color, s.items.iter().filter(|(_, l)| matches(l)).map(|(g, l)| (*g, *l, None)).collect(), s.ready)))
                    .collect();
                for (name, color, items, ready) in all {
                    if items.is_empty() { continue; }
                    ui.allocate_ui_with_layout(vec2(col_w, 10.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.set_width(col_w);
                        // colour-coded section bar
                        let (br, _) = ui.allocate_exact_size(vec2(col_w, 22.0), Sense::hover());
                        ui.painter().rect_filled(br, CornerRadius { nw: 6, ne: 6, se: 6, sw: 2 }, color);
                        ui.painter().text(br.left_center() + vec2(9.0, 0.0), Align2::LEFT_CENTER, name, semi(12.0), INK);
                        if !ready { ui.painter().text(br.right_center() - vec2(9.0, 0.0), Align2::RIGHT_CENTER, "v0.2", mono(10.0), INK); }
                        ui.add_space(4.0);
                        let sq = (col_w - 3.0 * 6.0) / 4.0;
                        let rows = items.chunks(4);
                        for row in rows {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                for (glyph, label, shape) in row {
                                    if part_square(ui, sq, glyph, label, color, ready) {
                                        match shape {
                                            Some(s) => pick = Some(*s),
                                            None => soon = Some(label),
                                        }
                                    }
                                }
                            });
                        }
                    });
                    n += 1;
                    if n % 3 == 0 { ui.end_row(); }
                }
            });
        });
    });
    if let Some(s) = pick {
        ed.placing = Some(s);
        ed.parts_open = false;
        ed.island.show(s.glyph(), format!("Placing {}", s.label()), "Click a surface · Shift keeps it upright · Esc cancels", Tone::Accent);
    } else if let Some(l) = soon {
        ed.island.show("…", format!("{l} arrives with script parts"), "v0.2: triggers, actions, flow, logic", Tone::Plain);
    }
    if (tab && !ed.parts_fresh) || esc { ed.parts_open = false; }
    ed.parts_fresh = false;
}

/// One square in the parts pop-up: glyph and name; the category colour lights the border on hover.
fn part_square(ui: &mut Ui, size: f32, glyph: &str, label: &str, color: Color32, ready: bool) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let hot = spring_hover(ui, resp.id, resp.hovered());
    let p = ui.painter();
    let fill = theme::lerp_color(white(10), white(24), hot);
    p.rect_filled(r, CornerRadius::same(10), fill);
    let stroke = if resp.hovered() { color } else { LINE };
    p.rect_stroke(r, CornerRadius::same(10), Stroke::new(if resp.hovered() { 1.5 } else { 1.0 }, stroke), StrokeKind::Inside);
    let a = if ready { 255 } else { 120 };
    let tc = Color32::from_rgba_unmultiplied(TEXT.r(), TEXT.g(), TEXT.b(), a);
    p.text(r.center() - vec2(0.0, 8.0 + 2.0 * hot), Align2::CENTER_CENTER, glyph, body(20.0), tc);
    p.text(r.center() + vec2(0.0, 16.0), Align2::CENTER_CENTER, label, body(10.5), if ready { white(200) } else { white(110) });
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

fn spring_hover(ui: &Ui, id: Id, on: bool) -> f32 { theme::spring(ui.ctx(), id.with("hover"), if on { 1.0 } else { 0.0 }, theme::SNAPPY).clamp(0.0, 1.2) }

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
