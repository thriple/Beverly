//! Beverly: a simple, no-code-first 3D game editor for Bevy. See README.md and docs/design.html.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() { beverly::editor::run("Beverly", "beverly-player", |_| {}); }
