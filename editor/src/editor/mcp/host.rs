//! The MCP tools, run on the main thread against the editor (once a frame, in `serve`).
use super::protocol::Output;
use crate::editor::actions::{self, ACTIONS};
use crate::editor::view::View;
use crate::editor::{Editor, TestRequest};
use crate::play::Play;
use crate::scripted::{input, Input};
use crate::world::{part_transform, PartId};
use bevy::input::mouse::AccumulatedMouseMotion;
use crate::level::{Part, Shape, Wire};
use crate::settings;
use bevy::prelude::*;
use serde_json::{json, Value};

mod project;
mod parts;
mod sounds;
mod paint;
mod materials;
mod session;
use project::*;
use parts::*;
use sounds::*;
use paint::*;
use materials::*;
use session::*;
mod args;
mod describe;
mod play;
mod shots;
use args::*;
use describe::*;
pub use play::*;
pub use shots::*;

/// Answer the calls that arrived since the last frame.
#[allow(clippy::too_many_arguments)]
pub fn serve(mut ed: ResMut<Editor>, mut view: ResMut<View>, mut exit: MessageWriter<AppExit>, mut commands: Commands, mut images: ResMut<Assets<Image>>, mut shots: Local<Vec<Shot>>, mut waiting: Local<Vec<Waiting>>, mut pl: Playing, gpu: Res<crate::gpu::GpuCaps>, perf: Res<crate::perf::Perf>) {
    take_shots(&mut commands, &mut shots);
    let now = pl.time.elapsed_secs_f64();
    for w in waiting.iter_mut() { w.frames = w.frames.saturating_sub(1); }
    let (ready, still): (Vec<Waiting>, Vec<Waiting>) = std::mem::take(&mut *waiting).into_iter().partition(|w| w.frames == 0 && w.until <= now);
    *waiting = still;
    for w in ready {
        let v = play_state(&ed, &pl);
        let _ = w.reply.send(Ok(Output::Text(serde_json::to_string_pretty(&v).unwrap_or_default())));
    }
    let calls = ed.mcp.take_calls();
    for c in calls {
        if matches!(c.name.as_str(), "start_test" | "stop_test" | "press_keys" | "blast" | "pull") {
            match playing(&mut ed, &view, &mut pl, &c.name, &c.args, now) {
                Ok(until) => {
                    ed.mcp.note(c.name.clone());
                    waiting.push(Waiting { reply: c.reply, until, frames: 3 });
                }
                Err(e) => {
                    ed.mcp.note(format!("{} ✕ {e}", c.name));
                    let _ = c.reply.send(Err(e));
                }
            }
            continue;
        }
        if c.name == "get_play" {
            let v = play_state(&ed, &pl);
            ed.mcp.note(c.name.clone());
            let _ = c.reply.send(Ok(Output::Text(serde_json::to_string_pretty(&v).unwrap_or_default())));
            continue;
        }
        if c.name == "screenshot" {
            match shot(&ed, &view, &c.args, &mut commands, &mut images) {
                Ok((cam, width)) => {
                    ed.mcp.note("screenshot".into());
                    shots.push(Shot { reply: c.reply, cam, width, frames: 2 });
                }
                Err(e) => {
                    ed.mcp.note(format!("screenshot ✕ {e}"));
                    let _ = c.reply.send(Err(e));
                }
            }
            continue;
        }
        let mut r = call(&mut ed, &mut view, &c.name, &c.args, &mut exit);
        if c.name == "get_project" && let Ok(v) = &mut r {
            v["graphics_card"] = json!({"name": gpu.name, "backend": gpu.backend, "kind": gpu.kind, "virtual_geometry": gpu.virtual_geometry()});
            v["performance"] = perf.json();
            let (entries, bytes) = ed.bakes.size();
            v["bake_cache"] = json!({"entries": entries, "megabytes": bytes as f64 / 1e6, "building": ed.bakes.running().iter().map(|(l, p)| json!({"what": l, "percent": (p * 100.0).round()})).collect::<Vec<_>>()});
        }
        let line = match &r {
            Ok(_) => c.name.clone(),
            Err(e) => format!("{} ✕ {e}", c.name),
        };
        ed.mcp.note(line);
        let _ = c.reply.send(r.map(|v| Output::Text(serde_json::to_string_pretty(&v).unwrap_or_default())));
    }
}

/// One tool call. The answer is JSON (sent as text); `Err` is a tool error the assistant sees.
pub fn call(ed: &mut Editor, view: &mut View, name: &str, args: &Value, exit: &mut MessageWriter<AppExit>) -> Result<Value, String> {
    match name {
        "get_project" | "get_level" | "find_path" | "change_level" | "list_parts" | "get_part" | "get_selection" | "select" | "list_wires" | "list_assets" | "get_log" | "list_part_kinds" => project_tools(ed, name, args),
        "add_part" | "change_part" | "delete_part" | "wire" | "lay_texture" => parts_tools(ed, view, name, args),
        "get_sound" | "change_sound" | "new_sound" | "play_sound" => sounds_tools(ed, name, args),
        "paint_stroke" | "list_shapes" | "draw_shape" | "bucket_fill" | "list_fills" => paint_tools(ed, name, args),
        "get_material" | "change_material" | "get_shader" | "write_shader" | "new_material" => materials_tools(ed, name, args),
        "get_camera" | "move_camera" | "run_job" | "get_job" | "list_jobs" | "list_actions" | "run_action" => session_tools(ed, view, name, args, exit),
        _ => Err(format!("No tool called {name}")),
    }
}

