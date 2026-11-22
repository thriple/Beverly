//! Compute shader for rasterizing small clusters into a visibility buffer.

import package::{
    meshlet::bindings::{
        meshlets,
        meshlet_instance_uniforms,
        meshlet_raster_clusters,
        meshlet_previous_raster_counts,
        meshlet_software_raster_cluster_count,
        meshlet_visibility_buffer,
        view,
        get_meshlet_vertex_count,
        get_meshlet_triangle_count,
        get_meshlet_vertex_id,
        get_meshlet_vertex_position,
    },
    render::{
        mesh_functions::mesh_position_local_to_world,
        view_transformations::ndc_to_uv,
    },
};
import bevy_render::maths::affine3_to_square;

// Tessera: fixed point, 1/256 pixel
const SUBPIXELS: f32 = 256.0;
// The largest triangle, in pixels, whose fixed-point edge functions fit in i32
const MAX_TRIANGLE_PIXELS: f32 = 120.0;

var<workgroup> viewport_vertices: array<vec3f, 256>;

@compute
@workgroup_size(128, 1, 1) // 128 threads per workgroup, 1-2 vertices per thread, 1 triangle per thread, 1 cluster per workgroup
fn rasterize_cluster(
    @builtin(workgroup_id) workgroup_id: vec3<u32>,
    @builtin(local_invocation_index) local_invocation_index: u32,
    @if(MESHLET_2D_DISPATCH)
    @builtin(num_workgroups) num_workgroups: vec3<u32>,
) {
    var workgroup_id_1d = workgroup_id.x;

@if(MESHLET_2D_DISPATCH) {
    workgroup_id_1d += workgroup_id.y * num_workgroups.x;
    if workgroup_id_1d >= meshlet_software_raster_cluster_count { return; }
}

    let cluster_id = workgroup_id_1d + meshlet_previous_raster_counts[0];
    let instanced_offset = meshlet_raster_clusters[cluster_id];
    var meshlet = meshlets[instanced_offset.offset];

    let instance_uniform = meshlet_instance_uniforms[instanced_offset.instance_id];
    let world_from_local = affine3_to_square(instance_uniform.world_from_local);

    // Load and project 1 vertex per thread, and then again if there are more than 128 vertices in the meshlet
    for (var i = 0u; i <= 128u; i += 128u) {
        let vertex_id = local_invocation_index + i;
        if vertex_id < get_meshlet_vertex_count(&meshlet) {
            let vertex_position = get_meshlet_vertex_position(&meshlet, vertex_id);

            // Project vertex to viewport space
            let world_position = mesh_position_local_to_world(world_from_local, vec4(vertex_position, 1.0));
            let clip_position = view.clip_from_world * vec4(world_position.xyz, 1.0);
            let ndc_position = clip_position.xyz / clip_position.w;
            let viewport_position_xy = ndc_to_uv(ndc_position.xy) * view.viewport.zw;

            // Write vertex to workgroup shared memory
            viewport_vertices[vertex_id] = vec3(viewport_position_xy, ndc_position.z);
        }
    }
    workgroupBarrier();

    // Load 1 triangle's worth of vertex data per thread
    let triangle_id = local_invocation_index;
    if triangle_id >= get_meshlet_triangle_count(&meshlet) { return; }
    let index_ids = meshlet.start_index_id + (triangle_id * 3u) + vec3(0u, 1u, 2u);
    let vertex_ids = vec3(get_meshlet_vertex_id(index_ids[0]), get_meshlet_vertex_id(index_ids[1]), get_meshlet_vertex_id(index_ids[2]));
    let vertex_0 = viewport_vertices[vertex_ids[2]];
    let vertex_1 = viewport_vertices[vertex_ids[1]];
    let vertex_2 = viewport_vertices[vertex_ids[0]];
    let packed_ids = (cluster_id << 7u) | triangle_id;

    // Tessera: fixed-point math and the top-left fill rule (Bevy used floats and counted pixel centres on an edge as
    // inside both triangles sharing it). Vertices are snapped to 1/256 pixel, measured from the triangle's bounding
    // box corner so the edge functions fit in i32; a pixel centre on an edge belongs to the triangle only if the
    // edge is a top or left one, so each pixel of a shared edge is drawn once and none is missed.
    let origin = floor(vec2(min3(vertex_0.x, vertex_1.x, vertex_2.x), min3(vertex_0.y, vertex_1.y, vertex_2.y)));
    let extent = vec2(max3(vertex_0.x, vertex_1.x, vertex_2.x), max3(vertex_0.y, vertex_1.y, vertex_2.y)) - origin;
    // Clusters here are at most 64 pixels across (cull_clusters.wesl); past this the edge functions would overflow
    if any(extent > vec2(MAX_TRIANGLE_PIXELS)) { return; }
    let p0 = vec2<i32>(round((vertex_0.xy - origin) * SUBPIXELS));
    let p1 = vec2<i32>(round((vertex_1.xy - origin) * SUBPIXELS));
    let p2 = vec2<i32>(round((vertex_2.xy - origin) * SUBPIXELS));

    // Backface culling (and triangles that snapping made degenerate)
    let triangle_double_area = edge_function_fixed(p0, p1, p2);
    if triangle_double_area <= 0 { return; }

    // Setup triangle gradients, per pixel step
    let w_x = vec3(p1.y - p2.y, p2.y - p0.y, p0.y - p1.y) * i32(SUBPIXELS);
    let w_y = vec3(p2.x - p1.x, p0.x - p2.x, p1.x - p0.x) * i32(SUBPIXELS);
    let vertices_z = vec3(vertex_0.z, vertex_1.z, vertex_2.z) / f32(triangle_double_area);

    // Top-left rule: the edge function grows into the triangle, so a left edge has w_x > 0 and a top edge (y points
    // down) has w_x == 0 and w_y > 0. Other edges don't own the pixel centres on them: inside means biased w >= 0.
    let top_left = (w_x > vec3(0)) | ((w_x == vec3(0)) & (w_y > vec3(0)));
    let bias = select(vec3(-1), vec3(0), top_left);

    // Compute triangle bounding box
    var min_x = origin.x;
    var min_y = origin.y;
    var max_x = floor(origin.x + extent.x);
    var max_y = floor(origin.y + extent.y);
    min_x = max(min_x, 0.0);
    min_y = max(min_y, 0.0);
    max_x = min(max_x, view.viewport.z - 1.0);
    max_y = min(max_y, view.viewport.w - 1.0);

    // Setup initial triangle equations, at the first pixel's centre
    let starting_pixel = vec2<i32>((vec2(min_x, min_y) - origin) * SUBPIXELS) + vec2(i32(SUBPIXELS) / 2);
    var w_row = vec3(
        edge_function_fixed(p1, p2, starting_pixel),
        edge_function_fixed(p2, p0, starting_pixel),
        edge_function_fixed(p0, p1, starting_pixel),
    ) + bias;

    // Rasterize triangle
    let width = max_x - min_x;
    if subgroupAny(width > 4.0) {
        // Scanline: each row's X interval from the edges, a pixel wider each way; the exact test below decides
        let inverse_w_x = select(1.0 / vec3<f32>(w_x), vec3(0.0), w_x == vec3(0));
        for (var y = min_y; y <= max_y; y += 1.0) {
            let cross_x = vec3<f32>(-w_row) * inverse_w_x;
            let lower = select(vec3(0.0), floor(cross_x) - 1.0, w_x > vec3(0));
            let upper = select(vec3(width), ceil(cross_x) + 1.0, w_x < vec3(0));
            let x0 = clamp(max3(lower[0], lower[1], lower[2]), 0.0, width);
            let x1 = clamp(min3(upper[0], upper[1], upper[2]), -1.0, width);

            var w = w_row + w_x * i32(x0);
            for (var x = x0; x <= x1; x += 1.0) {
                if all(w >= vec3(0)) {
                    let z = dot(vertices_z, vec3<f32>(w - bias));
                    write_visibility_buffer_pixel(min_x + x, y, z, packed_ids);
                }
                w += w_x;
            }
            w_row += w_y;
        }
    } else {
        // Iterate over every pixel in the triangle's bounding box
        for (var y = min_y; y <= max_y; y += 1.0) {
            var w = w_row;
            for (var x = min_x; x <= max_x; x += 1.0) {
                if all(w >= vec3(0)) {
                    let z = dot(vertices_z, vec3<f32>(w - bias));
                    write_visibility_buffer_pixel(x, y, z, packed_ids);
                }
                w += w_x;
            }
            w_row += w_y;
        }
    }
}

fn write_visibility_buffer_pixel(x: f32, y: f32, z: f32, packed_ids: u32) {
    let depth = bitcast<u32>(z);
    @if(MESHLET_VISIBILITY_BUFFER_RASTER_PASS_OUTPUT)
    let visibility = (u64(depth) << 32u) | u64(packed_ids);
    @else
    let visibility = depth;
    textureAtomicMax(meshlet_visibility_buffer, vec2(u32(x), u32(y)), visibility);
}

fn edge_function_fixed(a: vec2<i32>, b: vec2<i32>, c: vec2<i32>) -> i32 {
    return (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
}

fn min3(a: f32, b: f32, c: f32) -> f32 {
    return min(a, min(b, c));
}

fn max3(a: f32, b: f32, c: f32) -> f32 {
    return max(a, max(b, c));
}
