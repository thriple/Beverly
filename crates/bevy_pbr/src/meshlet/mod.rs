//! Render high-poly 3d meshes using an efficient GPU-driven method. See [`MeshletPlugin`] and [`MeshletMesh`] for details.

mod asset;
#[cfg(feature = "meshlet_processor")]
mod from_mesh;
mod instance_manager;
mod material_pipeline_prepare;
mod material_shade_nodes;
mod meshlet_mesh_manager;
mod persistent_buffer;
mod persistent_buffer_impls;
mod pipelines;
mod resource_manager;
mod visibility_buffer_raster_node;

pub(crate) use self::{
    instance_manager::{queue_material_meshlet_meshes, InstanceManager},
    material_pipeline_prepare::{
        prepare_material_meshlet_meshes_main_opaque_pass, prepare_material_meshlet_meshes_prepass,
    },
};

pub use self::asset::{
    MeshletMesh, MeshletMeshLoader, MeshletMeshSaveOrLoadError, MeshletMeshSaver,
    MESHLET_MESH_ASSET_VERSION,
};
#[cfg(feature = "meshlet_processor")]
pub use self::from_mesh::{
    MeshToMeshletMeshConversionError, MESHLET_DEFAULT_VERTEX_POSITION_QUANTIZATION_FACTOR,
};
use self::{
    instance_manager::extract_meshlet_mesh_entities,
    material_pipeline_prepare::{
        MeshletViewMaterialsDeferredGBufferPrepass, MeshletViewMaterialsMainOpaquePass,
        MeshletViewMaterialsPrepass,
    },
    material_shade_nodes::{
        meshlet_deferred_gbuffer_prepass, meshlet_main_opaque_pass, meshlet_prepass,
    },
    meshlet_mesh_manager::perform_pending_meshlet_mesh_writes,
    pipelines::*,
    resource_manager::{
        prepare_meshlet_per_frame_resources, prepare_meshlet_view_bind_groups, MeshletViewBindGroups,
        MeshletViewResources, ResourceManager,
    },
    visibility_buffer_raster_node::meshlet_visibility_buffer_raster,
};
use crate::render::{per_view_shadow_pass, EARLY_SHADOW_PASS};
use crate::{meshlet::meshlet_mesh_manager::init_meshlet_mesh_manager, PreviousGlobalTransform};
use bevy_app::{App, Plugin};
use bevy_asset::{embedded_asset, AssetApp, AssetId, Handle};
use bevy_camera::visibility::{self, Visibility, VisibilityClass};
use bevy_core_pipeline::{
    core_3d::main_opaque_pass_3d,
    prepass::{DeferredPrepass, MotionVectorPrepass, NormalPrepass},
    schedule::{Core3d, Core3dSystems},
};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::{
    component::Component,
    entity::{Entity, EntityHashSet},
    query::Has,
    reflect::ReflectComponent,
    schedule::IntoScheduleConfigs,
    system::{Commands, Local, Query, Res, ResMut},
    template::FromTemplate,
};
use bevy_reflect::{std_traits::ReflectDefault, Reflect};
use bevy_render::{
    renderer::RenderDevice,
    settings::WgpuFeatures,
    view::{prepare_view_targets, Msaa},
    ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems,
};
use bevy_shader::load_shader_library;
use bevy_transform::components::Transform;
use derive_more::From;
use tracing::{error, info};

