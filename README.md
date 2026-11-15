# Beverly

A simple, no-code-first editor for making 3D games with [Bevy](https://bevyengine.org), for Windows.
You build by moving things around in 3D: drop shapes onto surfaces, drag widgets, wire script parts
together. Code is only for new kinds of parts.

**Status: v0.1 in progress.** What works now:

- Two ways of moving: **Editor movement** (orbit, pan, proximity zoom) and **Builder movement** (fly where you
  look, 12 speeds from 1/8x to 400x). **Caps Lock** switches; it never types capitals in Beverly.
- **Tab** opens the parts pop-up. Basic shapes (box, ramp, stairs, arch, cylinder, sphere, plane) drop onto the
  surface under the cursor and double as colliders: switch **Visible** off for an invisible wall.
- Move and turn widgets with snapping, a properties panel, undo / redo.
- **Models:** drop `.glb` / `.gltf` files on the window, then place them from the Assets panel.
- **Textures:** drop `.png` / `.jpg` files on the window, then drag one from the Assets panel onto a face (Shift: the
  whole shape); the violet handles slide and size it.
- **Test it:** press `` ` `` to walk the level with a stand-in character (Shift+`` ` `` starts at the Player start).
- **Export:** File > Export Windows game writes a zip with `<project>.exe` and the project's files.
- Projects are folders of plain text files (`project.bev`, `levels/*.bev`, `assets/`); projects saved as `.ron`
  by older Beverlys still open and are updated when saved. Double-click a `.bev` file to open its project (Beverly
  registers the file type for your Windows user when it runs). Drag files from Explorer
  onto the window to import them.

Design: [`docs/design.html`](docs/design.html). The UI follows the Motion kit in [`kit/`](kit/).

## Get it

Download `Beverly-windows.zip` from the [latest release](../../releases/latest), unzip, run `Beverly.exe`
(keep `beverly-player.exe` next to it).

## Build it yourself

- On GitHub: every push to `editor/` builds and publishes a release (`.github/workflows/windows.yml`).
- On your PC: install [Rust](https://rustup.rs), then run `powershell -ExecutionPolicy Bypass -File tools\make-zip.ps1`.
- Developing: `cd editor && cargo run`.

## Credits

Fonts: Inter and JetBrains Mono (SIL Open Font License), DejaVu Sans for symbols (Bitstream Vera / DejaVu license).
