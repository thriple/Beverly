//! A section's settings as JSON, for the MCP server: `describe` lists them, `apply` sets them.
use super::*;

/// An f32 as JSON without float noise (0.05, not 0.05000000074505806).
pub fn num(x: f32) -> serde_json::Value { x.to_string().parse::<f64>().map_or(serde_json::Value::Null, serde_json::Value::from) }

pub fn nums(x: &[f32]) -> serde_json::Value { serde_json::Value::Array(x.iter().map(|v| num(*v)).collect()) }

/// Every described setting of `value` with its current value: for the MCP server and anything else that needs to
/// know what can be set.
pub fn describe(value: &dyn PartialReflect) -> serde_json::Value {
    use serde_json::{json, Value};
    let ReflectRef::Struct(st) = value.reflect_ref() else { return Value::Null };
    let mut out = vec![];
    for (name, s) in fields(value) {
        let Some(f) = st.field(name) else { continue };
        let mut d_tags = None;
        let v: Value = if let Some(x) = f.try_downcast_ref::<f32>() { num(*x) }
            else if let Some(x) = f.try_downcast_ref::<bool>() { json!(x) }
            else if let Some(x) = f.try_downcast_ref::<String>() { json!(x) }
            else if let Some(x) = f.try_downcast_ref::<[f32; 3]>() { nums(x) }
            else if let Some(x) = f.try_downcast_ref::<[f32; 4]>() { nums(x) }
            else if let ReflectRef::Enum(e) = f.reflect_ref() {
                match (e.variant_name(), e.field_at(0)) {
                    ("Some", Some(inner)) => describe(inner),
                    ("None", None) => Value::Null,
                    (n, _) => json!(n),
                }
            } else if let (Kind::Tags, Some((all, on))) = (&s.kind, tag_set(f)) {
                d_tags = Some(all.iter().map(|(_, l)| *l).collect::<Vec<_>>());
                json!(all.iter().filter(|(n, _)| on.iter().any(|o| o == n)).map(|(_, l)| *l).collect::<Vec<_>>())
            } else if let ReflectRef::List(l) = f.reflect_ref() { Value::Array(l.iter().map(describe).collect()) }
            else { Value::Null };
        let mut d = json!({ "field": name, "label": s.label, "section": s.section, "value": v });
        let kind = match &s.kind {
            Kind::Feel { group, words, lo, hi, log } => { d["slider"] = json!({ "group": group, "words": words, "values": [num(*lo), num(*hi)], "by_ratio": log }); "feel" }
            Kind::Number { min, max, .. } => { d["min"] = num(*min); d["max"] = num(*max); "number" }
            Kind::Turn => {
                if let Some(r) = f.try_downcast_ref::<[f32; 4]>() {
                    let (yaw, pitch, roll) = bevy::math::Quat::from_array(*r).to_euler(bevy::math::EulerRot::YXZ);
                    d["degrees"] = num((yaw.to_degrees() * 100.0).round() / 100.0);
                    d["upright"] = json!(pitch.abs() < 1e-3 && roll.abs() < 1e-3);
                }
                "turn"
            }
            Kind::Toggle => "toggle", Kind::Colour { .. } => "colour", Kind::Text => "text",
            Kind::Optional { .. } => "optional", Kind::Choice => "choice", Kind::List => "list",
            Kind::Tags => { d["choices"] = json!(d_tags); "tags" }
        };
        d["kind"] = json!(kind);
        if !s.unit.is_empty() { d["unit"] = json!(s.unit); }
        if !s.help.is_empty() { d["help"] = json!(s.help); }
        out.push(d);
    }
    Value::Array(out)
}

/// Set settings of `value` from JSON, as `describe` gives them: an object keyed by field or label (any case).
/// Numbers are kept in their range; a turn is degrees about the vertical (standing upright) or a quaternion; a
/// colour is [r, g, b] from 0 to 1 or "#rrggbb"; a choice is its name; an optional part is true, false or an object
/// of its own settings (turned on as needed). Returns the fields changed; on an error `value` may be half changed.
pub fn apply(value: &mut dyn PartialReflect, changes: &serde_json::Value) -> Result<Vec<&'static str>, String> {
    let serde_json::Value::Object(m) = changes else { return Err("Settings go in an object, such as {\"Colour\": [1, 0, 0]}".into()) };
    let list = fields(value);
    let ReflectMut::Struct(st) = value.reflect_mut() else { return Err("This has no settings".into()) };
    let mut done = vec![];
    for (key, v) in m {
        let k = key.to_lowercase();
        let Some((name, s)) = list.iter().find(|(n, s)| n.to_lowercase() == k || s.label.to_lowercase() == k) else {
            let names: Vec<&str> = list.iter().map(|(_, s)| s.label).collect();
            return Err(format!("No setting called \"{key}\" (there are: {})", names.join(", ")));
        };
        let f = st.field_mut(name).ok_or("missing field")?;
        apply_one(f, s, v).map_err(|e| format!("{}: {e}", s.label))?;
        done.push(*name);
    }
    Ok(done)
}

