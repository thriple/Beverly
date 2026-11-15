//! The MCP tools, run on the main thread against the editor (once a frame, in `serve`).
use super::protocol::Output;
use crate::editor::actions::{self, ACTIONS};
use crate::editor::view::View;
use crate::editor::Editor;
use crate::level::Part;
use crate::settings;
use bevy::prelude::*;
use serde_json::{json, Value};

/// Answer the calls that arrived since the last frame.
pub fn serve(mut ed: ResMut<Editor>, mut view: ResMut<View>, mut exit: MessageWriter<AppExit>) {
    let calls = ed.mcp.take_calls();
    for c in calls {
        let r = call(&mut ed, &mut view, &c.name, &c.args, &mut exit);
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
        "get_project" => Ok(json!({
            "name": ed.name,
            "folder": ed.dir.display().to_string(),
            "level": ed.level.name,
            "parts": ed.level.parts.len(),
            "wires": ed.level.wires.len(),
            "selected": ed.selected.and_then(|id| ed.level.get(id)).map(|p| json!({"id": p.id, "name": p.name})),
            "unsaved_changes": ed.dirty(),
            "undo_steps": ed.undo.len(),
            "redo_steps": ed.redo.len(),
            "testing": ed.testing.is_some(),
            "beverly": crate::update::this_build().map_or("a build made by hand".into(), |n| format!("build {n}")),
        })),
        "list_parts" => {
            let q = args.get("query").and_then(Value::as_str).unwrap_or("").to_lowercase();
            let words: Vec<&str> = q.split_whitespace().collect();
            Ok(Value::Array(ed.level.parts.iter().filter(|p| {
                let hay = format!("{} {}", p.name, p.shape.label()).to_lowercase();
                words.iter().all(|w| hay.contains(w))
            }).map(summary).collect()))
        }
        "get_part" => part_of(ed, args).map(|p| describe(ed, p)),
        "get_selection" => Ok(match ed.selected.and_then(|id| ed.level.get(id)) {
            Some(p) => describe(ed, p),
            None => json!({"selected": null}),
        }),
        "select" => {
            if args.get("none").and_then(Value::as_bool) == Some(true) {
                ed.selected = None;
                return Ok(json!({"selected": null}));
            }
            let id = part_of(ed, args)?.id;
            ed.selected = Some(id);
            Ok(json!({"selected": id}))
        }
        "list_wires" => {
            let name = |id: u64| ed.level.get(id).map(|p| p.name.clone()).unwrap_or_default();
            Ok(Value::Array(ed.level.wires.iter().map(|w| json!({
                "from": {"id": w.from, "name": name(w.from)}, "output": w.out,
                "to": {"id": w.to, "name": name(w.to)}, "input": w.input,
            })).collect()))
        }
        "list_assets" => {
            ed.refresh_assets();
            Ok(Value::Object(ed.assets.iter().map(|(f, files)| (f.to_string(), json!(files))).collect()))
        }
        "get_log" => Ok(json!(ed.island.history().iter().collect::<Vec<_>>())),
        "list_actions" => {
            let q = args.get("query").and_then(Value::as_str).unwrap_or("").to_lowercase();
            Ok(Value::Array(ACTIONS.iter().filter(|a| {
                let hay = format!("{} {} {}", a.id, a.name, a.place).to_lowercase();
                q.split_whitespace().all(|w| hay.contains(w))
            }).map(|a| {
                let why = if a.dialog { Some("Opens a file dialog, so it needs a person") } else { actions::why_not(ed, a.id) };
                json!({"id": a.id, "name": a.name, "place": a.place, "keys": a.keys, "can_run": why.is_none(), "why_not": why})
            }).collect()))
        }
        "run_action" => {
            let id = args.get("id").and_then(Value::as_str).ok_or("Give the action's id")?;
            if actions::get(id).is_some_and(|a| a.dialog) { return Err(format!("{id} opens a file dialog, so it needs a person")); }
            actions::run(ed, Some(view), id, exit)?;
            ed.commit();
            Ok(json!({"ran": id, "unsaved_changes": ed.dirty(), "undo_steps": ed.undo.len(), "redo_steps": ed.redo.len(), "unsaved_question_open": ed.pending.is_some()}))
        }
        _ => Err(format!("No tool called {name}")),
    }
}

/// The part named by `id` or `name` in the arguments.
fn part_of<'a>(ed: &'a Editor, args: &Value) -> Result<&'a Part, String> {
    if let Some(id) = args.get("id").and_then(Value::as_u64) {
        return ed.level.get(id).ok_or(format!("No part with id {id}"));
    }
    if let Some(n) = args.get("name").and_then(Value::as_str) {
        return ed.level.parts.iter().find(|p| p.name == n).ok_or(format!("No part named \"{n}\""));
    }
    Err("Give the part's id or name".into())
}

fn summary(p: &Part) -> Value {
    let mut v = json!({"id": p.id, "name": p.name, "kind": p.shape.label(), "position": settings::nums(&p.pos), "moves": p.mover.is_some()});
    if p.model.is_some() {
        v["scale"] = settings::nums(&p.size);
        v["model"] = json!(p.model);
    } else {
        v["size"] = settings::nums(&p.size);
    }
    v
}

/// A part with every setting it has (from the same descriptions its panel is drawn from) and its wires.
fn describe(ed: &Editor, p: &Part) -> Value {
    let name = |id: u64| ed.level.get(id).map(|q| q.name.clone()).unwrap_or_default();
    let mut v = summary(p);
    v["settings"] = settings::describe(p);
    v["outputs"] = json!(p.outputs());
    v["inputs"] = json!(p.inputs());
    v["wires_out"] = Value::Array(ed.level.wires.iter().filter(|w| w.from == p.id).map(|w| json!({"output": w.out, "to": name(w.to), "to_id": w.to, "input": w.input})).collect());
    v["wires_in"] = Value::Array(ed.level.wires.iter().filter(|w| w.to == p.id).map(|w| json!({"input": w.input, "from": name(w.from), "from_id": w.from, "output": w.out})).collect());
    v
}
