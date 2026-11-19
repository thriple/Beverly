//! What the tools say back: parts, tags, sounds, materials, shaders, curves and the level, as JSON.
use super::*;

pub(super) fn summary(p: &Part) -> Value {
    let mut v = json!({"id": p.id, "name": p.name, "kind": p.shape.label(), "position": settings::nums(&p.pos), "moves": p.mover.is_some()});
    if p.model.is_some() {
        v["scale"] = settings::nums(&p.size);
        v["model"] = json!(p.model);
    } else {
        v["size"] = settings::nums(&p.size);
    }
    if !p.tags.is_empty() { v["tags"] = tags_of(p); }
    v
}

/// A part's surface tags by label, as its Tags setting describes them.
pub(super) fn tags_of(p: &Part) -> Value {
    settings::describe(p).as_array().and_then(|a| a.iter().find(|d| d["field"] == "tags")).map_or(Value::Null, |d| d["value"].clone())
}

pub(super) fn describe_sound(ed: &Editor, path: &str) -> Value {
    let m = &ed.level.sounds[path];
    let sets: serde_json::Map<String, Value> = m.sets.iter().map(|s| (s.ground.label().to_string(), json!(s.clips))).collect();
    json!({"sound": path, "recordings": m.clips, "on_surfaces": sets, "used_by_parts": ed.level.sound_users(path), "settings": settings::describe(m)})
}

pub(super) fn describe_material(ed: &Editor, path: &str) -> Value {
    let m = &ed.level.materials[path];
    let (surfaces, parts) = ed.level.material_users(path);
    let mut v = json!({"material": path, "texture": m.texture, "sounds_like": m.sounds_like.map(|g| g.label()), "used_by": {"surfaces": surfaces, "parts": parts}, "settings": settings::describe(m)});
    if let Some(s) = &m.shader {
        v["shader"] = json!(s);
        v["shader_settings"] = std::fs::read_to_string(ed.dir.join(s)).map_or(Value::Null, |src| shader_json(&src, &m.shader_values)["settings"].clone());
    }
    v
}

/// A shader file's header for the MCP server: its settings with their kinds, ranges, defaults and the values given.
pub(super) fn shader_json(src: &str, given: &std::collections::BTreeMap<String, Vec<f32>>) -> Value {
    use crate::shader::Kind;
    let h = match crate::shader::parse(src) { Ok(h) => h, Err(e) => return json!({"error": e, "settings": []}) };
    let values = crate::shader::values(&h, given);
    let settings: Vec<Value> = h.settings.iter().enumerate().map(|(i, s)| {
        let (kind, extra, value) = match s.kind {
            Kind::Number { min, max } => ("number", json!({"min": min, "max": max, "default": s.default[0]}), settings::num(values[i].x)),
            Kind::Colour => ("colour", json!({"default": settings::nums(&s.default)}), settings::nums(&[values[i].x, values[i].y, values[i].z])),
            Kind::Toggle => ("toggle", json!({"default": s.default[0] > 0.5}), json!(values[i].x > 0.5)),
        };
        let mut o = json!({"name": s.name, "label": s.label, "kind": kind, "value": value});
        o.as_object_mut().expect("an object").extend(extra.as_object().cloned().unwrap_or_default());
        o
    }).collect();
    json!({"settings": settings, "see_through": h.see_through, "vertex_offset": h.vertex_offset, "alpha_mask": h.alpha_mask})
}

/// A stretch as the MCP server says it.
pub(super) fn stretch_word(s: crate::curve::Stretch) -> &'static str {
    match s { crate::curve::Stretch::Ground => "ground", crate::curve::Stretch::Over => "over", crate::curve::Stretch::Air => "air", crate::curve::Stretch::Fall => "fall" }
}