pub(super) fn apply_one(f: &mut dyn PartialReflect, s: &Setting, v: &serde_json::Value) -> Result<(), String> {
    use serde_json::Value;
    let keep = |x: f32| match s.kind {
        Kind::Number { min, max, .. } => x.clamp(min, max),
        Kind::Feel { log: true, .. } => x.max(1e-3),
        _ => x,
    };
    let num = |v: &Value| v.as_f64().map(|x| keep(x as f32)).ok_or_else(|| format!("expected a number, not {v}"));
    if let Some(x) = f.try_downcast_mut::<f32>() {
        *x = num(v)?;
    } else if let Some(x) = f.try_downcast_mut::<bool>() {
        *x = v.as_bool().ok_or_else(|| format!("expected true or false, not {v}"))?;
    } else if let Some(x) = f.try_downcast_mut::<String>() {
        *x = v.as_str().ok_or_else(|| format!("expected text, not {v}"))?.to_string();
    } else if let Some(x) = f.try_downcast_mut::<[f32; 3]>() {
        if let (Kind::Colour { .. }, Some(h)) = (&s.kind, v.as_str()) {
            let h = h.trim_start_matches('#');
            let c = (h.len() == 6).then(|| u32::from_str_radix(h, 16).ok()).flatten().ok_or("expected \"#rrggbb\"")?;
            *x = [(c >> 16) as f32 / 255.0, ((c >> 8) & 255) as f32 / 255.0, (c & 255) as f32 / 255.0];
        } else {
            let a = v.as_array().filter(|a| a.len() == 3).ok_or_else(|| format!("expected [x, y, z], not {v}"))?;
            for i in 0..3 { x[i] = num(&a[i])?; }
            if matches!(s.kind, Kind::Colour { .. }) { for c in x.iter_mut() { *c = c.clamp(0.0, 1.0); } }
        }
    } else if let Some(x) = f.try_downcast_mut::<[f32; 4]>() {
        use bevy::math::Quat;
        let q = match v {
            Value::Number(d) => Quat::from_rotation_y((d.as_f64().unwrap_or(0.0) as f32).to_radians()),
            Value::Array(a) if a.len() == 4 => {
                let mut q = [0.0; 4];
                for i in 0..4 { q[i] = num(&a[i])?; }
                { let q = Quat::from_array(q); if q.length() < 1e-6 { return Err("that quaternion has no length".into()) } q.normalize() }
            }
            _ => return Err(format!("expected degrees or a quaternion [x, y, z, w], not {v}")),
        };
        *x = q.to_array();
    } else if let Kind::Optional { .. } = s.kind {
        let Some(TypeInfo::Enum(info)) = f.get_represented_type_info() else { return Err("not an optional part".into()) };
        let ReflectMut::Enum(e) = f.reflect_mut() else { return Err("not an optional part".into()) };
        let on = e.variant_name() == "Some";
        match v {
            Value::Bool(false) | Value::Null => { e.apply(&DynamicEnum::new("None", DynamicVariant::Unit)); }
            Value::Bool(true) | Value::Object(_) => {
                if !on {
                    let inner = info.variant("Some").and_then(|v| v.as_tuple_variant().ok()).and_then(|t| t.field_at(0)).and_then(|f| default_of(f.type_id()));
                    let mut t = DynamicTuple::default();
                    t.insert_boxed(inner.ok_or("it has no default")?.into_partial_reflect());
                    e.apply(&DynamicEnum::new("Some", DynamicVariant::Tuple(t)));
                }
                if v.is_object() { apply(e.field_at_mut(0).ok_or("it didn't turn on")?, v)?; }
            }
            _ => return Err(format!("expected true, false or an object of its settings, not {v}")),
        }
    } else if let Kind::Choice = s.kind {
        let Some(TypeInfo::Enum(info)) = f.get_represented_type_info() else { return Err("not a choice".into()) };
        let want = v.as_str().ok_or_else(|| format!("expected a name, not {v}"))?.to_lowercase();
        let pick = info.iter().find(|x| x.name().to_lowercase() == want || x.get_attribute::<Label>().is_some_and(|l| l.0.to_lowercase() == want));
        let names: Vec<&str> = info.iter().map(|x| x.get_attribute::<Label>().map_or(x.name(), |l| l.0)).collect();
        let pick = pick.ok_or_else(|| format!("expected one of {}", names.join(", ")))?;
        f.apply(&DynamicEnum::new(pick.name(), DynamicVariant::Unit));
    } else if let Kind::Tags = s.kind {
        let (all, _) = tag_set(f).ok_or("not a set of tags")?;
        let a = v.as_array().ok_or_else(|| format!("expected a list of names, not {v}"))?;
        let mut names = vec![];
        for x in a {
            let want = x.as_str().ok_or_else(|| format!("expected a name, not {x}"))?.to_lowercase();
            let (n, _) = all.iter().find(|(n, l)| n.to_lowercase() == want || l.to_lowercase() == want)
                .ok_or_else(|| format!("no tag called {x} (there are: {})", all.iter().map(|(_, l)| *l).collect::<Vec<_>>().join(", ")))?;
            names.push(*n);
        }
        set_tags(f, &all, &names);
    } else if let Kind::List = s.kind {
        let item_ty = match f.get_represented_type_info() { Some(TypeInfo::List(l)) => l.item_ty().id(), _ => return Err("not a list".into()) };
        let a = v.as_array().ok_or_else(|| format!("expected a list of objects, not {v}"))?;
        let ReflectMut::List(l) = f.reflect_mut() else { return Err("not a list".into()) };
        while l.len() > 0 { l.remove(l.len() - 1); }
        for item in a {
            let mut d = default_of(item_ty).ok_or("its items have no default")?;
            apply(d.as_partial_reflect_mut(), item)?;
            l.push(d.into_partial_reflect());
        }
    } else {
        return Err("this kind of setting can't be set yet".into());
    }
    Ok(())
}
