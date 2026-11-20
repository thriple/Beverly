// A painted layer of a part (paint.rs): the material's own look, shown only as much as the part's brush strokes
// leave of it here. Strokes go in order: one of this layer's adds over what is there (or takes its own away, or sets
// it to exactly its strength), any other (another material, or erasing) covers it. Each stroke counts once where its
// pieces overlap (the most of them), as a brush stroke does, and lands only where its rules (slope, height) let it.
import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::{
    prepass::io::{VertexOutput, FragmentOutput},
    deferred::functions::deferred_output,
};
@else
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

struct Frame {
    // the part's middle in the world, and the turn that takes the world into its own frame
    centre: vec4<f32>,
    unturn: vec4<f32>,
    // x: this layer, y: how many pieces
    info: vec4<i32>,
}

// one piece of a stroke: from a to b (w: radius, hardness), c: strength, layer (-1 erases), stroke number, how it
// lays (0 add, 1 subtract, 2 set); d: its rules (the cosine of the steepest slope, below -1 any; lowest and highest
// height in the world). A fill's outline is made of pieces too: each an edge from a to b in the part's frame (a.w: the
// width of the feathered edge), marked by d.w of 1
struct Piece {
    a: vec4<f32>,
    b: vec4<f32>,
    c: vec4<f32>,
    d: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> frame: Frame;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<storage, read> pieces: array<Piece>;

fn turn(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

fn dab(p: Piece, at: vec3<f32>) -> f32 {
    let ab = p.b.xyz - p.a.xyz;
    let t = clamp(dot(at - p.a.xyz, ab) / max(dot(ab, ab), 1e-8), 0.0, 1.0);
    let d = length(at - (p.a.xyz + ab * t));
    let r = p.a.w;
    return 1.0 - smoothstep(r * min(p.b.w, 0.98), r, d);
}

// whether a fill outline's edge from a to b crosses the line going on from p in +x (seen from above): x, and how near
// the edge is to p: y
fn edge(a: vec3<f32>, b: vec3<f32>, p: vec3<f32>) -> vec2<f32> {
    var crosses = 0.0;
    if (a.z > p.z) != (b.z > p.z) && p.x < a.x + (p.z - a.z) * (b.x - a.x) / (b.z - a.z) { crosses = 1.0; }
    let ab = vec2<f32>(b.x - a.x, b.z - a.z);
    let ap = vec2<f32>(p.x - a.x, p.z - a.z);
    let t = clamp(dot(ap, ab) / max(dot(ab, ab), 1e-8), 0.0, 1.0);
    return vec2<f32>(crosses, length(ap - ab * t));
}

// how much a stroke's rules let through on a surface whose normal points up this much, at this height
fn allows(d: vec4<f32>, up: f32, y: f32) -> f32 {
    var slope = 1.0;
    if d.x >= -1.0 { slope = smoothstep(d.x - 0.03, d.x + 0.03, up); }
    return slope * smoothstep(d.y - 0.05, d.y + 0.05, y) * (1.0 - smoothstep(d.z - 0.05, d.z + 0.05, y));
}

fn coverage(at: vec3<f32>, up: f32, y: f32) -> f32 {
    var cov = 0.0;
    var i = 0;
    let n = frame.info.y;
    loop {
        if i >= n { break; }
        let stroke = pieces[i].c.z;
        let layer = i32(pieces[i].c.y);
        let strength = pieces[i].c.x;
        let lays = i32(pieces[i].c.w);
        let d = pieces[i].d;
        var m = 0.0;
        var fill = false;
        var inside = false;
        var near = 1e30;
        var feather = 0.0;
        loop {
            if i >= n || pieces[i].c.z != stroke { break; }
            if pieces[i].d.w > 0.5 {
                let e = edge(pieces[i].a.xyz, pieces[i].b.xyz, at);
                fill = true;
                if e.x > 0.5 { inside = !inside; }
                near = min(near, e.y);
                feather = pieces[i].a.w;
            } else {
                m = max(m, dab(pieces[i], at));
            }
            i = i + 1;
        }
        if fill {
            m = 0.0;
            if inside { m = smoothstep(0.0, max(feather, 0.001), near); }
        }
        m = m * allows(d, up, y);
        let f = m * strength;
        if layer == frame.info.x {
            if lays == 1 { cov = cov * (1.0 - f); }
            else if lays == 2 { cov = cov + (strength - cov) * m; }
            else { cov = cov + f * (1.0 - cov); }
        } else if !(lays == 1 && layer >= 0) {
            // taking another layer away leaves this one; anything else covers it
            cov = cov * (1.0 - f);
        }
    }
    return cov;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let local = turn(frame.unturn, in.world_position.xyz - frame.centre.xyz);
    let up = normalize(pbr_input.world_normal).y;
    pbr_input.material.base_color.a = pbr_input.material.base_color.a * coverage(local, up, in.world_position.y);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color
    );

    var out: FragmentOutput;
@if(PREPASS_PIPELINE) {
    out = deferred_output(in, pbr_input);
} @else {
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
}
    return out;
}
