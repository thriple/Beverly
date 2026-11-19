//! Reading the tools' arguments: parts, files, vectors and settings by name, with the errors that say what is wrong.
use super::*;

/// Edits wait while testing or while the user is dragging something (so their drag stays one undo step).
pub(super) fn editable(ed: &Editor) -> Result<(), String> {
    if ed.testing.is_some() { return Err("Not while testing (stop_test goes back to editing)".into()); }
    if ed.drag.is_some() { return Err("The user is dragging something; try again in a moment".into()); }
    if ed.looking.is_some() { return Err("Looking through a camera (run_action view.look_through, or Esc, goes back)".into()); }
    Ok(())
}

pub(super) fn vec3_of(v: &Value) -> Option<Vec3> {
    let a = v.as_array().filter(|a| a.len() == 3)?;
    Some(Vec3::new(a[0].as_f64()? as f32, a[1].as_f64()? as f32, a[2].as_f64()? as f32))
}

/// Turn the view to look along `d`.
pub(super) fn look(view: &mut View, d: Vec3) {
    view.yaw = (-d.x).atan2(-d.z);
    view.pitch = d.y.clamp(-1.0, 1.0).asin().clamp(-89f32.to_radians(), 89f32.to_radians());
}

/// A part given as an id or a name.
pub(super) fn part_ref(ed: &Editor, v: Option<&Value>) -> Result<u64, String> {
    match v {
        Some(Value::Number(n)) => n.as_u64().and_then(|id| ed.level.get(id)).map(|p| p.id).ok_or(format!("No part with id {n}")),
        Some(Value::String(n)) => ed.level.parts.iter().find(|p| &p.name == n).map(|p| p.id).ok_or(format!("No part named \"{n}\"")),
        _ => Err("Give a part's id or name".into()),
    }
}

/// The part named by `id` or `name` in the arguments.
pub(super) fn part_of<'a>(ed: &'a Editor, args: &Value) -> Result<&'a Part, String> {
    if let Some(id) = args.get("id").and_then(Value::as_u64) {
        return ed.level.get(id).ok_or(format!("No part with id {id}"));
    }
    if let Some(n) = args.get("name").and_then(Value::as_str) {
        return ed.level.parts.iter().find(|p| p.name == n).ok_or(format!("No part named \"{n}\""));
    }
    Err("Give the part's id or name".into())
}

/// A part with every setting it has (from the same descriptions its panel is drawn from) and its wires.
/// A material by name, file name or path.
pub(super) fn material_ref(ed: &Editor, m: &str) -> Result<String, String> {
    let m = m.replace('\\', "/");
    let name = m.trim_start_matches("assets/materials/").trim_end_matches(".bev");
    let path = format!("assets/materials/{name}.bev");
    if ed.level.materials.contains_key(&path) { return Ok(path); }
    let have: Vec<&str> = ed.level.materials.keys().map(|k| k.trim_start_matches("assets/materials/").trim_end_matches(".bev")).collect();
    Err(format!("No material {name} (there are: {})", list_or_none(&have)))
}

/// A sound file by name, file name or path.
pub(super) fn sound_ref(ed: &Editor, m: &str) -> Result<String, String> {
    let m = m.replace('\\', "/");
    let name = m.trim_start_matches("assets/sounds/").trim_end_matches(".bev");
    let path = format!("assets/sounds/{name}.bev");
    if ed.level.sounds.contains_key(&path) { return Ok(path); }
    let have: Vec<&str> = ed.level.sounds.keys().map(|k| k.trim_start_matches("assets/sounds/").trim_end_matches(".bev")).collect();
    Err(format!("No sound {name} (there are: {})", list_or_none(&have)))
}

/// A surface by name (Grass, Metal…).
pub(super) fn ground_ref(g: &str) -> Result<crate::level::Ground, String> {
    crate::level::Ground::ALL.into_iter().find(|x| x.label().eq_ignore_ascii_case(g))
        .ok_or_else(|| format!("No surface {g} (there are: {})", crate::level::Ground::ALL.map(|x| x.label()).join(", ")))
}

/// Recordings by path or file name (a list).
pub(super) fn recordings(ed: &Editor, v: &Value) -> Result<Vec<String>, String> {
    let list = v.as_array().ok_or("recordings is a list of files, such as [\"step 1.ogg\", \"step 2.ogg\"]")?;
    list.iter().map(|r| {
        let r = r.as_str().ok_or("recordings are file names")?.replace('\\', "/");
        let r = if r.contains('/') { r } else { format!("assets/sounds/{r}") };
        if !crate::level::is_recording(&r) { return Err(format!("{r}: recordings are .wav, .ogg, .mp3 or .flac files")); }
        if !ed.dir.join(&r).is_file() { return Err(format!("No file {r} in the project (list_assets)")); }
        Ok(r)
    }).collect()
}

/// The recordings a sound plays on particular surfaces, from {"Grass": ["grass 1.ogg"], "Metal": []} (an empty
/// list goes back to its usual recordings there).
pub(super) fn set_sets(ed: &Editor, m: &mut crate::level::Sound, v: &Value) -> Result<(), String> {
    let map = v.as_object().ok_or("sets is an object of surface → recordings, such as {\"Grass\": [\"grass 1.ogg\"]}")?;
    for (g, list) in map {
        let g = ground_ref(g)?;
        let clips = recordings(ed, list)?;
        m.sets.retain(|s| s.ground != g);
        if !clips.is_empty() { m.sets.push(crate::level::SoundSet { ground: g, clips }); }
    }
    m.sets.sort_by_key(|s| crate::level::Ground::ALL.iter().position(|g| *g == s.ground));
    Ok(())
}

