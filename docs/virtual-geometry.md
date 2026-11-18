# Virtual geometry (Nanite-like detail) for Beverly

Research of 2026-11-18. The roadmap feature is `virtual-geometry`. The work happens on the `virtual-geometry` branch; its job list is `crates/beverly_vg/JOBS.md` on that branch. Line references are to Bevy 0.20.1's `bevy_pbr-0.20.1/src/meshlet/` (call it `M/`) in the cargo registry.

## Decisions (user, 2026-11-18)
- **Fork Bevy's meshlet renderer** rather than write one from scratch. It already has most of a Nanite-style pipeline, and its wiring into Bevy's renderer saves weeks of wgpu work.
- **Exported games fall back** on cards that can't run it. Classic LOD and impostors remain for skinned and see-through things.
- **Shaders stay fully flexible.** Any custom shader must work, not a fixed set, so virtual geometry gets Nanite-style programmable raster through the `shaders` feature's raster hooks.
- **Its own branch.** Nightly unattended runs do the jobs that tests can judge. Interactive sessions review, answer questions and merge part by part.

## What Bevy 0.20.1 has
- **Plugin:** `MeshletPlugin { cluster_buffer_slots }` (`M/mod.rs:106-116`; at most 2^25 slots, because the visibility id is `cluster << 7 | triangle`).
  - Registers the asset and its loader, `InstanceManager`, and `RenderStartup` systems (feature check, `ResourceManager`, pipelines, mesh manager).
  - Extraction: `extract_meshlet_mesh_entities` in `ExtractSchedule`.
  - Render sets (`M/mod.rs:183-197`): PrepareAssets for uploads, PrepareViews for `configure_meshlet_views`, PrepareResources, PrepareBindGroups, QueueMeshes.
  - `Core3d` order (`M/mod.rs:198-213`):
    1. `meshlet_visibility_buffer_raster`, before the early shadow pass
    2. `meshlet_prepass`
    3. `meshlet_deferred_gbuffer_prepass`
    4. `meshlet_main_opaque_pass`, before `main_opaque_pass_3d`
- **Required wgpu features:** `TEXTURE_INT64_ATOMIC | TEXTURE_ATOMIC | SHADER_INT64 | SUBGROUP | DEPTH_CLIP_CONTROL | IMMEDIATES` (`M/mod.rs:120-127`). Only Vulkan and Metal provide them today.
  - The features the starting prompt named but Bevy doesn't need: `MULTI_DRAW_INDIRECT_COUNT` (Bevy uses `draw_indirect` with the instance count as the cluster count) and mesh shaders.
