//! The benchmark: measures the graphics card and processor at 1080p and 4K and writes a short log (see `beverly::bench`).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() { beverly::bench::run(); }
