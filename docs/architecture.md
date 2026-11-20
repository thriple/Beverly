# Architecture review and design decisions (2026-11-20)

Written after a review of the built code against the roadmap, and a stretch of design talk with the user. The
roadmap (`editor/features.json`, `docs/roadmap.html`) carries the resulting entries; this page keeps the reasons.

## The goal (user, 2026-11-20)

Anyone who picks up Beverly gets a very large polygon, light and logic budget in an editor that stays intuitive and
still lets them add modular features to a game. Reference card for "60 fps with the big budget": the user's own, with
8 GB of video memory. Last Ascent is the first game and the test case for everything below.

## What the review found in the built code

| # | Finding | Fix (roadmap entry) |
|---|---|---|
| 1 | Parts are a flat list with no parent or child links; movers slide but don't turn; riding a mover is a special case in the walker. | `part-hierarchy` |
| 2 | `Part` grows an optional field per feature; `Tag` and `Shape` are fixed lists in code. | `open-tags`, the module settings slot (`modules`) |
| 3 | The editor compares the whole level with the last undo step every frame (when nothing is dragged) and each undo step clones the whole level (300 kept). | `change-tracking` |
| 4 | Play reads raw keys (W A S D E Shift Space) although the rule is named actions. | `input-map`, `control-target` |
| 5 | A level is one file, loaded whole, ids unique per level; no regions; only the first level of a project is ever loaded. | `levels`, `streaming` (region model decided first) |
| 6 | Derived data is saved inside parts (pen strips and fills). The fill outline is in world coordinates, which breaks on a moving part or after a region or origin shift. | fix in `pen`; `runtime-edits` |
| 7 | The walker rides sliding parts but not turning ones. | `part-hierarchy`, `assemblies` |

Already in good shape: tags for climbing, digging, perches, grind rails and slippery surfaces exist; wires use text
port names (modules can add ports); shader settings already work from a header plus a values map (the pattern
modules use); the test harness runs on a fixed clock (module tests can be deterministic); materials and sounds load
from files and are not saved in the level.

Foundation order: fix the fill outline; change tracking and undo as diffs; part hierarchy; sockets; open tags and the
module settings slot; named input actions then control handover; the runtime edits layer and the save system's scope;
the region model (decided early, built later).

## Decisions recorded

- Logs stay local; no analytics or telemetry at all (user).
- Code signing is not planned: the export window shows a static note that the exe is unsigned and why (a yearly
  certificate) (user).
- A returning player finds their changes in a level by default; the default transition is a fade with a loading
  screen (user).
- Priority groundwork for Last Ascent: base-building, vehicles and riding. Crowds and armies come much later (user).
- The no-raster rule is for Beverly's own editor UI only; for game files and game UI it is a guideline (user).
- HUD and Screens are separate; both build on a UI core (user).

## Modules (design)

A module is one dropped-in file. Three nouns: **Module**, **Reach** (where it acts: everywhere, a zone, along a curve,
at pins, on a painted surface, near a part or tag) and **Dials** (the adjustables, each with a physical form in the
view: ring for distance, arrow for direction, plane for water level, box for area, gauge for strength, pickers for
sounds and materials; the same dial shows in the Inspect panel). Weather, ground, water, sound and behaviours are all
modules; a zone holds a stack of layers and blends them at its edges.

Two kinds: **presets** (a header only, combining built-in layers, no code) and **scripted** modules (a header plus a
Luau script). Fortnite's devices (no-code placeable objects with options and events, code as the fallback) and Lyra's
game features (genres as plugin bundles) work the same way. A module lists what it touches before it is placed.

The split that keeps many agents cheap: scripts **decide** a few times a second; Beverly's native systems **move**
(steering, avoidance, path following, neighbour lookup) every tick. The host gives each module an operation budget,
declared state (so saves, networking and hot reload work), seeded random numbers, a fixed step, and effects as
events; the performance meter shows the cost per module; simulation detail drops with distance.

Shared building blocks: **sockets** (named attachment frames) and **spots and claims** (a socket with a capacity: a
dock bay, a perch, a bench). Worked examples: ships docking (claim a bay, follow the approach curve, dock, unload),
a town crowd (spots with interests, linger times), birds (perches, scatter), an economy (production, consumption,
prices, traders). The economy is the first proof, since it needs no characters or flight.

## Level transitions and backdrops

One named action, **Go to level**, callable by wires, triggers, the timeline, quests, screens and modules: which
level, which entry point, the transition style, what to keep. Anything that outputs a finished event can ask for it:
a cutscene ending, a boss defeated, a quest complete, a death, a menu button, a module.

**Hand-off points** are for seamless changes (Last Ascent entering the ship): a trigger that unloads a region or level,
keeps what must stay, and leaves a **backdrop** (a baked low-res image plus heightmap, or a low-res mesh, in the bake
cache) shown at a fake distance with parallax. Backdrops also make the planet, sun and distant station in the space
level, and swap to real geometry on approach. A cinematic sets the cinematic control lock and can load the next level
behind it.

