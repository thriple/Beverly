// The editor's vector parts (vector.rs): triangles of the design's shapes, each pixel near an edge covered by how far
// it is from the shape's real edges (up to six edge segments near its triangle, each with its ends' outward
// directions), so edges are exact at any size: curves and thin strokes included.

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) e0: vec4<f32>, @location(3) e1: vec4<f32>, @location(4) e2: vec4<f32>,
    @location(5) e3: vec4<f32>, @location(6) e4: vec4<f32>, @location(7) e5: vec4<f32>,
    @location(8) e6: vec4<f32>, @location(9) e7: vec4<f32>, @location(10) e8: vec4<f32>,
}

// Six edge segments, six numbers each: ends a, b (window pixels), then the outward directions at a and b (angles).
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) @interpolate(flat) e0: vec4<f32>, @location(2) @interpolate(flat) e1: vec4<f32>,
    @location(3) @interpolate(flat) e2: vec4<f32>, @location(4) @interpolate(flat) e3: vec4<f32>,
    @location(5) @interpolate(flat) e4: vec4<f32>, @location(6) @interpolate(flat) e5: vec4<f32>,
    @location(7) @interpolate(flat) e6: vec4<f32>, @location(8) @interpolate(flat) e7: vec4<f32>,
    @location(9) @interpolate(flat) e8: vec4<f32>,
}

@vertex
fn vertex(v: VertexInput) -> VertexOutput {
    return VertexOutput(vec4<f32>(v.position, 0.0, 1.0), v.color, v.e0, v.e1, v.e2, v.e3, v.e4, v.e5, v.e6, v.e7, v.e8);
}

// A pixel's place against an edge segment (s: its ends; n: their outward directions): its distance (negative
// inside), and the way the segment faces. Past the segment's ends the side comes from the end's direction.
fn against(p: vec2<f32>, s: vec4<f32>, angles: vec2<f32>) -> vec3<f32> {
    let a = s.xy;
    let ab = s.zw - s.xy;
    let len2 = dot(ab, ab);
    if len2 < 1e-8 { return vec3<f32>(1e9, 0.0, 0.0); }
    let na = vec2<f32>(cos(angles.x), sin(angles.x));
    let nb = vec2<f32>(cos(angles.y), sin(angles.y));
    let t = dot(p - a, ab) / len2;
    // the side facing out: the one the ends' outward directions lean to
    var face = normalize(vec2<f32>(-ab.y, ab.x));
    if dot(face, na + nb) < 0.0 { face = -face; }
    let q = a + ab * clamp(t, 0.0, 1.0);
    var side = dot(p - a, face);
    if t < 0.0 { side = dot(p - a, na); } else if t > 1.0 { side = dot(p - s.zw, nb); }
    return vec3<f32>(select(-1.0, 1.0, side > 0.0) * length(p - q), face);
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
    let e = array<vec3<f32>, 6>(
        against(p, in.e0, in.e1.xy), against(p, vec4<f32>(in.e1.zw, in.e2.xy), in.e2.zw),
        against(p, in.e3, in.e4.xy), against(p, vec4<f32>(in.e4.zw, in.e5.xy), in.e5.zw),
        against(p, in.e6, in.e7.xy), against(p, vec4<f32>(in.e7.zw, in.e8.xy), in.e8.zw),
    );
    // the nearest edge says how much of the pixel is outside; pixels further than any edge is pushed out (two
    // pixels) are inside
    var k0 = 0u;
    for (var k = 1u; k < 6u; k++) { if abs(e[k].x) < abs(e[k0].x) { k0 = k; } }
    var coverage = 1.0;
    if abs(e[k0].x) < 2.5 {
        coverage = clamp(0.5 - e[k0].x, 0.0, 1.0);
        // a thin stroke: the edge on its other side, close by, takes its share too
        var other = 0.0;
        for (var k = 0u; k < 6u; k++) {
            if k != k0 && e[k].x < 0.0 && e[k].x > -1.0 && dot(e[k].yz, e[k0].yz) < -0.3 { other = max(other, 1.0 - clamp(0.5 - e[k].x, 0.0, 1.0)); }
        }
        coverage = clamp(coverage - other, 0.0, 1.0);
    }
    let c = in.color * coverage;
    return vec4<f32>(linear_from_gamma_rgb(c.rgb), c.a);
}
