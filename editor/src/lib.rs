//! Beverly: the editor (`beverly`) and the game player (`beverly-player`), as a library so a game with its own
//! code (e.g. its player character) can build its own copy of both with its plugins added: `editor::run`,
//! `player::run`. Shared by both: the level format, the basic shapes, imported models, how a level looks in 3D,
//! playing it, and script parts (buttons, wires, moving parts).
pub mod editor;
pub mod level;
pub mod models;
pub mod play;
pub mod player;
pub mod script;
pub mod scripted;
pub mod settings;
pub mod shapes;
pub mod update;
pub mod world;
