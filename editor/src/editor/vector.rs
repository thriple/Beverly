//! Drawing the design's parts as vectors (frame.rs builds them): each part is one draw in egui's order (an egui paint
//! callback), with its own shader (vector.wgsl) that covers each pixel near an edge by its distance to the real
//! edges, so the parts are exact at any size. No images: the triangles and edges are worked out from the design's
//! paths every time they're drawn.
use bevy::prelude::*;
use bevy::mesh::VertexBufferLayout;
use bevy::render::RenderApp;
use bevy::render::render_resource::{
    BlendState, Buffer, BufferInitDescriptor, BufferUsages, ColorTargetState, ColorWrites, FragmentState, MultisampleState, PipelineCache,
    PrimitiveState, RenderPipelineDescriptor, SpecializedRenderPipeline, SpecializedRenderPipelines, VertexFormat, VertexState,
    VertexStepMode,
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
        if let Some(r) = app.get_sub_app_mut(RenderApp) {
            r.insert_resource(VectorPipeline { shader }).init_resource::<SpecializedRenderPipelines<VectorPipeline>>().init_resource::<Drawn>();
        }
    }
}

/// One vertex: where (clip space), its colour (egui's: premultiplied, gamma), and up to six nearby edge segments:
/// their ends a, b (window pixels) and the outward directions there (angles); unused ones have a = b.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub color: [u8; 4],
    pub edges: [[f32; 6]; 6],
}

impl Vertex {
    fn bytes(v: &[Vertex]) -> &[u8] {
        // SAFETY: Vertex is repr(C), plain floats and bytes with no padding (2·4 + 4 + 6·6·4 bytes).
        unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
    }
}

/// Draw `verts` (a triangle list) in the painter's place in egui's order.
pub fn paint(p: &egui::Painter, verts: Vec<Vertex>) {
    if verts.is_empty() { return; }
    let frame = p.ctx().cumulative_pass_nr();
    let id = p.ctx().data_mut(|d| {
        let n = d.get_temp_mut_or_default::<(u64, u64)>(egui::Id::new("vector-draws"));
        if n.0 != frame { *n = (frame, 0); }
        n.1 += 1;
        n.1
    });
    p.add(EguiBevyPaintCallback::new_paint_callback(p.ctx().content_rect(), Draw { frame, id, verts: Arc::new(verts) }));
}

struct Draw {
    frame: u64,
    id: u64,
    verts: Arc<Vec<Vertex>>,
}

/// This frame's vertex buffers, by draw.
#[derive(Resource, Default)]
struct Drawn { frame: u64, buffers: HashMap<u64, (Buffer, u32)> }

#[derive(Component)]
struct VectorPipelineId(bevy::render::render_resource::CachedRenderPipelineId);

impl EguiBevyPaintCallbackImpl for Draw {
    fn update(&self, _info: egui::PaintCallbackInfo, render_entity: RenderEntity, key: EguiPipelineKey, world: &mut World) {
        let id = world.resource_scope(|world, mut pipelines: Mut<SpecializedRenderPipelines<VectorPipeline>>| {
            pipelines.specialize(world.resource::<PipelineCache>(), world.resource::<VectorPipeline>(), key)
        });
        world.entity_mut(render_entity.id()).insert(VectorPipelineId(id));
        world.resource_mut::<PipelineCache>().block_on_render_pipeline(id);
        let buffer = world.resource::<RenderDevice>().create_buffer_with_data(&BufferInitDescriptor {
            label: Some("beverly vector part"),
            contents: Vertex::bytes(&self.verts),
            usage: BufferUsages::VERTEX,
        });
        let mut drawn = world.resource_mut::<Drawn>();
        if drawn.frame != self.frame {
            drawn.frame = self.frame;
            drawn.buffers.clear();
        }
        drawn.buffers.insert(self.id, (buffer, self.verts.len() as u32));
    }

    fn render<'pass>(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut bevy::render::render_phase::TrackedRenderPass<'pass>,
        render_entity: RenderEntity,
        _key: EguiPipelineKey,
        world: &'pass World,
    ) {
        let Some(pipeline) = world.get_entity(render_entity.id()).ok().and_then(|e| e.get::<VectorPipelineId>()).and_then(|p| world.resource::<PipelineCache>().get_render_pipeline(p.0)) else { return };
        let Some((buffer, n)) = world.resource::<Drawn>().buffers.get(&self.id) else { return };
        render_pass.set_render_pipeline(pipeline);
        render_pass.set_vertex_buffer(0, buffer.slice(..));
        render_pass.draw(0..*n, 0..1);
    }
}

#[derive(Resource)]
struct VectorPipeline { shader: Handle<Shader> }

impl SpecializedRenderPipeline for VectorPipeline {
    type Key = EguiPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let mut formats = vec![VertexFormat::Float32x2, VertexFormat::Unorm8x4];
        formats.extend([VertexFormat::Float32x4; 9]);
        RenderPipelineDescriptor {
            label: Some("beverly vector parts".into()),
            layout: vec![],
            immediate_size: 0,
            vertex: VertexState {
                shader: self.shader.clone(),
                shader_defs: vec![],
                entry_point: Some("vertex".into()),
                buffers: vec![VertexBufferLayout::from_vertex_formats(VertexStepMode::Vertex, formats)],
                constants: vec![],
            },
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
