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
            "tests": super::jobs::tests_in(&ed.dir),
            "beverly": crate::update::this_build().map_or("a build made by hand".into(), |n| format!("build {n}")),
        })),
        "get_level" => Ok(describe_level(ed)),
        "find_path" => {
            let a = args.get("from").and_then(vec3_of).ok_or("from is [x, y, z]")?;
            let b = args.get("to").and_then(vec3_of).ok_or("to is [x, y, z]")?;
            let f = ed.finder.as_ref().ok_or("The navigation area isn't built yet (run_action level.build_navigation)")?;
            let found = f.path(a, b);
            // shown in the view, as Try a path shows it
            ed.path_try = Some(super::super::PathTry { from: Some(a), to: Some(b), found: Some(found.clone()) });
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
            let mut changed = match args.get("settings") { Some(set) => settings::apply(&mut l, set)?, None => vec![] };
            let mut n = ed.level.navigation.clone();
            if let Some(set) = args.get("navigation") { changed.extend(settings::apply(&mut n, set)?); }
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
        "add_part" => {
            editable(ed)?;
            let want = args.get("kind").and_then(Value::as_str).ok_or("Give the kind (list_part_kinds)")?.to_lowercase();
            let mut kinds: Vec<Shape> = Shape::ALL.to_vec();
            kinds.extend([Shape::Interact, Shape::PlayerStart, Shape::Model, Shape::PointLight, Shape::SpotLight, Shape::Sound]);
            kinds.extend(Shape::CAMERAS);
            let shape = *kinds.iter().find(|k| k.label().to_lowercase() == want).ok_or_else(|| {
                format!("No kind called \"{want}\" (there are: {})", kinds.iter().map(|k| k.label()).collect::<Vec<_>>().join(", "))
            })?;
            let model = match shape {
                Shape::Model => {
                    let m = args.get("model").and_then(Value::as_str).ok_or("A Model needs its file: model, such as assets/models/tower.glb")?;
                    if !ed.dir.join(m).is_file() { return Err(format!("No file {m} in the project (list_assets)")); }
                    Some(m.replace('\\', "/"))
                }
                _ => None,
            };
            let size = shape.default_size();
            let pos = match args.get("position") {
                Some(p) => vec3_of(p).ok_or("position is [x, y, z]")?,
                None => {
                    // where the view looks, standing on what is there (a model's origin sits on it)
                    let ray = Ray3d::new(view.pos, Dir3::new(view.forward()).unwrap_or(Dir3::NEG_Z));
                    let (at, n) = ed.hit(ray).map(|h| (h.point, h.normal)).unwrap_or((view.pos + view.forward() * 10.0, Vec3::Y));
                    // out from the face it lands on by half its size that way (a model's origin sits on it)
                    if model.is_some() { at } else { at + n * (n.abs().dot(Vec3::from(size)) * 0.5) }
                }
            };
            // named after its file: a model's, or the sound a Sound part plays
            let file = model.as_deref().or(args.get("sound").and_then(Value::as_str).filter(|_| shape == Shape::Sound));
            let base = match file {
                Some(m) => std::path::Path::new(m).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| shape.label().into()),
                None => shape.label().to_string(),
            };
            // a light shines away from the surface it stands on (a part placed in the view's line)
            let up = match args.get("position") {
                None if shape.is_light() => ed.hit(Ray3d::new(view.pos, Dir3::new(view.forward()).unwrap_or(Dir3::NEG_Z))).map_or(Vec3::Y, |h| h.normal),
                _ => Vec3::Y,
            };
            let mut p = Part::placed(shape, model, ed.level.fresh_name(&base), pos.to_array(), Quat::from_rotation_arc(Vec3::Y, up).to_array(), size);
            // a camera looks the way the view does: at eye height on what the view looks at, or where it is put
            if shape.is_camera() {
                let ground = match args.get("position") { Some(_) => pos - Vec3::Y * crate::editor::camera::EYE, None => pos - Vec3::Y * size[1] * 0.5 };
                crate::editor::camera::place(&mut p, ground, view.yaw);
            }
            if let Some(set) = args.get("settings") { settings::apply(&mut p, set)?; }
            aim(&mut p, args)?;
            plays(ed, &mut p, args)?;
            let id = ed.level.add(p);
            ed.selected = Some(id);
            ed.placing = None;
            ed.commit();
            Ok(describe(ed, ed.level.get(id).expect("just added")))
        }
        "change_part" => {
            editable(ed)?;
            let mut p = part_of(ed, args)?.clone();
            let set = args.get("settings");
            if ["settings", "aim", "sound", "sounds", "sounds_like"].iter().all(|k| args.get(*k).is_none()) { return Err("Give settings, such as {\"Colour\": \"#ff8000\"}".into()); }
            let mut changed = match set { Some(set) => settings::apply(&mut p, set)?, None => vec![] };
            if aim(&mut p, args)? { changed.push("rot"); }
            if plays(ed, &mut p, args)? { changed.push("sound"); }
            if let Some(map) = args.get("sounds") {
                let map = map.as_object().ok_or("sounds is an object of event → sound, such as {\"Pressed\": \"click\"} (null for none)")?;
                let events = p.events();
                for (e, v) in map {
                    let e = *events.iter().find(|x| x.eq_ignore_ascii_case(e)).ok_or_else(|| format!("{} has no event {e} (it has: {})", p.name, list_or_none(&events)))?;
                    let sound = match v.as_str() { Some(s) => sound_ref(ed, s)?, None => String::new() };
                    p.sounds.retain(|h| h.on != e);
                    if !sound.is_empty() { p.sounds.push(crate::level::Hook { on: e.into(), sound }); }
                }
                changed.push("sounds");
            }
            if let Some(g) = args.get("sounds_like") {
                p.sounds_like = match g.as_str() { Some(g) => Some(ground_ref(g)?), None => None };
                changed.push("sounds_like");
            }
            let id = p.id;
            *ed.level.get_mut(id).expect("found above") = p;
            settle_instances(ed, args)?;
            // a part that stopped moving keeps no wires to its inputs
            let inputs = ed.level.get(id).map(|p| p.inputs()).unwrap_or_default();
            let outputs = ed.level.get(id).map(|p| p.outputs()).unwrap_or_default();
            ed.level.wires.retain(|w| (w.to != id || inputs.contains(&w.input.as_str())) && (w.from != id || outputs.contains(&w.out.as_str())));
            ed.commit();
            Ok(json!({"changed": changed, "part": describe(ed, ed.level.get(id).expect("found above"))}))
        }
        "delete_part" => {
            editable(ed)?;
            let (id, name) = part_of(ed, args).map(|p| (p.id, p.name.clone()))?;
            ed.level.parts.retain(|p| p.id != id);
            ed.level.wires.retain(|w| w.from != id && w.to != id);
            if ed.selected == Some(id) { ed.selected = None; }
            ed.commit();
            Ok(json!({"deleted": {"id": id, "name": name}, "parts": ed.level.parts.len()}))
        }
        "wire" => {
            editable(ed)?;
            let from = part_ref(ed, args.get("from"))?;
            let to = part_ref(ed, args.get("to"))?;
            let out = args.get("output").and_then(Value::as_str).ok_or("Give the output")?;
            let input = args.get("input").and_then(Value::as_str).ok_or("Give the input")?;
            let (out, input) = {
                let (f, t) = (ed.level.get(from).expect("found"), ed.level.get(to).expect("found"));
                let o = f.outputs().into_iter().find(|o| o.eq_ignore_ascii_case(out))
                    .ok_or_else(|| format!("{} has no output {out} (it has: {})", f.name, list_or_none(&f.outputs())))?;
                let i = t.inputs().iter().find(|i| i.eq_ignore_ascii_case(input))
                    .ok_or_else(|| format!("{} has no input {input} (it has: {}; a part gets Go, Back and Toggle once it moves; a Sound part takes Play and Stop)", t.name, list_or_none(t.inputs())))?;
                (o.to_string(), i.to_string())
            };
            let w = Wire { from, out, to, input };
            let removing = args.get("remove").and_then(Value::as_bool) == Some(true);
            let had = ed.level.wires.contains(&w);
            if removing {
                if !had { return Err("There's no such wire (list_wires)".into()); }
                ed.level.wires.retain(|x| x != &w);
            } else if !had {
                ed.level.wires.push(w);
            }
            ed.commit();
            Ok(json!({"wires": ed.level.wires.len(), "removed": removing, "already_there": had && !removing}))
        }
        "lay_texture" => {
            editable(ed)?;
            let p = part_of(ed, args)?;
            let id = p.id;
            let names = ed.geo.shapes.get(&p.shape).map(|t| t.face_names()).unwrap_or_default();
            let face = match args.get("face").and_then(Value::as_str) {
                None => None,
                Some(f) if f.eq_ignore_ascii_case("all") => None,
                Some(f) => Some(names.iter().position(|n| n.eq_ignore_ascii_case(f)).ok_or_else(|| format!("{} has no face {f} (it has: {}, or all)", p.name, names.join(", ")))? as u8),
            };
            let pair = |k: &str| -> Result<Option<[f32; 2]>, String> {
                args.get(k).map(|v| v.as_array().filter(|a| a.len() == 2).and_then(|a| Some([a[0].as_f64()? as f32, a[1].as_f64()? as f32])).ok_or(format!("{k} is [across, down] in metres"))).transpose()
            };
            let (tile, offset) = (pair("tile")?, pair("offset")?);
            if tile.is_some_and(|t| t.iter().any(|x| *x < 0.05)) { return Err("tile is at least 0.05 m each way".into()); }
            if args.get("remove").and_then(Value::as_bool) == Some(true) {
                let p = ed.level.get_mut(id).expect("found above");
                let before = p.surfaces.len();
                p.surfaces.retain(|s| s.face != face);
                if p.surfaces.len() == before { return Err("There's no texture there".into()); }
            } else if let Some(m) = args.get("material").and_then(Value::as_str) {
                let m = material_ref(ed, m)?;
                ed.lay_texture(id, face, &m)?;
            } else if let Some(t) = args.get("texture").and_then(Value::as_str) {
                let t = texture_ref(ed, t)?;
                ed.lay_texture(id, face, &t)?;
            } else if !ed.level.get(id).is_some_and(|p| p.surfaces.iter().any(|s| s.face == face)) {
                return Err("Give a texture or material (there's none there to change)".into());
            }
            if let Some(s) = ed.level.get_mut(id).and_then(|p| p.surfaces.iter_mut().find(|s| s.face == face)) {
                if let Some(t) = tile { s.tile = t; }
                if let Some(o) = offset { s.offset = o; }
            }
            settle_instances(ed, args)?;
            ed.commit();
            Ok(describe(ed, ed.level.get(id).expect("found above")))
        }
        "get_sound" => {
            let path = sound_ref(ed, args.get("sound").and_then(Value::as_str).ok_or("Give the sound")?)?;
            Ok(describe_sound(ed, &path))
        }
        "change_sound" => {
            editable(ed)?;
            let path = sound_ref(ed, args.get("sound").and_then(Value::as_str).ok_or("Give the sound")?)?;
            let mut m = ed.level.sounds[&path].clone();
            let mut changed = match args.get("settings") { Some(set) => settings::apply(&mut m, set)?, None => vec![] };
            if let Some(c) = args.get("recordings") {
                m.clips = recordings(ed, c)?;
                changed.push("recordings");
            }
            if let Some(sets) = args.get("sets") {
                set_sets(ed, &mut m, sets)?;
                changed.push("sets");
            }
            ed.level.sounds.insert(path.clone(), m);
            ed.commit();
            Ok(json!({"changed": changed, "sound": describe_sound(ed, &path)}))
        }
        "new_sound" => {
            editable(ed)?;
            let mut m = match args.get("copy_of").and_then(Value::as_str) {
                Some(c) => ed.level.sounds[&sound_ref(ed, c)?].clone(),
                None => crate::level::Sound::default(),
            };
            if let Some(c) = args.get("recordings") { m.clips = recordings(ed, c)?; }
            if let Some(sets) = args.get("sets") { set_sets(ed, &mut m, sets)?; }
            if let Some(set) = args.get("settings") { settings::apply(&mut m, set)?; }
            let name = args.get("name").and_then(Value::as_str).unwrap_or("Sound").replace(['/', '\\'], " ");
            let path = ed.level.new_sound(&name, m);
            ed.commit();
            Ok(describe_sound(ed, &path))
        }
        "play_sound" => {
            let path = match (args.get("sound").and_then(Value::as_str), args.get("part")) {
                (Some(s), _) => sound_ref(ed, s)?,
                (None, Some(r)) => {
                    let p = ed.level.get(part_ref(ed, Some(r))?).expect("found");
                    p.sound.as_ref().map(|s| s.sound.clone()).filter(|s| !s.is_empty()).ok_or_else(|| format!("{} plays no sound", p.name))?
                }
                _ => return Err("Give the sound (or a Sound part)".into()),
            };
            let ground = match args.get("ground").and_then(Value::as_str) {
                Some(g) => Some(ground_ref(g)?),
                None => None,
            };
            if ed.level.sounds[&path].clips_on(ground).is_empty() { return Err(format!("{path} has no recordings yet (change_sound)")); }
            ed.preview_sound = Some((path.clone(), ground));
            Ok(json!({"playing": path, "on": ground.map(|g| g.label())}))
        }
        "paint_stroke" => {
            editable(ed)?;
            let p = part_of(ed, args)?;
            let (id, name) = (p.id, p.name.clone());
            if !crate::paint::paintable(p) { return Err(format!("{name} can't be painted (the brush paints visible basic shapes)")); }
            if args.get("clear").and_then(Value::as_bool) == Some(true) {
                ed.level.get_mut(id).expect("found above").strokes.clear();
                ed.commit();
                return Ok(json!({"part": name, "strokes": 0}));
            }
            let erase = args.get("erase").and_then(Value::as_bool) == Some(true);
            let material = match (erase, args.get("material").and_then(Value::as_str)) {
                (true, _) => String::new(),
                (false, Some(m)) => material_ref(ed, m)?,
                (false, None) => return Err("Give the material to paint (or erase: true)".into()),
            };
            let points: Vec<Vec3> = args.get("points").and_then(Value::as_array).map(|a| a.iter().filter_map(vec3_of).collect()).unwrap_or_default();
            if points.is_empty() { return Err("Give points: [[x, y, z], ...] on its surface, in the world".into()); }
            let num = |k: &str, d: f32, lo: f32, hi: f32| args.get(k).and_then(Value::as_f64).map_or(d, |v| (v as f32).clamp(lo, hi));
            let b = ed.brush.clone();
            let lays = match args.get("lays").and_then(Value::as_str) {
                None => b.lays,
                Some("add") => crate::level::Lays::Add,
                Some("subtract") => crate::level::Lays::Subtract,
                Some("set") => crate::level::Lays::Set,
                Some(o) => return Err(format!("lays is add, subtract or set, not {o}")),
            };
            if erase && lays != crate::level::Lays::Add { return Err("Erasing takes every material away; give lays only when painting".into()); }
            let mut rules = b.rules;
            if let Some(r) = args.get("rules") {
                if let Some(v) = r.get("slope").and_then(Value::as_f64) { rules.slope = (v as f32).clamp(0.0, 90.0); }
                match r.get("band") {
                    Some(Value::Null) => rules.band = None,
                    Some(v) => {
                        let lh: Vec<f32> = v.as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect()).unwrap_or_default();
                        let [lo, hi] = lh[..] else { return Err("band is [lowest, highest] in metres, or null".into()) };
                        rules.band = Some([lo.min(hi), lo.max(hi)]);
                    }
                    None => {}
                }
                match r.get("on") {
                    Some(Value::Null) => rules.on = None,
                    Some(v) => {
                        let want = v.as_str().unwrap_or_default().to_lowercase().replace([' ', '_'], "");
                        rules.on = Some(crate::level::Tag::ALL.into_iter().find(|t| format!("{t:?}").to_lowercase() == want).ok_or_else(|| format!("No surface tag called {v} (Climbable, Diggable, Water, Grind rail, Perch spot, Slippery, Ground, Paths ignore)"))?);
                    }
                    None => {}
                }
            }
            if let Some(t) = rules.on {
                if !ed.level.get(id).expect("found above").tags.contains(&t) { return Err(format!("{name} isn't tagged {t:?}, and the rules paint only on parts that are")); }
            }
            let p = ed.level.get_mut(id).expect("found above");
            let local = points.iter().map(|w| p.to_local(*w).to_array()).collect();
            p.strokes.push(crate::level::Stroke { material, size: num("size", b.size, 0.05, 500.0), hardness: num("hardness", b.hardness, 0.0, 1.0), strength: num("strength", b.strength, 0.0, 1.0), points: local, lays, rules, curve: None });
            settle_instances(ed, args)?;
            ed.commit();
            Ok(describe(ed, ed.level.get(id).expect("found above")))
        }
        "list_shapes" => Ok(Value::Array(ed.level.curves.iter().map(describe_curve).collect())),
        "draw_shape" => {
            editable(ed)?;
            let existing = match args.get("shape") {
                None | Some(Value::Null) => None,
                Some(v) => Some(ed.level.curves.iter().find(|c| v.as_u64() == Some(c.id) || v.as_str() == Some(c.name.as_str())).map(|c| c.id).ok_or_else(|| format!("No curve {v} (list_shapes shows them)"))?),
            };
            if args.get("delete").and_then(Value::as_bool) == Some(true) {
                let id = existing.ok_or("Give the shape to delete")?;
                ed.level.curves.retain(|c| c.id != id);
                if ed.pen.curve == Some(id) { ed.pen.curve = None; ed.pen.drawing = false; }
                super::super::pen::strips(ed);
                ed.commit();
                return Ok(json!({"deleted": id}));
            }
            let given = args.get("points").and_then(Value::as_array);
            if given.is_none() && existing.is_none() { return Err("Give points: [[x, y, z], ...] (or {at, handle_in, handle_out} for a smooth point)".into()); }
            let points: Option<Vec<crate::curve::PenPoint>> = match given {
                None => None,
                Some(a) => {
                    let mut v = vec![];
                    for (i, p) in a.iter().enumerate() {
                        let v3 = |k: &str| p.get(k).map(|h| vec3_of(h).ok_or_else(|| format!("point {i}: {k} is [x, y, z]"))).transpose();
                        v.push(match vec3_of(p) {
                            Some(at) => crate::curve::PenPoint::corner(at),
                            None if p.is_object() => crate::curve::PenPoint {
                                at: v3("at")?.ok_or_else(|| format!("point {i}: give at: [x, y, z]"))?.to_array(),
                                into: v3("handle_in")?.unwrap_or_default().to_array(),
                                out: v3("handle_out")?.unwrap_or_default().to_array(),
                                stretch: match p.get("stretch") {
                                    None | Some(Value::Null) => crate::curve::Stretch::Ground,
                                    Some(w) => crate::curve::Stretch::ALL.into_iter().find(|s| w.as_str().is_some_and(|w| stretch_word(*s) == w.to_lowercase())).ok_or_else(|| format!("point {i}: stretch is ground, over, air or fall"))?,
                                },
                            },
                            None => return Err(format!("point {i} is [x, y, z] or {{at, handle_in, handle_out}}")),
                        });
                    }
                    if v.len() < 2 { return Err("A curve needs at least two points".into()); }
                    Some(v)
                }
            };
            let closed = args.get("closed").and_then(Value::as_bool);
            // a strip: {material, width, edge, strength}, or null to take it away
            let strip: Option<Option<crate::curve::Strip>> = match args.get("strip") {
                None => None,
                Some(Value::Null) => Some(None),
                Some(v) => {
                    let old = existing.and_then(|id| ed.level.curve(id)).and_then(|c| c.strip.clone());
                    let mut st = old.unwrap_or_default();
                    let num = |k: &str, lo: f32, hi: f32, cur: f32| v.get(k).map_or(Ok(cur), |n| n.as_f64().map(|n| (n as f32).clamp(lo, hi)).ok_or_else(|| format!("strip {k} is a number")));
                    if let Some(m) = v.get("material").and_then(Value::as_str) { st.material = material_ref(ed, m)?; }
                    if st.material.is_empty() { st.material = ed.level.materials.keys().next().cloned().ok_or("No materials yet: new_material makes one")?; }
                    (st.size, st.hardness, st.strength) = (num("width", 0.1, 50.0, st.size)?, num("edge", 0.0, 1.0, st.hardness)?, num("strength", 0.05, 1.0, st.strength)?);
                    Some(Some(st))
                }
            };
            let id = match existing {
                Some(id) => {
                    let c = ed.level.curve_mut(id).expect("found above");
                    if let Some(p) = points { c.points = p; }
                    if let Some(k) = closed { c.closed = k; }
                    if c.points.len() < 3 { c.closed = false; }
                    if let Some(st) = strip { c.strip = st; }
                    id
                }
                None => {
                    let id = ed.level.add_curve(points.unwrap_or_default(), closed.unwrap_or(false));
                    let c = ed.level.curve_mut(id).expect("just added");
                    if c.points.len() < 3 { c.closed = false; }
                    if let Some(n) = args.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()) { c.name = n.to_string(); }
                    if let Some(st) = strip { c.strip = st; }
                    id
                }
            };
            // the strip's strokes are made now, so the curve and they are one undo step
            super::super::pen::lay(ed);
            super::super::pen::strips(ed);
            ed.commit();
            Ok(describe_curve(ed.level.curve(id).expect("found")))
        }
        "get_material" => {
            let path = material_ref(ed, args.get("material").and_then(Value::as_str).ok_or("Give the material")?)?;
            Ok(describe_material(ed, &path))
        }
        "change_material" => {
            editable(ed)?;
            let path = material_ref(ed, args.get("material").and_then(Value::as_str).ok_or("Give the material")?)?;
            let mut m = ed.level.materials[&path].clone();
            let mut changed = match args.get("settings") { Some(set) => settings::apply(&mut m, set)?, None => vec![] };
            if let Some(g) = args.get("sounds_like") {
                m.sounds_like = match g.as_str() { Some(g) => Some(ground_ref(g)?), None => None };
                changed.push("sounds_like");
            }
            if let Some(t) = args.get("texture") {
                m.texture = match t.as_str() { Some(t) => Some(texture_ref(ed, t)?), None => None };
                changed.push("texture");
            }
            if let Some(s) = args.get("shader") {
                m.shader = match s.as_str() { Some(s) => Some(shader_ref(ed, s)?), None => None };
                m.shader_values.clear();
                changed.push("shader");
            }
            if let Some(given) = args.get("shader_values") {
                let shader = m.shader.clone().ok_or("The material has no shader to give values to (set shader first)")?;
                let src = std::fs::read_to_string(ed.dir.join(&shader)).map_err(|e| format!("{shader}: {e}"))?;
                m.shader_values = shader_values(&crate::shader::parse(&src)?, given)?;
                changed.push("shader_values");
            }
            ed.level.materials.insert(path.clone(), m);
            ed.commit();
            Ok(json!({"changed": changed, "material": describe_material(ed, &path)}))
        }
        "get_shader" => {
            let path = shader_ref(ed, args.get("shader").and_then(Value::as_str).ok_or("Give the shader")?)?;
            let src = std::fs::read_to_string(ed.dir.join(&path)).map_err(|e| format!("{path}: {e}"))?;
            let users: Vec<&str> = ed.level.materials.iter().filter(|(_, m)| m.shader.as_deref() == Some(path.as_str())).map(|(k, _)| k.trim_start_matches("assets/materials/")).collect();
            let mut v = shader_json(&src, &std::collections::BTreeMap::new());
            v["shader"] = json!(path);
            v["source"] = json!(src);
            v["used_by"] = json!(users);
            // what is wrong with it now (known once a material uses it): its header, or code the graphics card refused
            v["problem"] = json!(ed.shader_info.get(&path).and_then(|i| i.1.clone()));
            Ok(v)
        }
        "write_shader" => {
            editable(ed)?;
            let name = args.get("name").and_then(Value::as_str).ok_or("Give the name")?;
            let name = name.trim_end_matches(".wesl");
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') { return Err(format!("{name:?} can't be a shader name (letters, digits, - and _)")); }
            let src = args.get("source").and_then(Value::as_str).ok_or("Give the source")?;
            crate::shader::parse(src)?;
            let path = format!("assets/shaders/{name}.wesl");
            let existed = ed.dir.join(&path).is_file();
            std::fs::create_dir_all(ed.dir.join("assets/shaders")).map_err(|e| e.to_string())?;
            std::fs::write(ed.dir.join(&path), src).map_err(|e| format!("{path}: {e}"))?;
            ed.refresh_assets();
            ed.island.show("✦", format!("{} {name}.wesl", if existed { "Rewrote" } else { "Wrote" }), "a shader", crate::editor::theme::Tone::Accent);
            Ok(json!({"shader": path, "written": !existed, "replaced": existed}))
        }
        "new_material" => {
            editable(ed)?;
            let mut m = match args.get("copy_of").and_then(Value::as_str) {
                Some(c) => ed.level.materials[&material_ref(ed, c)?].clone(),
                None => crate::level::Material::default(),
            };
            if let Some(t) = args.get("texture").and_then(Value::as_str) { m.texture = Some(texture_ref(ed, t)?); }
            if let Some(set) = args.get("settings") { settings::apply(&mut m, set)?; }
            let name = args.get("name").and_then(Value::as_str).unwrap_or("Material").replace(['/', '\\'], " ");
            let path = ed.level.new_material(&name, m);
            ed.commit();
            Ok(describe_material(ed, &path))
        }
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
                    let all = super::jobs::tests_in(&ed.dir);
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
        _ => Err(format!("No tool called {name}")),
    }
}

