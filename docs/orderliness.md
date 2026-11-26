# Orderliness: an audit of how Beverly's systems agree (2026-11-26)

Stage 1 of the Prism brief "Orderliness" (`docs/briefs/beverly-orderliness.md` in the Prism repository): **report
only, no code changes.** It lists where Beverly decides "close enough", what runs after an edit and in what order,
the bugs that came from between systems, and the slow paths. Then it does the same for the features still on the
roadmap, since most clashes will come from systems not built yet. It ends with a proposal for the owner to decide:
the tolerance, the shared numbers, the order of steps, who owns what, and the stages.

Nothing here is decided until the owner says so. Decisions go in the last section with their dates.

---

## Part A: the built code

### A1. Where "close enough" is decided

Beverly works in metres, in `f32`. There is no project tolerance: each system picks its own numbers, written into
the code. They fall into four kinds, and only the first is a tolerance in the brief's sense.

**1. Tolerances (is this the same point / touching / flat?)**

| Where | Value | What it decides |
|---|---|---|
| `editor/pen.rs:176`, `editor/scatter.rs:101` | 1e-6 m² (1 mm) | two outline points are the same point (dedup) |
| `level/hierarchy.rs:119` | 1e-12 m² (1 µm), quaternion 1e-9 | an attached part has already followed its parent |
| `bucket.rs` `TOUCH` | 0.05 m | two parts touch (Bucket fill, Connected only) |
| `bucket.rs:78` | colour gap 0.02 | "the same colour" when no material is laid (Material fill) |
| `editor/edit/interact.rs:136`, `editor/pen.rs:370,400` | normal.y > 0.999 | a surface is level, so grid snap applies |
| `nav.rs:60` | up.y < 0.999 | a part is upright (navigation's box short-cut) |
| `grass/sheets.rs:40` | normal.y > 0.7 | a face is a floor or ceiling, not a wall (sheet layout) |
| `grass/sheets.rs:83` | 1 mm | a face is too thin to grow on |
| `shapes.rs:139` | 1e-5 | a ray hit is in front of the ray |
| `scatter.rs:86` | 1e-4 m | a segment is long enough to walk along |

**2. Design numbers (what the game means by "a floor", "a step", "walkable")**

These aren't tolerances, but they're the same kind of trouble: one idea, several numbers.

| Idea | Where, and its number |
|---|---|
| Walkable slope | Navigation `slope` 45° (setting) · Bucket "Floor" up ≥ 0.7 (≈ 45.6°, fixed) · the walker: bevy_tnua's default (not set by Beverly) · paint and scatter rules: per stroke (user's choice, fine) |
| Highest step | Navigation `step` 0.45 m (setting) · pen Ground stretch `STEP` 0.52 m (fixed) · the walker: tnua's float height 1.1 and cling 0.3 (not tied to either) |
| "The same floor" | Bucket `FLOOR` 0.3 m above or below the height clicked |
| Grid snap | 0.25 m, written five times (`interact.rs` ×3, `pen.rs` ×2); scale snap 0.05; turn snap 15° |
| Walker width | the walker's capsule radius 0.4 · Navigation `width` 0.8 (agrees, by hand) |

**3. Display and quality allowances (about pixels and looks, not data)**

The bucket outline's lift off a face (0.01 m) · the paint shader's soft edges (slope ±0.03, height ±0.05) · the
grass sheets' spot size (`TEXEL` 0.5 m) · the paint texture tile (2 m) · the vector UI's tessellation (0.02 px).
These are fine as their own numbers; they should be named as display numbers and not reused for data.

**4. Guards against dividing by zero**

About 30 of the `1e-3 … 1e-12` values (`.max(1e-6)`, `den.abs() < 1e-9` and the like) in `widget.rs`,
`camera.rs`, `light.rs`, `texture.rs`, `paint.rs`, `curve.rs`, `scatter.rs`, `grass/sheets.rs`. They decide nothing
about the data. They should become named constants and be left alone.

**Things that ignore any project number:** physics uses Avian's default contact slop and solver settings;
navigation uses Recast's default cell size and height (derived from the agent radius); the walker uses tnua's
defaults for slope and step. None of these are worse than the others, but none of them agree.

