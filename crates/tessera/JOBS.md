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
   - `GpuCaps` from `gpu-setup`
   - split meshes from `models-split`
   - raster hooks from `shaders`
   - the cache directory from `bake-cache`
   - the SMAA swap from `gpu-setup`
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

1. ⬜ **Vendor and build.**
   - Vendor `bevy_pbr-0.20.1` into `crates/bevy_pbr`, untouched, in its own commit. Add the patch and the `tessera` feature, and make both builds pass.
   - Fill in `crates/tessera` (a crate named `tessera`) with a procedural dense test mesh (a displaced, subdivided sphere; about 1 M triangles; seeded; no third-party assets).
   - **Check the GPU here:** can lavapipe (Mesa's software Vulkan, `/usr/share/vulkan/icd.d/lvp_icd.json`) provide the six features (`docs/tessera.md`)?
     - Write a small headless check program that prints the adapter's features.
     - If they are there, render the test mesh with Bevy's meshlet path off-screen and save a screenshot.
     - Record the answer at the top of *Run log* as **GPU-testable here: yes/no**. Later jobs use it to decide what they can check themselves.
2. ⬜ **No exit on a missing feature.**
   - In the fork, replace the `exit(1)`s (`mod.rs` feature check, MSAA check) with a disabled state that leaves the app running.
   - Limit the MSAA check to views that actually draw virtual geometry.
   - Add `tessera::GpuCaps`: probe the adapter before the app is built (backend, the six features, mesh shaders, ray query), and choose Vulkan on Windows when it qualifies.
   - Wiring note: main's `gpu-setup` should adopt this type.
3. ⬜ **Bug fixes** (the list in `docs/tessera.md`):
   - the prepass and deferred material bind group 2 → 3
   - `unclipped_depth` on the directional-shadow pipeline
   - `DEFERRED_PREPASS` carrying over in the prepass loop (confirm first)
   - the stale imports in the raster shaders
   - the cluster-slot cost in the doc comment

   One commit per fix, each message saying why. A top-left fill rule in fixed point for the software rasterizer, if GPU-testable here; otherwise ⏸ with a note.
4. ⬜ **Baking library** (`tessera::bake`):
   - Any Bevy `Mesh` (a glTF primitive) to `MeshletMesh`. Fill in missing normals (smooth by angle) and UVs (zero), drop extra attributes, weld and index.
   - Cache key: SHA-256 over the source bytes, bake settings and format version.
   - Cache folder: `%LOCALAPPDATA%\Beverly\cache` / `~/.cache/beverly`, behind one function main's `bake-cache` can replace.
   - A `vg-bake` binary that bakes the test mesh and prints time, output bytes and bytes per triangle.
   - Tests: the result is the same every time (or say why not), a save/load round trip, a wrong-version file is rejected.
5. ⬜ **Instances by change.**
   - Rewrite the fork's `InstanceManager` so it updates only what changed, and cache the per-view bind groups (both are Bevy TODOs).
   - Add a headless benchmark with 10k and 100k instances, and record the CPU time per frame before and after.
6. ⬜ **More of the material.**
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
- Wiring into the editor's cameras and the SMAA swap (`gpu-setup`).
- Drawing imported models through split meshes (`models-split`).
- Beverly's paint layers inside the material pass (`shaders`).
- Bake status in Assets and the LOD drawer.
- Anything only the user's RTX card can show.

## Wiring main needs
(Unattended runs add entries here.)

## Questions for the user
(Unattended runs add questions here. Interactive sessions answer them and remove them.)

## Run log
(Newest first.)