/// A Sound part's sound file, from `sound`: a sound file or a recording (which gets a sound file of its own).
/// Returns whether there was one.
pub(super) fn plays(ed: &mut Editor, p: &mut Part, args: &Value) -> Result<bool, String> {
    let Some(s) = args.get("sound").and_then(Value::as_str) else { return Ok(false) };
    let Some(sp) = p.sound.as_mut() else { return Err(format!("{} isn't a Sound part (sound is for those)", p.name)) };
    sp.sound = if crate::level::is_recording(s) {
        let r = recordings(ed, &json!([s]))?.remove(0);
        ed.level.sound_for_clip(&r)
    } else {
        sound_ref(ed, s)?
    };
    Ok(true)
}

/// A texture file by path or file name.
pub(super) fn texture_ref(ed: &Editor, t: &str) -> Result<String, String> {
    let t = t.replace('\\', "/");
    let t = if t.contains('/') { t } else { format!("assets/textures/{t}") };
    if !ed.dir.join(&t).is_file() { return Err(format!("No file {t} in the project (list_assets)")); }
    if !crate::editor::texture::is_texture(&t) { return Err("Textures are .png or .jpg files".into()); }
    Ok(t)
}

/// A shader by name, file name or path.
pub(super) fn shader_ref(ed: &Editor, s: &str) -> Result<String, String> {
    let s = s.replace('\\', "/");
    let name = s.trim_start_matches("assets/shaders/").trim_end_matches(".wesl");
    let path = format!("assets/shaders/{name}.wesl");
    if ed.dir.join(&path).is_file() { return Ok(path); }
    let have: Vec<String> = crate::shader::list(&ed.dir).iter().map(|p| p.trim_start_matches("assets/shaders/").trim_end_matches(".wesl").to_string()).collect();
    Err(format!("No shader {name} (there are: {}; write_shader makes one)", list_or_none(&have.iter().map(String::as_str).collect::<Vec<_>>())))
}

/// The values a call gives a shader's settings, by name (a number, true/false or [r, g, b]), checked against its header.
pub(super) fn shader_values(h: &crate::shader::Header, given: &Value) -> Result<std::collections::BTreeMap<String, Vec<f32>>, String> {
    use crate::shader::Kind;
    let obj = given.as_object().ok_or("shader_values is an object of setting name → value")?;
    let mut out = std::collections::BTreeMap::new();
    for (name, v) in obj {
        let s = h.settings.iter().find(|s| s.name == *name).ok_or_else(|| format!("The shader has no setting {name} (it has: {})", list_or_none(&h.settings.iter().map(|s| s.name.as_str()).collect::<Vec<_>>())))?;
        let own: Vec<f32> = match (&s.kind, v) {
            (Kind::Number { .. }, v) => vec![v.as_f64().ok_or_else(|| format!("{name} is a number"))? as f32],
            (Kind::Toggle, v) => vec![v.as_bool().ok_or_else(|| format!("{name} is true or false"))? as u8 as f32],
            (Kind::Colour, v) => vec3_of(v).map(|c| c.to_array().to_vec()).ok_or_else(|| format!("{name} is [r, g, b] from 0 to 1"))?,
        };
        out.insert(name.clone(), own);
    }
    Ok(out)
}

/// Point a spot light at `aim` [x, y, z]: it turns to it and reaches it. Returns whether there was one.
pub(super) fn aim(p: &mut Part, args: &Value) -> Result<bool, String> {
    let Some(a) = args.get("aim") else { return Ok(false) };
    if p.shape.is_camera() {
        crate::editor::camera::aim(p, vec3_of(a).ok_or("aim is [x, y, z]")?)?;
        return Ok(true);
    }
    if p.shape != Shape::SpotLight { return Err(format!("{} isn't a spot light or a camera (aim is for those)", p.name)); }
    let to = vec3_of(a).ok_or("aim is [x, y, z]")? - Vec3::from(p.pos);
    if to.length() < 0.1 { return Err("aim is where it shines, away from the light".into()); }
    p.rot = Quat::from_rotation_arc(Vec3::Y, to.normalize()).to_array();
    if let Some(l) = p.light.as_mut() { l.reach = to.length(); }
    Ok(true)
}

/// A change to a part's contents that its instances didn't get: settled by `instances` ("all", or "this one",
/// which makes the part Unique), or, without it, taken back and refused.
pub(super) fn settle_instances(ed: &mut Editor, args: &Value) -> Result<(), String> {
    let Some((id, them)) = ed.level.split_from(&ed.committed) else { return Ok(()) };
    match args.get("instances").and_then(Value::as_str) {
        Some("all") => ed.level.change_all(id, &them),
        Some("this one") => ed.level.get_mut(id).expect("in the level").unique = true,
        other => {
            ed.level = ed.committed.clone();
            let name = ed.level.get(id).map(|p| p.name.clone()).unwrap_or_default();
            if let Some(o) = other { return Err(format!("instances is \"all\" or \"this one\", not {o:?}")); }
            return Err(format!("{name} has {} instances (parts with the same shape, colour and textures): say instances \"all\" to change them all, or \"this one\" to make {name} Unique and change only it", them.len()));
        }
    }
    Ok(())
}
