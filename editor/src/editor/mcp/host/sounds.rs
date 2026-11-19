//! The sound tools.
use super::*;

pub(super) fn sounds_tools(ed: &mut Editor, name: &str, args: &Value) -> Result<Value, String> {
    match name {
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
        _ => unreachable!("routed in `call`"),
    }
}