**Precision ceiling.** In `f32`, one step of precision is about 1 mm at 10 km from the origin. A tolerance finer
than 1 mm can't hold at the far edges of a big world until `big-coords` (a floating origin) exists.

### A2. What runs after an edit, and in what order

Every edit, from the panels, the 3D handles, keys, the MCP, undo or redo, lands in `level` (the `Level`
resource), and then:

**Each frame (the `Update` chain in `editor.rs`), in this order:**

1. `follow_parents`: attached parts follow their parent (**writes** child `pos` and `rot`).
2. `sync`: the 3D entities match the parts (draws only).
3. `paint`: the painted layers match the strokes (draws only).
4. `grass`: the grass sheets match the strokes and materials (draws only; never stored).
5. `stage`: the sun and fill light follow the level and the zones around the camera (draws only).
6. `commit`, which waits until nothing is being dragged, then:
   1. `pen::strips`: each pen curve's strip and fill strokes are made again (**writes** strokes with `curve: Some`).
   2. `scatter::scatters`: each curve's scattered items are made again (**writes** whole parts with
      `scatter: Some`, adds and removes parts).
   3. `Editor::commit`: attached parts follow again (as in step 1); `bucket::fills` remakes the fill strokes
      (**writes** strokes with `bucket: Some`); the change becomes one undo step (`Delta`); if an instanced part's
      contents changed, the editor asks "Change all, or make this one Unique?".

**When playing:** physics bodies are made from the level once at the start (`physics::spawn`), navigation is worked
out once at the start, and moving and loose parts go back to their places at the end (`homes`).

**On demand:** navigation (Build navigation, or play), the bake cache, model splitting.

What each step may change today is written only in its own doc comment. Nothing checks that two steps keep off
each other's fields.

### A3. Bugs from between systems

| # | Bug | Which steps | State |
|---|---|---|---|
| 1 | A bucket fill on copied floors asked "Change all, or make this one Unique?" | the instances step compared strokes the bucket step owns | fixed 2026-11-22: copies compare hand-painted strokes only |
| 2 | An MCP change and the fill strokes it caused were two undo steps | `bucket::fills` ran outside the commit | fixed 2026-11-22: moved into `Editor::commit` |
| 3 | **An MCP change that moves ground under a pen curve or a scatter makes two undo steps** | 22 places in the MCP tools call `Editor::commit` directly; only the paint tools call `pen::strips` and `scatters` first, so the frame's `commit` makes the strips and items again as a second step | open (found by reading; to confirm with a test) |
| 4 | **Moving or changing a scattered item is silently undone later** | `scatters` owns those parts, but nothing stops the user editing one. The edit stays until the curve or the ground next changes, and then it is quietly reverted | open (found by reading) |
| 5 | A fill outline was stored in world coordinates and broke on a moving part | pen fill vs hierarchy | fixed 2026-11-20 |
| 6 | Navigation and the walker can disagree on what is walkable | `Navigation.step` / `slope` vs tnua's own defaults | open (by reading; a walker can fail a step the path uses, or the reverse) |
| 7 | The paint and grass draw one frame behind a fill | `paint` and `grass` run before `commit` in the chain | cosmetic |

### A4. Slow paths (not yet timed)

The brief says to time before guessing, so these are **candidates**, to be timed by stage with the performance
meter before any change:

- **Three different "has anything changed?" hashes** walk the whole level: `pen::world_stamp`,
  `bucket::level_stamp` and the grass's per-part key, which formats each stroke's rules as text. They run on every
  commit or every level change.
- **No spatial index anywhere.** Picking, surface placement, pen projection and the walker's checks each scan every
  part (`world::raycast`). The same goes for Bucket's Connected only, which compares parts pairwise, and for
  scatter's ground checks.
- **Whole-level work at play start:** physics bodies and navigation are built from everything at once.
- **Grass sheets** work out coverage for every half-metre spot against every stroke piece on the part. This is
  throttled while painting, but it isn't indexed.

---

## Part B: the features still to come

