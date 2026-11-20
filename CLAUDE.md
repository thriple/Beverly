# CLAUDE.md

Beverly: a standalone, no-code-first 3D game editor for Bevy (Windows). The user is not a programmer; they
build by direct manipulation in 3D. The design lives in `docs/design.html` (keep it current when plans change).

## Rules from the user

- **Work on `main` only:** every session commits and pushes to `main`, never to a session branch (user, 2026-11-15).
  One exception (user, 2026-11-18): the virtual-geometry plugin, Tessera, lives on the branch `tessera`
  (`crates/tessera`, its job list in `crates/tessera/JOBS.md` there), worked on by nightly unattended runs and
  merged into main part by part after review. Unattended runs never push to main, never release, and stop on taste
  questions (they go in JOBS.md). On that branch only the `tessera` entry of `editor/features.json` is edited
  and `tools/roadmap.py` is not run; main reruns it after each merge.
- **The editor's look (user, 2026-11-17):** the user's redesign in `docs/ui/` is the source of truth: `HANDOFF.md`
  (frame, tab colours, type, panels, controls, icons, checklists for new tabs and controls), the boards in
  `docs/ui/design/boards` (live canvas linked in HANDOFF.md), `tokens/` and `icons/`. Every new window, panel, control
  or tab is built from its pieces; add new ones to it. (Replaces the earlier `docs/ui.svg` design.)
- **No rasters in the editor's GUI (user, 2026-11-16):** everything Beverly's own editor interface draws is vector,
  rendered at the screen's resolution; no images, no scaled bitmaps. Previews of the user's content (asset
  thumbnails) don't count as UI (user, 2026-11-17), but live 3D views are preferred. This is only for Beverly's own
  editor UI: game files and the UI of games made with Beverly may use images (user, 2026-11-20).
- **Motion kit (user, 2026-11-17):** the Motion kit (`kit/motion-kit.zip`, ported in `editor/src/editor/theme.rs`)
  gives the movement only: its springs (SPRING / SNAPPY / SOFT) and entrances, and how its tooltips spring up, its
  dock panel moves (sliding pill, content sliding the way you went, springing width), its dynamic island pops open
  for each action and its inputs animate typed characters. Everything you see (shapes, colours, sizes, type) follows
  the redesign.
- Caps Lock is the movement-mode switch only. Tab opens the parts pop-up. Mouse 5 / Page Up faster, Mouse 4 / Page Down slower.
  `[` and `]` belong to brush size, as in Photoshop (user, 2026-11-17).
- Basic shapes double as colliders (Visible off = invisible collider).
- Projects are folders of plain text (RON) so they work with git.
- Build with multiplayer in mind (it comes late, but nothing should block it): game state lives in data (components,
  resources, the level and character files) that could be sent over a network, input goes through named actions,
  play runs on a fixed time step, and effects, sounds and the camera react to events instead of driving the game.
- A guideline, not a rule (user, 2026-11-20): prefer a vector, table, curve or stroke description to images, grids
  or baked meshes in game files when it does the job as well (shaders are fine; textures only the user's own). It
  keeps files small; where raster data is clearly the better tool, use it.
- No third-party asset packs: the user makes Beverly's built-in models and characters. Libraries are fine.
- Build each feature to its full plan in the roadmap (its parts in order), not a cut-down first version.
- Every feature says what the MCP server gets from it (most comes free from setting descriptions and named
  editor actions; anything else is part of the feature).
- Keep it simple: minimum code for what was asked, surgical changes, ask when unclear.
- **Say when something is getting too big (user, 2026-11-19):** when a file passes about 1,200 lines, a function
  about 150, or a screen, panel or system starts mixing jobs, tell the user at the end of that piece of work (what it
  is, why it is hard to change now, and what the split would be) and ask before reorganising; never let it grow
  silently. A reorganisation moves code without changing how anything looks or behaves, and gets its own commit.
- Never put model names or ids in repository files. Never ask for keys in chat or write them into files.
- **Built to spec for virtual geometry (user, 2026-11-18; docs/tessera.md):** every material or shader goes
  through Beverly's shader pipeline (`shaders`), with vertex movement and cut-out edges declared as raster hooks and
  see-through materials marked; models are used through their split meshes (`models-split`), never a spawned glTF
  scene; anything worked out from the user's files (bakes, atlases, probes) goes through the bake cache, never into the
  project; nothing relies on MSAA or one graphics card (read `GpuCaps`, work on the fallback); and each feature's
  checks include the performance meter's numbers once it exists.

## Tech

Bevy 0.20.1 + bevy_egui 0.43.1 (egui 0.36), pinned together with the toolchain in `editor/rust-toolchain.toml`.
Windows build: `.github/workflows/windows.yml` (release per push) or `tools/make-zip.ps1` locally.

Scripted tests: `beverly-player PROJECT --test tests/NAME.ron` plays a level on a fixed clock, presses keys on a
timeline, saves screenshots and checks positions (file format in `editor/src/scripted.rs`); `tools/run-tests.sh`
runs every test project under `tests/`. Each test's report ends with a `perf:` line (the performance meter's numbers, `editor/src/perf.rs`); compare it between runs. Give each finished feature a test project there. The MCP server's check is
`tests/mcp/check.py` (open tests/lift in Beverly with the server on, then run it).

A game with its own code (e.g. its player character) depends on this crate and builds its own copy of the editor
and player: `beverly::editor::run(title, player_exe, plugins)`, `beverly::player::run(plugins)`. A custom player
character sets `play::Play::custom` and takes over while `Play::on` (see the doc comment there).

## Releases

Push work to the repository freely, but only cut a release (commit message containing `[release]`, or a manual
run of the Windows build) for finished features, never half-done work.

Every release updates the feature list, `editor/features.json`: what the release finished becomes `done` with `built`
(the date) and `build` (its build number: the next run of the Windows build); new features get their needs, effort
S / M / L / XL and, for L and XL, session-sized parts. Then `python3 tools/roadmap.py` checks it, renumbers the build
order and writes `docs/roadmap.html`. Help > Features shows the same list.

Keep the roadmap current as work goes (user, 2026-11-15): it lives in the repository, `docs/roadmap.html`, so every
session shares it. After each finished part, not only each release, update the feature's note in
`editor/features.json`, run `python3 tools/roadmap.py` and commit both with the work.
