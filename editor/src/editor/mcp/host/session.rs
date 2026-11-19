//! The editor's session: the camera, jobs and actions.
use super::*;

pub(super) fn session_tools(ed: &mut Editor, view: &mut View, name: &str, args: &Value, exit: &mut MessageWriter<AppExit>) -> Result<Value, String> {
    match name {
        "get_camera" => {
            let (yaw, pitch) = (view.yaw.to_degrees(), view.pitch.to_degrees());
            Ok(json!({"position": settings::nums(&view.pos.to_array()), "looking": settings::nums(&view.forward().to_array()), "yaw": settings::num((yaw * 10.0).round() / 10.0), "pitch": settings::num((pitch * 10.0).round() / 10.0)}))
        }
        "move_camera" => {
            if let Some(f) = args.get("frame") {
                let id = part_ref(ed, Some(f))?;
                let p = ed.level.get(id).expect("found");
                let r = Vec3::from(p.size).length();
                if let Some(at) = args.get("position").and_then(vec3_of) { view.pos = at; }
                let c = Vec3::from(p.pos);
                let d = (c - view.pos).normalize_or(view.forward());
                look(view, d);
                view.pos = c - d * (r * 1.6 + 2.0);
            } else {
                if let Some(at) = args.get("position") { view.pos = vec3_of(at).ok_or("position is [x, y, z]")?; }
                if let Some(t) = args.get("look_at") {
                    let t = vec3_of(t).ok_or("look_at is [x, y, z]")?;
                    look(view, (t - view.pos).normalize_or(view.forward()));
                }
                if let Some(y) = args.get("yaw").and_then(Value::as_f64) { view.yaw = (y as f32).to_radians(); }
                if let Some(p) = args.get("pitch").and_then(Value::as_f64) { view.pitch = (p as f32).clamp(-89.0, 89.0).to_radians(); }
            }
            call(ed, view, "get_camera", &json!({}), exit)
        }
        "run_job" => {
            if ed.dirty() { return Err("There are unsaved changes: jobs work on the saved files, so save first (run_action file.save)".into()); }
            let player = ed.player_path().ok_or(format!("Couldn't find the game player ({} should be next to the editor)", ed.player_exe()))?;
            let id = match args.get("kind").and_then(Value::as_str) {
                Some("test") => {
                    let all = crate::editor::mcp::jobs::tests_in(&ed.dir);
                    let names = match args.get("test").and_then(Value::as_str) {
                        Some(n) if all.iter().any(|t| t == n) => vec![n.to_string()],
                        Some(n) => return Err(format!("No test called {n} (there are: {})", list_or_none(&all.iter().map(String::as_str).collect::<Vec<_>>()))),
                        None if all.is_empty() => return Err("The project has no tests (tests/*.ron)".into()),
                        None => all,
                    };
                    ed.mcp.jobs.tests(player, ed.dir.clone(), names)
                }
                Some("export") => {
                    let out = ed.dir.join("exports").join(format!("{}.zip", ed.name));
                    std::fs::create_dir_all(out.parent().expect("has one")).map_err(|e| e.to_string())?;
                    ed.mcp.jobs.export(player, ed.dir.clone(), ed.name.clone(), ed.bakes.pack(), out)
                }
                _ => return Err("kind is \"test\" or \"export\"".into()),
            };
            Ok(json!({"job": id, "check_with": "get_job"}))
        }
        "get_job" => {
            let id = args.get("id").and_then(Value::as_u64).ok_or("Give the job's id")? as u32;
            ed.mcp.jobs.get(id).map(|j| j.describe(true)).ok_or(format!("No job {id} (list_jobs)"))
        }
        "list_jobs" => Ok(Value::Array(ed.mcp.jobs.all().iter().map(|j| j.describe(false)).collect())),
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
        _ => unreachable!("routed in `call`"),
    }
}
