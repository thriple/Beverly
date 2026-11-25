//! Pieces the panels share: groups and their headers, the identity row, part kinds and their icons.
use super::*;

/// A sound file's name: "assets/sounds/birds.bev" is "birds".
pub(super) fn sound_name(path: &str) -> String { std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default() }

/// A group of settings in the dock: a line across, its heading (an icon in its colour, its name, a chevron to
/// fold it) and its contents. `title` is a settings section (or one of the panels' own); its look comes from
/// [`group_look`].
pub(super) fn column(ui: &mut Ui, _w: f32, title: &str, add: impl FnOnce(&mut Ui)) { group(ui, title, None, add); }

/// How a group looks: its name, icon and colour.
pub(super) fn group_look(title: &str) -> (String, &'static str, Color32) {
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
        crate::curve::FILL => ("Fill", "drop", theme::VIOLET),
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
pub(super) fn group(ui: &mut Ui, title: &str, count: Option<usize>, add: impl FnOnce(&mut Ui)) {
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
pub(super) fn identity(ui: &mut Ui, colour: Color32, icon: &str, name: Result<&mut String, &str>, sub: &str, actions: &mut [(&str, &str, &[&str], &mut bool)]) {
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
pub(super) fn project_palette(l: &level::Level) -> Vec<[f32; 3]> {
    let mut seen: Vec<([u8; 3], [f32; 3], usize)> = vec![];
    for p in l.parts.iter().filter(|p| !p.shape.is_marker() && p.shape != Shape::PlayerStart) {
        let k = p.color.map(|x| (x * 255.0).round() as u8);
        match seen.iter_mut().find(|s| s.0 == k) { Some(s) => s.2 += 1, None => seen.push((k, p.color, 1)) }
    }
    seen.sort_by_key(|s| std::cmp::Reverse(s.2));
    seen.into_iter().take(12).map(|s| s.1).collect()
}

/// A part's kind: its list section, icon and category colour.
pub(super) fn kind(shape: Shape) -> (&'static str, &'static str, Color32) {
    match shape {
        s if s.is_light() => ("Lights", "bulb", theme::CAT_LIGHTS),
        s if s.is_camera() => ("Cameras", "camera", theme::CAT_CAMERAS),
        Shape::PlayerStart => ("Game", "person", theme::CAT_GAME),
        Shape::Interact | Shape::Zone => ("Triggers", "bolt", theme::CAT_TRIGGERS),
        Shape::Sound => ("Actions", "play", theme::CAT_ACTIONS),
        Shape::Model => ("Models", "cube", theme::CAT_SHAPES),
        _ => ("Shapes", "cube", theme::CAT_SHAPES),
    }
}

/// While placing: how the wheel has turned and sized it, when it has.
pub(super) fn placed_how(ed: &Editor) -> String {
    let mut s = String::new();
    let deg = ed.place_turn.to_degrees().rem_euclid(360.0).round();
    if deg != 0.0 && deg != 360.0 { s += &format!(" · turned {deg}°"); }
    if (ed.place_scale - 1.0).abs() > 1e-3 { s += &format!(" · ×{:.2}", ed.place_scale); }
    s
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
        Shape::Model | Shape::Sound | Shape::Zone | Shape::PlayerStart | Shape::Interact | Shape::PointLight | Shape::SpotLight | Shape::FixedCamera | Shape::OrbitCamera | Shape::FollowCamera => {}
    }
}
