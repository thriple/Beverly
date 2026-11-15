//! Beverly's named actions: every menu item, top-bar button and shortcut, in one list. The menus, the keys and the
//! MCP server all run them from here, so they behave the same whichever way they're asked.
use super::view::View;
use super::{theme::Tone, Editor, Pending};
use bevy::prelude::*;

pub struct Action {
    pub id: &'static str,
    pub name: &'static str,
    /// Where it lives: a menu, the top bar or a panel.
    pub place: &'static str,
    pub keys: &'static str,
    /// Opens a file dialog, so it needs a person (the MCP server refuses it).
    pub dialog: bool,
}

const fn a(id: &'static str, name: &'static str, place: &'static str, keys: &'static str) -> Action {
    Action { id, name, place, keys, dialog: false }
}

const fn d(id: &'static str, name: &'static str, place: &'static str, keys: &'static str) -> Action {
    Action { id, name, place, keys, dialog: true }
}

pub const ACTIONS: &[Action] = &[
    a("file.new", "New project", "File", "Ctrl+N"),
    d("file.open", "Open project…", "File", "Ctrl+O"),
    a("file.save", "Save", "File", "Ctrl+S"),
    d("file.save_as", "Save as…", "File", "Ctrl+Shift+S"),
    d("file.export", "Export Windows game…", "File", ""),
    a("file.show_folder", "Show project folder", "File", ""),
    a("file.quit", "Quit", "File", "Ctrl+Q"),
    a("edit.undo", "Undo", "Top bar", "Ctrl+Z"),
    a("edit.redo", "Redo", "Top bar", "Ctrl+Y"),
    a("edit.snap", "Snap on or off", "Top bar", ""),
    a("edit.duplicate", "Duplicate", "Part panel", "Ctrl+D"),
    a("edit.delete", "Delete", "Part panel", "Del"),
    a("edit.deselect", "Select nothing", "Keys", "Esc"),
    a("edit.change_all", "Change all instances", "Change all question", "Enter"),
    a("edit.make_unique", "Make Unique", "Part panel", "U"),
    a("view.parts", "Parts pop-up", "Top bar", "Tab"),
    a("view.frame", "Frame the selection", "Keys", "F"),
    a("view.look_through", "Look through the camera", "Part panel", "L"),
    a("help.about", "About Beverly", "Help", ""),
    a("help.updates", "Check for updates…", "Help", ""),
    a("help.features", "Features", "Help", ""),
    a("help.mcp", "MCP Server…", "Help", ""),
];

pub fn get(id: &str) -> Option<&'static Action> { ACTIONS.iter().find(|a| a.id == id) }

/// Why an action can't run right now (None: it can).
pub fn why_not(ed: &Editor, id: &str) -> Option<&'static str> {
    let editing = id.starts_with("edit.") || id.starts_with("view.");
    if editing && ed.testing.is_some() { return Some("Not while testing (` goes back to editing)"); }
    if editing && ed.looking.is_some() && id != "view.look_through" { return Some("Not while looking through a camera (Esc goes back)"); }
    if ed.pending.is_some() && id.starts_with("file.") { return Some("The unsaved-changes question is open"); }
    match id {
        "edit.undo" if ed.undo.is_empty() => Some("Nothing to undo"),
        "edit.redo" if ed.redo.is_empty() => Some("Nothing to redo"),
        "edit.duplicate" | "edit.delete" | "view.frame" if ed.selected.is_none() => Some("Nothing is selected"),
        "edit.deselect" if ed.selected.is_none() && ed.placing.is_none() => Some("Nothing is selected"),
        "edit.change_all" if ed.ask.is_none() => Some("Nothing to change all of (it asks after a change to a part with instances)"),
        "edit.make_unique" if ed.ask.is_none() && ed.selected.is_none() => Some("Nothing is selected"),
        "edit.make_unique" if ed.ask.is_none() && ed.selected.and_then(|id| ed.level.get(id)).is_some_and(|p| p.unique) => Some("It is Unique already"),
        "view.look_through" if ed.looking.is_none() && !ed.selected.and_then(|id| ed.level.get(id)).is_some_and(|p| p.shape.is_camera()) => Some("Select a camera part first"),
        _ => None,
    }
}

/// Run an action. `view` is needed only by the ones that move the camera.
pub fn run(ed: &mut Editor, view: Option<&mut View>, id: &str, exit: &mut MessageWriter<AppExit>) -> Result<(), String> {
    if get(id).is_none() { return Err(format!("No action called {id} (list_actions shows them)")); }
    if let Some(why) = why_not(ed, id) { return Err(why.into()); }
    match id {
        "file.new" => ed.request(Pending::New, exit),
        "file.open" => ed.request(Pending::Open, exit),
        "file.save" => ed.save(),
        "file.save_as" => ed.save_as(),
        "file.export" => ed.export_zip(),
        "file.show_folder" => {
            if let Err(e) = super::ui::open_folder(&ed.dir) { ed.island.show("!", "Couldn't open the folder", e.to_string(), Tone::Danger); }
        }
        "file.quit" => ed.request(Pending::Quit, exit),
        "edit.undo" => ed.undo(),
        "edit.redo" => ed.redo(),
        "edit.snap" => ed.snap = !ed.snap,
        "edit.duplicate" => ed.duplicate_selected(),
        "edit.delete" => ed.delete_selected(),
        "edit.change_all" => ed.settle(true),
        "edit.make_unique" => {
            if ed.ask.is_none() { ed.ask = ed.selected.map(|id| (id, vec![])); }
            ed.settle(false);
        }
        "edit.deselect" => {
            if ed.placing.is_some() { ed.placing = None } else { ed.selected = None }
        }
        "view.parts" => {
            ed.parts_open = true;
            ed.parts_fresh = true;
            ed.parts_query.clear();
            ed.placing = None;
        }
        "view.frame" => {
            let Some(view) = view else { return Err("This needs the 3D view".into()) };
            if let Some(p) = ed.selected.and_then(|id| ed.level.get(id)) {
                let c = Vec3::from(p.pos);
                let r = Vec3::from(p.size).length();
                let f = view.forward();
                view.pos = c - f * (r * 1.6 + 2.0);
            }
        }
        "view.look_through" => {
            let Some(view) = view else { return Err("This needs the 3D view".into()) };
            super::camera::toggle(ed, view);
        }
        "help.about" => ed.about_open = true,
        "help.updates" => {
            ed.updates_open = true;
            ed.updater.check();
        }
        "help.features" => ed.features_open = true,
        "help.mcp" => ed.mcp_open = true,
        _ => unreachable!("every action in ACTIONS is handled"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_has_a_unique_id_a_name_and_a_place() {
        for (i, x) in ACTIONS.iter().enumerate() {
            assert!(!x.name.is_empty() && !x.place.is_empty(), "{}", x.id);
            assert!(x.id.contains('.'), "{}: ids look like menu.thing", x.id);
            assert!(ACTIONS[i + 1..].iter().all(|y| y.id != x.id), "{} twice", x.id);
        }
    }
}
