//! The game player: runs a Beverly project as a game (see `beverly::player`).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() { beverly::player::run(|_| {}); }
