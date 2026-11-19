//! The colour picker: a saturation square, hue and opacity strips, the eyedropper.
use super::*;

/// Temp data: the colour picker (its popup id) whose eyedropper is out; the editor's view answers it.
pub const EYEDROPPER: &str = "eyedropper";
/// Temp data: (the picker's popup id, the colour picked in the view).
pub const EYEDROPPED: &str = "eyedropped";

pub(super) fn rgb3(x: u32) -> Color32 { Color32::from_rgb((x >> 16) as u8, (x >> 8) as u8, x as u8) }

/// The opacity strip: the colour from clear to solid over a checker, with a thumb; drag to set `a` (0..1).
pub(super) fn opacity_strip(ui: &mut Ui, a: &mut f32, c: Color32) -> bool {
    let (strip, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 12.0), Sense::click_and_drag());
    let p = ui.painter().with_clip_rect(strip);
    p.rect_filled(strip, CornerRadius::same(6), rgb3(0x26262B));
    let n = (strip.width() / 6.0).ceil() as i32;
    for i in 0..n {
        for j in 0..2 {
            if (i + j) % 2 == 0 { continue; }
            let q = egui::Rect::from_min_size(pos2(strip.left() + i as f32 * 6.0, strip.top() + j as f32 * 6.0), vec2(6.0, 6.0));
            p.rect_filled(q.intersect(strip.shrink2(vec2(3.0, 0.0))), CornerRadius::ZERO, rgb3(0x3A3A40));
        }
    }
    let mut mesh = egui::Mesh::default();
    let clear = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 0);
    for (x, k) in [(strip.left(), clear), (strip.right(), c)] {
        mesh.colored_vertex(pos2(x, strip.top()), k);
        mesh.colored_vertex(pos2(x, strip.bottom()), k);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    p.add(mesh);
    let before = *a;
    if let Some(q) = resp.interact_pointer_pos() { *a = ((q.x - strip.left()) / strip.width()).clamp(0.0, 1.0); }
    let at = pos2(strip.left() + strip.width() * *a, strip.center().y);
    ui.painter().circle_filled(at, 8.0, c);
    ui.painter().circle_stroke(at, 8.0, Stroke::new(2.5, Color32::WHITE));
    resp.tip("Opacity: clear at the left, solid at the right");
    *a != before
}

/// The picker: a square of saturation (across) and brightness (up), and a strip of hues under it.
pub(super) fn picker(ui: &mut Ui, c: &mut [f32; 3]) -> bool {
    use bevy::color::{Color, Hsva};
    let id = ui.id().with("hsv");
    let rgb = |h: [f32; 3]| { let s = Color::from(Hsva::new(h[0], h[1], h[2], 1.0)).to_srgba(); [s.red, s.green, s.blue] };
    let col = |c: [f32; 3]| Color32::from_rgb((c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8);
    // the hue is kept while the colour is grey (where any hue gives it)
    let mut hsv: [f32; 3] = ui.data(|d| d.get_temp(id)).filter(|h| col(rgb(*h)) == col(*c)).unwrap_or_else(|| {
        let h = Hsva::from(Color::srgb(c[0], c[1], c[2]));
        [h.hue, h.saturation, h.value]
    });
    let before = hsv;
    let w = ui.available_width();
    let (sq, resp) = ui.allocate_exact_size(vec2(w, 150.0), Sense::click_and_drag());
    let n = 16;
    let mut mesh = egui::Mesh::default();
    for j in 0..=n {
        for i in 0..=n {
            let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
            mesh.colored_vertex(pos2(sq.left() + sq.width() * u, sq.top() + sq.height() * v), col(rgb([hsv[0], u, 1.0 - v])));
            if i < n && j < n {
                let k = (j * (n + 1) + i) as u32;
                mesh.add_triangle(k, k + 1, k + n as u32 + 1);
                mesh.add_triangle(k + 1, k + n as u32 + 2, k + n as u32 + 1);
            }
        }
    }
    ui.painter().add(mesh);
    if let Some(p) = resp.interact_pointer_pos() {
        hsv[1] = ((p.x - sq.left()) / sq.width()).clamp(0.0, 1.0);
        hsv[2] = 1.0 - ((p.y - sq.top()) / sq.height()).clamp(0.0, 1.0);
    }
    let at = pos2(sq.left() + sq.width() * hsv[1], sq.top() + sq.height() * (1.0 - hsv[2]));
    ui.painter().circle_stroke(at, 8.0, Stroke::new(2.0, Color32::WHITE));
    ui.painter().circle_stroke(at, 9.5, Stroke::new(1.0, Color32::from_black_alpha(90)));
    // the hues
    let (strip, resp) = ui.allocate_exact_size(vec2(w, 12.0), Sense::click_and_drag());
    let mut mesh = egui::Mesh::default();
    for k in 0..=12 {
        let u = k as f32 / 12.0;
        let x = strip.left() + strip.width() * u;
        let c = col(rgb([360.0 * u, 1.0, 1.0]));
        mesh.colored_vertex(pos2(x, strip.top()), c);
        mesh.colored_vertex(pos2(x, strip.bottom()), c);
        if k < 12 { let q = 2 * k as u32; mesh.add_triangle(q, q + 1, q + 2); mesh.add_triangle(q + 1, q + 3, q + 2); }
    }
    ui.painter().add(mesh);
    if let Some(p) = resp.interact_pointer_pos() { hsv[0] = 360.0 * ((p.x - strip.left()) / strip.width()).clamp(0.0, 0.9999); }
    let hx = pos2(strip.left() + strip.width() * hsv[0] / 360.0, strip.center().y);
    ui.painter().circle_filled(hx, 8.0, col(rgb([hsv[0], 1.0, 1.0])));
    ui.painter().circle_stroke(hx, 8.0, Stroke::new(2.0, Color32::WHITE));
    ui.data_mut(|d| d.insert_temp(id, hsv));
    if hsv != before { *c = rgb(hsv); return true; }
    false
}