pub(super) fn describe_curve(c: &crate::curve::Curve) -> Value {
    let round = |v: Vec3| settings::nums(&v.to_array());
    json!({
        "id": c.id, "name": c.name, "closed": c.closed, "strip": c.strip.as_ref().map(|s| json!({"material": s.material, "width": settings::num(s.size), "edge": settings::num(s.hardness), "strength": settings::num(s.strength)})), "length": settings::num((c.length() * 100.0).round() / 100.0),
        "points": c.points.iter().map(|p| json!({"at": round(p.pos()), "handle_in": round(p.into_handle()), "handle_out": round(p.out_handle()), "stretch": stretch_word(p.stretch)})).collect::<Vec<_>>(),
    })
}

pub(super) fn describe_level(ed: &Editor) -> Value {
    let worst = crate::editor::light::budget(&ed.level);
    json!({
        "name": ed.level.name,
        "settings": settings::describe(&ed.level.lighting),
        "walker_sounds": {"Step": ed.level.walker.steps, "Jump": ed.level.walker.jump, "Land": ed.level.walker.land},
        "navigation": {
            "settings": settings::describe(&ed.level.navigation),
            "built": ed.nav.is_some(),
            "out_of_date": ed.nav_out_of_date(),
            "walkable_m2": ed.nav.as_ref().map(|(m, _)| settings::num((crate::nav::area(m) * 10.0).round() / 10.0)),
            "separate_areas": ed.nav.as_ref().map(|(m, _)| crate::nav::islands(m).len()),
        },
        "light_budget": {
            "worst": worst.len(),
            "budget": crate::editor::light::BUDGET,
            "lights": worst.iter().filter_map(|i| ed.level.get(*i)).map(|p| p.name.clone()).collect::<Vec<_>>(),
        },
    })
}

pub(super) fn describe(ed: &Editor, p: &Part) -> Value {
    let name = |id: u64| ed.level.get(id).map(|q| q.name.clone()).unwrap_or_default();
    let mut v = summary(p);
    v["settings"] = settings::describe(p);
    v["outputs"] = json!(p.outputs());
    v["inputs"] = json!(p.inputs());
    // parts with the same contents; changing those asks for instances "all" or "this one"
    v["instances"] = json!(ed.level.instances(p.id).len());
    if p.shape.is_camera() {
        let (_, rot) = crate::level::camera_view(p, 0.0, Vec3::ZERO, Vec3::ZERO);
        v["looks"] = settings::nums(&(rot * Vec3::NEG_Z).to_array().map(|x| (x * 1e4).round() / 1e4 + 0.0));
    }
    if let Some(s) = p.sound.as_ref() { v["sound"] = json!(s.sound); }
    v["events"] = json!(p.events());
    if !p.strokes.is_empty() { v["painted"] = json!({"strokes": p.strokes.len(), "materials": p.painted()}); }
    v["sounds"] = Value::Object(p.sounds.iter().map(|h| (h.on.clone(), json!(h.sound))).collect());
    if let Some(g) = p.sounds_like { v["sounds_like"] = json!(g.label()); }
    if p.shape == Shape::SpotLight { v["shines"] = settings::nums(&crate::editor::light::aim(p).to_array().map(|x| (x * 1e4).round() / 1e4 + 0.0)); }
    v["wires_out"] = Value::Array(ed.level.wires.iter().filter(|w| w.from == p.id).map(|w| json!({"output": w.out, "to": name(w.to), "to_id": w.to, "input": w.input})).collect());
    v["wires_in"] = Value::Array(ed.level.wires.iter().filter(|w| w.to == p.id).map(|w| json!({"input": w.input, "from": name(w.from), "from_id": w.from, "output": w.out})).collect());
    if crate::editor::texture::takes_textures(p) {
        let names = ed.geo.shapes[&p.shape].face_names();
        v["faces"] = json!(names);
        v["textures"] = Value::Array(p.surfaces.iter().map(|s| json!({
            "face": s.face.map_or("all".into(), |f| names.get(f as usize).cloned().unwrap_or_default()),
            "material": s.material,
            "texture": ed.level.materials.get(&s.material).and_then(|m| m.texture.clone()),
            "tile": settings::nums(&s.tile), "offset": settings::nums(&s.offset),
        })).collect());
    }
    v
}

pub(super) fn list_or_none(x: &[&str]) -> String { if x.is_empty() { "none".into() } else { x.join(", ") } }
