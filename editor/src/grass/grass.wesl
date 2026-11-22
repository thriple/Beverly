// The grass carpet (grass.rs): the user's grass system, ported from their game. One instanced lattice per sheet (a
// flat face that grows grass, sheets.rs), every blade placed, shaped, coloured and swayed here: lattice cell -> spot
// on the face -> what grows there (the sheet's map) -> species (by the grower's shares) -> blade shape, colour and
// wind. Blades beyond the carpet's reach of the camera, off the face or where nothing grows are thrown outside the
// clip volume. The roots take the colour of the surface under them (the sheet's colour map).
import bevy_pbr::render::{
    mesh_functions::get_world_from_local,
    view_transformations::position_world_to_clip,
    mesh_view_bindings::{view, globals},
};

struct Params {
    origin: vec4<f32>,     // the sheet's corner in the world, lattice spacing
    across_u: vec4<f32>,   // its first direction (unit), metres along it
    across_v: vec4<f32>,   // its second direction (unit), metres along it
    normal: vec4<f32>,     // its normal, a number of its own (so sheets don't repeat each other)
    lattice: vec4<f32>,    // radius, edge, min_scale, density_keep
    shape: vec4<f32>,      // height, height_var, width, bend
    shape2: vec4<f32>,     // jitter, clump, clump_size, min_width
    look: vec4<f32>,       // flat, ground, patch, tip
    head: vec4<f32>,       // head_r, head_var, head_var_hi, plume_at
    misc: vec4<f32>,       // pad_lift, ivy_normal_bias, ivy_direct_mul, ivy_ground_tint
    grad: vec4<f32>,       // amount, alt_share, lo, hi
    grad2: vec4<f32>,      // pow, species count, 0, 0
    wind: vec4<f32>,       // dir.x, dir.z, strength, gust
    dark: vec4<f32>,
    mid: vec4<f32>,
    light: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    ambient: vec4<f32>,
    flat_scale: vec4<f32>,
};

struct Species {
    shape: vec4<f32>,   // hS, wS, taperS, bendS
    shape2: vec4<f32>,  // swayS, headR, class (0 blade,1 plume,2 flower,3 pad), head kind (0 disc,1 sphere,2 spike)
    flags: vec4<f32>,   // orient_flat, gradient id, abs_amount, 0
    tint_a: vec4<f32>,
    tint_b: vec4<f32>,
    abs_c: vec4<f32>,
};
struct SpeciesTable { items: array<Species, 32> };
// a grass-growing material: up to four species and their shares, added up (the last is 1)
struct Grower {
    species: vec4<f32>,
    upto: vec4<f32>,
};
struct GrowerTable { items: array<Grower, 32> };

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> P: Params;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<uniform> SP: SpeciesTable;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var<uniform> GR: GrowerTable;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var grows_tex: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var colour_tex: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(5) var grad_tex: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(6) var grad_samp: sampler;

struct VIn {
    @builtin(instance_index) instance_index: u32,
    // x = +-0.5 blade width, y = t along the blade (0..1), z = which of the two crossed planes
    @location(0) position: vec3<f32>,
    // blade index inside the tile; the tile's offset (in cells) comes from the entity translation
    @location(1) cell: vec2<f32>,
};

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) grad_t: f32,
    @location(2) light: vec3<f32>,
};

// ---- hash / noise: unchanged from the original (no sin, bounded input) -------------------------------
fn ghash(p: vec2<f32>) -> f32 {
    let q = p - 4096.0 * floor(p / 4096.0);          // GLSL mod(), not WGSL %
    var p3 = fract(q.xyx * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 19.19);
    return fract((p3.x + p3.y) * p3.z);
}
fn gvn(p: vec2<f32>) -> f32 {
    let i = floor(p);
    var f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(ghash(i), ghash(i + vec2<f32>(1.0, 0.0)), f.x),
               mix(ghash(i + vec2<f32>(0.0, 1.0)), ghash(i + vec2<f32>(1.0, 1.0)), f.x), f.y);
}

// the sheet's maps at a spot (metres along u and v): what grows (x: grower, y: how thick) and the surface's colour
fn spot(uv: vec2<f32>) -> vec2<i32> {
    let dims = vec2<i32>(textureDimensions(grows_tex));
    return clamp(vec2<i32>(floor(uv / vec2<f32>(P.across_u.w, P.across_v.w) * vec2<f32>(dims))), vec2<i32>(0), dims - vec2<i32>(1));
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3<f32>(2.4)), step(vec3<f32>(0.04045), c));
}
fn grad_color(gid: f32, t: f32) -> vec3<f32> {
    let u = clamp(mix(P.grad.z, P.grad.w, pow(clamp(t, 0.0, 1.0), P.grad2.x)), 0.0, 1.0);
    let v = (gid - 0.5) / 2.0;                         // 2 gradient rows in the atlas
    return srgb_to_linear(textureSampleLevel(grad_tex, grad_samp, vec2<f32>(u, v), 0.0).rgb);
}

