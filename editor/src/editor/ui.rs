//! The 2D parts of the editor: the user's framed layout (frame.rs, docs/ui.svg) in the Motion kit's style: the
//! menus and tabs along the top, the Assets card, the three bottom panels with their drawers, the parts pop-up
//! (Tab), the status island, the speed strip, hints, and the unsaved-changes question.
use crate::level::{self, Shape, Wire};
use crate::settings::{self, Setting};
use crate::update::{self, Status};
use super::theme::{self, highlight, INK, LINE, MUTED, PANEL, R_CARD, TEXT, TEXT_2, Tip, Tone, mono, semi, white};
use super::view::{Mode, SPEED_LABELS, View};
use super::{actions, audio, frame, mcp, Editor, LevelMode, Pending};

use bevy::prelude::*;
use bevy_egui::egui::{self, Align2, Color32, CornerRadius, Id, Rect as ERect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use bevy_egui::{EguiContexts, egui::emath::TSTransform};

mod assets;
mod chrome;
mod foliage;
mod modals;
mod overlays;
mod panels;
mod parts;
mod widgets;
mod audio_ui;
use assets::*;
use chrome::*;
use foliage::*;
use modals::*;
use overlays::*;
use panels::*;
use parts::*;
use widgets::*;

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
    if ed.controls_open { controls_modal(&ctx, &mut ed); }
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

pub(super) fn open_folder(dir: &std::path::Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let cmd = "explorer";
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let cmd = "xdg-open";
    std::process::Command::new(cmd).arg(dir).spawn().map(|_| ())
}

fn body(size: f32) -> egui::FontId { theme::body(size) }
