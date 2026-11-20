use super::{instance_manager::InstanceManager, meshlet_mesh_manager::MeshletMeshManager, CpuTimer, MeshletCpuTimes};
use crate::ShadowView;
use bevy_camera::{visibility::RenderLayers, Camera3d};
use bevy_core_pipeline::{
    mip_generation::experimental::depth::{self, ViewDepthPyramid},
    prepass::{PreviousViewData, PreviousViewUniforms},
};
use alloc::sync::Arc;
use bevy_ecs::{
    component::Component,
    entity::{Entity, EntityHashMap},
    query::AnyOf,
    resource::Resource,
    system::{Commands, Query, Res, ResMut},
};
use bevy_image::ToExtents;
use bevy_math::{UVec2, Vec4Swizzles};
use bevy_render::{
    render_resource::*,
    renderer::{RenderDevice, RenderQueue},
    texture::{CachedTexture, TextureCache},
    view::{ExtractedView, Msaa, ViewUniform, ViewUniforms},
};
use binding_types::*;

/// Manages per-view and per-cluster GPU resources for [`MeshletPlugin`](`super::MeshletPlugin`).
#[derive(Resource)]
pub struct ResourceManager {
    /// Intermediate buffer of cluster IDs for use with rasterizing the visibility buffer
    visibility_buffer_raster_clusters: Buffer,
    /// Intermediate buffer of previous counts of clusters in rasterizer buckets
    pub visibility_buffer_raster_cluster_prev_counts: Buffer,
    /// Intermediate buffer of count of clusters to software rasterize
    software_raster_cluster_count: Buffer,
    /// BVH traversal queues
    bvh_traversal_queues: [Buffer; 2],
    /// Cluster cull candidate queue
    cluster_cull_candidate_queue: Buffer,
    /// Rightmost slot index of [`Self::visibility_buffer_raster_clusters`], [`Self::bvh_traversal_queues`], and [`Self::cluster_cull_candidate_queue`]
    cull_queue_rightmost_slot: u32,

    /// Second pass instance candidates
    second_pass_candidates: Option<Buffer>,
    /// Sampler for a depth pyramid
    depth_pyramid_sampler: Sampler,
    /// Dummy texture view for binding depth pyramids with less than the maximum amount of mips
    depth_pyramid_dummy_texture: TextureView,

    // TODO
    previous_depth_pyramids: EntityHashMap<TextureView>,
    /// Tessera: each view's counter and indirect-argument buffers (`VIEW_BUFFERS`)
    view_buffers: EntityHashMap<[Buffer; 16]>,
    /// Tessera: each view's depth pyramid, with the size it was made for
    view_depth_pyramids: EntityHashMap<(UVec2, Arc<ViewDepthPyramid>)>,
    /// Tessera: each view's bind groups, with the resources they were made from
    view_bind_groups: EntityHashMap<(Vec<BindKey>, MeshletViewBindGroups)>,

    // Bind group layouts
    pub clear_visibility_buffer_bind_group_layout: BindGroupLayoutDescriptor,
    pub clear_visibility_buffer_shadow_view_bind_group_layout: BindGroupLayoutDescriptor,
    pub first_instance_cull_bind_group_layout: BindGroupLayoutDescriptor,
    pub second_instance_cull_bind_group_layout: BindGroupLayoutDescriptor,
    pub first_bvh_cull_bind_group_layout: BindGroupLayoutDescriptor,
    pub second_bvh_cull_bind_group_layout: BindGroupLayoutDescriptor,
    pub first_meshlet_cull_bind_group_layout: BindGroupLayoutDescriptor,
    pub second_meshlet_cull_bind_group_layout: BindGroupLayoutDescriptor,
    pub visibility_buffer_raster_bind_group_layout: BindGroupLayoutDescriptor,
    pub visibility_buffer_raster_shadow_view_bind_group_layout: BindGroupLayoutDescriptor,
    pub downsample_depth_bind_group_layout: BindGroupLayoutDescriptor,
    pub downsample_depth_shadow_view_bind_group_layout: BindGroupLayoutDescriptor,
    pub resolve_depth_bind_group_layout: BindGroupLayoutDescriptor,
    pub resolve_depth_shadow_view_bind_group_layout: BindGroupLayoutDescriptor,
    pub resolve_material_depth_bind_group_layout: BindGroupLayoutDescriptor,
    pub material_shade_bind_group_layout: BindGroupLayoutDescriptor,
    pub fill_counts_bind_group_layout: BindGroupLayoutDescriptor,
    pub remap_1d_to_2d_dispatch_bind_group_layout: Option<BindGroupLayoutDescriptor>,
}

