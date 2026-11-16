//! Drawing vector shapes in egui's draw order (egui paint callbacks): the design's parts (frame.rs) and every letter
//! (text.rs). Each shape is its triangles plus its own edges; the shader (vector.wgsl) covers each pixel by exactly
//! how much of it the shape covers, worked out from those edges. No images: everything is worked out from the
//! design's paths and the fonts' outlines each time it is drawn.
use bevy::mesh::VertexBufferLayout;
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::render_resource::binding_types::storage_buffer_read_only_sized;
use bevy::render::render_resource::{
    BindGroup, BindGroupEntry, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendState, Buffer, BufferInitDescriptor, BufferUsages,
    CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState, MultisampleState, PipelineCache, PrimitiveState, RenderPipelineDescriptor,
    ShaderStages, SpecializedRenderPipeline, SpecializedRenderPipelines, VertexFormat, VertexState, VertexStepMode,
};
use bevy::render::renderer::RenderDevice;
use bevy::render::sync_world::RenderEntity;
use bevy_egui::egui;
use bevy_egui::render::{EguiBevyPaintCallback, EguiBevyPaintCallbackImpl, EguiPipelineKey};
use std::collections::HashMap;
use std::sync::Arc;

pub struct VectorPlugin;

impl Plugin for VectorPlugin {
    fn build(&self, app: &mut App) {
        let shader = app.world_mut().resource_mut::<Assets<Shader>>().add(Shader::from_wgsl(include_str!("vector.wgsl"), "beverly/vector.wgsl"));
        let edges = BindGroupLayoutDescriptor::new("beverly edges", &BindGroupLayoutEntries::single(ShaderStages::FRAGMENT, storage_buffer_read_only_sized(false, None)));
        if let Some(r) = app.get_sub_app_mut(RenderApp) {
            r.insert_resource(VectorPipeline { shader, edges })
                .init_resource::<SpecializedRenderPipelines<VectorPipeline>>()
                .init_resource::<Drawn>()
                .init_resource::<LetterStore>()
                // each frame's buffers are made afresh
                .add_systems(bevy::render::ExtractSchedule, |mut d: ResMut<Drawn>| d.0.clear());
        }
    }
}

/// A part's vertex: where (clip space), its colour (egui's: premultiplied, gamma), which edges are its shape's.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub color: [u8; 4],
    pub range: [u32; 2],
}

/// A letter's vertex (in ems): its point and outward step, which edges are its shape's.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LetterVertex {
    pub pn: [f32; 4],
    pub range: [u32; 2],
}

/// One letter placed: its origin (the baseline's start, window pixels), size (pixels per em) and turn, the window's
/// size, and its colour.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Instance {
    pub place: [f32; 4],
    pub screen: [f32; 2],
    pub color: [u8; 4],
}

