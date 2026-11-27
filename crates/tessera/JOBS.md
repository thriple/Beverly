# Tessera: jobs for the nightly runs

This branch (`tessera`) holds Tessera, Beverly's virtual-geometry plugin: a fork of Bevy 0.20.1's meshlet renderer plus Beverly's own parts. The research and decisions are in `docs/tessera.md`; read it first. The roadmap entry is `tessera` in `editor/features.json`.

Each night a scheduled run starts a fresh session. It does **one** job from the list below (or one clear part of a big job), reports what it did, and stops. Interactive sessions review the work, answer the questions at the bottom, and merge finished parts into main.

## Rules for unattended runs
1. **Branch only.** Work on `tessera`; push only to it.
   - Never push to `main`, never force-push, and never put `[release]` in a commit message.
   - Pushes here don't start Windows builds: the workflow builds main only.
2. **Start of a run:**
   1. Run `git fetch origin`, check out `tessera`, then `git merge origin/main`.
   2. If the merge conflicts, abort it, write the conflict under *Questions for the user*, and stop. In `editor/features.json`, take main's version, then re-apply only the `tessera` entry's note.
3. **Pick the first job below that is not ✅ done and not ⏸ waiting.** Do that job only.
   - If it is too big for one run, do a clear part of it and mark it 🔶 with what's left.
   - Don't start a second job in the same run. Each run should use only part of a usage window.
4. **No taste calls.** If the work needs a decision about how something looks, feels, is worded or is set up for the user, don't guess. Write the question under *Questions for the user*, mark the job ⏸, and move to the next job, or stop if there is none.
   - Engine API shapes that only code sees are fine to decide. Explain them in the run log.
5. **Leave the wiring in place.** When a job needs something that main hasn't built yet, build this side behind clean seams and write what main must call or provide under *Wiring main needs*:
   - `GpuCaps` is built on main (`editor/src/gpu.rs`); read it, don't make another
   - split meshes from `models-split`
   - raster hooks from `shaders`
   - the cache directory from `bake-cache`
   - the SMAA swap is built on main (editor and player cameras on `Msaa::Off` + SMAA)
6. **Keep main's behaviour.** With the `tessera` feature off, the editor must build and behave exactly as on main. Every run checks:
   - `cd editor && CARGO_INCREMENTAL=0 cargo build && cargo test` (feature off)
   - the same with `--features tessera`, once that feature exists
   - `DISPLAY=:99 BEVERLY_LITE=1 tools/run-tests.sh` under Xvfb: `Xvfb :99 -screen 0 1600x960x24 &`
7. **Disk:** if space runs low, delete `editor/target/debug/incremental` and stale `deps`. Never delete source.
8. **Roadmap:** edit only the `tessera` entry's `note` in `editor/features.json`. Don't run `tools/roadmap.py`; main does that after merging.
9. **End of a run:**
   1. Add an entry to *Run log*: the date (Guam time), the job, what changed, tests run and their results, numbers measured, and anything that only the user's RTX card can check.
   2. Commit with a plain message (no `[release]`) and the usual co-author lines, then push the branch.
10. Follow `CLAUDE.md`, except that this branch is the agreed exception to "main only". Never put model names or ids in files.
11. **Bevy's files keep Bevy's shape (user, 2026-11-20).** Don't split or reorganise files in `crates/bevy_pbr`, even past
    CLAUDE.md's size limits, so they stay easy to compare with upstream. The size rule applies to `crates/tessera` and the editor.

## Layout
- `editor/Cargo.toml` gets `[patch.crates-io] bevy_pbr = { path = "../crates/bevy_pbr" }`.
  - `crates/bevy_pbr` is Bevy 0.20.1's `bevy_pbr`, vendored. Its **first commit is the untouched copy**, so every later change shows as a diff.
  - It keeps Bevy's licence files (MIT / Apache-2.0).
  - With the feature off it must behave exactly like upstream.
- `crates/tessera`: Beverly's side, with no dependency on the editor crate:
  - the GPU capability probe
  - baking (mesh to cluster hierarchy, with the cache key)
  - a test-mesh generator
  - glue the editor calls
- The editor's `tessera` Cargo feature turns on Bevy's `meshlet` / `meshlet_processor` features and `tessera`. It is off by default.

## Jobs (in order; each depends only on the ones above it)