- **Preprocessing** (`M/from_mesh.rs`, feature `meshlet_processor`: meshopt 0.6, metis 0.2):
  - Input: an indexed triangle list with exactly POSITION, NORMAL and UV_0.
  - LOD 0: METIS splits the triangle graph into partitions of about 126 triangles, then `meshopt::build_meshlets(256 vertices, 128 triangles)`.
  - The DAG:
    1. Group 8 meshlets with METIS.
    2. Lock group borders.
    3. Simplify each group to half with normals as attributes, rejecting results that keep more than 60% of triangles.
    4. Group error = max(simplify error, children's errors).
    5. Repeat.
  - A BVH8 per LOD, built with SAH.
  - Positions quantized (default 1/16 cm) and bit-packed per meshlet. Normals are octahedral 2x16snorm; UVs are raw.
- **Asset** (`M/asset.rs`): `Meshlet` (32 B), `BvhNode` (8 AABBs with error, 8 LOD spheres, 8 child counts), `MeshletCullData`. The file is lz4-compressed, with magic and version 3.
- **Culling:**
  - Two-pass HZB against the previous frame's depth pyramid. The first pass's occluded items are re-tested in the second.
  - Frustum test with 5 planes.
  - LOD cut: draw a cluster when its own error projects to under 1 px and its parent's doesn't (`M/cull_shared.wesl:13-36`, after zeux's nanite.cpp).
  - BVH traversal runs one indirect dispatch per level.
- **Raster:**
  - Clusters of 64×64 px or less go to the software rasterizer: 128 threads per cluster, `textureAtomicMax` on R64Uint holding `depth << 32 | id`.
  - Larger clusters go to hardware `draw_indirect`.
  - Shadow views use R32 depth only.
- **Shading:**
  - Resolve writes depth, then a 16-bit "material depth".
  - Each material draws a fullscreen triangle with depth test Equal. Vertex data, barycentrics and derivatives are rebuilt analytically (`M/visibility_buffer_resolve.wesl`).
  - `StandardMaterial` and `ExtendedMaterial` forward the meshlet shader hooks.

## Its limits: missing work, or built into the technique?
**Cheap to add (missing work):**
- Feature detection instead of `exit(1)`.
- Filling in missing attributes at bake time.
- UV transform: scale the analytic derivatives too.
- Vertex colours and UV1.
- Instances updated by change instead of rebuilt every frame, and cached bind groups.
- Per-vertex tangents for baked normal maps.

**Real work, done by Nanite:**
- **Programmable raster:** a material's vertex offset and alpha mask, with bound padding, and clusters binned per raster shader.
- Streaming pages driven by GPU feedback.
- Virtual shadow maps.
- Skinned meshes.
- A mesh-shader hardware path.

**Built into the technique (work around):**
- Blending and glass: one surface per pixel, so these stay on the normal renderer.
- MSAA: use SMAA or TAA.
- Built-in derivative functions in custom materials: use the analytic derivatives Bevy passes.

**The fallback can stay virtual geometry.** Only the software rasterizer needs 64-bit atomics. A hardware-only path writing the id into an R32 target with a normal depth test runs on almost any card and keeps the seamless LOD and the culling. The Vulkan-only rule comes from wgpu: DX12 SM6.6 has 64-bit atomics, but wgpu doesn't expose them there yet.

**Beverly's paint layers** (`editor/src/paint.rs`, `paint.wesl`) are an alpha-blended overlay mesh. On virtual-geometry parts they have to be worked out inside the material pass.

## Bugs and oddities to fix in the fork
- The prepass and deferred passes bind the material at group 2 (`M/material_shade_nodes.rs:223,345`), but the layout puts it at index 3 (`M/material_pipeline_prepare.rs:407-417`). Verified.
- The "unclipped" directional-shadow pipeline sets no `unclipped_depth`, so it is the same as the clipped one (`M/pipelines.rs:359-383`).
- `view_key |= DEFERRED_PREPASS` changes the key inside the per-material loop, so it carries over to later materials (`M/material_pipeline_prepare.rs:353-354`). To check.
- Both raster shaders import `meshlet_cluster_meshlet_ids` and `meshlet_cluster_instance_ids`, which `bindings.wesl` doesn't define (`M/visibility_buffer_*_raster.wesl:5,7`).
- The software rasterizer has no top-left fill rule or fixed-point math (its own TODO, `M/visibility_buffer_software_raster.wesl:26`).
- The doc says a cluster slot costs 4 bytes (`M/mod.rs:112`). The queues use about 32 (`M/resource_manager.rs:78-120`).
- The MSAA check hits every view with `Msaa`, not only meshlet views (`M/mod.rs:260-263`).

## What changes in Beverly (the spec, also in CLAUDE.md)
1. Materials and shaders go through the `shaders` pipeline. Raster hooks are declared there, and see-through materials are marked.
2. Models are used through `models-split` meshes, never a spawned glTF scene.
3. Derived data goes through `bake-cache`: local, never in the project.
4. Nothing relies on MSAA or on one card. `GpuCaps` decides, and everything works on the fallback.
5. Verification includes `perf-meter` numbers.

## Size estimates
- **Code:** the meshlet module is about 7.9k lines. The fork plus additions should be about 15 to 20k.
- **App:** today's release zip is 64 MB for both programs. meshopt and METIS add about 1 to 3 MB to the editor; the game player needs only the runtime (under 1 MB).
- **Baked data:** Nanite is about 14 B per input triangle; Bevy's format should be 20 to 30 B, so about 20 to 30 MB per million triangles. Copies share it, it lives in the bake cache, and exports carry it plus the classic LODs.

## Sessions (estimate)
| Work | Sessions |
|---|---|
| Foundations on main (`gpu-setup`, `models-split`, `bake-cache`, `perf-meter`) | 3–4 |
| `shaders` | 2–3 |
| Virtual geometry, all six parts | 8–15 |
| Lighting (`shadow-cache`, `baked-light`, `realtime-gi`) | 6–8 |
| `virtual-shadows` | 4+ |

The biggest unknown is testing on the user's RTX card. Whether this container's lavapipe runs Bevy's meshlet path is job 1 on the branch.
