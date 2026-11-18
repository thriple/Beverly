// The ground grid (view.rs): 1 m lines, brighter every 10 m, worked out per pixel so they stay smooth with any
// anti-aliasing, fading where they crowd together and toward the edge of its reach.
import bevy_pbr::render::{forward_io::VertexOutput, mesh_view_bindings::view};

// how much of a line of the grid with this step covers the pixel at p (about a pixel wide)
fn lines(p: vec2<f32>, step: f32) -> f32 {
    let q = p / step;
    let w = fwidth(q);
    let d = abs(fract(q - 0.5) - 0.5) / w;
    let l = 1.0 - min(min(d.x, d.y), 1.0);
    // lines a few pixels apart fade before they turn to noise
    return l * (1.0 - smoothstep(0.2, 0.5, max(w.x, w.y)));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.world_position.xz;
    let cam = view.world_position;
    let reach = clamp(abs(cam.y) * 3.0, 30.0, 400.0);
    let fade = 1.0 - smoothstep(reach * 0.6, reach, length(p - cam.xz));
    let a = max(lines(p, 1.0) * 0.045, lines(p, 10.0) * 0.12) * fade;
    return vec4(1.0, 1.0, 1.0, a);
}
