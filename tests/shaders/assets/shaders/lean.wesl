// A part that leans and has holes in it: it moves its vertices sideways the higher they are, and cuts round holes out
// of its faces (Beverly's test of the raster hooks: the depth and shadow passes follow both).
// @setting lean "Lean" 0..2 = 0.4
// @setting holes "Holes" 0..0.5 = 0.3
// @raster vertex_offset 2.0
// @raster alpha_mask
import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
};

// the main pass's own: the depth and shadow passes use only the hooks (see shader.rs)
@if(!PREPASS_PIPELINE)
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

fn vertex_offset(world: vec3<f32>, normal: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    return vec3<f32>(world.y * setting_lean(), 0.0, 0.0);
}

fn alpha_mask(world: vec3<f32>, uv: vec2<f32>) -> f32 {
    let cell = fract(uv * 3.0) - vec2<f32>(0.5, 0.5);
    return select(1.0, 0.0, length(cell) < setting_holes());
}

@if(!PREPASS_PIPELINE)
@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    if alpha_mask(in.world_position.xyz, in.uv) < 0.5 { discard; }
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