/// Provides a plugin for rendering large amounts of high-poly 3d meshes using an efficient GPU-driven method. See also [`MeshletMesh`].
///
/// Rendering dense scenes made of high-poly meshes with thousands or millions of triangles is extremely expensive in Bevy's standard renderer.
/// Once meshes are pre-processed into a [`MeshletMesh`], this plugin can render these kinds of scenes very efficiently.
///
/// In comparison to Bevy's standard renderer:
/// * Much more efficient culling. Meshlets can be culled individually, instead of all or nothing culling for entire meshes at a time.
///   Additionally, occlusion culling can eliminate meshlets that would cause overdraw.
/// * Much more efficient batching. All geometry can be rasterized in a single draw.
/// * Scales better with large amounts of dense geometry and overdraw. Bevy's standard renderer will bottleneck sooner.
/// * Near-seamless level of detail (LOD).
/// * Much greater base overhead. Rendering will be slower and use more memory than Bevy's standard renderer
///   with small amounts of geometry and overdraw.
/// * Requires preprocessing meshes. See [`MeshletMesh`] for details.
/// * Limitations on the kinds of materials you can use. See [`MeshletMesh`] for details.
///
/// This plugin requires a fairly recent GPU that supports [`WgpuFeatures::TEXTURE_INT64_ATOMIC`].
///
/// This plugin currently works only on the Vulkan and Metal backends.
///
/// This plugin is not compatible with [`Msaa`]. Any camera rendering a [`MeshletMesh`] must have
/// [`Msaa`] set to [`Msaa::Off`].
///
/// Mixing forward+prepass and deferred rendering for opaque materials is not currently supported when using this plugin.
/// You must use one or the other by setting [`crate::DefaultOpaqueRendererMethod`].
/// Do not override [`crate::Material::opaque_render_method`] for any material when using this plugin.
///
/// ![A render of the Stanford dragon as a `MeshletMesh`](https://raw.githubusercontent.com/bevyengine/bevy/main/crates/bevy_pbr/src/meshlet/meshlet_preview.png)
pub struct MeshletPlugin {
    /// The maximum amount of clusters that can be processed at once,
    /// used to control the size of a pre-allocated GPU buffer.
    ///
    /// If this number is too low, you'll see rendering artifacts like missing or blinking meshes.
    ///
    /// Each cluster slot costs 32 bytes of VRAM: four queues (the raster clusters, the two BVH traversal queues and
    /// the cluster cull queue) of an (instance, index) pair of `u32`s each (Tessera; Bevy said 4).
    ///
    /// Must not be greater than 2^25.
    pub cluster_buffer_slots: u32,
    /// Tessera: whether the graphics card can draw virtual geometry, as the app found out before it started (Beverly:
    /// `GpuCaps::virtual_geometry`). When false, or when the render device turns out to lack
    /// [`MeshletPlugin::required_wgpu_features`], [`MeshletMesh`] assets and entities still load but nothing is drawn,
    /// and the app keeps running (Bevy exited). [`MeshletAvailable`] says which it is.
    pub supported: bool,
}

impl MeshletPlugin {
    /// [`WgpuFeatures`] required for this plugin to function.
    pub fn required_wgpu_features() -> WgpuFeatures {
        WgpuFeatures::TEXTURE_INT64_ATOMIC
            | WgpuFeatures::TEXTURE_ATOMIC
            | WgpuFeatures::SHADER_INT64
            | WgpuFeatures::SUBGROUP
            | WgpuFeatures::DEPTH_CLIP_CONTROL
            | WgpuFeatures::IMMEDIATES
    }
}