@vertex
fn vertex(vin: VIn) -> VOut {
    var out: VOut;
    out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);      // default: culled
    out.color = vec3<f32>(0.0);
    out.grad_t = 0.0;
    out.light = vec3<f32>(1.0);

    let t = vin.position.y;
    let spacing = P.origin.w;
    let tile = get_world_from_local(vin.instance_index)[3].xz;
    let a_cell = vin.cell + tile;

    // ---------------- lattice cell -> jittered spot on the sheet (metres along u and v)
    let id = a_cell + vec2<f32>(P.normal.w, P.normal.w * 0.37);
    var uv = (a_cell + 0.5) * spacing;
    let r1 = ghash(id);
    let r2 = ghash(id + 7.3);
    let r3 = ghash(id + 19.1);
    let r4 = ghash(id + 3.7);
    let r5 = ghash(id + 61.3);
    let jit = min(P.shape2.x, 0.999);
    uv += (vec2<f32>(r1, r2) - 0.5) * spacing * jit;

    let clump_s = P.shape2.z;
    let cc = floor(uv / clump_s);
    let k1 = ghash(cc + 51.7);
    let k2 = ghash(cc + 8.11);
    let k3 = ghash(cc + 29.3);
    let centre = (cc + vec2<f32>(0.25 + k1 * 0.5, 0.25 + k2 * 0.5)) * clump_s;
    uv = mix(uv, centre, P.shape2.y * (0.35 + r4 * 0.65));
    if (uv.x < 0.0 || uv.y < 0.0 || uv.x >= P.across_u.w || uv.y >= P.across_v.w) { return out; }
    let U = P.across_u.xyz;
    let V = P.across_v.xyz;
    let nrm = P.normal.xyz;
    let root = P.origin.xyz + U * uv.x + V * uv.y;
    let wp = root.xz;

    // ---------------- the sphere around the camera
    let radius = P.lattice.x;
    let d = length(root - view.world_position);
    var s = 0.0;
    if (d <= radius) {
        s = mix(P.lattice.z, 1.0, clamp((radius - d) / max(1e-3, radius * P.lattice.y), 0.0, 1.0));
    }
    let px_at = spot(uv);
    let grows = textureLoad(grows_tex, px_at, 0);
    if (ghash(id + 41.7) > P.lattice.w * grows.y) { s = 0.0; }
    if (s <= 0.0) { return out; }
    let surface = textureLoad(colour_tex, px_at, 0).rgb;

    // ---------------- species: by the grower's shares; colour from patch noise
    let G = GR.items[u32(clamp(floor(grows.x * 255.0 + 0.5), 0.0, 31.0))];
    let pick = ghash(id + 97.1);
    var sp = G.species.w;
    if (pick < G.upto.z) { sp = G.species.z; }
    if (pick < G.upto.y) { sp = G.species.y; }
    if (pick < G.upto.x) { sp = G.species.x; }
    let sid = u32(clamp(sp, 0.0, max(P.grad2.y - 1.0, 0.0)));
    let S = SP.items[sid];
    let hS = S.shape.x;  let wS = S.shape.y;  let taperS = S.shape.z;  let bendS = S.shape.w;
    let swayS = S.shape2.x;  let headR = S.shape2.y;
    let cls = i32(S.shape2.z + 0.5);
    let hKind = i32(S.shape2.w + 0.5);
    let orient_flat = S.flags.x;
    var grad_id = S.flags.y;
    if (grad_id > 0.5 && ghash(id + 523.9) < P.grad.y) { grad_id += 1.0; }

    let g_patch = P.look.z;
    let n = gvn(wp / g_patch) * 0.68 + gvn(wp / g_patch * 2.7 + 13.0) * 0.32;
    let m = clamp((n - 0.5) * 1.7 + 0.5 + (k3 - 0.5) * 0.22, 0.0, 1.0);
    var gc = select(mix(P.mid.rgb, P.light.rgb, (m - 0.5) * 2.0),
                    mix(P.dark.rgb, P.mid.rgb, m * 2.0), m < 0.5);
    gc *= 1.0 - P.look.w * 0.5;                        // base->tip handled per vertex below
    let col_base = mix(gc, surface, P.look.y);

    let hgt = P.shape.x * (1.0 - P.shape.y * 0.5 + r3 * P.shape.y) * hS;
    let yaw = mix(r5, k1, P.shape2.y) * 6.2831853;

    // ---------------- foliageMorph: colour
    var col = col_base * mix(S.tint_a.rgb, S.tint_b.rgb, t);
    // the roots take the surface's colour
    col = mix(surface, col, smoothstep(0.0, 0.3, t));
    col = mix(col, S.abs_c.rgb, S.flags.z * (0.32 + 0.68 * t));
    if (cls == 2) { col = mix(col, S.tint_b.rgb, step(0.5, t)); }

    let nrm_base = normalize(mix(nrm, vec3<f32>(0.0, 1.0, 0.0), P.misc.y * orient_flat));
    col *= mix(1.0, P.misc.z, orient_flat);
    col = mix(col, surface, P.misc.w * orient_flat);

    var out_grad = 0.0;
    if (grad_id > 0.5) { out_grad = grad_id + t * 0.499; }

    // ---------------- foliageMorph: shape classes (0 blade, 1 plume, 2 flower, 3 pad)
    let head_r = P.head.x;
    let hv = mix(P.head.y, P.head.z, ghash(id + 211.9));
    let hR = head_r * headR * hv;
    let g_bend = P.shape.w;
    let wEff = max(P.shape.z * wS, P.shape2.w);
    let px = vin.position.x;
    var yT = t;
    var w = 0.0;
    var bd = 0.0;
    var yAdd = 0.0;
    if (cls == 2) {
        if (hKind == 2) {                               // spike
            yT = t;
            w = px * mix(wEff, hR, smoothstep(0.25, 0.60, t)) * s;
            bd = g_bend * (0.35 + r4) * t * t * hgt * bendS;
        } else {                                        // disc / sphere
            let ht = min(t * 2.0, 1.0);
            let sph = select(0.0, 1.0, hKind == 1);
            yT = ht;
            yAdd = sph * hR * smoothstep(0.5, 1.0, t) * s;
            w = px * 2.0 * mix(wEff, hR, ht) * s;
            bd = g_bend * (0.35 + r4) * t * t * hgt * bendS
               + hR * mix(1.0, 0.33, sph) * smoothstep(0.5, 1.0, t);
        }
    } else if (cls == 1) {                              // plume
        let plume_at = P.head.w;
        yT = select(plume_at + (t - 0.5) * 2.0 * (1.0 - plume_at), t * 2.0 * plume_at, t <= 0.5);
        w = px * wEff * s * mix(1.0 - taperS * t, headR, smoothstep(0.15, 0.50, t));
        bd = g_bend * (0.35 + r4) * t * t * hgt * bendS;
    } else if (cls == 3) {                              // pad
        yT = 0.0;
        yAdd = P.misc.x * s;
        w = px * 2.0 * hR * s * mix(0.45, 1.0, min(t * 2.0, 1.0));
        bd = hR * 2.2 * t * s;
    } else {                                            // blade
        yT = t;
        w = px * wEff * s * (1.0 - taperS * t);
        bd = g_bend * (0.35 + r4) * t * t * hgt * bendS;
    }

    let gust = mix(1.0, 0.55 + 0.45 * sin(globals.time * 0.7 + dot(wp, vec2<f32>(0.021, 0.017))), P.wind.w);
    let lean = P.wind.xy * (P.wind.z * gust * t * t * hgt * swayS);

    // crossed double quad: plane A (yaw) or plane B (yaw + 90 deg)
    let plane_yaw = yaw + vin.position.z * 1.5707963;
    let dir_local = vec2<f32>(cos(plane_yaw), sin(plane_yaw));

    var world_pos: vec3<f32>;
    let wind3 = vec3<f32>(lean.x, 0.0, lean.y);
    if (orient_flat > 0.5) {
        // flat: the card lies on the face and reaches across it (ivy)
        let fwd = U * dir_local.x + V * dir_local.y;
        let side = U * (-dir_local.y) + V * dir_local.x;
        world_pos = root + fwd * (yT * hgt + yAdd) + side * w + nrm * bd + wind3;
    } else {
        // upright: stands out of the face, along its normal
        world_pos = root + U * (dir_local.y * w + dir_local.x * bd) + V * (-dir_local.x * w + dir_local.y * bd)
                  + nrm * (yT * hgt + yAdd) + wind3;
    }

    out.position = position_world_to_clip(world_pos);
    out.color = col;
    out.grad_t = out_grad;
    // Lambert, per blade (every vertex shares the face normal) -- the "non-flat" 28% of the look
    out.light = P.ambient.rgb + P.sun_color.rgb * max(dot(nrm_base, P.sun_dir.xyz), 0.0);
    return out;
}

@fragment
fn fragment(in: VOut) -> @location(0) vec4<f32> {
    var base = in.color;
    let gid = floor(in.grad_t);
    if (gid >= 0.5) {
        let t = clamp((in.grad_t - gid) / 0.499, 0.0, 1.0);
        base = mix(base, grad_color(gid, t), P.grad.x);
    }
    let lit = mix(base * in.light, base * P.flat_scale.rgb, P.look.x);
    return vec4<f32>(lit, 1.0);
}
