//! The Level tab's Foliage topic: the dock lists the Mixes (materials that grow plants, with their ground under them) to
//! paint with, the grass carpet's reach and quality, and the grass species with their settings. The brush, pen and
//! bucket lay a Mix as they lay any material.
use super::*;
use crate::grass::{species, GrassSpecies, GrassSpeciesRegistry, MAX_SPECIES};

/// Whether the dock shows the Foliage topic's panel (the Level tab, that topic, nothing selected).
pub(super) fn foliage_open(ed: &Editor) -> bool { ed.tab == audio::LEVEL && ed.level_mode == LevelMode::Foliage && ed.selected.is_none() }

fn short(path: &str) -> String { path.trim_start_matches("assets/materials/").trim_end_matches(".bev").to_string() }

pub(super) fn foliage_panel(ui: &mut Ui, ed: &mut Editor) {
    let mixes: Vec<String> = ed.level.materials.iter().filter(|(_, m)| !m.grass.is_empty() || m.ground.is_some()).map(|(k, _)| k.clone()).collect();
    let sub = format!("Level · {} Mix{}", mixes.len(), if mixes.len() == 1 { "" } else { "es" });
    identity(ui, theme::CAT_FLOW, "leaf", Err("Foliage"), &sub, &mut []);
    column(ui, 0.0, "MIXES", |ui| {
        let (mut edit, mut new) = (None, false);
        for k in &mixes {
            let m = &ed.level.materials[k];
            let on = ed.brush_on && ed.brush.material == *k;
            let what = if m.grass.is_empty() { "ground".to_string() } else { m.grass.iter().map(|g| g.species.replace('_', " ")).collect::<Vec<_>>().join(", ") };
            ui.horizontal(|ui| {
                let w = ui.available_width() - 56.0;
                let r = ui.add(egui::Button::selectable(on, egui::RichText::new(format!("{}  ·  {what}", short(k))).color(if on { highlight() } else { TEXT })).frame(false).min_size(vec2(w, 24.0))).tip("Paint with it: the brush, the pen and the bucket (B, P, G) lay it as they lay any material");
                if r.clicked() { ed.brush.material = k.clone(); ed.brush_on = true; ed.pen_on = false; ed.bucket_on = false; }
                if ui.button("Edit").tip("Its species, ground and thickness").clicked() { edit = Some(k.clone()); }
            });
        }
        if mixes.is_empty() { ui.label(egui::RichText::new("No Mixes yet. A Mix is a material that grows plants and lays a ground under them.").color(MUTED).size(11.0)); }
        if ui.button("+ New Mix").tip("A material that grows grass; give it a ground in its Edit panel").clicked() { new = true; }
        if let Some(k) = edit { ed.material_open = Some(k); }
        if new {
            let (ground, _) = ed.level.materials.iter().find(|(_, m)| m.grass.is_empty() && m.ground.is_none()).map(|(k, _)| (Some(k.clone()), ())).unwrap_or((None, ()));
            let path = ed.level.new_material("Mix", level::Material { grass: vec![level::Grows { species: "grass".into(), share: 1.0 }], ground, tint: [0.35, 0.28, 0.2], ..Default::default() });
            ed.island.show("◈", "Made a Mix", short(&path), theme::Tone::Accent);
            ed.brush.material = path.clone();
            ed.brush_on = true;
            ed.material_open = Some(path);
        }
    });
    // what the brush lays, while it is out
    if ed.brush_on { brush_body(ui, ed); }
    let mut f = ed.level.foliage.clone();
    let mut ranges = ed.level.ranges.clone();
    let mut ctx = settings::Ctx { replace: &[], ranges: &mut ranges, ranges_changed: false, palette: vec![] };
    column(ui, 0.0, level::CARPET, |ui| { settings::section(ui, &mut *f, level::CARPET, &mut ctx); });
    let reg = GrassSpeciesRegistry::with(&ed.level.foliage.species);
    column(ui, 0.0, "SPECIES", |ui| {
        let open = ed.species_open.clone().filter(|n| reg.id_of(n).is_some());
        for s in &reg.species {
            let on = open.as_deref() == Some(&s.name);
            let changed = ed.level.foliage.species.iter().any(|o| o.name == s.name);
            let label = format!("{}{}", s.name.replace('_', " "), if changed { "  ·  changed" } else { "" });
            if ui.add(egui::Button::selectable(on, egui::RichText::new(label).color(if on { highlight() } else { TEXT })).frame(false)).clicked() { ed.species_open = if on { None } else { Some(s.name.clone()) }; }
        }
        if let Some(mut s) = open.and_then(|n| reg.species.iter().find(|s| s.name == n).cloned()) {
            ui.add_space(6.0);
            let before = s.clone();
            settings::section(ui, &mut s, species::SPECIES, &mut ctx);
            if s != before { ed.level.foliage.keep(s.clone()); }
            let built_in = GrassSpeciesRegistry::default().id_of(&s.name).is_some();
            ui.horizontal(|ui| {
                if ui.button("+ New species").tip("A copy of this one to change").clicked() && reg.species.len() < MAX_SPECIES {
                    let name = (2..).map(|n| format!("{} {n}", s.name.split(' ').next().unwrap_or("grass"))).map(|n| n.replace(' ', "_")).find(|n| reg.id_of(n).is_none()).unwrap_or_default();
                    ed.level.foliage.keep(GrassSpecies { name: name.clone(), ..s.clone() });
                    ed.species_open = Some(name);
                }
                let changed = ed.level.foliage.species.iter().any(|o| o.name == s.name);
                if changed && ui.button(if built_in { "Back to built-in" } else { "Delete" }).tip(if built_in { "Undo this level's changes to it" } else { "Remove it; materials growing it grow nothing" }).clicked() {
                    ed.level.foliage.species.retain(|o| o.name != s.name);
                    if !built_in { ed.species_open = None; }
                }
            });
        }
        if ed.species_open.is_none() { ui.label(egui::RichText::new("Pick one to change how it grows").color(MUTED).size(11.0)); }
    });
    column(ui, 0.0, "PLANT SETS", |ui| {
        ui.label(egui::RichText::new("Scattered models (flowers, ferns, trees) come with Scatter").color(MUTED).size(11.0));
    });
    if ctx.ranges_changed { ed.level.ranges = ranges; }
    if f != ed.level.foliage { ed.level.foliage = f; }
}
