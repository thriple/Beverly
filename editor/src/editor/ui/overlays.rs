//! What is drawn over the 3D view: the status pills, tags, the eyedropper, the pen's stretch button, the light budget and the wire menu.
use super::*;

/// The selection tag: a small pill in the highlight with the part's icon and name, at its top left in the view.
/// A colour picker's eyedropper is out: the part under the pointer shows its colour (a light's light, a material's
/// tint on that face, else the part's colour); a click in the view hands it to the picker, Esc puts it away.
pub(super) fn eyedropper(ctx: &egui::Context, ed: &Editor, view: &View, cam: (&Camera, &GlobalTransform)) {
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

pub(super) fn selection_tag(ctx: &egui::Context, ed: &Editor, view: &View, cam: (&Camera, &GlobalTransform)) {
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
pub(super) fn stage_tag(ctx: &egui::Context, ed: &Editor, view: &View, cam: (&Camera, &GlobalTransform)) {
    let Some(path) = ed.sound_open.as_ref().filter(|p| ed.level.sounds.contains_key(*p)) else { return };
    let top = if ed.stage.walker { audio::walker_at(ed.stage.turn) + Vec3::Y * 1.8 } else { audio::SPEAKER_AT + Vec3::Y * 0.6 };
    let Some(at) = view.to_screen(cam.0, cam.1, top) else { return };
    paint_tag(ctx, view, at - Vec2::new(24.0, 0.0), "wave", &sound_name(path), Id::new(("tag-in", path)));
}

/// The pen's stretch button: a small pill by the selected point with how the line leaving it treats what it meets
/// (Ground, Over everything, Air, Fall, or "· path" when it does as the path says); a click goes on to the next, and
/// after Fall back to following the path.
pub(super) fn stretch_button(ctx: &egui::Context, ed: &mut Editor, view: &View, cam: (&Camera, &GlobalTransform)) {
    if !ed.pen_on { return; }
    let (Some(id), Some(i)) = (ed.pen.curve, ed.pen.point) else { return };
    let Some(c) = ed.level.curve(id) else { return };
    let Some(point) = c.points.get(i) else { return };
    // the last point of an open curve has no stretch yet, unless the next point is about to go on
    if i + 1 == c.points.len() && !c.closed && !ed.pen.drawing { return; }
    let Some(at) = view.to_screen(cam.0, cam.1, point.pos()) else { return };
    let own = point.stretch;
    let name = match own { Some(s) => s.name().to_string(), None => format!("{} · path", c.path.stretch.name()) };
    let name = name.as_str();
    let w = 12.0 + text_w(ctx, name, theme::bold(12.0)) + 12.0;
    let v = view.rect;
    let pos = pos2((at.x + 16.0).clamp(v.min.x + 8.0, (v.max.x - w - 8.0).max(v.min.x)), (at.y - 38.0).clamp(v.min.y + 48.0, v.max.y - 60.0));
    let mut next = None;
    egui::Area::new(Id::new("stretch-button")).order(egui::Order::Middle).fixed_pos(pos).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
        let fill = if resp.hovered() { highlight() } else { highlight().gamma_multiply(0.85) };
        ui.painter().rect_filled(r, CornerRadius::same(7), fill);
        ui.painter().text(r.center(), Align2::CENTER_CENTER, name, theme::bold(12.0), look().highlight_text);
        let resp = resp.tip_keys("How the line leaving this point treats what it meets: Ground, Over everything, Air or Fall, or as the path says", &["Click"]);
        if resp.clicked() { next = Some(match own { None => Some(crate::curve::Stretch::Ground), Some(crate::curve::Stretch::Fall) => None, Some(s) => Some(s.next()) }); }
    });
    if let Some(s) = next {
        if let Some(p) = ed.level.curve_mut(id).and_then(|c| c.points.get_mut(i)) { p.stretch = s; }
    }
}

/// A tag: the small pill in the highlight with an icon and a name, its top left just above `at` (screen points),
/// kept in the view; it springs in.
pub(super) fn paint_tag(ctx: &egui::Context, view: &View, at: Vec2, icon: &str, name: &str, id: Id) {
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

pub(super) fn overlays(ctx: &egui::Context, ed: &mut Editor, view: &View, perf: &crate::perf::Perf, now: f64) {
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
        let f = crate::editor::camera::frame(r);
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
pub(super) fn stats_pill(ctx: &egui::Context, perf: &crate::perf::Perf, at: egui::Pos2) {
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
pub(super) fn light_budget(ctx: &egui::Context, ed: &mut Editor, at: egui::Pos2) {
    let id = Id::new("light-budget");
    let worst = crate::editor::light::budget(&ed.level);
    if worst.is_empty() { theme::spring_reset(ctx, id.with("fade")); return; }
    let (n, max) = (worst.len(), crate::editor::light::BUDGET);
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
pub(super) fn wire_menu(ctx: &egui::Context, ed: &mut Editor, from: u64, out: &'static str, to: u64, at: egui::Pos2) {
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

pub(super) fn speed_text(i: usize) -> String {
    let s = crate::editor::view::SPEEDS[i];
    if s < 1.0 { format!("{}×", SPEED_LABELS[i]) } else { format!("{s}×") }
}
