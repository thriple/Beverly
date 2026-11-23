//! The foliage tools: the grass carpet's reach and quality, and the grass species.
use super::*;
use crate::grass::{GrassSpecies, GrassSpeciesRegistry, MAX_SPECIES};

pub(super) fn foliage_tools(ed: &mut Editor, name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "get_foliage" => Ok(describe_foliage(ed)),
        "change_foliage" => {
            editable(ed)?;
            let set = args.get("settings").ok_or("Give settings, such as {\"Reach\": 60}")?;
            let changed = settings::apply(&mut *ed.level.foliage, set)?;
            ed.commit();
            Ok(json!({"changed": changed, "foliage": describe_foliage(ed)}))
        }
        "change_species" => {
            editable(ed)?;
            let n = args.get("species").and_then(Value::as_str).ok_or("Give the species")?;
            let mut s = species_named(ed, n)?;
            let changed = settings::apply(&mut s, args.get("settings").ok_or("Give settings, such as {\"Height\": 1.5}")?)?;
            ed.level.foliage.keep(s);
            ed.commit();
            Ok(json!({"changed": changed, "species": describe_species(ed, n)}))
        }
        "new_species" => {
            editable(ed)?;
            let n = args.get("name").and_then(Value::as_str).ok_or("Give the name")?;
            if n.is_empty() || !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') { return Err(format!("{n:?} can't be a species name (letters, digits and _)")); }
            if GrassSpeciesRegistry::with(&ed.level.foliage.species).id_of(n).is_some() { return Err(format!("There is already a species called {n}")); }
            if GrassSpeciesRegistry::with(&ed.level.foliage.species).species.len() >= MAX_SPECIES { return Err(format!("A level has at most {MAX_SPECIES} species")); }
            let mut s = match args.get("copy_of").and_then(Value::as_str) { Some(c) => species_named(ed, c)?, None => GrassSpecies::default() };
            s.name = n.to_string();
            if let Some(set) = args.get("settings") { settings::apply(&mut s, set)?; }
            ed.level.foliage.keep(s);
            ed.commit();
            Ok(describe_species(ed, n))
        }
        _ => unreachable!("routed in `call`"),
    }
}

/// A species by name, as the level has it.
fn species_named(ed: &Editor, name: &str) -> Result<GrassSpecies, String> {
    let reg = GrassSpeciesRegistry::with(&ed.level.foliage.species);
    reg.species.iter().find(|s| s.name == name).cloned().ok_or_else(|| format!("No grass species {name:?}; the species: {}", reg.species.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ")))
}

fn describe_species(ed: &Editor, name: &str) -> Value {
    let s = species_named(ed, name).expect("named");
    json!({"name": s.name, "changed_in_level": ed.level.foliage.species.iter().any(|o| o.name == name), "settings": settings::describe(&s)})
}

fn describe_foliage(ed: &Editor) -> Value {
    let reg = GrassSpeciesRegistry::with(&ed.level.foliage.species);
    let mixes: Vec<Value> = ed.level.materials.iter().filter(|(_, m)| !m.grass.is_empty() || m.ground.is_some()).map(|(k, m)| {
        json!({"material": k, "grass": m.grass.iter().map(|g| json!({"species": g.species, "share": g.share})).collect::<Vec<_>>(), "ground": m.ground, "settings": settings::describe(m)})
    }).collect();
    json!({"settings": settings::describe(&*ed.level.foliage), "species": reg.species.iter().map(|s| describe_species(ed, &s.name)).collect::<Vec<_>>(), "mixes": mixes})
}