The roadmap has about 170 features left. Most clashes will come from them. Grouped by what they will write, these
are the places where two or more of them would decide the same thing.

| Shared thing | Planned features that will write or decide it | The clash to prevent |
|---|---|---|
| **A part's transform** | hierarchy (built), movers (built), assemblies, mounts, attach-anywhere, build-mode, sockets, kits, vehicles, physics, runtime-edits, structures, tethers, two-point | two systems placing one part in the same frame; a snap fighting a joint |
| **A part's collider** | shapes (built), asset-prep collision, prism-import roles, destructibles, assemblies, vehicles, ragdolls, caves, terrain | an import refreshing a collider the user tuned; a snap or a scale changing a fitted collider |
| **Surface identity** (footsteps, grip, hiding, burning) | surface tags (built), material `sounds_like` (built), Mixes' tags (built), open-tags, prism-import material words, terrain-paint, pack-ground-water, damage, fire, stealth | four places say what a surface is; footsteps could disagree with grip |
| **Painted surfaces** | brush, pen, bucket and Mixes (built), terrain-paint (per vertex), decals, foliage scatter | two painting models (strokes on parts, vertex blends on terrain) with different rules |
| **Things placed for you** | pen scatter (built), foliage scatter, dungeon-gen, spawner, kits, array-tool, destructibles' pieces, backdrops | a generator overwriting what the user edited (bug A3-4, everywhere) |
| **"Walkable"** | navigation (built), char-modes, moves-*, swim, flight, dynamic-nav, structures, build-mode, terrain, caves | the walker, the path-finder and each move judging slopes and steps by their own numbers |
| **Snapping** | grid (built), sockets, kits, build-mode, attach-anywhere, vehicles, game-editors, array-tool | several snaps wanting the same drag |
| **The environment** (sun, fill, sky, haze, weather, wind, post) | lighting and zones (built), world-settings, sky, time-of-day, atmosphere, wind, pack-weather, post, ssao-ssr, styles, baked-light, gi tiers, quality-tiers, graphics-settings, accessibility | ten systems setting the same light or fog; the grass's own wind vs the level's wind vs wind zones |
| **Gravity and time** | zones (built), space, char-modes zero-g, flight, jet-grapple, time-of-day speed, slow motion (debug-tools, defence's perfect dodge), photo-mode and screens pausing | two slow-motions stacking; a space level and a zone both setting gravity |
| **Input** | input map (built), control-target, mounts, vehicles, build-mode, screens, hud, debug-tools, photo-mode, game-editors, accessibility remapping | two systems reading the same key in one frame |
| **The camera** | camera parts and the walker's follow (built), char-camera, control-target, mounts, vehicles, animatics, photo-mode, plane-lock, accessibility shake reduction | who has the camera right now; "camera behaviour is always the creator's choice" (owner rule) |
| **Values** (health, fuel, power, money, flags) | variables, readouts, abilities, health, resource-flow, economy, inventory, quests, achievements | two systems owning the same number; readouts writing what they should only show |
| **Navigation data** | navigation (built), dynamic-nav, runtime-edits, structures, build-mode, destructibles, streaming | when the walkable area is rebuilt, and from which state of the level |
| **The level's state while playing** | movers and loose parts (built), runtime-edits, test-snapshot, save-system, autosave, levels, streaming, away-sim, multiplayer | edits during play leaking into the level file; undo seeing play changes |
| **Imports** | prism-import, asset-prep, asset-library, models-split (built), lod, impostors, tessera | a re-import discarding the user's changes to the imported pieces |
| **Detail and cost** | lod, impostors, tessera, quality-tiers, graphics-settings, texture-pipeline, shadow-cache, virtual-shadows, budget-readout | a quality tier and a per-model setting both choosing the draw path |
| **Sound** | sounds and surface sets (built), mixer, ambience, music, zones' sound layer, sound-synth | which footstep set plays; a zone's reverb against the mixer's |

---

## Part C: the proposal

### C1. One tolerance per project

- **`precision`: one distance, stored in `project.bev`, default 1 mm** (the `f32` ceiling at 10 km, and well under
  anything a player can see). It's read everywhere through one accessor, the same in every level of the project,
  and never worked out from the size of what is asking.
- **Derived by stated ratios:**
  - the same point = precision (the pen's and scatter's dedup, the hierarchy's "already followed");
  - touching = 50 × precision (5 cm: Bucket's Connected only, and later sockets and structures). Hand-placed parts
    at a 0.25 m grid sit this close without meeting exactly;
  - level and upright: a face counts as level when its height changes by no more than the precision over its own
    length, which replaces the fixed 0.999;
  - fits and bakes (navigation cells, colliders fitted to models, LOD simplification) stay finer than "touching",
    so a fitted thing never misses what it should meet.
- **A setting for odd scales,** with presets (Tabletop 0.1 mm · Human 1 mm · Vehicle 5 mm · Planet 5 cm) and a typed
  value.
- **Display allowances** are named in one place, apart from the data numbers.
- **Guards** become named constants.

### C2. One set of shared design numbers per project

These aren't tolerances, but each is one idea with one number:

- **The walker profile** (width, height, step, slope) is used by the walker, navigation, Bucket's Floor, the pen's
  Ground stretch and, later, every character's default. Today's Navigation settings become this profile. A
  character file can override it for that character only.
- **The grid** (move step, scale step, turn step) is used by every snap. It's 0.25 m, 0.05 and 15° today, set in one
  place.

### C3. The order of steps after an edit

One pipeline, `Editor::commit`, which every edit goes through (panels, handles, keys, MCP, undo). The order, and
what each step owns:

| # | Step | Owns (only it may change) | Never changes |
|---|---|---|---|
| 0 | **The user's edit**, as made | whatever the user touched | — |
| 1 | **Attached parts follow** | child transforms | contents, strokes |
| 2 | **Instances** (change all / unique) | the contents of instances | boxes (name, place, size), generated strokes |
| 3 | **Generated parts** (pen scatter; later foliage scatter, dungeon-gen, array-tool) | parts marked as theirs | any other part |
| 4 | **Generated strokes**: pen strips and fills, then bucket fills | strokes marked as theirs | hand-painted strokes and their order |
| 5 | **The undo step**: everything above is one step | — | — |
| 6 | **Derived, never stored**: paint layers, grass sheets, colliders, navigation (marked out of date, or rebuilt), zone blends | their own caches | the level |

**No step undoes a decision another step owns.** When the user edits something a generator owns (a scattered
item), the editor doesn't silently revert it later. It does the nearest thing and says so: the item becomes the
user's own ("Turned into an item: it no longer follows the curve"), as Turn into items does now.

### C4. Who owns what, for the systems to come

Each shared thing from Part B gets one owner and a written precedence. Features plug into these; a new one says
where it sits.

- **Transform:** the parent → joints and assemblies → physics (playing only) → snapping (editing only, never while
  physics owns it). Snapping never changes a collider or a size.
- **Collider:** asset-prep's choice → prism-import roles → the shape's own. A re-import refreshes only what the
  user hasn't changed (the user's changes are overrides, kept).
- **Surface identity:** the topmost painted layer's material → the face's material → the part's `sounds_like` → the
  part's tags. Tags combine; sounds take the first that says anything. Footsteps, grip, stealth and fire all read
  this one answer.
- **Snapping, in order:** a socket → a kit's grid → standing on a surface → the project grid → free (Ctrl).
- **Environment stack, lowest first:** project defaults → the level's World → time of day → weather (modules) →
  zones (in level order; a later one wins; each blends over its edge) → the camera's own post → momentary effects
  (events) → the quality tier's caps → accessibility (flash and shake reduction always last). There's one wind
  (`wind`); the grass's own wind becomes a fallback that only runs while no level wind exists.
- **Gravity:** one value, read at each body: the level type (space: 0) → zones → a character mode's override.
  Jetpacks and grapples are forces, not gravity.
- **Time:** one time scale, as a stack: screens' pause → photo mode → debug slow motion → gameplay slow motion →
  time-of-day speed. The strongest wins; they don't multiply.
- **Input, first to claim a key wins:** the editor's own UI focus → modal screens → debug tools → the control
  target's input map (vehicle, mount, turret, character) → the rest.
- **Camera:** animatics → photo mode → the control target's camera → level cameras → the default follow. The
  creator chooses each rig; accessibility's shake reduction applies last.
- **Values:** each value has one owner (abilities own a character's values, variables own flags, resource-flow owns
  flows, economy owns money). Readouts and the HUD only read. The save system stores what each owner declares.
- **The level while playing:** play never writes the level. Movers, loose parts, runtime edits, digging and
  building all live in the runtime-edits layer. Test-snapshot throws it away or offers it; the save system stores
  it; undo never sees it.
- **Navigation:** worked out from colliders after steps 1 to 4, with the walker profile. While playing,
  dynamic-nav rebuilds only the cells that runtime edits touched.
- **Detail:** the quality tier sets the ceiling, a model's own setting chooses within it, and the budget readout
  reports both.

### C5. Keeping the rules current (definition of done)

Proposed addition to `CLAUDE.md`: *"A feature that changes the level (or any shared thing in `docs/orderliness.md`)
is not done until it says which step it is in, what it owns, which tolerance and shared numbers it uses, and has
a test that another step can't undo its decision."* Each roadmap entry's note names its step and owner.

### C6. Build and warn, preview fast, old data keeps behaving

- **Build and warn:**
  - an action that can't be done as asked does the nearest thing, marks the affected parts in the view, and says
    what was done differently, in the island;
  - the warning stays until the next action;
  - refuse only when there is nothing to make;
  - a drag may hold the last good value and say why.
- **Preview fast, commit exact:** this is already the shape of the pen, the bucket and the grass (a preview while
  dragging or hovering, made exactly on the commit). Keep it as the rule.
- **Old projects:** fills, strips, scatters and grass are rules that are worked out again whenever a level opens.
  So **changing any number in C1 or C2 changes how old levels look.** A project opened after a change keeps the
  numbers it was made with (stored in `project.bev` only when they differ from the defaults, and said once in the
  island). New projects start at the defaults, and a project at the defaults saves byte for byte as before.
- **Compare before and after every stage:** the test projects' screenshots and the saved files.

### C7. Stages (each tested and released, as Prism did)

1. **Time and confirm.** Time the A4 paths on a big level with the performance meter. Write tests for bugs A3-3 and
   A3-4 that fail.
2. **One pipeline and one tolerance:** every edit through `Editor::commit` with steps 1 to 4 inside it (fixes
   A3-3); `precision` in the project with one accessor; the dedup and follow checks on it.
3. **Shared numbers:** the walker profile and the grid, each read from one place (fixes A3-6); level and upright
   tests that use length; Bucket's floor band and touching derived.
4. **Ownership enforced:** a test per rule ("a fill never changes a hand-painted stroke", "scatter never moves a
   part it doesn't own", "an instance question never sees generated strokes"); editing a generated item turns it
   into the user's own, with a warning (fixes A3-4).
5. **The setting:** Precision with presets in project settings, its warning for odd scales, old projects keeping
   their numbers.
6. **Sweep:** name the guards and the display numbers; one spatial index for picking, placement, pen, bucket and
   scatter if the timings say so.

Part C4 needs no code now. It becomes the rule each planned feature follows when it is built, and the roadmap
entries for wind, zones' layers, runtime-edits, prism-import, sockets and the character modes get a line pointing
here.

---

## Open questions for the owner

1. **Precision default:** 1 mm, with presets? Or a different default?
2. **Touching = 50 × precision (5 cm)?** It's coarser than a CAD program would use, but level parts are hand-placed
   on a 0.25 m grid.
3. **One walker profile** that navigation, the bucket's Floor, the pen and the default walker all share?
4. **The order in C3**, and **editing a generated item turns it into the user's own** (rather than refusing the
   edit, or letting the generator win)?
5. **The stacks in C4** (environment, time, input, camera, surface identity): agree, or change any order?
6. **The definition-of-done line** in C5 for `CLAUDE.md`?
7. **Stages in C7:** in this order, ahead of the roadmap's next features, or folded in between them?

## Decisions

(none yet)