fn bytes_of<T: Copy>(v: &[T]) -> &[u8] {
    // SAFETY: used only for the repr(C) types here: plain floats, integers and bytes with no padding.
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

fn next_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Draw a part's triangles (`verts`) with its edges (window pixels) in the painter's place in egui's order.
pub fn paint(p: &egui::Painter, verts: Vec<Vertex>, edges: Vec<[f32; 4]>) {
    if verts.is_empty() { return; }
    p.add(EguiBevyPaintCallback::new_paint_callback(p.ctx().content_rect(), Draw { id: next_id(), verts: Arc::new(verts), edges: Arc::new(edges) }));
}

struct Draw { id: u64, verts: Arc<Vec<Vertex>>, edges: Arc<Vec<[f32; 4]>> }

/// This frame's buffers by draw: vertices (a part's) or instances (letters'), how many, and the edges to bind.
#[derive(Resource, Default)]
struct Drawn(HashMap<u64, (Buffer, u32, Option<BindGroup>)>);

#[derive(Component)]
struct Pipelines { part: CachedRenderPipelineId, letter: CachedRenderPipelineId }

/// The pipelines for this target (parts and letters), ready.
fn pipelines(render_entity: RenderEntity, key: EguiPipelineKey, world: &mut World) {
    let ids = world.resource_scope(|world, mut sp: Mut<SpecializedRenderPipelines<VectorPipeline>>| {
        let (cache, pipe) = (world.resource::<PipelineCache>(), world.resource::<VectorPipeline>());
        Pipelines { part: sp.specialize(cache, pipe, (key, false)), letter: sp.specialize(cache, pipe, (key, true)) }
    });
    let mut cache = world.resource_mut::<PipelineCache>();
    cache.block_on_render_pipeline(ids.part);
    cache.block_on_render_pipeline(ids.letter);
    world.entity_mut(render_entity.id()).insert(ids);
}

fn edge_group(world: &World, edges: &[[f32; 4]]) -> BindGroup {
    let device = world.resource::<RenderDevice>();
    // (never empty: a storage buffer can't be)
    let none = [[0.0f32; 4]];
    let data = if edges.is_empty() { &none[..] } else { edges };
    let buffer = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("beverly edges"), contents: bytes_of(data), usage: BufferUsages::STORAGE });
    let layout = world.resource::<PipelineCache>().get_bind_group_layout(&world.resource::<VectorPipeline>().edges);
    device.create_bind_group(Some("beverly edges"), &layout, &[BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }])
}

fn pipeline<'w>(world: &'w World, render_entity: RenderEntity, letter: bool) -> Option<&'w bevy::render::render_resource::RenderPipeline> {
    let ids = world.get_entity(render_entity.id()).ok()?.get::<Pipelines>()?;
    world.resource::<PipelineCache>().get_render_pipeline(if letter { ids.letter } else { ids.part })
}

impl EguiBevyPaintCallbackImpl for Draw {
    fn update(&self, _info: egui::PaintCallbackInfo, render_entity: RenderEntity, key: EguiPipelineKey, world: &mut World) {
        pipelines(render_entity, key, world);
        let group = edge_group(world, &self.edges);
        let buffer = world.resource::<RenderDevice>().create_buffer_with_data(&BufferInitDescriptor { label: Some("beverly part"), contents: bytes_of(&self.verts), usage: BufferUsages::VERTEX });
        world.resource_mut::<Drawn>().0.insert(self.id, (buffer, self.verts.len() as u32, Some(group)));
    }

    fn render<'pass>(&self, _info: egui::PaintCallbackInfo, pass: &mut bevy::render::render_phase::TrackedRenderPass<'pass>, render_entity: RenderEntity, _key: EguiPipelineKey, world: &'pass World) {
        let Some(pipeline) = pipeline(world, render_entity, false) else { return };
        let Some((buffer, n, Some(group))) = world.resource::<Drawn>().0.get(&self.id) else { return };
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..*n, 0..1);
    }
}

/// The letters' shapes (all of them: triangles and edges, in ems) as the GPU keeps them, and which version.
#[derive(Resource, Default)]
struct LetterStore { version: u64, verts: Option<Buffer>, group: Option<BindGroup> }

/// A run of letters (one clip): their instances grouped by letter shape, each group one draw of that shape.
pub struct GlyphBatch {
    id: u64,
    store: Arc<(Vec<LetterVertex>, Vec<[f32; 4]>)>,
    version: u64,
    instances: Vec<Instance>,
    draws: Vec<(u32, u32, u32, u32)>,
}

impl GlyphBatch {
    /// From each letter's place in the store (first vertex, count) and its instance.
    pub fn new(mut list: Vec<((u32, u32), Instance)>, store: Arc<(Vec<LetterVertex>, Vec<[f32; 4]>)>, version: u64) -> Self {
        list.sort_by_key(|(r, _)| *r);
        let mut draws: Vec<(u32, u32, u32, u32)> = vec![];
        for (k, (r, _)) in list.iter().enumerate() {
            match draws.last_mut() {
                Some(d) if (d.0, d.1) == *r => d.3 += 1,
                _ => draws.push((r.0, r.1, k as u32, 1)),
            }
        }
        GlyphBatch { id: next_id(), store, version, instances: list.into_iter().map(|(_, i)| i).collect(), draws }
    }
}