impl ResourceManager {
    pub fn new(cluster_buffer_slots: u32, render_device: &RenderDevice) -> Self {
        let needs_dispatch_remap =
            cluster_buffer_slots > render_device.limits().max_compute_workgroups_per_dimension;
        // The IDs are a (u32, u32) of instance and index.
        let cull_queue_size = 2 * cluster_buffer_slots as u64 * size_of::<u32>() as u64;

        Self {
            visibility_buffer_raster_clusters: render_device.create_buffer(&BufferDescriptor {
                label: Some("meshlet_visibility_buffer_raster_clusters"),
                size: cull_queue_size,
                usage: BufferUsages::STORAGE,
                mapped_at_creation: false,
            }),
            visibility_buffer_raster_cluster_prev_counts: render_device.create_buffer(
                &BufferDescriptor {
                    label: Some("meshlet_visibility_buffer_raster_cluster_prev_counts"),
                    size: size_of::<u32>() as u64 * 2,
                    usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                },
            ),
            software_raster_cluster_count: render_device.create_buffer(&BufferDescriptor {
                label: Some("meshlet_software_raster_cluster_count"),
                size: size_of::<u32>() as u64,
                usage: BufferUsages::STORAGE,
                mapped_at_creation: false,
            }),
            bvh_traversal_queues: [
                render_device.create_buffer(&BufferDescriptor {
                    label: Some("meshlet_bvh_traversal_queue_0"),
                    size: cull_queue_size,
                    usage: BufferUsages::STORAGE,
                    mapped_at_creation: false,
                }),
                render_device.create_buffer(&BufferDescriptor {
                    label: Some("meshlet_bvh_traversal_queue_1"),
                    size: cull_queue_size,
                    usage: BufferUsages::STORAGE,
                    mapped_at_creation: false,
                }),
            ],
            cluster_cull_candidate_queue: render_device.create_buffer(&BufferDescriptor {
                label: Some("meshlet_cluster_cull_candidate_queue"),
                size: cull_queue_size,
                usage: BufferUsages::STORAGE,
                mapped_at_creation: false,
            }),
            cull_queue_rightmost_slot: cluster_buffer_slots - 1,

            second_pass_candidates: None,
            depth_pyramid_sampler: render_device.create_sampler(&SamplerDescriptor {
                label: Some("meshlet_depth_pyramid_sampler"),
                ..SamplerDescriptor::default()
            }),
            depth_pyramid_dummy_texture: depth::create_depth_pyramid_dummy_texture(
                render_device,
                "meshlet_depth_pyramid_dummy_texture",
                "meshlet_depth_pyramid_dummy_texture_view",
            ),

            previous_depth_pyramids: EntityHashMap::default(),
            view_buffers: EntityHashMap::default(),
            view_depth_pyramids: EntityHashMap::default(),
            view_bind_groups: EntityHashMap::default(),

            // TODO: Buffer min sizes
            clear_visibility_buffer_bind_group_layout: BindGroupLayoutDescriptor::new(
                "meshlet_clear_visibility_buffer_bind_group_layout",
                &BindGroupLayoutEntries::single(
                    ShaderStages::COMPUTE,
                    texture_storage_2d(TextureFormat::R64Uint, StorageTextureAccess::WriteOnly),
                ),
            ),
            clear_visibility_buffer_shadow_view_bind_group_layout: BindGroupLayoutDescriptor::new(
                "meshlet_clear_visibility_buffer_shadow_view_bind_group_layout",
                &BindGroupLayoutEntries::single(
                    ShaderStages::COMPUTE,
                    texture_storage_2d(TextureFormat::R32Uint, StorageTextureAccess::WriteOnly),
                ),
            ),
            first_instance_cull_bind_group_layout: BindGroupLayoutDescriptor::new(
                "meshlet_first_instance_culling_bind_group_layout",
                &BindGroupLayoutEntries::sequential(
                    ShaderStages::COMPUTE,
                    (
                        texture_2d(TextureSampleType::Float { filterable: false }),
                        uniform_buffer::<ViewUniform>(true),
                        uniform_buffer::<PreviousViewData>(true),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                    ),
                ),
            ),
            second_instance_cull_bind_group_layout: BindGroupLayoutDescriptor::new(
                "meshlet_second_instance_culling_bind_group_layout",
                &BindGroupLayoutEntries::sequential(
                    ShaderStages::COMPUTE,
                    (
                        texture_2d(TextureSampleType::Float { filterable: false }),
                        uniform_buffer::<ViewUniform>(true),
                        uniform_buffer::<PreviousViewData>(true),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                    ),
                ),
            ),
            first_bvh_cull_bind_group_layout: BindGroupLayoutDescriptor::new(
                "meshlet_first_bvh_culling_bind_group_layout",
                &BindGroupLayoutEntries::sequential(
                    ShaderStages::COMPUTE,
                    (
                        texture_2d(TextureSampleType::Float { filterable: false }),
                        uniform_buffer::<ViewUniform>(true),
                        uniform_buffer::<PreviousViewData>(true),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                    ),
                ),
            ),
            second_bvh_cull_bind_group_layout: BindGroupLayoutDescriptor::new(
                "meshlet_second_bvh_culling_bind_group_layout",
                &BindGroupLayoutEntries::sequential(
                    ShaderStages::COMPUTE,
                    (
                        texture_2d(TextureSampleType::Float { filterable: false }),
                        uniform_buffer::<ViewUniform>(true),
                        uniform_buffer::<PreviousViewData>(true),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                    ),
                ),
            ),
            first_meshlet_cull_bind_group_layout: BindGroupLayoutDescriptor::new(
                "meshlet_first_meshlet_culling_bind_group_layout",
                &BindGroupLayoutEntries::sequential(
                    ShaderStages::COMPUTE,
                    (
                        texture_2d(TextureSampleType::Float { filterable: false }),
                        uniform_buffer::<ViewUniform>(true),
                        uniform_buffer::<PreviousViewData>(true),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                    ),
                ),
            ),
            second_meshlet_cull_bind_group_layout: BindGroupLayoutDescriptor::new(
                "meshlet_second_meshlet_culling_bind_group_layout",
                &BindGroupLayoutEntries::sequential(
                    ShaderStages::COMPUTE,
                    (
                        texture_2d(TextureSampleType::Float { filterable: false }),
                        uniform_buffer::<ViewUniform>(true),
                        uniform_buffer::<PreviousViewData>(true),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                    ),
                ),
            ),
            downsample_depth_bind_group_layout: BindGroupLayoutDescriptor::new(
                "meshlet_downsample_depth_bind_group_layout",
                &BindGroupLayoutEntries::sequential(ShaderStages::COMPUTE, {
                    let write_only_r32float = || {
                        texture_storage_2d(TextureFormat::R32Float, StorageTextureAccess::WriteOnly)
                    };
                    (
                        texture_storage_2d(TextureFormat::R64Uint, StorageTextureAccess::ReadOnly),
                        write_only_r32float(),
                        write_only_r32float(),
                        write_only_r32float(),
                        write_only_r32float(),
                        write_only_r32float(),
                        texture_storage_2d(
                            TextureFormat::R32Float,
                            StorageTextureAccess::ReadWrite,
                        ),
                        write_only_r32float(),
                        write_only_r32float(),
                        write_only_r32float(),
                        write_only_r32float(),
                        write_only_r32float(),
                        write_only_r32float(),
                        sampler(SamplerBindingType::NonFiltering),
                    )
                }),
            ),
            downsample_depth_shadow_view_bind_group_layout: BindGroupLayoutDescriptor::new(
                "meshlet_downsample_depth_shadow_view_bind_group_layout",
                &BindGroupLayoutEntries::sequential(ShaderStages::COMPUTE, {
                    let write_only_r32float = || {
                        texture_storage_2d(TextureFormat::R32Float, StorageTextureAccess::WriteOnly)
                    };
                    (
                        texture_storage_2d(TextureFormat::R32Uint, StorageTextureAccess::ReadOnly),
                        write_only_r32float(),
                        write_only_r32float(),
                        write_only_r32float(),
                        write_only_r32float(),
                        write_only_r32float(),
                        texture_storage_2d(
                            TextureFormat::R32Float,
                            StorageTextureAccess::ReadWrite,
                        ),
                        write_only_r32float(),
                        write_only_r32float(),
                        write_only_r32float(),
                        write_only_r32float(),
                        write_only_r32float(),
                        write_only_r32float(),
                        sampler(SamplerBindingType::NonFiltering),
                    )
                }),
            ),
            visibility_buffer_raster_bind_group_layout: BindGroupLayoutDescriptor::new(
                "meshlet_visibility_buffer_raster_bind_group_layout",
                &BindGroupLayoutEntries::sequential(
                    ShaderStages::FRAGMENT | ShaderStages::VERTEX | ShaderStages::COMPUTE,
                    (
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        texture_storage_2d(TextureFormat::R64Uint, StorageTextureAccess::Atomic),
                        uniform_buffer::<ViewUniform>(true),
                    ),
                ),
            ),
            visibility_buffer_raster_shadow_view_bind_group_layout: BindGroupLayoutDescriptor::new(
                "meshlet_visibility_buffer_raster_shadow_view_bind_group_layout",
                &BindGroupLayoutEntries::sequential(
                    ShaderStages::FRAGMENT | ShaderStages::VERTEX | ShaderStages::COMPUTE,
                    (
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        texture_storage_2d(TextureFormat::R32Uint, StorageTextureAccess::Atomic),
                        uniform_buffer::<ViewUniform>(true),
                    ),
                ),
            ),
            resolve_depth_bind_group_layout: BindGroupLayoutDescriptor::new(
                "meshlet_resolve_depth_bind_group_layout",
                &BindGroupLayoutEntries::single(
                    ShaderStages::FRAGMENT,
                    texture_storage_2d(TextureFormat::R64Uint, StorageTextureAccess::ReadOnly),
                ),
            ),
            resolve_depth_shadow_view_bind_group_layout: BindGroupLayoutDescriptor::new(
                "meshlet_resolve_depth_shadow_view_bind_group_layout",
                &BindGroupLayoutEntries::single(
                    ShaderStages::FRAGMENT,
                    texture_storage_2d(TextureFormat::R32Uint, StorageTextureAccess::ReadOnly),
                ),
            ),
            resolve_material_depth_bind_group_layout: BindGroupLayoutDescriptor::new(
                "meshlet_resolve_material_depth_bind_group_layout",
                &BindGroupLayoutEntries::sequential(
                    ShaderStages::FRAGMENT,
                    (
                        texture_storage_2d(TextureFormat::R64Uint, StorageTextureAccess::ReadOnly),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                    ),
                ),
            ),
            material_shade_bind_group_layout: BindGroupLayoutDescriptor::new(
                "meshlet_mesh_material_shade_bind_group_layout",
                &BindGroupLayoutEntries::sequential(
                    ShaderStages::FRAGMENT,
                    (
                        texture_storage_2d(TextureFormat::R64Uint, StorageTextureAccess::ReadOnly),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                        storage_buffer_read_only_sized(false, None),
                    ),
                ),
            ),
            fill_counts_bind_group_layout: if needs_dispatch_remap {
                BindGroupLayoutDescriptor::new(
                    "meshlet_fill_counts_bind_group_layout",
                    &BindGroupLayoutEntries::sequential(
                        ShaderStages::COMPUTE,
                        (
                            storage_buffer_sized(false, None),
                            storage_buffer_sized(false, None),
                            storage_buffer_sized(false, None),
                            storage_buffer_sized(false, None),
                        ),
                    ),
                )
            } else {
                BindGroupLayoutDescriptor::new(
                    "meshlet_fill_counts_bind_group_layout",
                    &BindGroupLayoutEntries::sequential(
                        ShaderStages::COMPUTE,
                        (
                            storage_buffer_sized(false, None),
                            storage_buffer_sized(false, None),
                            storage_buffer_sized(false, None),
                        ),
                    ),
                )
            },
            remap_1d_to_2d_dispatch_bind_group_layout: needs_dispatch_remap.then(|| {
                BindGroupLayoutDescriptor::new(
                    "meshlet_remap_1d_to_2d_dispatch_bind_group_layout",
                    &BindGroupLayoutEntries::sequential(
                        ShaderStages::COMPUTE,
                        (
                            storage_buffer_sized(false, None),
                            storage_buffer_sized(false, None),
                        ),
                    ),
                )
            }),
        }
    }
}