impl Plugin for MeshletPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(target_endian = "big")]
        compile_error!("MeshletPlugin is only supported on little-endian processors.");

        // Tessera: clamped instead of exiting
        let cluster_buffer_slots = if self.cluster_buffer_slots > 2_u32.pow(25) {
            error!("MeshletPlugin::cluster_buffer_slots must not be greater than 2^25; using 2^25.");
            2_u32.pow(25)
        } else {
            self.cluster_buffer_slots
        };

        load_shader_library!(app, "bindings.wesl");
        load_shader_library!(app, "visibility_buffer_resolve.wesl");
        load_shader_library!(app, "cull_shared.wesl");
        embedded_asset!(app, "clear_visibility_buffer.wesl");
        embedded_asset!(app, "cull_instances.wesl");
        embedded_asset!(app, "cull_bvh.wesl");
        embedded_asset!(app, "cull_clusters.wesl");
        embedded_asset!(app, "visibility_buffer_software_raster.wesl");
        embedded_asset!(app, "visibility_buffer_hardware_raster.wesl");
        embedded_asset!(app, "meshlet_mesh_material.wesl");
        embedded_asset!(app, "resolve_render_targets.wesl");
        embedded_asset!(app, "remap_1d_to_2d_dispatch.wesl");
        embedded_asset!(app, "fill_counts.wesl");

        app.init_asset::<MeshletMesh>()
            .register_asset_loader(MeshletMeshLoader);

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        // Tessera: on a card that can't draw virtual geometry, nothing on the render side is added
        render_app.insert_resource(MeshletAvailable(false));
        if !self.supported {
            info!("MeshletPlugin: the graphics card can't draw virtual geometry; meshlet meshes won't be drawn.");
            return;
        }

        let init_resource_manager_system =
            move |mut commands: Commands, render_device: Res<RenderDevice>| {
                commands
                    .insert_resource(ResourceManager::new(cluster_buffer_slots, &render_device));
            };

        render_app
            .insert_resource(InstanceManager::new())
            .init_resource::<MeshletCpuTimes>()
            .add_systems(
                RenderStartup,
                (
                    check_meshlet_features,
                    (
                        (init_resource_manager_system, init_meshlet_pipelines).chain(),
                        init_meshlet_mesh_manager,
                    )
                        .run_if(meshlet_available),
                )
                    .chain(),
            )
            .add_systems(
                ExtractSchedule,
                extract_meshlet_mesh_entities.run_if(meshlet_available),
            )
            .add_systems(
                Render,
                (
                    perform_pending_meshlet_mesh_writes.in_set(RenderSystems::PrepareAssets),
                    configure_meshlet_views
                        .after(prepare_view_targets)
                        .in_set(RenderSystems::PrepareViews),
                    prepare_meshlet_per_frame_resources.in_set(RenderSystems::PrepareResources),
                    prepare_meshlet_view_bind_groups.in_set(RenderSystems::PrepareBindGroups),
                    queue_material_meshlet_meshes.in_set(RenderSystems::QueueMeshes),
                    prepare_material_meshlet_meshes_main_opaque_pass
                        .in_set(RenderSystems::QueueMeshes)
                        .before(queue_material_meshlet_meshes),
                )
                    .run_if(meshlet_available),
            )
            .add_systems(
                Core3d,
                (
                    meshlet_visibility_buffer_raster
                        .before(per_view_shadow_pass::<EARLY_SHADOW_PASS>),
                    meshlet_prepass
                        .after(per_view_shadow_pass::<EARLY_SHADOW_PASS>)
                        .in_set(Core3dSystems::Prepass),
                    meshlet_deferred_gbuffer_prepass
                        .after(meshlet_prepass)
                        .in_set(Core3dSystems::Prepass),
                    meshlet_main_opaque_pass
                        .before(main_opaque_pass_3d)
                        .in_set(Core3dSystems::MainPass),
                )
                    .run_if(meshlet_available),
            );
    }
}

/// Tessera: the CPU time the meshlet systems took in the last frame, in the render world (for Beverly's performance
/// meter and the instance benchmark).
#[derive(bevy_ecs::resource::Resource, Clone, Debug, Default)]
pub struct MeshletCpuTimes {
    /// `extract_meshlet_mesh_entities`
    pub extract: core::time::Duration,
    /// `queue_material_meshlet_meshes`
    pub queue: core::time::Duration,
    /// `prepare_meshlet_per_frame_resources`
    pub prepare_resources: core::time::Duration,
    /// `prepare_meshlet_view_bind_groups`
    pub prepare_bind_groups: core::time::Duration,
}

/// Writes the time since it was made into its slot when dropped, so early returns are timed too.
pub(crate) struct CpuTimer<'a>(&'a mut core::time::Duration, bevy_platform::time::Instant);

impl<'a> CpuTimer<'a> {
    pub(crate) fn start(slot: &'a mut core::time::Duration) -> Self {
        Self(slot, bevy_platform::time::Instant::now())
    }
}

