// The editor's vector shapes (vector.rs): the design's parts and every letter. Each is drawn as its triangles, and
// each pixel is covered by exactly how much of the pixel's square the shape covers, summed from the shape's own
// edges (the signed area each edge cuts off, as font rasterizers work it out): exact at any size, for curves,
// corners, thin strokes and holes alike.

// The shapes' edges, each walked with the shape on the same side (so holes cancel out).
@group(0) @binding(0) var<storage, read> edges: array<vec4<f32>>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    // which edges are this shape's (first, count)
    @location(1) @interpolate(flat) range: vec2<u32>,
    // how the edges are placed: origin (window pixels), size (pixels per unit), turn
    @location(2) @interpolate(flat) place: vec4<f32>,
}

// A part (frame.rs): already placed in the window; its edges are in window pixels.
@vertex
fn part(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>, @location(2) range: vec2<u32>) -> VertexOutput {
    return VertexOutput(vec4<f32>(position, 0.0, 1.0), color, range, vec4<f32>(0.0, 0.0, 1.0, 0.0));
}

// A letter (text.rs): its shape in ems (y down) from the store, placed by its instance.
@vertex
fn letter(
    @location(0) pn: vec4<f32>,
    @location(1) range: vec2<u32>,
    @location(2) place: vec4<f32>,
    @location(3) screen: vec2<f32>,
    @location(4) color: vec4<f32>,
) -> VertexOutput {
    let turn = mat2x2<f32>(cos(place.w), sin(place.w), -sin(place.w), cos(place.w));
    // a corner on the outline steps a pixel out, so every pixel the letter touches is drawn
    let p = place.xy + turn * (pn.xy * place.z + pn.zw);
    let ndc = vec2<f32>(p.x / screen.x * 2.0 - 1.0, 1.0 - p.y / screen.y * 2.0);
    return VertexOutput(vec4<f32>(ndc, 0.0, 1.0), color, range, place);
}

// The integral of clamp(x, -0.5, 0.5) from 0 to x.
fn ramp(x: f32) -> f32 {
    if x <= -0.5 { return -0.5 * x; }
    if x >= 0.5 { return 0.5 * x; }
    return 0.5 * x * x + 0.125;
}

// The signed area an edge from a to b (relative to the pixel's middle) covers of the pixel: the part of the pixel's
// square between the edge and the square's right side, within the edge's rows.
fn cut(a: vec2<f32>, b: vec2<f32>) -> f32 {
    if a.y == b.y { return 0.0; }
    let lo = max(min(a.y, b.y), -0.5);
    let hi = min(max(a.y, b.y), 0.5);
    if lo >= hi { return 0.0; }
    // the edge within those rows
    let k = (b.x - a.x) / (b.y - a.y);
    let x0 = a.x + (lo - a.y) * k;
    let x1 = a.x + (hi - a.y) * k;
    // the mean of the edge's x clamped to the pixel, over those rows
    var mean: f32;
    if abs(x1 - x0) < 1e-4 { mean = clamp(x0, -0.5, 0.5); } else { mean = (ramp(x1) - ramp(x0)) / (x1 - x0); }
    return sign(b.y - a.y) * (hi - lo) * (0.5 - mean);
}

// 0-1 linear from 0-1 sRGB gamma (as egui's own shader does, so colours match its).
fn linear_from_gamma_rgb(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < vec3<f32>(0.04045);
    let lower = srgb / vec3<f32>(12.92);
    let higher = pow((srgb + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.position.xy;
    let turn = mat2x2<f32>(cos(in.place.w), sin(in.place.w), -sin(in.place.w), cos(in.place.w));
    var area = 0.0;
    for (var i = in.range.x; i < in.range.x + in.range.y; i++) {
        let e = edges[i];
        let a = in.place.xy + turn * (e.xy * in.place.z) - p;
        let b = in.place.xy + turn * (e.zw * in.place.z) - p;
        // an edge wholly right of the pixel, or above or below it, covers none of it
        if min(a.x, b.x) >= 0.5 || max(a.y, b.y) <= -0.5 || min(a.y, b.y) >= 0.5 { continue; }
        area += cut(a, b);
    }
    let coverage = clamp(abs(area), 0.0, 1.0);
    let c = in.color * coverage;
    return vec4<f32>(linear_from_gamma_rgb(c.rgb), c.a);
}
