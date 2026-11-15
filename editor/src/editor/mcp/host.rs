//! The MCP tools, run on the main thread against the editor (once a frame, in `serve`).
use super::protocol::Output;
use crate::editor::actions::{self, ACTIONS};
use crate::editor::view::View;
use crate::editor::Editor;
use crate::level::{Part, Shape, Wire};
use crate::settings;
use bevy::prelude::*;
use serde_json::{json, Value};

/// A screenshot on its way: taken a couple of frames after it is asked for, so the last changes are drawn.
pub struct Shot {
    reply: std::sync::mpsc::Sender<Result<Output, String>>,
    /// The camera made for a view from elsewhere (none: the window).
    cam: Option<(Entity, Handle<Image>)>,
    width: u32,
    frames: u8,
}

/// Answer the calls that arrived since the last frame.
pub fn serve(mut ed: ResMut<Editor>, mut view: ResMut<View>, mut exit: MessageWriter<AppExit>, mut commands: Commands, mut images: ResMut<Assets<Image>>, mut shots: Local<Vec<Shot>>) {
    take_shots(&mut commands, &mut shots);
    let calls = ed.mcp.take_calls();
    for c in calls {
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
        "list_part_kinds" => {
            let mut kinds: Vec<Shape> = Shape::ALL.to_vec();
            kinds.extend([Shape::Interact, Shape::PlayerStart, Shape::Model]);
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
            kinds.extend([Shape::Interact, Shape::PlayerStart, Shape::Model]);
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
            let base = match &model {
                Some(m) => std::path::Path::new(m).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Model".into()),
                None => shape.label().to_string(),
            };
            let mut p = Part::placed(shape, model, ed.level.fresh_name(&base), pos.to_array(), [0.0, 0.0, 0.0, 1.0], size);
            if let Some(set) = args.get("settings") { settings::apply(&mut p, set)?; }
            let id = ed.level.add(p);
            ed.selected = Some(id);
            ed.placing = None;
            ed.commit();
            Ok(describe(ed, ed.level.get(id).expect("just added")))
        }
        "change_part" => {
            editable(ed)?;
            let mut p = part_of(ed, args)?.clone();
            let set = args.get("settings").ok_or("Give settings, such as {\"Colour\": \"#ff8000\"}")?;
            let changed = settings::apply(&mut p, set)?;
            let id = p.id;
            *ed.level.get_mut(id).expect("found above") = p;
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
                    .ok_or_else(|| format!("{} has no input {input} (it has: {}; a part gets Go, Back and Toggle once it moves)", t.name, list_or_none(t.inputs())))?;
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

/// Edits wait while testing or while the user is dragging something (so their drag stays one undo step).
fn editable(ed: &Editor) -> Result<(), String> {
    if ed.testing.is_some() { return Err("Not while testing (run_action is refused too; the user presses ` to go back to editing)".into()); }
    if ed.drag.is_some() { return Err("The user is dragging something; try again in a moment".into()); }
    Ok(())
}

fn list_or_none(x: &[&str]) -> String { if x.is_empty() { "none".into() } else { x.join(", ") } }

fn vec3_of(v: &Value) -> Option<Vec3> {
    let a = v.as_array().filter(|a| a.len() == 3)?;
    Some(Vec3::new(a[0].as_f64()? as f32, a[1].as_f64()? as f32, a[2].as_f64()? as f32))
}

/// Turn the view to look along `d`.
fn look(view: &mut View, d: Vec3) {
    view.yaw = (-d.x).atan2(-d.z);
    view.pitch = d.y.clamp(-1.0, 1.0).asin().clamp(-89f32.to_radians(), 89f32.to_radians());
}

/// A part given as an id or a name.
fn part_ref(ed: &Editor, v: Option<&Value>) -> Result<u64, String> {
    match v {
        Some(Value::Number(n)) => n.as_u64().and_then(|id| ed.level.get(id)).map(|p| p.id).ok_or(format!("No part with id {n}")),
        Some(Value::String(n)) => ed.level.parts.iter().find(|p| &p.name == n).map(|p| p.id).ok_or(format!("No part named \"{n}\"")),
        _ => Err("Give a part's id or name".into()),
    }
}

/// Start a screenshot: of the window, or from a camera made for it. Returns the camera and the width.
fn shot(ed: &Editor, view: &View, args: &Value, commands: &mut Commands, images: &mut Assets<Image>) -> Result<(Option<(Entity, Handle<Image>)>, u32), String> {
    let width = args.get("width").and_then(Value::as_u64).unwrap_or(1024).clamp(256, 1920) as u32;
    let Some(from) = args.get("from") else { return Ok((None, width)) };
    let from = vec3_of(from).ok_or("from is [x, y, z]")?;
    let target = match args.get("look_at") {
        Some(t) => vec3_of(t).ok_or("look_at is [x, y, z]")?,
        None => match ed.selected.and_then(|id| ed.level.get(id)) {
            Some(p) => Vec3::from(p.pos),
            None if !ed.level.parts.is_empty() => ed.level.parts.iter().map(|p| Vec3::from(p.pos)).sum::<Vec3>() / ed.level.parts.len() as f32,
            None => from + view.forward(),
        },
    };
    if (target - from).length() < 1e-3 { return Err("from and look_at are the same point".into()); }
    let up = if (target - from).normalize().y.abs() > 0.999 { Vec3::Z } else { Vec3::Y };
    let (w, h) = (width, width * 9 / 16);
    use bevy::render::render_resource::TextureFormat;
    let image = images.add(Image::new_target_texture(w, h, TextureFormat::Rgba8UnormSrgb, None));
    let cam = commands.spawn((
        Camera3d::default(),
        Camera { order: -1, ..default() },
        bevy::camera::RenderTarget::Image(image.clone().into()),
        Msaa::Off,
        Transform::from_translation(from).looking_at(target, up),
    )).id();
    Ok((Some((cam, image)), width))
}

/// Take the screenshots whose frames are up; each answers its call when the picture arrives.
fn take_shots(commands: &mut Commands, shots: &mut Vec<Shot>) {
    use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
    for s in shots.iter_mut() { s.frames = s.frames.saturating_sub(1); }
    let (ready, waiting): (Vec<Shot>, Vec<Shot>) = std::mem::take(shots).into_iter().partition(|s| s.frames == 0);
    *shots = waiting;
    for s in ready {
        let what = match &s.cam { Some((_, img)) => Screenshot::image(img.clone()), None => Screenshot::primary_window() };
        let (reply, cam, width) = (s.reply, s.cam.map(|c| c.0), s.width);
        commands.spawn(what).observe(move |ev: On<ScreenshotCaptured>, mut commands: Commands| {
            let _ = reply.send(png_of(&ev.image, width));
            if let Some(c) = cam { commands.entity(c).despawn(); }
        });
    }
}

/// The picture as a PNG at most `width` pixels wide.
fn png_of(img: &Image, width: u32) -> Result<Output, String> {
    let rgba = img.clone().try_into_dynamic().map_err(|e| format!("Couldn't read the picture: {e}"))?.to_rgba8();
    let (w, h) = rgba.dimensions();
    let rgba = if w > width { image::imageops::resize(&rgba, width, (h * width / w).max(1), image::imageops::FilterType::Triangle) } else { rgba };
    let (w, h) = rgba.dimensions();
    let mut png = vec![];
    image::DynamicImage::ImageRgba8(rgba).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(Output::Image { png, text: format!("{w} × {h}") })
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