1. ✅ **Vendor and build.** (2026-11-20)
   - Vendor `bevy_pbr-0.20.1` into `crates/bevy_pbr`, untouched, in its own commit. Add the patch and the `tessera` feature, and make both builds pass.
   - Fill in `crates/tessera` (a crate named `tessera`) with a procedural dense test mesh (a displaced, subdivided sphere; about 1 M triangles; seeded; no third-party assets).
   - **Check the GPU here:** can lavapipe (Mesa's software Vulkan, `/usr/share/vulkan/icd.d/lvp_icd.json`) provide the six features (`docs/tessera.md`)?
     - Main's `gpu-setup` already found that lavapipe reports all six (`beverly::gpu::GpuCaps::virtual_geometry()` is true here; the editor logs `Graphics card: ...`). Confirm it with that, no separate check program.
     - Since they are there, render the test mesh with Bevy's meshlet path off-screen and save a screenshot.
     - Record the answer at the top of *Run log* as **GPU-testable here: yes/no**. Later jobs use it to decide what they can check themselves.
2. ✅ **No exit on a missing feature.** (2026-11-21)
   - In the fork, replace the `exit(1)`s (`mod.rs` feature check, MSAA check) with a disabled state that leaves the app running.
   - Limit the MSAA check to views that actually draw virtual geometry.
   - Use main's `beverly::gpu::GpuCaps` (`editor/src/gpu.rs`: probed before the app is built, Vulkan chosen when it can run virtual geometry, the renderer pinned to that adapter). Don't add a second probe.
     - The `tessera` crate can't depend on the editor, so the editor passes what it needs in (e.g. a `bool` or the `WgpuFeatures`) when it adds the plugin; turn the plugin's parts on or off from that.
     - If Tessera needs more from the card (mesh shaders, ray query, limits), add a method on `GpuCaps` in `gpu.rs` on this branch, and note it under *Wiring main needs* so main takes the same change when it merges.
3. ✅ **Bug fixes** (the list in `docs/tessera.md`): the five below done 2026-11-22, the fill rule 2026-11-23.
   - the prepass and deferred material bind group 2 → 3
   - `unclipped_depth` on the directional-shadow pipeline
   - `DEFERRED_PREPASS` carrying over in the prepass loop (confirm first)
   - the stale imports in the raster shaders
   - the cluster-slot cost in the doc comment

   One commit per fix, each message saying why. A top-left fill rule in fixed point for the software rasterizer, if GPU-testable here; otherwise ⏸ with a note.
4. ✅ **Baking library** (`tessera::bake`): (2026-11-25)
   - Any Bevy `Mesh` (a glTF primitive) to `MeshletMesh`. Fill in missing normals (smooth by angle) and UVs (zero), drop extra attributes, weld and index.
   - The cache is main's, already built (`editor/src/bake.rs`: `Bakes`, `bake::key(kind, parts)`, the folder from `Bakes::default_root()`; exports carry the entries they used). Don't make a cache folder or a hash of your own: `tessera::bake` turns a mesh into bytes (and bytes back into a `MeshletMesh`) and names the `kind` ("meshlet") and the key `parts` (source bytes, bake settings, format version); the editor-side glue stores them with `Bakes`. `vg-bake` may use a plain temp folder.
   - Keep the coarsest level of the cluster hierarchy reachable as an ordinary mesh (`proxy_mesh`): main's `realtime-gi` (ray tracing against a proxy, as Nanite does) and `backdrops` (a far, low-poly stand-in) both want it.
   - A `vg-bake` binary that bakes the test mesh and prints time, output bytes and bytes per triangle.
   - Tests: the result is the same every time (or say why not), a save/load round trip, a wrong-version file is rejected.
5. ✅ **Instances by change.** (2026-11-20, in an interactive session ahead of order, at the user's request)
   - Rewrite the fork's `InstanceManager` so it updates only what changed, and cache the per-view bind groups (both are Bevy TODOs).
   - Add a headless benchmark with 10k and 100k instances, and record the CPU time per frame before and after.
6. ✅ **More of the material.** Texture tiling 2026-11-26, vertex colours 2026-11-27, the second UV set 2026-11-28, each with a screenshot test against the normal renderer.
   - Texture tiling (the UV transform applied to the UVs *and* the analytic derivatives in `visibility_buffer_resolve.wesl`).
   - Vertex colours and a second UV set in the asset format (bump the version).
   - If GPU-testable here: a screenshot test against the normal renderer.
7. ⬜ **The 32-bit fallback.**
   - A hardware-only raster path that writes `cluster << 7 | triangle` into an R32Uint target with a normal depth test. It needs no 64-bit atomics, `SHADER_INT64` or `SUBGROUP`.
   - Chosen by `GpuCaps`, and it keeps the DAG LOD and the culling.
   - Test it on lavapipe even if the full path can't run there.
8. ⬜ **Raster hooks (programmable raster).**
   - A material can supply a WESL vertex-offset function, with its maximum distance for culling, and an alpha-mask function.
   - Clusters are sorted into bins by raster shader; each bin gets its own software and hardware raster pipelines and its own material pass.
   - Keep the hook signatures generic, so main's `shaders` feature can fill them from any project shader. Write the contract under *Wiring main needs*.
9. ⬜ **Streaming** (several runs):
   - Clusters packed into pages by DAG group.
   - The GPU records which groups it wanted but didn't have; the CPU loads those pages and drops pages that weren't used recently.
   - Report memory use against `cluster_buffer_slots`.
10. ⬜ **Mesh shaders.**
    - Find out what wgpu 30 offers (`EXPERIMENTAL_MESH_SHADER` or the current name) on Vulkan and DX12, and write it up.
    - If usable, add a mesh-shader hardware raster path chosen by `GpuCaps`; otherwise ⏸ with the findings.

**Waiting on interactive sessions or main** (don't start these):
- Wiring into the editor's cameras (the SMAA swap and `GpuCaps` are built on main).
- Drawing imported models through split meshes (`models-split` is built on main; the wiring is an interactive job).
- Beverly's paint layers inside the material pass (`shaders` is not built on main yet).
- Bake status in Assets and the LOD drawer.
- Anything only the user's RTX card can show.

## Roadmap links (review of 2026-11-20)
Main's roadmap changed shape: every unbuilt feature has a `stage` (the build ladder in `tools/roadmap.py`), and the stage list puts `tessera` in Foundations, the first stage, so main's later stages can count on it. Built on main now: `gpu-setup`, `models-split`, `bake-cache`, `perf-meter` and, since 2026-11-20, change tracking (the level's collections are `Tracked`) and the part hierarchy (parts can be attached to parts and carried by movers). Tessera sees only Bevy entities, so neither touches it, but keep these in mind:
- Parts that are carried by a moving part get their `Transform` written each frame only when it changed (`script::carry`), so `Changed<Transform>`, which job 5's instance manager reads, stays quiet for parts at rest.
- Later features that lean on Tessera, so keep their seams clean:
  - `realtime-gi` needs a proxy mesh (job 4); `virtual-shadows` and `lod` need to know which meshes are virtual geometry and which fall back (classic LOD, impostors); `foliage` wants masked leaves and wind through the raster hooks (job 8; the hooks should do both); `export-hardening` needs one shared player that carries the runtime, with the fallback when the card lacks the features.
  - `budget-readout` and `quality-tiers` will show Tessera's memory (job 9) and pick the path from `GpuCaps` (job 7).
- A real-card baseline is coming from main's `beverly-bench` (a separate download; scenes in `editor/src/bench/scenes.rs`, among them a field of 100k-triangle tiles up to 64 million triangles and a lit town with moving headlights). When a job wants numbers against the normal renderer, build the same field with Tessera, and read `Bench` scenes rather than inventing new ones.
- On a merge conflict in `editor/features.json` keep taking main's version and put back only the `tessera` entry's note (it also carries a `stage` now; leave it as main has it), as the last run did.

## Wiring main needs
(Unattended runs add entries here.)
- **Container set-up (job 1, 2026-11-20):** winit on X11 needs `libxkbcommon-x11-0`, which this container didn't have
  (`vg-render` panicked in `xkbcommon-dl`). The run installed it by hand; since 2026-11-20 the environment's setup
  script installs it (with clang and cmake for METIS).
- **Turning Tessera on in the editor and player (job 2):** add `MeshletPlugin { cluster_buffer_slots, supported:
  caps.virtual_geometry() }`, with `caps` the `GpuCaps` that `gpu::render_plugin` put in the app (as `vg-render` does).
  With `supported` false nothing is added on the render side and meshlet meshes aren't drawn (the fallback is
  later work). No `GpuCaps` change was needed for this job.
- **Baking through main's cache (job 4):** the editor-side glue (not written yet; it belongs with drawing models
  through split meshes) bakes a model's mesh with `tessera::bake::bake(mesh, &BakeSettings::default())` inside
  `Bakes::request(tessera::bake::KIND, key, label, ...)`, where
  `key = bake::key(KIND, &tessera::bake::key_parts(source_bytes, &settings))` (as `vg-bake` does), and reads it back with
  `tessera::bake::load(bytes)` (the `MeshletMesh`) and `tessera::bake::proxy_mesh(bytes)` (the coarsest level as
  an ordinary mesh, for `realtime-gi` and `backdrops`). A `BakeError::Format` or `Bytes` error on reading means the
  entry is from another version: bake again.
- **Known limit, camera viewports (job 6):** the meshlet passes size their depth targets to a camera's viewport but
  draw into the whole render target, so a camera with a `viewport` smaller than its target fails wgpu validation
  ("attachments have differing sizes") in `meshlet_main_opaque_pass`. Beverly's cameras don't use viewports today;
  a split view or picture-in-picture would need this fixed in the fork first, or render to its own image as
  `vg-render tiling` does.
- **Merging job 1 into main:** `editor/Cargo.toml` gains the `[patch.crates-io] bevy_pbr` (path `../crates/bevy_pbr`),
  the off-by-default `tessera` feature, the optional `tessera` dependency (`../crates/tessera`) and the `vg-render`
  binary (`required-features = ["tessera"]`, so default builds and the release zip don't contain it). Main's
  `.github/workflows/windows.yml` builds without the feature, so nothing there changes.

## Questions for the user
(Unattended runs add questions here. Interactive sessions answer them and remove them.)

## Run log
(Newest first.)

**GPU-testable here: yes.** Lavapipe (`llvmpipe (LLVM 20.1.2, 256 bits)`, Vulkan, Cpu) has all six features
(`GpuCaps::virtual_geometry()` true, the editor's `Graphics card:` log says `virtual geometry: yes`) and really draws
Bevy's meshlet path: `vg-render` saves a lit, shadowed picture of the test mesh. Slowly, so it is a correctness check,
not a speed one.

### 2026-11-28 (Guam): job 6, the second UV set ✅ (job 6 done)
- **Merge:** main merged in cleanly.
- **Format:** `MeshletMesh.vertex_uvs_b` holds UV1, uncompressed like UV0, empty when the mesh has none.
  - Bevy's asset version 4 → 5; `tessera::bake::FORMAT_VERSION` 3 → 4.
  - `from_mesh` accepts `UV_1` and/or `COLOR` after `UV_0`. In the packed vertex UV1 comes before the colour, so
    the colour's offset is 32 or 40.
- **GPU:** its own buffer, allocated in step with `vertex_uvs` (8 bytes per vertex both). Meshes without UV1 upload
  their UV0 again, so `uv_b` is `uv` as in the normal renderer. That costs 8 bytes of VRAM per vertex for every
  meshlet mesh, the same trade as the colours.
- **Shaders:**
  - material-shade binding 9 holds UV1; the resolve outputs `uv_b` and its own analytic gradients
    (`ddx_uv_b` / `ddy_uv_b`); both meshlet material pipelines define `VERTEX_UVS_B`
  - `SampleBias` carries both sets' gradients, both multiplied by the UV transform's linear part
  - each of `pbr_fragment.wesl`'s 14 meshlet sample sites now uses the gradients of the set its coordinates come
    from (`@if(STANDARD_MATERIAL_<texture>_UV_B)`), and so does the prepass normal map
- **Bake:** `tessera::bake::prepare` keeps `UV_1` and welds on it. New test `keeps_the_second_uv_set` (with colours
  too, so the moved colour offset is exercised).
- **Screenshot test: `vg-render OUT uv1`.** A plane whose UV1 tiles the checker 4 times (UV0 would show it once),
  with the base-colour texture on `UvChannel::Uv1`, compared with the normal renderer: **0.44** per channel (limit
  2.0); both halves show the 4× tiling. Reading UV0 instead would show a quarter as many cells and fail by far.
  - Not measured separately: using UV1's gradients rather than UV0's. Wrong gradients would pick mips 4× off, the
    error `tiling` measured at 5.51.
- **Checks:**
  - `vg-render`: draw, grid (0 holes), tiling (0.77), colors (0.05), uv1 (0.44), unsupported, msaa and device all
    exit 0
  - `vg-bake`: 1,310,720 triangles in 19.8 s, 20,757,272 bytes (15.8 B per triangle), proxy 78 triangles
  - `tessera` crate tests ✅ (11)
  - feature off: `cargo build` ✅, `cargo test` ✅ (114)
  - `--features tessera`: `cargo build` ✅, `cargo test` ✅ (114)
  - `tools/run-tests.sh` ✅ (all 26)
- **Only the RTX card can show:** the three comparisons on a real driver.

### 2026-11-27 (Guam): job 6, vertex colours 🔶 (the second UV set left)
- **Merge:** main merged in cleanly.
- **Format:** `MeshletMesh.vertex_colors` holds RGBA8 per vertex, the colour sRGB-encoded (8 bits spent where the
  eye sees them; alpha linear), in the same per-meshlet order as the normals and UVs. It is empty when the mesh has
  none, so a mesh without colours costs 89 bytes more on disk, not 4 per vertex.
  - Bevy's asset version 3 → 4; `tessera::bake::FORMAT_VERSION` 2 → 3.
  - `from_mesh` accepts an optional `COLOR` (`Float32x4`) after `UV_0`.
- **GPU:** the mesh manager gives colours their own `PersistentGpuBuffer`, allocated in step with the normals (4
  bytes per vertex both, same order), so the meshlets' `start_vertex_attribute_id` finds them as it finds the UVs.
  Meshes without colours upload white, which costs 4 bytes of VRAM per vertex for every meshlet mesh. That was the
  choice that needs no change to the `Meshlet` struct; a per-meshlet colour offset would avoid it.
- **Shaders:** material-shade binding 8 holds the colours; `get_meshlet_vertex_color` decodes them to linear.
  `visibility_buffer_resolve` interpolates them into `VertexOutput.color`, and both meshlet material pipelines
  define `VERTEX_COLORS`, so `pbr_fragment` applies them as the normal renderer does. White for meshes without
  colours changes nothing.
- **Bake:** `tessera::bake::prepare` keeps `COLOR` and welds on it. New test `keeps_vertex_colours`;
  `fills_in_and_drops_attributes` now drops a tangent instead.
- **Screenshot test: `vg-render OUT colors`.** A plane coloured red to green across and blue down, drawn by the
  meshlet path and the normal renderer side by side like `tiling`: **0.05** per channel (limit 2.0). Without this
  work the meshlet side drew white, far past the limit.
- **Checks:**
  - `vg-render`: draw, grid (0 holes), tiling (0.77), colors (0.05), unsupported, msaa and device all exit 0
  - `vg-bake`: 1,310,720 triangles in 28.4 s, 20,759,322 bytes (15.8 B per triangle), proxy 78 triangles
  - `tessera` crate tests ✅ (10)
  - feature off: `cargo build` ✅, `cargo test` ✅ (102)
  - `--features tessera`: `cargo build` ✅, `cargo test` ✅ (102)
  - `tools/run-tests.sh` ✅ (all 25)
- **Left for the next run:** the second UV set, the same way (in step with the UVs, 8 bytes per vertex; meshes
  without it use the first set), with a `vg-render` case using a material on `UvChannel::Uv1`. The proxy mesh
  doesn't carry colours yet; worth adding when `realtime-gi` needs albedo from it.

### 2026-11-26 (Guam): job 6, texture tiling 🔶 (vertex colours and UV1 left)
- **Merge:** main merged in cleanly.
- **The fix:** the meshlet material pass samples textures with `textureSampleGrad` and the analytic UV gradients
  from `visibility_buffer_resolve.wesl`, but `pbr_fragment.wesl` then transforms the UVs by the material's
  `uv_transform` and left the gradients alone (Bevy's TODO). Both gradients are now multiplied by the transform's
  2×2 linear part (chain rule), in `pbr_fragment.wesl` and in `pbr_prepass.wesl`'s normal-map path. The two
  remaining TODOs of that wording are about parallax mapping's UV shift, outside this job.
- **Screenshot test against the normal renderer: `vg-render OUT tiling`.**
  - The scene: a 2 m plane under a mipmapped 256² checker (white and blue, with its whole mip chain), tiled ×8 with a
    repeating sampler, unlit, seen from 0.5 m up so the distance needs high mips. It is drawn twice, as a meshlet
    mesh (baked with `tessera::bake`) and as an ordinary mesh, each by its own camera into its own 400×600 image.
  - The picture shows the two images side by side. It fails if they differ by more than 2.0 per channel on average
    (`TILING_MAX_DIFF`).
  - Measured: **0.77 with the fix, 5.51 without** (Bevy's shaders put back temporarily). Without the fix the
    meshlet side's distant checker stays sharp and aliased where the normal renderer blurs it.
- **Found on the way:** a camera viewport smaller than its target breaks the meshlet passes (wgpu validation). The
  test first split one window between two cameras; see *Wiring main needs*.
- **Checks:**
  - `vg-render`: draw, grid (0 holes), tiling (0.77), unsupported, msaa and device all exit 0
  - feature off: `cargo build` ✅, `cargo test` ✅ (100)
  - `--features tessera`: `cargo build` ✅, `cargo test` ✅ (100)
  - `tools/run-tests.sh` ✅ (all 25)
- **Left for the next run:** vertex colours and a second UV set in the asset format (bump the version), with a
  test of each.
- **Only the RTX card can show:** the same comparison on a real driver; its texture filtering may differ a little
  from lavapipe's, so the threshold may need a look there.

### 2026-11-24 and 2026-11-25 (Guam): job 4, baking library ✅
The 2026-11-24 run ended before its log entry; its work is described here with tonight's.
- **Merges:** main merged in cleanly both nights.
- **Fork:**
  - `MeshletMesh::write` / `read` write and read the `.meshlet_mesh` format without async. Bevy's saver and loader
    use them too, so there is one format path.
  - `MeshletMesh::from_mesh_with_proxy` also returns the coarsest level: the meshlets no simpler group was made
    from, taken before the BVH reorders them, as one ordinary mesh. `from_mesh` calls it and drops the proxy.
  - **Fixed: bakes differed whenever two ran at once.** METIS keeps its random generator in global state
    (GKlib's `random.c`: `static mt[]`, seeded at the start of each call), so concurrent bakes interleaved its
    draws. Found because the same-bytes test failed whenever `cargo test` ran in parallel and passed on one
    thread. Now one METIS call runs at a time (`metis_lock`). The editor's bake jobs run on threads, so this would
    have hit real use.
- **`tessera::bake`:**
  - `prepare`: triangle lists only; keeps positions, normals and UVs; fills missing normals with angle-weighted
    smooth ones (Bevy's `compute_smooth_normals`) and missing UVs with zeros; drops other attributes; welds
    identical vertices and indexes.
  - `bake` → bytes; `load` → `MeshletMesh`; `proxy_mesh` → the coarse `Mesh`.
  - `KIND` ("meshlet") and `key_parts(source, settings)`: the source bytes, the settings, and the versions
    (Bevy's asset version and `FORMAT_VERSION`). No cache folder or hash of its own: main's `Bakes` keeps them.
  - The bytes: `TSRA`, `FORMAT_VERSION` 2, Bevy's bytes (length first), then the proxy.
- **The crate** now depends on the fork's `bevy_pbr` (`meshlet_processor`), `bevy_tasks` (a bake outside an app
  makes its own task pool) and `bytemuck`. It carries the same `[patch.crates-io]` as the editor, so it builds and
  tests on its own.
- **`vg-bake`** (editor binary, `tessera` feature): bakes the test mesh into a temp folder under its
  `bake::key`, reads it and the proxy back, and prints:
  - 1,310,720 triangles baked in 23.5 s (24.0 s the night before), read back in 17–35 ms
  - 20,759,233 bytes: **15.8 bytes per triangle**, under the 20–30 B estimate in `docs/tessera.md`
  - proxy: 78 triangles
- **Tests** (`crates/tessera`, 9; all pass, repeatedly and in parallel):
  - the same bytes every time, including four bakes on four threads at once
  - a save/load round trip
  - both version numbers checked: a changed `FORMAT_VERSION` or Bevy asset version is rejected, and so are bytes
    cut short
  - missing attributes filled in and extra ones dropped, from an unindexed mesh
  - the key parts change with the settings
  - the proxy has under a tenth of the triangles and the same size
- **Checks:**
  - `vg-render`: the picture looks as before; grid 0 holes
  - feature off: `cargo build` ✅, `cargo test` ✅ (98)
  - `--features tessera`: `cargo build` ✅, `cargo test` ✅ (98)
  - `tools/run-tests.sh` ✅ (all 24)
- **Only the RTX card / the user's machine can show:** bake time on the user's CPU (here 4 cores, debug build with
  dependencies at opt-level 3).

### 2026-11-23 (Guam): job 3, the fill rule ✅ (job 3 done)
- **Merge:** main merged in cleanly.
- **Software rasterizer** (`visibility_buffer_software_raster.wesl`, Bevy's "fixed-point math and top-left rule"
  TODO):
  - Vertices are snapped to 1/256 pixel, measured from the triangle's bounding-box corner, so the edge functions
    are exact `i32`s.
  - A pixel centre exactly on an edge belongs to the triangle only if that edge is a top or left one (worked out from
    the edge function's gradient, so it doesn't depend on winding). Each pixel of an edge shared by two triangles is
    drawn by exactly one of them, and none is missed.
  - Depth is interpolated from the exact weights.
  - The scanline path keeps Bevy's per-row X interval (now from the integer weights, a pixel wider each way); the
    exact per-pixel test decides, so the old "shouldn't be needed, but there's bugs without it" test is now the real
    test.
  - Triangles over 120 pixels across are skipped, because their edge functions would overflow `i32`. Clusters
    reach the software rasterizer only at 64 pixels or less (`cull_clusters.wesl`), so none should reach that.
  - Committed as `a17719a`.
- **New check: `vg-render OUT grid`.**
  - `tessera::test_mesh::jittered_grid(160, 1)` is a square of 51,200 triangles with jittered inner corners (every
    shared edge at an uneven sub-pixel spot) and 0.02 depth bumps (a flat grid simplifies losslessly and would be
    drawn coarse, through the hardware rasterizer).
  - It is drawn unlit; the run fails if any background pixel shows inside the square.
  - Result: 0 holes. Bevy's float rasterizer, put back temporarily, also gave 0 holes here, so on lavapipe the
    check can't tell the two apart. It guards against cracks coming back. The fix shows on cards whose float
    rounding cracks, and in no double writes on shared edges, which a screenshot can't show.
- **Not the fill rule:** `vg-render`'s sphere has 9–13 single dark pixels on the surface in every run, before and
  after the change. They are something else (shadow acne or detail-level seams); worth a look when the shadow or
  LOD work comes.
- **Checks:**
  - `vg-render`: draw, grid, unsupported, msaa and device all exit 0
  - `tessera` crate tests ✅ (3)
  - feature off: `cargo build` ✅, `cargo test` ✅ (81)
  - `--features tessera`: `cargo build` ✅, `cargo test` ✅ (81)
  - `tools/run-tests.sh` ✅ (all 19)
- **Only the RTX card can show:** whether its float rounding cracked before (the grid case there), and the
  rasterizer's speed; integer math may be slightly slower or faster than float.

### 2026-11-22 (Guam): job 3, bug fixes 🔶 (five done, the fill rule left)
- **Merge:** main merged in cleanly; the branch also had an interactive update to this file (main's bake cache,
  the roadmap links).
- **One commit each:**
  1. `e0e84de`: the meshlet prepass and deferred passes bound the material at group 2; their layout is (view, empty,
     meshlet shade, material), so it replaced the shade bind group and left 3 unset. Now 3, like the main pass.
  2. `e3f061a`: the "unclipped" directional-shadow hardware raster pipeline set no primitive state, so it clipped
     depth like the plain one. It now sets `unclipped_depth` (needs `DEPTH_CLIP_CONTROL`, already required). The
     software rasterizer never clipped depth, so the two now agree.
  3. `94842d4`: confirmed by reading `prepare_material_meshlet_meshes_prepass`: `view_key` is made once per view
     and `DEFERRED_PREPASS` was OR-ed into it in the per-material loop, so every later material was treated as
     deferred. Each material now starts from the view's key.
  4. `3cdbffa`: both raster shaders imported `meshlet_cluster_meshlet_ids` / `meshlet_cluster_instance_ids`, which
     `bindings.wesl` doesn't define and nothing uses. Removed.
  5. `9dc0c6e`: a cluster slot costs 32 bytes of VRAM (four queues of a `u32` pair), not 4 as the doc said; the
     2^25 maximum is 1 GiB.
- **Left: the top-left fill rule in fixed point** for the software rasterizer. It is GPU-testable here, but it needs
  its own check: render two triangles sharing an edge and read the visibility buffer back to show no pixel is
  missed or drawn twice. That is the next run's work. `vg-render`'s picture has a few single dark pixels on the
  surface, there on earlier nights too; possibly the float edge test's cracks, which that check should settle.
- **Checks:**
  - `vg-render`: all four cases exit 0 (draw / unsupported / msaa / device); the draw picture looks as before.
  - feature off: `cargo build` ✅, `cargo test` ✅ (79)
  - `--features tessera`: `cargo build` ✅, `cargo test` ✅ (79)
  - `tools/run-tests.sh` ✅ (all 18)
- **Only the RTX card can show:** fixes 1 and 3 matter only with a prepass or deferred camera, which the editor
  doesn't use yet. Fix 2 shows as shadows of things behind the sun's near plane.

### 2026-11-21 (Guam): job 2, no exit on a missing feature ✅
- **Merge:** main merged in; `editor/features.json` conflicted. Took main's version and put back the `tessera`
  note, keeping main's new "Review 2026-11-20" sentence in it.
- **`MeshletPlugin.supported`** (new field): the app's own check, passed in when it adds the plugin (the editor will
  pass `GpuCaps::virtual_geometry()`; the fork can't see the editor). When false, the asset and loader are still
  registered, so meshlet meshes and entities load, but nothing is added on the render side.
- **`MeshletAvailable`** (render-world resource): the device check at `RenderStartup` now sets it instead of
  calling `exit(1)`. Every meshlet render system runs only while it's true (`meshlet_available`), including
  `prepare_material_meshlet_meshes_prepass`, which `prepass/mod.rs` registers and only gated on `InstanceManager`
  (it panicked without `ResourceManager`; the new `device` case found it).
- **MSAA:** a view with MSAA has the meshlet components taken off and is skipped in
  `prepare_meshlet_per_frame_resources`, instead of exiting. The error is logged once per view, and only when there
  are meshlet instances, so cameras that draw no virtual geometry (thumbnails, previews) are left alone. That camera's
  shadow views draw no meshlet meshes either, since the raster pass runs per camera.
- **`cluster_buffer_slots` over 2^25** is clamped with an error instead of exiting.
- **Checks:**
  - `vg-render OUT CASE`, all four exit 0 and save a screenshot:
    - `draw`: mesh drawn (218k pixels differ from the background)
    - `unsupported` (plugin told no): info line, nothing drawn
    - `msaa` (camera `Msaa::Sample4`): error once, nothing drawn
    - `device` (render device made without `TEXTURE_INT64_ATOMIC`): error naming the missing feature, nothing
      drawn
  - feature off: `cargo build` ✅, `cargo test` ✅ (79)
  - `--features tessera`: `cargo build` ✅, `cargo test` ✅ (79)
  - `tools/run-tests.sh` ✅ (all 18)
- **Only the RTX card can show:** nothing new; the real card has the features, so it takes the `draw` path.

### 2026-11-20 (Guam, interactive session): job 5, instances by change ✅
The user asked for this ahead of jobs 2–4: the meshlet systems' CPU cost per frame grew with the number of copies
and, at 100k, took more than a whole 60 fps frame even with nothing moving.
- **Measuring:** the fork records each meshlet system's CPU time in `MeshletCpuTimes` (render world; Beverly's
  performance meter can show it later). `vg-bench COUNT` (editor binary, `tessera` feature) runs headless with COUNT
  copies of a small baked mesh, a camera and a shadowed sun (5 views), and prints ms per frame over 60 frames in three
  phases: static, 1% moving, 1% replaced each frame (despawned and spawned again with another material).
- **`InstanceManager`** keeps instances from frame to frame (packed; a removed one is replaced by the last).
  - Extract reads only changed entities (`Changed` on the mesh, transforms and layers; `Added` and
    `RemovedComponents` for the markers), plus entities waiting for their mesh to load or reload.
  - Material changes come from `RenderMaterialInstances`: `material.rs` now notes each inserted or removed entry
    (`meshlet_changes`, only while the meshlet queue reads them). Material IDs stay stable, and are handed out
    afresh past 60,000 (the material depth target is 16-bit).
  - Uploads write only the changed elements, merged into runs (one full write past a quarter of the elements).
  - Each view's visibility bits are built in full only for a new view or new view layers; otherwise only the
    changed instances are redone.
- **Per view, kept instead of made each frame:**
  - the 16 counter and indirect-argument buffers (reset with a write each frame, so they gained `COPY_DST`)
  - the depth pyramid (remade when the view's size changes; its own texture cache, so its texture is never handed
    to another view)
  - the bind groups (made again only when one of the buffers or texture views they use is a different one)
- **Numbers** (lavapipe, 4 cores, ms of CPU per frame in the four meshlet systems; "frame" is lavapipe drawing and
  doesn't count):

  | copies, phase | before | after |
  |---|---|---|
  | 10k static | 5.82 | 0.77–1.01 |
  | 10k 1% moving | 6.07 | 0.83–1.08 |
  | 100k static | 27.19 | 1.05–1.45 |
  | 100k 1% moving | 27.09 | 1.25–1.59 |
  | 100k 1% replaced | (not measured before) | 3.83 |

  Ranges are two runs on different workers. What's left at 100k: extract 0.3–0.5 ms (the change checks themselves
  look at every entity), resources 0.7–0.9 ms (80 small resets and the texture cache for 5 views), bind groups
  0.015 ms (were 1.2–1.3).
- **Pictures:** `vg-render`'s picture after the change differs from before by about 700 pixels on the edge,
  the same as two runs of the same code differ (the bake or the culling isn't identical run to run; job 4's
  determinism test should look at this).
- **Checks:** feature off: `cargo build` ✅, `cargo test` ✅ (66); `--features tessera`: `cargo test` ✅;
  `tools/run-tests.sh` ✅ (all 17); `vg-render` ✅.
- **Only the RTX card can show:** the same numbers on a real driver, where making buffers and bind groups costs less
  than on lavapipe, so the saving there is mostly the per-copy work.

### 2026-11-20 (Guam): job 1, vendor and build ✅
- **Vendored** `bevy_pbr-0.20.1` from the cargo registry into `crates/bevy_pbr`, untouched (only cargo's `.cargo-ok`
  marker left out), in its own commit, `f12bb5f`. It keeps `LICENSE-MIT` and `LICENSE-APACHE`.
- **Patch and feature:** `editor/Cargo.toml` patches `bevy_pbr` to the vendored path. The `tessera` feature (off by default)
  turns on `bevy/meshlet`, `bevy/meshlet_processor` and the `tessera` crate. METIS builds with the clang and cmake
  already in the container.
- **`crates/tessera`:** `test_mesh::test_mesh(levels, seed)`. It is an icosphere subdivided `levels` times (20 × 4^levels
  triangles, so 8 levels give 1,310,720) with shared edge midpoints, so it is welded and closed. Five octaves of
  seeded value noise (SplitMix64 lattice hash, no `rand`) push it in and out by up to ±15%. It has smooth area-weighted
  normals and zero UVs: exactly POSITION, NORMAL and UV_0, which `MeshletMesh::from_mesh` takes. Two tests: the counts
  are right and the mesh is closed (V − E + F = 2); the same seed gives the same mesh and another seed a different one.
- **`vg-render` (editor binary, `tessera` feature only):**
  1. Probes with `beverly::gpu::render_plugin`, and exits 2 with the missing features if `GpuCaps::virtual_geometry()`
     is false, so the plugin's `exit(1)` is never reached.
  2. Bakes the test mesh in a Startup system (the bake runs on Bevy's task pools, which don't exist before the app).
  3. Draws it with `MeshletPlugin { cluster_buffer_slots: 1 << 16 }`, a `Msaa::Off` camera and a shadowed sun.
  4. Saves `test-output/tessera/test-mesh.png` after 300 frames. At 30 frames the window was still black: the
     pipelines compile in the background, slowly on lavapipe.
- **Numbers (lavapipe, 4 CPU cores):**
  - test mesh 1,310,720 triangles, made in 0.23 s
  - `MeshletMesh::from_mesh` in 34.4–35.0 s (debug profile, dependencies at opt-level 3)
- **Checks:**
  - feature off: `cargo build` ✅ (17 m 55 s from a clean cache), `cargo test` ✅ (66 passed)
  - `--features tessera`: `cargo build` ✅ (9 m 16 s more), `cargo test` ✅ (66 passed)
  - `tessera` crate tests ✅ (2 passed; run in `crates/tessera` with `CARGO_TARGET_DIR=../../editor/target`, since it
    isn't a workspace member)
  - `DISPLAY=:99 BEVERLY_LITE=1 tools/run-tests.sh` ✅, all 17 test projects pass (feature-off player)
  - `vg-render` under Xvfb ✅ (picture checked by eye)
- **Warnings:** the vendored `from_mesh.rs` uses the deprecated `std::f32::MAX` three times (lines 557, 709, 776).
  Left untouched, as upstream has it.
- **Only the RTX card can show:** real frame times, and whether the meshlet path picks the same features through
  wgpu's Vulkan driver there. (gpu-setup says it should.)
