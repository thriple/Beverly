#![recursion_limit = "256"] // the MCP server's tool list is one long json! literal
//! Beverly: the editor (`beverly`) and the game player (`beverly-player`), as a library so a game with its own
//! code (e.g. its player character) can build its own copy of both with its plugins added: `editor::run`,
//! `player::run`. Shared by both: the level format, the basic shapes, imported models, how a level looks in 3D,
//! playing it, script parts (buttons, wires, moving parts) and sounds.
pub mod bake;
pub mod editor;
pub mod gpu;
pub mod level;
pub mod models;
pub mod nav;
pub mod paint;
pub mod perf;
pub mod physics;
pub mod play;
pub mod player;
pub mod script;
pub mod scripted;
pub mod settings;
pub mod shader;
pub mod shapes;
pub mod sound;
pub mod update;
pub mod world;