impl EguiBevyPaintCallbackImpl for GlyphBatch {
    fn update(&self, _info: egui::PaintCallbackInfo, render_entity: RenderEntity, key: EguiPipelineKey, world: &mut World) {
        pipelines(render_entity, key, world);
        let device = world.resource::<RenderDevice>().clone();
        let stale = { let s = world.resource::<LetterStore>(); s.verts.is_none() || s.version < self.version };
        if stale && !self.store.0.is_empty() {
            let verts = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("beverly letters"), contents: bytes_of(&self.store.0), usage: BufferUsages::VERTEX });
            let group = edge_group(world, &self.store.1);
            *world.resource_mut::<LetterStore>() = LetterStore { version: self.version, verts: Some(verts), group: Some(group) };
        }
        let buffer = device.create_buffer_with_data(&BufferInitDescriptor { label: Some("beverly letters placed"), contents: bytes_of(&self.instances), usage: BufferUsages::VERTEX });
        world.resource_mut::<Drawn>().0.insert(self.id, (buffer, self.instances.len() as u32, None));
    }

    fn render<'pass>(&self, _info: egui::PaintCallbackInfo, pass: &mut bevy::render::render_phase::TrackedRenderPass<'pass>, render_entity: RenderEntity, _key: EguiPipelineKey, world: &'pass World) {
        let Some(pipeline) = pipeline(world, render_entity, true) else { return };
        let store = world.resource::<LetterStore>();
        let (Some(verts), Some(group)) = (store.verts.as_ref(), store.group.as_ref()) else { return };
        let Some((instances, _, _)) = world.resource::<Drawn>().0.get(&self.id) else { return };
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.set_vertex_buffer(0, verts.slice(..));
        pass.set_vertex_buffer(1, instances.slice(..));
        for &(first, count, inst, n) in &self.draws {
            pass.draw(first..first + count, inst..inst + n);
        }
    }
}

#[derive(Resource)]
struct VectorPipeline { shader: Handle<Shader>, edges: BindGroupLayoutDescriptor }

impl SpecializedRenderPipeline for VectorPipeline {
    /// The target, and whether for letters (else parts).
    type Key = (EguiPipelineKey, bool);

    fn specialize(&self, (key, letter): Self::Key) -> RenderPipelineDescriptor {
        let buffers = if letter {
            let mut placed = VertexBufferLayout::from_vertex_formats(VertexStepMode::Instance, [VertexFormat::Float32x4, VertexFormat::Float32x2, VertexFormat::Unorm8x4]);
            for (k, a) in placed.attributes.iter_mut().enumerate() { a.shader_location = 2 + k as u32; }
            vec![VertexBufferLayout::from_vertex_formats(VertexStepMode::Vertex, [VertexFormat::Float32x4, VertexFormat::Uint32x2]), placed]
        } else {
            vec![VertexBufferLayout::from_vertex_formats(VertexStepMode::Vertex, [VertexFormat::Float32x2, VertexFormat::Unorm8x4, VertexFormat::Uint32x2])]
        };
        RenderPipelineDescriptor {
            label: Some(if letter { "beverly letters" } else { "beverly parts" }.into()),
            layout: vec![self.edges.clone()],
            immediate_size: 0,
            vertex: VertexState { shader: self.shader.clone(), shader_defs: vec![], entry_point: Some(if letter { "letter" } else { "part" }.into()), buffers, constants: vec![] },
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs: vec![],
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState { format: key.target_format, blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING), write_mask: ColorWrites::ALL })],
                constants: vec![],
            }),
            zero_initialize_workgroup_memory: false,
        }
    }
}
