//! The tools that change parts: add, change, delete, wire and lay textures.
use super::*;

pub(super) fn parts_tools(ed: &mut Editor, view: &mut View, name: &str, args: &Value) -> Result<Value, String> {
    match name {
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
        _ => unreachable!("routed in `call`"),
    }
}