// ------------ TODO: Everything under here needs to be rewritten and cached ------------

/// Tessera: what a view's counter or indirect-argument buffer holds at the start of each frame.
#[derive(Clone, Copy)]
enum ViewBufferStart {
    Count,
    Dispatch,
    Draw,
}

impl ViewBufferStart {
    fn bytes(self) -> Vec<u8> {
        match self {
            Self::Count => bytemuck::bytes_of(&0u32).to_vec(),
            Self::Dispatch => DispatchIndirectArgs { x: 0, y: 1, z: 1 }.as_bytes().to_vec(),
            Self::Draw => DrawIndirectArgs {
                vertex_count: 128 * 3,
                instance_count: 0,
                first_vertex: 0,
                first_instance: 0,
            }
            .as_bytes()
            .to_vec(),
        }
    }
}

const STORAGE: BufferUsages = BufferUsages::STORAGE;
const STORAGE_INDIRECT: BufferUsages = BufferUsages::STORAGE.union(BufferUsages::INDIRECT);

/// Tessera: a view's counter and indirect-argument buffers, in the order `prepare_meshlet_per_frame_resources`
/// takes them (Bevy made these anew each frame; they're kept and reset with a write instead, hence COPY_DST).
const VIEW_BUFFERS: [(&str, ViewBufferStart, BufferUsages); 16] = [
    ("meshlet_second_pass_count", ViewBufferStart::Count, STORAGE),
    ("meshlet_second_pass_dispatch", ViewBufferStart::Dispatch, STORAGE_INDIRECT),
    ("meshlet_first_bvh_cull_count_front", ViewBufferStart::Count, STORAGE),
    ("meshlet_first_bvh_cull_dispatch_front", ViewBufferStart::Dispatch, STORAGE_INDIRECT),
    ("meshlet_first_bvh_cull_count_back", ViewBufferStart::Count, STORAGE),
    ("meshlet_first_bvh_cull_dispatch_back", ViewBufferStart::Dispatch, STORAGE_INDIRECT),
    ("meshlet_second_bvh_cull_count_front", ViewBufferStart::Count, STORAGE),
    ("meshlet_second_bvh_cull_dispatch_front", ViewBufferStart::Dispatch, STORAGE_INDIRECT),
    ("meshlet_second_bvh_cull_count_back", ViewBufferStart::Count, STORAGE),
    ("meshlet_second_bvh_cull_dispatch_back", ViewBufferStart::Dispatch, STORAGE_INDIRECT),
    ("meshlet_front_meshlet_cull_count", ViewBufferStart::Count, STORAGE),
    ("meshlet_front_meshlet_cull_dispatch", ViewBufferStart::Dispatch, STORAGE_INDIRECT),
    ("meshlet_back_meshlet_cull_count", ViewBufferStart::Count, STORAGE),
    ("meshlet_back_meshlet_cull_dispatch", ViewBufferStart::Dispatch, STORAGE_INDIRECT),
    ("meshlet_visibility_buffer_software_raster_indirect_args", ViewBufferStart::Dispatch, STORAGE_INDIRECT),
    ("meshlet_visibility_buffer_hardware_raster_indirect_args", ViewBufferStart::Draw, STORAGE_INDIRECT),
];