impl Drop for CpuTimer<'_> {
    fn drop(&mut self) {
        *self.0 = self.1.elapsed();
    }
}

/// Tessera: whether meshlet meshes are being drawn (render world): the card was said to support virtual geometry
/// ([`MeshletPlugin::supported`]) and the render device has the features.
#[derive(bevy_ecs::resource::Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeshletAvailable(pub bool);

/// Tessera: run condition for the meshlet render systems (false without the plugin, too).
pub(crate) fn meshlet_available(available: Option<Res<MeshletAvailable>>) -> bool {
    available.is_some_and(|available| available.0)
}

/// Tessera: turns the meshlet systems on when the render device has the features; logs and leaves them off
/// (Bevy exited) when it doesn't.
fn check_meshlet_features(
    render_device: Res<RenderDevice>,
    mut available: ResMut<MeshletAvailable>,
) {
    let features = render_device.features();
    available.0 = features.contains(MeshletPlugin::required_wgpu_features());
    if !available.0 {
        error!(
            "MeshletPlugin can't be used. GPU lacks support for required features: {:?}. Meshlet meshes won't be drawn.",
            MeshletPlugin::required_wgpu_features().difference(features)
        );
    }
}

/// The meshlet mesh equivalent of [`bevy_mesh::Mesh3d`].
#[derive(
    Component, FromTemplate, Clone, Debug, Default, Deref, DerefMut, Reflect, PartialEq, Eq, From,
)]
#[reflect(Component, Default, Clone, PartialEq)]
#[require(Transform, PreviousGlobalTransform, Visibility, VisibilityClass)]
#[component(on_add = visibility::add_visibility_class::<MeshletMesh3d>)]
pub struct MeshletMesh3d(pub Handle<MeshletMesh>);

impl From<MeshletMesh3d> for AssetId<MeshletMesh> {
    fn from(mesh: MeshletMesh3d) -> Self {
        mesh.id()
    }
}

impl From<&MeshletMesh3d> for AssetId<MeshletMesh> {
    fn from(mesh: &MeshletMesh3d) -> Self {
        mesh.id()
    }
}

fn configure_meshlet_views(
    mut views_3d: Query<(
        Entity,
        &Msaa,
        Has<NormalPrepass>,
        Has<MotionVectorPrepass>,
        Has<DeferredPrepass>,
    )>,
    mut commands: Commands,
    instance_manager: Res<InstanceManager>,
    mut warned: Local<EntityHashSet>,
) {
    warned.retain(|entity| views_3d.contains(*entity));
    for (entity, msaa, normal_prepass, motion_vector_prepass, deferred_prepass) in &mut views_3d {
        // Tessera: a view with MSAA draws no meshlet meshes (Bevy exited); said once per view, and only when there
        // are meshlet meshes to draw
        if *msaa != Msaa::Off {
            if instance_manager.scene_instance_count > 0 && warned.insert(entity) {
                error!("MeshletPlugin can't be used with MSAA. Add Msaa::Off to your camera to draw meshlet meshes in it.");
            }
            commands.entity(entity).remove::<(
                MeshletViewMaterialsMainOpaquePass,
                MeshletViewMaterialsPrepass,
                MeshletViewMaterialsDeferredGBufferPrepass,
                MeshletViewResources,
                MeshletViewBindGroups,
            )>();
            continue;
        }

        if !(normal_prepass || motion_vector_prepass || deferred_prepass) {
            commands
                .entity(entity)
                .insert(MeshletViewMaterialsMainOpaquePass::default());
        } else {
            // TODO: Should we add both Prepass and DeferredGBufferPrepass materials here, and in other systems/nodes?
            commands.entity(entity).insert((
                MeshletViewMaterialsMainOpaquePass::default(),
                MeshletViewMaterialsPrepass::default(),
                MeshletViewMaterialsDeferredGBufferPrepass::default(),
            ));
        }
    }
}
