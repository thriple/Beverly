//! The tools that read the project and the level: what is in it, what is selected, and the paths and wires.
use super::*;

pub(super) fn project_tools(ed: &mut Editor, name: &str, args: &Value) -> Result<Value, String> {
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
            "change_stamp": ed.level.stamps().iter().max(),
            "testing": ed.testing.is_some(),
            "tests": crate::editor::mcp::jobs::tests_in(&ed.dir),
            "beverly": crate::update::this_build().map_or("a build made by hand".into(), |n| format!("build {n}")),
        })),
        "get_level" => Ok(describe_level(ed)),
        "find_path" => {
            let a = args.get("from").and_then(vec3_of).ok_or("from is [x, y, z]")?;
            let b = args.get("to").and_then(vec3_of).ok_or("to is [x, y, z]")?;
            let f = ed.finder.as_ref().ok_or("The navigation area isn't built yet (run_action level.build_navigation)")?;
            let found = f.path(a, b);
            // shown in the view, as Try a path shows it
            ed.path_try = Some(crate::editor::PathTry { from: Some(a), to: Some(b), found: Some(found.clone()) });
            let p = found?;
            let corners: Vec<Value> = p.iter().map(|c| json!(settings::nums(&((*c * 100.0).round() / 100.0).to_array()))).collect();
            Ok(json!({"length_m": settings::num((crate::nav::length(&p) * 100.0).round() / 100.0), "corners": corners, "out_of_date": ed.nav_out_of_date()}))
        }
        "change_level" => {
            editable(ed)?;
            if args.get("settings").is_none() && args.get("navigation").is_none() && args.get("walker_sounds").is_none() { return Err("Give settings, such as {\"Interior\": true}, or navigation, such as {\"Step\": 0.3}".into()); }
            let mut w = ed.level.walker.clone();
            if let Some(map) = args.get("walker_sounds") {
                let map = map.as_object().ok_or("walker_sounds is an object such as {\"Step\": \"footsteps\"} (null for none)")?;
                for (e, v) in map {
                    let key = crate::level::WalkerSounds::EVENTS.into_iter().find(|x| x.eq_ignore_ascii_case(e)).ok_or_else(|| format!("The walker has no {e} (it has: Step, Jump, Land)"))?;
                    *w.get_mut(key).expect("one of its events") = match v.as_str() { Some(s) => sound_ref(ed, s)?, None => String::new() };
                }
            }
            let mut l = ed.level.lighting.clone();
            let mut changed = match args.get("settings") { Some(set) => settings::apply(&mut *l, set)?, None => vec![] };
            let mut n = ed.level.navigation.clone();
            if let Some(set) = args.get("navigation") { changed.extend(settings::apply(&mut *n, set)?); }
            ed.level.lighting = l;
            ed.level.navigation = n;
            if w != ed.level.walker { changed.push("walker_sounds"); ed.level.walker = w; }
            ed.commit();
            Ok(json!({"changed": changed, "level": describe_level(ed)}))
        }
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
            let mut v: serde_json::Map<String, Value> = ed.assets.iter().map(|(f, files)| (f.to_string(), json!(files))).collect();
            v.insert("materials".into(), json!(ed.level.materials.keys().map(|k| k.trim_start_matches("assets/materials/")).collect::<Vec<_>>()));
            // sound files as the editor holds them, then the recordings
            let recordings: Vec<String> = ed.assets.iter().find(|(f, _)| *f == "sounds").map(|(_, t)| t.iter().filter(|t| crate::level::is_recording(t)).cloned().collect()).unwrap_or_default();
            v.insert("sounds".into(), json!(ed.level.sounds.keys().map(|k| k.trim_start_matches("assets/sounds/").to_string()).chain(recordings).collect::<Vec<_>>()));
            Ok(Value::Object(v))
        }
        "get_log" => Ok(json!(ed.island.history().iter().collect::<Vec<_>>())),
        "list_part_kinds" => {
            let mut kinds: Vec<Shape> = Shape::ALL.to_vec();
            kinds.extend([Shape::Interact, Shape::PlayerStart, Shape::Model, Shape::PointLight, Shape::SpotLight, Shape::Sound]);
            kinds.extend(Shape::CAMERAS);
            let template = Part::placed(Shape::Box, None, "Box 1".into(), [0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0], Shape::Box.default_size());
            Ok(json!({
                "kinds": kinds.iter().map(|k| json!({"kind": k.label(), "what": k.description(), "size": settings::nums(&k.default_size()), "outputs": k.outputs()})).collect::<Vec<_>>(),
                "settings": settings::describe(&template),
            }))
        }
        _ => unreachable!("routed in `call`"),
    }
}