fn view_buffers(render_device: &RenderDevice) -> [Buffer; 16] {
    VIEW_BUFFERS.map(|(label, start, usage)| {
        render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some(label),
            contents: &start.bytes(),
            usage: usage | BufferUsages::COPY_DST,
        })
    })
}

/// Tessera: one GPU resource a view's bind groups were made from; they're made again only when one changes.
#[derive(Clone, PartialEq, Eq)]
enum BindKey {
    Buffer(Option<BufferId>),
    View(TextureViewId),
    ShadowView(bool),
}

impl MeshletViewResources {
    /// Tessera: everything of the view's that its bind groups use.
    fn bind_key(&self, key: &mut Vec<BindKey>) {
        for buffer in [
            &self.instance_visibility,
            &self.second_pass_count,
            &self.second_pass_dispatch,
            &self.second_pass_candidates,
            &self.first_bvh_cull_count_front,
            &self.first_bvh_cull_dispatch_front,
            &self.first_bvh_cull_count_back,
            &self.first_bvh_cull_dispatch_back,
            &self.first_bvh_cull_queue,
            &self.second_bvh_cull_count_front,
            &self.second_bvh_cull_dispatch_front,
            &self.second_bvh_cull_count_back,
            &self.second_bvh_cull_dispatch_back,
            &self.second_bvh_cull_queue,
            &self.front_meshlet_cull_count,
            &self.front_meshlet_cull_dispatch,
            &self.back_meshlet_cull_count,
            &self.back_meshlet_cull_dispatch,
            &self.meshlet_cull_queue,
            &self.visibility_buffer_software_raster_indirect_args,
            &self.visibility_buffer_hardware_raster_indirect_args,
        ] {
            key.push(BindKey::Buffer(Some(buffer.id())));
        }
        key.push(BindKey::View(self.visibility_buffer.default_view.id()));
        key.push(BindKey::View(self.depth_pyramid.all_mips.id()));
        key.push(BindKey::View(self.previous_depth_pyramid.id()));
        if let Some(material_depth) = &self.material_depth {
            key.push(BindKey::View(material_depth.default_view.id()));
        }
        key.push(BindKey::ShadowView(self.not_shadow_view));
    }
}

#[derive(Component)]
pub struct MeshletViewResources {
    pub scene_instance_count: u32,
    pub rightmost_slot: u32,
    pub max_bvh_depth: u32,
    instance_visibility: Buffer,
    pub dummy_render_target: CachedTexture,
    pub visibility_buffer: CachedTexture,
    pub second_pass_count: Buffer,
    pub second_pass_dispatch: Buffer,
    pub second_pass_candidates: Buffer,
    pub first_bvh_cull_count_front: Buffer,
    pub first_bvh_cull_dispatch_front: Buffer,
    pub first_bvh_cull_count_back: Buffer,
    pub first_bvh_cull_dispatch_back: Buffer,
    pub first_bvh_cull_queue: Buffer,
    pub second_bvh_cull_count_front: Buffer,
    pub second_bvh_cull_dispatch_front: Buffer,
    pub second_bvh_cull_count_back: Buffer,
    pub second_bvh_cull_dispatch_back: Buffer,
    pub second_bvh_cull_queue: Buffer,
    pub front_meshlet_cull_count: Buffer,
    pub front_meshlet_cull_dispatch: Buffer,
    pub back_meshlet_cull_count: Buffer,
    pub back_meshlet_cull_dispatch: Buffer,
    pub meshlet_cull_queue: Buffer,
    pub visibility_buffer_software_raster_indirect_args: Buffer,
    pub visibility_buffer_hardware_raster_indirect_args: Buffer,
    pub depth_pyramid: Arc<ViewDepthPyramid>,
    previous_depth_pyramid: TextureView,
    pub material_depth: Option<CachedTexture>,
    pub view_size: UVec2,
    not_shadow_view: bool,
}

#[derive(Component, Clone)]
pub struct MeshletViewBindGroups {
    pub clear_visibility_buffer: BindGroup,
    pub first_instance_cull: BindGroup,
    pub second_instance_cull: BindGroup,
    pub first_bvh_cull_ping: BindGroup,
    pub first_bvh_cull_pong: BindGroup,
    pub second_bvh_cull_ping: BindGroup,
    pub second_bvh_cull_pong: BindGroup,
    pub first_meshlet_cull: BindGroup,
    pub second_meshlet_cull: BindGroup,
    pub downsample_depth: BindGroup,
    pub visibility_buffer_raster: BindGroup,
    pub resolve_depth: BindGroup,
    pub resolve_material_depth: Option<BindGroup>,
    pub material_shade: Option<BindGroup>,
    pub remap_1d_to_2d_dispatch: Option<BindGroup>,
    pub fill_counts: BindGroup,
}

