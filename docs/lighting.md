# Lighting: many lights, baked light and realtime GI

Research of 2026-11-18. The roadmap features are `shadow-cache`, `baked-light`, `realtime-gi` and `virtual-shadows`.

## Where the cost is today
- Bevy's clustered forward renderer handles hundreds of point and spot lights cheaply when they cast no shadows.
- The cost is shadows. A shadowed point light renders the scene into 6 cube faces every frame, which is why Beverly's light budget is 4 shadowed lights reaching one place (`editor/src/editor/light.rs`, `BUDGET`).

## Plan
1. **Cached shadows** (`shadow-cache`): render a light's shadow once, and redraw it only when something inside its reach moves. Shadow detail goes by the light's size on screen.
   - Aim: 20 shadowed lights without notice, and around 100 when most are still or unshadowed.
2. **Baked lighting** (`baked-light`): the user's idea of baking still lights the way impostors bake models.
   - Each light gets a Still or Moving switch. Still lights are baked into light probes (Bevy's irradiance volumes) and reflection boxes, so they cost nothing while playing.
   - Baked data is derived, so it lives in the bake cache, never in the project.
   - Probes come first, because virtual geometry has no lightmaps yet. Lightmaps come once models have a second UV set.
   - The baker captures cubemaps per probe on any card; a path tracer can bake on ray-tracing cards.
3. **Realtime GI, the Lumen-like option** (`realtime-gi`): Bevy's Solari on ray-tracing cards (an RTX 3000 qualifies). It gives many-light direct lighting, glowing surfaces that light their surroundings, and bounced light.
   - It is the Ultra preset; the cached and baked paths sit below it.
   - Virtual-geometry meshes need a proxy mesh to be ray traced, as Nanite does for Lumen.
   - **Unverified:** what Solari does in Bevy 0.20, its requirements and its cost. docs.rs and bevy.org were out of reach when this was written, so the feature starts with a planning pass.
4. **Virtual shadow maps** (`virtual-shadows`, later): one huge shadow, drawn only where the view needs it and redrawn only where things changed. Only worth it for very dense levels.
