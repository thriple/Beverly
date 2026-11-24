//! The questions and windows: unsaved changes, Features, About, updates and the MCP server.
use super::*;

pub(super) fn unsaved_modal(ctx: &egui::Context, ed: &mut Editor, what: Pending, exit: &mut MessageWriter<AppExit>) {
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
pub(super) struct Roadmap { updated: String, features: Vec<Feature> }

#[derive(serde::Deserialize)]
pub(super) struct Feature {
    id: String, name: String, area: String, status: String, effort: String, needs: Vec<String>, what: String, chunks: Vec<String>, note: String, order: u32,
    /// When a built feature was finished (YYYY-MM-DD) and the release build it shipped in.
    #[serde(default)]
    built: String,
    #[serde(default)]
    build: u32,
}

/// "2026-11-15" as "15 Nov 2026" (or "15 Nov" when `short`).
pub(super) fn day(s: &str, short: bool) -> String {
    const M: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let p: Vec<&str> = s.split('-').collect();
    let (Some(y), Some(m), Some(d)) = (p.first(), p.get(1).and_then(|m| m.parse::<usize>().ok()).filter(|m| (1..=12).contains(m)), p.get(2).and_then(|d| d.parse::<u32>().ok())) else { return s.into() };
    if short { format!("{d} {}", M[m - 1]) } else { format!("{d} {} {y}", M[m - 1]) }
}

pub(super) fn roadmap() -> &'static Roadmap {
    static R: std::sync::OnceLock<Roadmap> = std::sync::OnceLock::new();
    R.get_or_init(|| serde_json::from_str(include_str!("../../../features.json")).expect("features.json"))
}

pub(super) fn features_modal(ctx: &egui::Context, ed: &mut Editor) {
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

pub(super) fn dialog_frame() -> egui::Frame { egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, LINE)).corner_radius(R_CARD).inner_margin(egui::Margin::same(22)) }

pub(super) fn this_build() -> String { update::this_build().map_or("a build made by hand".into(), |n| format!("build {n}")) }

pub(super) fn about_modal(ctx: &egui::Context, ed: &mut Editor) {
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
pub(super) fn updates_modal(ctx: &egui::Context, ed: &mut Editor) {
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
pub(super) fn mcp_modal(ctx: &egui::Context, ed: &mut Editor) {
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

/// Play > Controls: each action and the inputs that do it, as keycaps. Click a keycap to take it off; + waits for the
/// next key, mouse button or gamepad button pressed (see `capture_input`). Saved with the project; undo covers it.
pub(super) fn controls_modal(ctx: &egui::Context, ed: &mut Editor) {
    let mut close = false;
    let mut c = (*ed.level.controls).clone();
    let waiting = ed.controls_capture.clone();
    egui::Modal::new(Id::new("controls")).frame(dialog_frame()).show(ctx, |ui| {
        ui.set_width(560.0);
        ui.label(egui::RichText::new("Controls").font(semi(17.0)).color(TEXT));
        ui.add(egui::Label::new(egui::RichText::new("What players press for each action: keys, mouse buttons and a gamepad all work. Test mode and the exported game use these; the mouse always turns the view too.").color(MUTED)).wrap());
        ui.add_space(12.0);
        egui::Grid::new("controls-grid").num_columns(2).spacing(vec2(16.0, 8.0)).show(ui, |ui| {
            for a in &mut c.0 {
                let help = crate::input::Controls::help(&a.name);
                let name = ui.label(egui::RichText::new(&a.name).font(semi(13.0)).color(TEXT));
                if !help.is_empty() { name.tip(help); }
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let mut gone = None;
                    for (i, n) in a.inputs.iter().enumerate() {
                        let size = theme::kbd_size(ui, n);
                        let (r, resp) = ui.allocate_exact_size(size, Sense::click());
                        theme::paint_kbd(ui.painter(), r, n, if resp.hovered() { theme::DANGER } else { TEXT });
                        if resp.tip("Click to take it off").clicked() { gone = Some(i); }
                    }
                    if let Some(i) = gone { a.inputs.remove(i); }
                    if waiting.as_deref() == Some(a.name.as_str()) {
                        ui.label(egui::RichText::new("Press a key, mouse button or gamepad button…").color(look().accent));
                        theme::kbd(ui, "Esc");
                    } else if ui.small_button("+").tip("Add an input: click, then press it").clicked() {
                        ed.controls_capture = Some(a.name.clone());
                    }
                });
                ui.end_row();
            }
        });
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(!c.is_default(), egui::Button::new("Beverly's defaults")).tip("Every action back to the inputs Beverly starts with").clicked() {
                c = crate::input::Controls::default();
                ed.controls_capture = None;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                theme::kbd(ui, "Esc");
                if ui.add(egui::Button::new("Close").min_size(vec2(80.0, 32.0))).clicked() { close = true; }
            });
        });
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if waiting.is_some() { ed.controls_capture = None; } else { close = true; }
        }
    });
    if c != *ed.level.controls {
        *ed.level.controls = c;
        ed.commit();
    }
    if close {
        ed.controls_open = false;
        ed.controls_capture = None;
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