Regions (seamless open ground) and levels (discrete places) load through the same machinery.

## Screens, HUD and the patchbay

- **UI core:** widgets, layout, bindings and the patchbay. **HUD:** the always-on overlay during play. **Screens:**
  navigated full-screen flows (stacks, conditions, input modes, gamepad navigation, in-world screens).
- **Readouts** are named values (number, text, yes/no, percentage, time, colour) published by variables, attributes,
  modules and level stats. The **patchbay** is an overlay: a searchable rack of readouts grouped by source with live
  values, springy 2D noodles to widget inputs, pulses for events and labelled values for readouts, red where a source
  is missing. A jack on every setting lets a readout drive any setting of any part.
- **HUD skins:** the same readouts drawn as classic 2D, as 3D panels, ground shapes and physical models around the
  character, or on a companion; the player can choose. Panels are drawn to a texture and placed in 3D, updated only
  when their values change.

## Lighting and budgets

- Cached shadows in two layers: the shadows of what never moves are drawn once and kept; moving things are drawn
  each frame on top. Invalidate on level-of-detail and streaming changes; animated characters always count as moving.
  Headlights get a short shadow range, the nearest cars only, and a small map.
- Tiers by card: baked probes and unshadowed lights everywhere; a middle tier of dynamic diffuse light bounce without
  ray tracing (to be chosen by a measured spike: radiance cascades against a surface cache with cards, on primitives);
  Solari's ray-traced direct and indirect light on cards that have ray queries.
- A separate benchmark program (`beverly-bench`, its own download, 1080p and 4K) measures a card and writes a short
  log; its scenes become the stress level. Video memory is a budget of its own (texture compression and mips).

## Genres: shared building blocks

Sockets and snap rules; live yes/no feedback while placing; a structure-strength graph; assemblies with joints and
interiors in the body's own frame; control handover for vehicles, mounts and turrets; simulation detail by distance;
volumes with events; a saved edits layer over the authored world (digging, building, destruction); orders and fog of
war for RTS packs later; game packs bundling a game rules module with modules, so a genre is a pack and not
hard-coded. Determinism stance: keep the editor friendly to an authoritative server (fixed step, commands as data);
lockstep, which needs deterministic maths, only if an RTS pack is wanted.

## Last Ascent as the test case

- Level one: inside the ship, a hand-off that leaves a backdrop of the city and forest, an escape-ship cinematic
  (multiple cameras, controls locked) while level two loads behind it. Story beat: the enemy coats the main character
  in goo until the ship's AI takes them for an enemy and sends a military-spec mech (factions and identity).
- Level two: space. Planet, sun and a distant station as backdrops; slowly spinning debris and asteroids as scatter in
  a 3D volume with turning movers; a long-range missile shooter that hides among cover and jumps between spots; a
  stealth flanker that gets behind the player and fires a restraining shot of goo (a projectile with a status effect
  and a control lock; a physical grab stays possible through sockets), then melee; a director module mixes them.
- The mech: its HUD sits near the mech (momentum is the fun), with things like the countdown timer still on screen;
  a geometric sphere drone follows it as a diegetic inventory (power-ups, healing packs, weapon modifiers) and scout.
  The mech is still in flux.

## Build order: stages

Every unbuilt feature has a stage, and the roadmap lists them stage by stage (dependencies are checked, so nothing
needs a later stage). Foundations come first, then the interface, then stages that each leave a more complete game
level: author the world (land, sea and sky), move through it, make it alive, inhabitants, challenge, build and ride,
story and flow, ship and scale. So, for example, terrain, water and sky come before base building. The stage list
and what a level can do at the end of each is in `tools/roadmap.py`; the page `docs/roadmap.html` shows it.

## Editor layout

The top bar's eight fixed tabs become a scrolling strip of grouped tabs (roadmap `tab-strip`): the frame goes neutral, the
group colour sits in a thin bezel and the selected tab, the wheel scrolls the strip, and tabs squash to slices at the
sides. Tabs are data, so modules and packs can add them. The design and the first cut of the groups are in
`docs/ui/HANDOFF.md`.

## Export

The fundamentals hold: the player plus the project folder in a zip. What changes is in `export-hardening`: one shared
player must carry everything modules and Tessera need; unused and editor-only files are left out and the rest packed;
a friendly error window, a log file, an icon, window and quality settings and licence notices.

## Still open

- Luau or Rhai: Luau is the pick; check how it builds into the Windows player.
- Whether the navigation library can rebuild tiles at runtime (`dynamic-nav`).
- Primary-source reading of Unity's Surface Cache GI and Epic's Lumen notes (the research session couldn't reach
  those sites).
- In level two: does the player pilot a ship or float in a suit?