// TODO: Cache things per-view and skip running this system / optimize this system
pub fn prepare_meshlet_per_frame_resources(
    mut resource_manager: ResMut<ResourceManager>,
    mut instance_manager: ResMut<InstanceManager>,
    views: Query<(
        Entity,
        &ExtractedView,
        Option<&RenderLayers>,
        AnyOf<(&Camera3d, &ShadowView)>,
        Option<&Msaa>,
    )>,
    mut texture_cache: ResMut<TextureCache>,
    render_queue: Res<RenderQueue>,
    render_device: Res<RenderDevice>,
    mut commands: Commands,
    mut times: ResMut<MeshletCpuTimes>,
) {
    let _timer = CpuTimer::start(&mut times.prepare_resources);
    if instance_manager.scene_instance_count == 0 {
        return;
    }

    let instance_manager = instance_manager.as_mut();

    // TODO: Move this and the submit to a separate system and remove pub from the fields
    // Tessera: only what changed is uploaded
    instance_manager
        .instance_uniforms
        .upload(&render_device, &render_queue);
    instance_manager
        .instance_aabbs
        .upload(&render_device, &render_queue);
    instance_manager
        .instance_material_ids
        .upload(&render_device, &render_queue);
    instance_manager
        .instance_bvh_root_nodes
        .upload(&render_device, &render_queue);

    let needed_buffer_size = 4 * instance_manager.scene_instance_count as u64;
    let second_pass_candidates = match &mut resource_manager.second_pass_candidates {
        Some(buffer) if buffer.size() >= needed_buffer_size => buffer.clone(),
        slot => {
            let buffer = render_device.create_buffer(&BufferDescriptor {
                label: Some("meshlet_second_pass_candidates"),
                size: needed_buffer_size,
                usage: BufferUsages::STORAGE,
                mapped_at_creation: false,
            });
            *slot = Some(buffer.clone());
            buffer
        }
    };

    // Tessera: drop what was kept for views that are gone
    instance_manager
        .view_instance_visibility
        .retain(|view_entity, _| views.contains(*view_entity));
    resource_manager
        .previous_depth_pyramids
        .retain(|view_entity, _| views.contains(*view_entity));
    resource_manager
        .view_buffers
        .retain(|view_entity, _| views.contains(*view_entity));
    resource_manager
        .view_depth_pyramids
        .retain(|view_entity, _| views.contains(*view_entity));

    for (view_entity, view, render_layers, (_, shadow_view), msaa) in &views {
        // Tessera: views with MSAA draw no meshlet meshes (`configure_meshlet_views`)
        if msaa.is_some_and(|msaa| *msaa != Msaa::Off) {
            continue;
        }
        let not_shadow_view = shadow_view.is_none();

        // Tessera: built in full for a new view or when the view's layers changed; otherwise only the
        // instances in `visibility_changes` are looked at again
        let view_layers = render_layers.cloned().unwrap_or_default();
        let mut new_view = false;
        let (built_layers, built_shadow, instance_visibility) = instance_manager
            .view_instance_visibility
            .entry(view_entity)
            .or_insert_with(|| {
                new_view = true;
                let mut buffer = StorageBuffer::default();
                buffer.set_label(Some("meshlet_view_instance_visibility"));
                (RenderLayers::default(), false, buffer)
            });
        let full = new_view || (&*built_layers, *built_shadow) != (&view_layers, shadow_view.is_some());
        if full || !instance_manager.visibility_changes.is_empty() {
            let instances = &instance_manager.instances;
            let vec = instance_visibility.get_mut();
            if full {
                vec.clear();
            }
            vec.resize(instances.len().div_ceil(32).max(1), 0);
            // If either the layers don't match the view's layers or this is a shadow view
            // and the instance is not a shadow caster, hide the instance for this view
            let mut update = |instance_index: usize| {
                let (_, layers, not_shadow_caster) = &instances[instance_index];
                let hidden = !view_layers.intersects(layers)
                    || (shadow_view.is_some() && *not_shadow_caster);
                let bit = 1 << (instance_index % 32);
                if hidden {
                    vec[instance_index / 32] |= bit;
                } else {
                    vec[instance_index / 32] &= !bit;
                }
            };
            if full {
                (0..instances.len()).for_each(&mut update);
            } else {
                for &i in &instance_manager.visibility_changes {
                    if (i as usize) < instances.len() {
                        update(i as usize);
                    }
                }
            }
            instance_visibility.write_buffer(&render_device, &render_queue);
            (*built_layers, *built_shadow) = (view_layers, shadow_view.is_some());
        }
        let instance_visibility = instance_visibility.buffer().unwrap().clone();

        // TODO: Remove this once wgpu allows render passes with no attachments
        let dummy_render_target = texture_cache.get(
            &render_device,
            TextureDescriptor {
                label: Some("meshlet_dummy_render_target"),
                size: view.viewport.zw().to_extents(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::R8Uint,
                usage: TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            },
        );

        let visibility_buffer = texture_cache.get(
            &render_device,
            TextureDescriptor {
                label: Some("meshlet_visibility_buffer"),
                size: view.viewport.zw().to_extents(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: if not_shadow_view {
                    TextureFormat::R64Uint
                } else {
                    TextureFormat::R32Uint
                },
                usage: TextureUsages::STORAGE_ATOMIC | TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            },
        );

        // Tessera: the view's counters and indirect arguments are made once, and reset here each frame
        let [second_pass_count, second_pass_dispatch, first_bvh_cull_count_front, first_bvh_cull_dispatch_front, first_bvh_cull_count_back, first_bvh_cull_dispatch_back, second_bvh_cull_count_front, second_bvh_cull_dispatch_front, second_bvh_cull_count_back, second_bvh_cull_dispatch_back, front_meshlet_cull_count, front_meshlet_cull_dispatch, back_meshlet_cull_count, back_meshlet_cull_dispatch, visibility_buffer_software_raster_indirect_args, visibility_buffer_hardware_raster_indirect_args] =
            resource_manager
                .view_buffers
                .entry(view_entity)
                .or_insert_with(|| view_buffers(&render_device))
                .clone();
        for ((_, start, _), buffer) in VIEW_BUFFERS.iter().zip([
            &second_pass_count,
            &second_pass_dispatch,
            &first_bvh_cull_count_front,
            &first_bvh_cull_dispatch_front,
            &first_bvh_cull_count_back,
            &first_bvh_cull_dispatch_back,
            &second_bvh_cull_count_front,
            &second_bvh_cull_dispatch_front,
            &second_bvh_cull_count_back,
            &second_bvh_cull_dispatch_back,
            &front_meshlet_cull_count,
            &front_meshlet_cull_dispatch,
            &back_meshlet_cull_count,
            &back_meshlet_cull_dispatch,
            &visibility_buffer_software_raster_indirect_args,
            &visibility_buffer_hardware_raster_indirect_args,
        ]) {
            render_queue.write_buffer(buffer, 0, &start.bytes());
        }

        // Tessera: the depth pyramid and its mip views are made once per view size, from a texture cache of
        // their own so a kept pyramid's texture is never handed to another view
        let size = view.viewport.zw();
        let depth_pyramid = match resource_manager.view_depth_pyramids.get(&view_entity) {
            Some((built_size, pyramid)) if *built_size == size => pyramid.clone(),
            _ => {
                let pyramid = Arc::new(ViewDepthPyramid::new(
                    &render_device,
                    &mut TextureCache::default(),
                    &resource_manager.depth_pyramid_dummy_texture,
                    size,
                    "meshlet_depth_pyramid",
                    "meshlet_depth_pyramid_texture_view",
                ));
                resource_manager
                    .view_depth_pyramids
                    .insert(view_entity, (size, pyramid.clone()));
                pyramid
            }
        };

        let previous_depth_pyramid =
            match resource_manager.previous_depth_pyramids.get(&view_entity) {
                Some(texture_view) => texture_view.clone(),
                None => depth_pyramid.all_mips.clone(),
            };
        resource_manager
            .previous_depth_pyramids
            .insert(view_entity, depth_pyramid.all_mips.clone());

        let material_depth = TextureDescriptor {
            label: Some("meshlet_material_depth"),
            size: view.viewport.zw().to_extents(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth16Unorm,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };

        commands.entity(view_entity).insert(MeshletViewResources {
            scene_instance_count: instance_manager.scene_instance_count,
            rightmost_slot: resource_manager.cull_queue_rightmost_slot,
            max_bvh_depth: instance_manager.max_bvh_depth,
            instance_visibility,
            dummy_render_target,
            visibility_buffer,
            second_pass_count,
            second_pass_dispatch,
            second_pass_candidates: second_pass_candidates.clone(),
            first_bvh_cull_count_front,
            first_bvh_cull_dispatch_front,
            first_bvh_cull_count_back,
            first_bvh_cull_dispatch_back,
            first_bvh_cull_queue: resource_manager.bvh_traversal_queues[0].clone(),
            second_bvh_cull_count_front,
            second_bvh_cull_dispatch_front,
            second_bvh_cull_count_back,
            second_bvh_cull_dispatch_back,
            second_bvh_cull_queue: resource_manager.bvh_traversal_queues[1].clone(),
            front_meshlet_cull_count,
            front_meshlet_cull_dispatch,
            back_meshlet_cull_count,
            back_meshlet_cull_dispatch,
            meshlet_cull_queue: resource_manager.cluster_cull_candidate_queue.clone(),
            visibility_buffer_software_raster_indirect_args,
            visibility_buffer_hardware_raster_indirect_args,
            depth_pyramid,
            previous_depth_pyramid,
            material_depth: not_shadow_view
                .then(|| texture_cache.get(&render_device, material_depth)),
            view_size: view.viewport.zw(),
            not_shadow_view,
        });
    }
    // Tessera: every view is up to date with them now
    instance_manager.visibility_changes.clear();
}

pub fn prepare_meshlet_view_bind_groups(
    meshlet_mesh_manager: Res<MeshletMeshManager>,
    mut resource_manager: ResMut<ResourceManager>,
    instance_manager: Res<InstanceManager>,
    views: Query<(Entity, &MeshletViewResources)>,
    view_uniforms: Res<ViewUniforms>,
    previous_view_uniforms: Res<PreviousViewUniforms>,
    render_device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    mut commands: Commands,
    mut times: ResMut<MeshletCpuTimes>,
) {
    let _timer = CpuTimer::start(&mut times.prepare_bind_groups);
    // Tessera: the resources every view's bind groups share
    let mut shared_key = vec![];
    for buffer in [
        view_uniforms.uniforms.buffer(),
        previous_view_uniforms.uniforms.buffer(),
        instance_manager.instance_uniforms.buffer(),
        instance_manager.instance_aabbs.buffer(),
        instance_manager.instance_material_ids.buffer(),
        instance_manager.instance_bvh_root_nodes.buffer(),
        Some(&resource_manager.software_raster_cluster_count),
        Some(&resource_manager.visibility_buffer_raster_cluster_prev_counts),
    ] {
        shared_key.push(BindKey::Buffer(buffer.map(Buffer::id)));
    }
    shared_key.extend(
        [
            meshlet_mesh_manager.bvh_nodes.buffer_id(),
            meshlet_mesh_manager.meshlet_cull_data.buffer_id(),
            meshlet_mesh_manager.meshlets.buffer_id(),
            meshlet_mesh_manager.indices.buffer_id(),
            meshlet_mesh_manager.vertex_positions.buffer_id(),
            meshlet_mesh_manager.vertex_normals.buffer_id(),
            meshlet_mesh_manager.vertex_uvs.buffer_id(),
        ]
        .map(|id| BindKey::Buffer(Some(id))),
    );
    resource_manager
        .view_bind_groups
        .retain(|view_entity, _| views.contains(*view_entity));
    let resource_manager = resource_manager.into_inner();

    let (Some(view_uniforms), Some(previous_view_uniforms)) = (
        view_uniforms.uniforms.binding(),
        previous_view_uniforms.uniforms.binding(),
    ) else {
        return;
    };

    // TODO: Some of these bind groups can be reused across multiple views
    for (view_entity, view_resources) in &views {
        // Tessera: made again only when a resource they use changed
        let mut key = shared_key.clone();
        view_resources.bind_key(&mut key);
        if let Some((built, bind_groups)) = resource_manager.view_bind_groups.get(&view_entity)
            && *built == key
        {
            commands.entity(view_entity).insert(bind_groups.clone());
            continue;
        }

        let clear_visibility_buffer = render_device.create_bind_group(
            "meshlet_clear_visibility_buffer_bind_group",
            &pipeline_cache.get_bind_group_layout(if view_resources.not_shadow_view {
                &resource_manager.clear_visibility_buffer_bind_group_layout
            } else {
                &resource_manager.clear_visibility_buffer_shadow_view_bind_group_layout
            }),
            &BindGroupEntries::single(&view_resources.visibility_buffer.default_view),
        );

        let first_instance_cull = render_device.create_bind_group(
            "meshlet_first_instance_cull_bind_group",
            &pipeline_cache
                .get_bind_group_layout(&resource_manager.first_instance_cull_bind_group_layout),
            &BindGroupEntries::sequential((
                &view_resources.previous_depth_pyramid,
                view_uniforms.clone(),
                previous_view_uniforms.clone(),
                instance_manager.instance_uniforms.binding().unwrap(),
                view_resources.instance_visibility.as_entire_binding(),
                instance_manager.instance_aabbs.binding().unwrap(),
                instance_manager.instance_bvh_root_nodes.binding().unwrap(),
                view_resources
                    .first_bvh_cull_count_front
                    .as_entire_binding(),
                view_resources
                    .first_bvh_cull_dispatch_front
                    .as_entire_binding(),
                view_resources.first_bvh_cull_queue.as_entire_binding(),
                view_resources.second_pass_count.as_entire_binding(),
                view_resources.second_pass_dispatch.as_entire_binding(),
                view_resources.second_pass_candidates.as_entire_binding(),
            )),
        );

        let second_instance_cull = render_device.create_bind_group(
            "meshlet_second_instance_cull_bind_group",
            &pipeline_cache
                .get_bind_group_layout(&resource_manager.second_instance_cull_bind_group_layout),
            &BindGroupEntries::sequential((
                &view_resources.previous_depth_pyramid,
                view_uniforms.clone(),
                previous_view_uniforms.clone(),
                instance_manager.instance_uniforms.binding().unwrap(),
                view_resources.instance_visibility.as_entire_binding(),
                instance_manager.instance_aabbs.binding().unwrap(),
                instance_manager.instance_bvh_root_nodes.binding().unwrap(),
                view_resources
                    .second_bvh_cull_count_front
                    .as_entire_binding(),
                view_resources
                    .second_bvh_cull_dispatch_front
                    .as_entire_binding(),
                view_resources.second_bvh_cull_queue.as_entire_binding(),
                view_resources.second_pass_count.as_entire_binding(),
                view_resources.second_pass_candidates.as_entire_binding(),
            )),
        );

        let first_bvh_cull_ping = render_device.create_bind_group(
            "meshlet_first_bvh_cull_ping_bind_group",
            &pipeline_cache
                .get_bind_group_layout(&resource_manager.first_bvh_cull_bind_group_layout),
            &BindGroupEntries::sequential((
                &view_resources.previous_depth_pyramid,
                view_uniforms.clone(),
                previous_view_uniforms.clone(),
                meshlet_mesh_manager.bvh_nodes.binding(),
                instance_manager.instance_uniforms.binding().unwrap(),
                view_resources
                    .first_bvh_cull_count_front
                    .as_entire_binding(),
                view_resources.first_bvh_cull_count_back.as_entire_binding(),
                view_resources
                    .first_bvh_cull_dispatch_back
                    .as_entire_binding(),
                view_resources.first_bvh_cull_queue.as_entire_binding(),
                view_resources.front_meshlet_cull_count.as_entire_binding(),
                view_resources.back_meshlet_cull_count.as_entire_binding(),
                view_resources
                    .front_meshlet_cull_dispatch
                    .as_entire_binding(),
                view_resources
                    .back_meshlet_cull_dispatch
                    .as_entire_binding(),
                view_resources.meshlet_cull_queue.as_entire_binding(),
                view_resources
                    .second_bvh_cull_count_front
                    .as_entire_binding(),
                view_resources
                    .second_bvh_cull_dispatch_front
                    .as_entire_binding(),
                view_resources.second_bvh_cull_queue.as_entire_binding(),
            )),
        );

        let first_bvh_cull_pong = render_device.create_bind_group(
            "meshlet_first_bvh_cull_pong_bind_group",
            &pipeline_cache
                .get_bind_group_layout(&resource_manager.first_bvh_cull_bind_group_layout),
            &BindGroupEntries::sequential((
                &view_resources.previous_depth_pyramid,
                view_uniforms.clone(),
                previous_view_uniforms.clone(),
                meshlet_mesh_manager.bvh_nodes.binding(),
                instance_manager.instance_uniforms.binding().unwrap(),
                view_resources.first_bvh_cull_count_back.as_entire_binding(),
                view_resources
                    .first_bvh_cull_count_front
                    .as_entire_binding(),
                view_resources
                    .first_bvh_cull_dispatch_front
                    .as_entire_binding(),
                view_resources.first_bvh_cull_queue.as_entire_binding(),
                view_resources.front_meshlet_cull_count.as_entire_binding(),
                view_resources.back_meshlet_cull_count.as_entire_binding(),
                view_resources
                    .front_meshlet_cull_dispatch
                    .as_entire_binding(),
                view_resources
                    .back_meshlet_cull_dispatch
                    .as_entire_binding(),
                view_resources.meshlet_cull_queue.as_entire_binding(),
                view_resources
                    .second_bvh_cull_count_front
                    .as_entire_binding(),
                view_resources
                    .second_bvh_cull_dispatch_front
                    .as_entire_binding(),
                view_resources.second_bvh_cull_queue.as_entire_binding(),
            )),
        );

        let second_bvh_cull_ping = render_device.create_bind_group(
            "meshlet_second_bvh_cull_ping_bind_group",
            &pipeline_cache
                .get_bind_group_layout(&resource_manager.second_bvh_cull_bind_group_layout),
            &BindGroupEntries::sequential((
                &view_resources.previous_depth_pyramid,
                view_uniforms.clone(),
                previous_view_uniforms.clone(),
                meshlet_mesh_manager.bvh_nodes.binding(),
                instance_manager.instance_uniforms.binding().unwrap(),
                view_resources
                    .second_bvh_cull_count_front
                    .as_entire_binding(),
                view_resources
                    .second_bvh_cull_count_back
                    .as_entire_binding(),
                view_resources
                    .second_bvh_cull_dispatch_back
                    .as_entire_binding(),
                view_resources.second_bvh_cull_queue.as_entire_binding(),
                view_resources.front_meshlet_cull_count.as_entire_binding(),
                view_resources.back_meshlet_cull_count.as_entire_binding(),
                view_resources
                    .front_meshlet_cull_dispatch
                    .as_entire_binding(),
                view_resources
                    .back_meshlet_cull_dispatch
                    .as_entire_binding(),
                view_resources.meshlet_cull_queue.as_entire_binding(),
            )),
        );

        let second_bvh_cull_pong = render_device.create_bind_group(
            "meshlet_second_bvh_cull_pong_bind_group",
            &pipeline_cache
                .get_bind_group_layout(&resource_manager.second_bvh_cull_bind_group_layout),
            &BindGroupEntries::sequential((
                &view_resources.previous_depth_pyramid,
                view_uniforms.clone(),
                previous_view_uniforms.clone(),
                meshlet_mesh_manager.bvh_nodes.binding(),
                instance_manager.instance_uniforms.binding().unwrap(),
                view_resources
                    .second_bvh_cull_count_back
                    .as_entire_binding(),
                view_resources
                    .second_bvh_cull_count_front
                    .as_entire_binding(),
                view_resources
                    .second_bvh_cull_dispatch_front
                    .as_entire_binding(),
                view_resources.second_bvh_cull_queue.as_entire_binding(),
                view_resources.front_meshlet_cull_count.as_entire_binding(),
                view_resources.back_meshlet_cull_count.as_entire_binding(),
                view_resources
                    .front_meshlet_cull_dispatch
                    .as_entire_binding(),
                view_resources
                    .back_meshlet_cull_dispatch
                    .as_entire_binding(),
                view_resources.meshlet_cull_queue.as_entire_binding(),
            )),
        );

        let first_meshlet_cull = render_device.create_bind_group(
            "meshlet_first_meshlet_cull_bind_group",
            &pipeline_cache
                .get_bind_group_layout(&resource_manager.first_meshlet_cull_bind_group_layout),
            &BindGroupEntries::sequential((
                &view_resources.previous_depth_pyramid,
                view_uniforms.clone(),
                previous_view_uniforms.clone(),
                meshlet_mesh_manager.meshlet_cull_data.binding(),
                instance_manager.instance_uniforms.binding().unwrap(),
                view_resources
                    .visibility_buffer_software_raster_indirect_args
                    .as_entire_binding(),
                view_resources
                    .visibility_buffer_hardware_raster_indirect_args
                    .as_entire_binding(),
                resource_manager
                    .visibility_buffer_raster_cluster_prev_counts
                    .as_entire_binding(),
                resource_manager
                    .visibility_buffer_raster_clusters
                    .as_entire_binding(),
                view_resources.front_meshlet_cull_count.as_entire_binding(),
                view_resources.back_meshlet_cull_count.as_entire_binding(),
                view_resources
                    .back_meshlet_cull_dispatch
                    .as_entire_binding(),
                view_resources.meshlet_cull_queue.as_entire_binding(),
            )),
        );

        let second_meshlet_cull = render_device.create_bind_group(
            "meshlet_second_meshlet_cull_bind_group",
            &pipeline_cache
                .get_bind_group_layout(&resource_manager.second_meshlet_cull_bind_group_layout),
            &BindGroupEntries::sequential((
                &view_resources.previous_depth_pyramid,
                view_uniforms.clone(),
                previous_view_uniforms.clone(),
                meshlet_mesh_manager.meshlet_cull_data.binding(),
                instance_manager.instance_uniforms.binding().unwrap(),
                view_resources
                    .visibility_buffer_software_raster_indirect_args
                    .as_entire_binding(),
                view_resources
                    .visibility_buffer_hardware_raster_indirect_args
                    .as_entire_binding(),
                resource_manager
                    .visibility_buffer_raster_cluster_prev_counts
                    .as_entire_binding(),
                resource_manager
                    .visibility_buffer_raster_clusters
                    .as_entire_binding(),
                view_resources.back_meshlet_cull_count.as_entire_binding(),
                view_resources.meshlet_cull_queue.as_entire_binding(),
            )),
        );

        let downsample_depth = view_resources.depth_pyramid.create_bind_group(
            &render_device,
            "meshlet_downsample_depth_bind_group",
            &pipeline_cache.get_bind_group_layout(if view_resources.not_shadow_view {
                &resource_manager.downsample_depth_bind_group_layout
            } else {
                &resource_manager.downsample_depth_shadow_view_bind_group_layout
            }),
            &view_resources.visibility_buffer.default_view,
            &resource_manager.depth_pyramid_sampler,
        );

        let visibility_buffer_raster = render_device.create_bind_group(
            "meshlet_visibility_raster_buffer_bind_group",
            &pipeline_cache.get_bind_group_layout(if view_resources.not_shadow_view {
                &resource_manager.visibility_buffer_raster_bind_group_layout
            } else {
                &resource_manager.visibility_buffer_raster_shadow_view_bind_group_layout
            }),
            &BindGroupEntries::sequential((
                resource_manager
                    .visibility_buffer_raster_clusters
                    .as_entire_binding(),
                meshlet_mesh_manager.meshlets.binding(),
                meshlet_mesh_manager.indices.binding(),
                meshlet_mesh_manager.vertex_positions.binding(),
                instance_manager.instance_uniforms.binding().unwrap(),
                resource_manager
                    .visibility_buffer_raster_cluster_prev_counts
                    .as_entire_binding(),
                resource_manager
                    .software_raster_cluster_count
                    .as_entire_binding(),
                &view_resources.visibility_buffer.default_view,
                view_uniforms.clone(),
            )),
        );

        let resolve_depth = render_device.create_bind_group(
            "meshlet_resolve_depth_bind_group",
            &pipeline_cache.get_bind_group_layout(if view_resources.not_shadow_view {
                &resource_manager.resolve_depth_bind_group_layout
            } else {
                &resource_manager.resolve_depth_shadow_view_bind_group_layout
            }),
            &BindGroupEntries::single(&view_resources.visibility_buffer.default_view),
        );

        let resolve_material_depth = view_resources.material_depth.as_ref().map(|_| {
            render_device.create_bind_group(
                "meshlet_resolve_material_depth_bind_group",
                &pipeline_cache.get_bind_group_layout(
                    &resource_manager.resolve_material_depth_bind_group_layout,
                ),
                &BindGroupEntries::sequential((
                    &view_resources.visibility_buffer.default_view,
                    resource_manager
                        .visibility_buffer_raster_clusters
                        .as_entire_binding(),
                    instance_manager.instance_material_ids.binding().unwrap(),
                )),
            )
        });

        let material_shade = view_resources.material_depth.as_ref().map(|_| {
            render_device.create_bind_group(
                "meshlet_mesh_material_shade_bind_group",
                &pipeline_cache
                    .get_bind_group_layout(&resource_manager.material_shade_bind_group_layout),
                &BindGroupEntries::sequential((
                    &view_resources.visibility_buffer.default_view,
                    resource_manager
                        .visibility_buffer_raster_clusters
                        .as_entire_binding(),
                    meshlet_mesh_manager.meshlets.binding(),
                    meshlet_mesh_manager.indices.binding(),
                    meshlet_mesh_manager.vertex_positions.binding(),
                    meshlet_mesh_manager.vertex_normals.binding(),
                    meshlet_mesh_manager.vertex_uvs.binding(),
                    instance_manager.instance_uniforms.binding().unwrap(),
                )),
            )
        });

        let remap_1d_to_2d_dispatch = resource_manager
            .remap_1d_to_2d_dispatch_bind_group_layout
            .as_ref()
            .map(|layout| {
                render_device.create_bind_group(
                    "meshlet_remap_1d_to_2d_dispatch_bind_group",
                    &pipeline_cache.get_bind_group_layout(layout),
                    &BindGroupEntries::sequential((
                        view_resources
                            .visibility_buffer_software_raster_indirect_args
                            .as_entire_binding(),
                        resource_manager
                            .software_raster_cluster_count
                            .as_entire_binding(),
                    )),
                )
            });

        let fill_counts = if resource_manager
            .remap_1d_to_2d_dispatch_bind_group_layout
            .is_some()
        {
            render_device.create_bind_group(
                "meshlet_fill_counts_bind_group",
                &pipeline_cache
                    .get_bind_group_layout(&resource_manager.fill_counts_bind_group_layout),
                &BindGroupEntries::sequential((
                    view_resources
                        .visibility_buffer_software_raster_indirect_args
                        .as_entire_binding(),
                    view_resources
                        .visibility_buffer_hardware_raster_indirect_args
                        .as_entire_binding(),
                    resource_manager
                        .visibility_buffer_raster_cluster_prev_counts
                        .as_entire_binding(),
                    resource_manager
                        .software_raster_cluster_count
                        .as_entire_binding(),
                )),
            )
        } else {
            render_device.create_bind_group(
                "meshlet_fill_counts_bind_group",
                &pipeline_cache
                    .get_bind_group_layout(&resource_manager.fill_counts_bind_group_layout),
                &BindGroupEntries::sequential((
                    view_resources
                        .visibility_buffer_software_raster_indirect_args
                        .as_entire_binding(),
                    view_resources
                        .visibility_buffer_hardware_raster_indirect_args
                        .as_entire_binding(),
                    resource_manager
                        .visibility_buffer_raster_cluster_prev_counts
                        .as_entire_binding(),
                )),
            )
        };

        let bind_groups = MeshletViewBindGroups {
            clear_visibility_buffer,
            first_instance_cull,
            second_instance_cull,
            first_bvh_cull_ping,
            first_bvh_cull_pong,
            second_bvh_cull_ping,
            second_bvh_cull_pong,
            first_meshlet_cull,
            second_meshlet_cull,
            downsample_depth,
            visibility_buffer_raster,
            resolve_depth,
            resolve_material_depth,
            material_shade,
            remap_1d_to_2d_dispatch,
            fill_counts,
        };
        resource_manager
            .view_bind_groups
            .insert(view_entity, (key, bind_groups.clone()));
        commands.entity(view_entity).insert(bind_groups);
    }
}
