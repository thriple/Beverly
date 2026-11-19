//! The parts pop-up (Tab).
use super::*;

/// A section of the parts pop-up. Only Shapes work in this version; the rest show what's coming.
pub(super) struct Section { name: &'static str, color: Color32, items: &'static [(&'static str, &'static str)], ready: bool }

pub(super) const SCRIPT_SECTIONS: [Section; 5] = [
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

pub(super) struct Item { glyph: &'static str, label: &'static str, shape: Option<Shape> }

/// The parts pop-up (Tab): a search field over three columns of colour-coded sections, each a grid of squares.
/// Pops in like the kit's palette (scale .92 → 1, rising 8 px, fading in; sections follow in a short stagger).
/// Arrow keys move through the shapes, Enter picks, Tab or Esc closes.
pub(super) fn parts_popup(ctx: &egui::Context, ed: &mut Editor) {
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
pub(super) fn section_bar(ui: &mut Ui, w: f32, name: &str, color: Color32, ready: bool, count: usize) {
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
pub(super) fn part_square(ui: &mut Ui, size: f32, glyph: &str, shape: Option<Shape>, label: &str, color: Color32, ready: bool, selected: bool) -> (bool, bool) {
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
