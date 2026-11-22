//! The material and shader tools.
use super::*;

pub(super) fn materials_tools(ed: &mut Editor, name: &str, args: &Value) -> Result<Value, String> {
    match name {
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
            if let Some(g) = args.get("grass") {
                m.grass = grass_ref(g)?;
                changed.push("grass");
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
            if let Some(g) = args.get("grass") { m.grass = grass_ref(g)?; }
            let name = args.get("name").and_then(Value::as_str).unwrap_or("Material").replace(['/', '\\'], " ");
            let path = ed.level.new_material(&name, m);
            ed.commit();
            Ok(describe_material(ed, &path))
        }
        _ => unreachable!("routed in `call`"),
    }
}

/// The grass a material grows, as the MCP gives it: species names, or {species, share}; null or [] for none.
fn grass_ref(v: &Value) -> Result<Vec<crate::level::Grows>, String> {
    let names = crate::grass::species_names();
    let list = match v { Value::Null => return Ok(vec![]), Value::Array(a) => a, _ => return Err("Give grass as a list of species, such as [\"grass\", {\"species\": \"daisy_snow\", \"share\": 0.1}]".into()) };
    if list.len() > 4 { return Err("A material grows at most four species".into()); }
    list.iter().map(|g| {
        let (species, share) = match g {
            Value::String(s) => (s.clone(), 1.0),
            _ => (g.get("species").and_then(Value::as_str).ok_or("Each grass needs its species")?.to_string(), g.get("share").and_then(Value::as_f64).unwrap_or(1.0) as f32),
        };
        if !names.contains(&species) { return Err(format!("No grass species {species:?}; the species: {}", names.join(", "))); }
        Ok(crate::level::Grows { species, share: share.max(0.0) })
    }).collect()
}
