// Stripes that run across a surface: the material's own colour, striped in a second one (Beverly's test shader).
// @setting count "Stripes" 1..20 = 6
// @setting stripe colour "Stripe colour" = 0.1 0.2 0.9
// @setting glow toggle "Glows" = off
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

@if(!PREPASS_PIPELINE)
@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let band = step(0.5, fract(in.uv.x * setting_count()));
    pbr_input.material.base_color = vec4<f32>(mix(pbr_input.material.base_color.rgb, setting_stripe(), band), 1.0);
    if setting_glow() { pbr_input.material.emissive = vec4<f32>(setting_stripe() * band * 4.0, 1.0); }

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
