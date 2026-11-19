use super::{
    meshlet_mesh_manager::MeshletMeshManager, CpuTimer, MeshletCpuTimes, MeshletMesh, MeshletMesh3d,
};
use crate::{
    meshlet::asset::MeshletAabb, MeshFlags, MeshTransforms, MeshUniform, PreviousGlobalTransform,
    RenderMaterialInstances,
};
use bevy_asset::{AssetEvent, AssetId, AssetServer, Assets, UntypedAssetId};
use bevy_camera::visibility::RenderLayers;
use bevy_ecs::{
    entity::{Entity, EntityHashMap, EntityHashSet},
    lifecycle::RemovedComponents,
    message::MessageReader,
    query::{Added, Changed, Has, Or},
    resource::Resource,
    system::{Local, Query, Res, ResMut, SystemState},
};
use bevy_light::{NotShadowCaster, NotShadowReceiver};
use bevy_platform::collections::{HashMap, HashSet};
use bevy_render::{
    material_bind_groups::{MaterialBindGroupSlot, RenderMaterialBindings},
    render_resource::{
        encase::{internal::WriteInto, StorageBuffer as EncaseBuffer},
        BindingResource, Buffer, BufferDescriptor, BufferUsages, ShaderSize, ShaderType, StorageBuffer,
    },
    renderer::{RenderDevice, RenderQueue},
    sync_world::MainEntity,
    MainWorld,
};
use bevy_transform::components::GlobalTransform;
use core::ops::Range;


/// Tessera: material ids go into a 16-bit depth target (`resolve_render_targets.wesl`); past this many the ids are
/// handed out afresh.
const MATERIAL_ID_LIMIT: u32 = 60_000;

/// Manages data for each entity with a [`MeshletMesh`].
///
/// Tessera: kept from frame to frame and updated only where something changed (Bevy rebuilt and uploaded every
/// instance each frame). Instances are packed: removing one moves the last into its place.
#[derive(Resource)]
pub struct InstanceManager {
    /// Amount of instances in the scene.
    pub scene_instance_count: u32,
    /// The max BVH depth of any instance in the scene. This is used to control the number of
    /// dependent dispatches emitted for BVH traversal.
    pub max_bvh_depth: u32,

    /// Per-instance [`MainEntity`], [`RenderLayers`], and [`NotShadowCaster`].
    pub instances: Vec<(MainEntity, RenderLayers, bool)>,
    /// Per-instance [`MeshUniform`].
    pub instance_uniforms: GpuArray<MeshUniform>,
    /// Per-instance model-space AABB.
    pub instance_aabbs: GpuArray<MeshletAabb>,
    /// Per-instance material ID.
    pub instance_material_ids: GpuArray<u32>,
    /// Per-instance index to the root node of the instance's BVH.
    pub instance_bvh_root_nodes: GpuArray<u32>,
    /// Per-view per-instance visibility bit. Used for [`RenderLayers`] and [`NotShadowCaster`] support.
    /// Tessera: with the view layers and shadow-view flag it was built for, and kept up to date from
    /// `visibility_changes`.
    pub view_instance_visibility: EntityHashMap<(RenderLayers, bool, StorageBuffer<Vec<u32>>)>,
    /// Tessera: instances added, moved into another's place, or whose layers or shadow casting changed, since the
    /// views' visibility bits were last brought up to date.
    pub visibility_changes: Vec<u32>,

    /// Next material ID available.
    next_material_id: u32,
    /// Map of material asset to material ID.
    material_id_lookup: HashMap<UntypedAssetId, u32>,
    /// Set of material IDs used in the scene.
    material_ids_present_in_scene: HashSet<u32>,

    /// Where each entity's instance is.
    slots: EntityHashMap<u32>,
    /// Per-instance mesh, BVH depth and material asset.
    meshes: Vec<AssetId<MeshletMesh>>,
    bvh_depths: Vec<u32>,
    materials: Vec<Option<UntypedAssetId>>,
    /// Entities to look at again next frame: their mesh is still loading, or was reloaded.
    waiting: EntityHashSet,
    /// New instances whose material the queue system still has to look up.
    material_pending: Vec<MainEntity>,
    /// Per material asset in use: (instances using it, material ID and slot as last written).
    material_users: HashMap<UntypedAssetId, (u32, u32, MaterialBindGroupSlot)>,
    /// An instance with the deepest BVH was removed or changed.
    max_bvh_depth_stale: bool,
}

impl InstanceManager {
    pub fn new() -> Self {
        Self {
            scene_instance_count: 0,
            max_bvh_depth: 0,

            instances: Vec::new(),
            instance_uniforms: GpuArray::new("meshlet_instance_uniforms"),
            instance_aabbs: GpuArray::new("meshlet_instance_aabbs"),
            instance_material_ids: GpuArray::new("meshlet_instance_material_ids"),
            instance_bvh_root_nodes: GpuArray::new("meshlet_instance_bvh_root_nodes"),
            view_instance_visibility: EntityHashMap::default(),
            visibility_changes: Vec::new(),

            next_material_id: 0,
            material_id_lookup: HashMap::default(),
            material_ids_present_in_scene: HashSet::default(),

            slots: EntityHashMap::default(),
            meshes: Vec::new(),
            bvh_depths: Vec::new(),
            materials: Vec::new(),
            waiting: EntityHashSet::default(),
            material_pending: Vec::new(),
            material_users: HashMap::default(),
            max_bvh_depth_stale: false,
        }
    }

    /// Adds the entity's instance, or updates it in place.
    fn upsert_instance(
        &mut self,
        instance: Entity,
        meshlet_mesh: AssetId<MeshletMesh>,
        root_bvh_node: u32,
        aabb: MeshletAabb,
        bvh_depth: u32,
        transform: &GlobalTransform,
        previous_transform: Option<&PreviousGlobalTransform>,
        render_layers: Option<&RenderLayers>,
        not_shadow_receiver: bool,
        not_shadow_caster: bool,
    ) {
        // Build a MeshUniform for the instance
        let transform = transform.affine();
        let previous_transform = previous_transform.map(|t| t.0).unwrap_or(transform);
        let mut flags = if not_shadow_receiver {
            MeshFlags::empty()
        } else {
            MeshFlags::SHADOW_RECEIVER
        };
        if transform.matrix3.determinant().is_sign_positive() {
            flags |= MeshFlags::SIGN_DETERMINANT_MODEL_3X3;
        }
        let transforms = MeshTransforms {
            world_from_local: transform.into(),
            previous_world_from_local: previous_transform.into(),
            flags: flags.bits(),
        };
        let layers = render_layers.cloned().unwrap_or(RenderLayers::default());

        match self.slots.get(&instance) {
            Some(&index) => {
                let i = index as usize;
                // The material slot is the queue system's to write; keep it.
                let slot = self.instance_uniforms.values[i].material_and_lightmap_bind_group_slot
                    & 0xFFFF;
                self.instance_uniforms.set(
                    i,
                    MeshUniform::new(&transforms, 0, slot.into(), None, None, None, None, None),
                );
                self.instance_aabbs.set(i, aabb);
                self.instance_bvh_root_nodes.set(i, root_bvh_node);
                let (_, old_layers, old_not_shadow_caster) = &self.instances[i];
                if *old_layers != layers || *old_not_shadow_caster != not_shadow_caster {
                    self.visibility_changes.push(index);
                }
                self.instances[i] = (instance.into(), layers, not_shadow_caster);
                self.meshes[i] = meshlet_mesh;
                if bvh_depth < self.bvh_depths[i] && self.bvh_depths[i] == self.max_bvh_depth {
                    self.max_bvh_depth_stale = true;
                }
                self.bvh_depths[i] = bvh_depth;
            }
            None => {
                self.slots.insert(instance, self.instances.len() as u32);
                self.instances
                    .push((instance.into(), layers, not_shadow_caster));
                self.instance_uniforms.push(MeshUniform::new(
                    &transforms,
                    0,
                    MaterialBindGroupSlot::default(),
                    None,
                    None,
                    None,
                    None,
                    None,
                ));
                self.instance_aabbs.push(aabb);
                self.instance_material_ids.push(0);
                self.instance_bvh_root_nodes.push(root_bvh_node);
                self.meshes.push(meshlet_mesh);
                self.bvh_depths.push(bvh_depth);
                self.materials.push(None);
                self.material_pending.push(instance.into());
                self.visibility_changes.push(self.instances.len() as u32 - 1);
            }
        }
        self.max_bvh_depth = self.max_bvh_depth.max(bvh_depth);
        self.scene_instance_count = self.instances.len() as u32;
    }

    /// Removes the entity's instance, if it has one; the last instance moves into its place.
    fn remove_instance(&mut self, instance: Entity) {
        let Some(index) = self.slots.remove(&instance) else {
            return;
        };
        let i = index as usize;
        if let Some(material) = self.materials[i] {
            self.release_material(material);
        }
        if self.bvh_depths[i] == self.max_bvh_depth {
            self.max_bvh_depth_stale = true;
        }
        self.instances.swap_remove(i);
        self.instance_uniforms.swap_remove(i);
        self.instance_aabbs.swap_remove(i);
        self.instance_material_ids.swap_remove(i);
        self.instance_bvh_root_nodes.swap_remove(i);
        self.meshes.swap_remove(i);
        self.bvh_depths.swap_remove(i);
        self.materials.swap_remove(i);
        if let Some((moved, _, _)) = self.instances.get(i) {
            self.slots.insert(moved.id(), index);
            self.visibility_changes.push(index);
        }
        self.scene_instance_count = self.instances.len() as u32;
    }

    fn release_material(&mut self, material: UntypedAssetId) {
        if let Some((users, _, _)) = self.material_users.get_mut(&material) {
            *users -= 1;
            if *users == 0 {
                self.material_users.remove(&material);
            }
        }
    }

    /// Points instance `i` at `material` and writes its material ID and slot.
    fn set_material(
        &mut self,
        i: usize,
        material: Option<UntypedAssetId>,
        bindings: &RenderMaterialBindings,
    ) {
        if self.materials[i] == material {
            return;
        }
        if let Some(old) = self.materials[i] {
            self.release_material(old);
        }
        self.materials[i] = material;
        let (id, slot) = match material {
            Some(material) => {
                let id = self.material_id_lookup.get(&material).copied().unwrap_or(0);
                let slot = bindings
                    .get(&material)
                    .map(|binding| binding.slot)
                    .unwrap_or_default();
                let entry = self.material_users.entry(material).or_insert((0, id, slot));
                entry.0 += 1;
                (entry.1, entry.2)
            }
            None => (0, MaterialBindGroupSlot::default()),
        };
        self.write_material(i, id, slot);
    }

    fn write_material(&mut self, i: usize, material_id: u32, slot: MaterialBindGroupSlot) {
        self.instance_material_ids
            .set(i, if material_id == u32::MAX { 0 } else { material_id });
        let uniform = self.instance_uniforms.get_mut(i);
        uniform.material_and_lightmap_bind_group_slot =
            (uniform.material_and_lightmap_bind_group_slot & !0xFFFF) | u32::from(slot);
    }

    /// Get the material ID for a [`crate::Material`].
    pub fn get_material_id(&mut self, material_asset_id: UntypedAssetId) -> u32 {
        *self
            .material_id_lookup
            .entry(material_asset_id)
            .or_insert_with(|| {
                self.next_material_id += 1;
                self.next_material_id
            })
    }

    pub fn material_present_in_scene(&self, material_id: &u32) -> bool {
        self.material_ids_present_in_scene.contains(material_id)
    }
}

pub fn extract_meshlet_mesh_entities(
    mut meshlet_mesh_manager: ResMut<MeshletMeshManager>,
    mut instance_manager: ResMut<InstanceManager>,
    // TODO: Replace main_world and system_state when Extract<ResMut<Assets<MeshletMesh>>> is possible
    mut main_world: ResMut<MainWorld>,
    mut system_state: Local<
        Option<
            SystemState<(
                Query<
                    InstanceData,
                    Or<(
                        Changed<MeshletMesh3d>,
                        Changed<GlobalTransform>,
                        Changed<PreviousGlobalTransform>,
                        Changed<RenderLayers>,
                        Added<NotShadowReceiver>,
                        Added<NotShadowCaster>,
                    )>,
                >,
                Query<InstanceData>,
                (
                    RemovedComponents<MeshletMesh3d>,
                    RemovedComponents<RenderLayers>,
                    RemovedComponents<NotShadowReceiver>,
                    RemovedComponents<NotShadowCaster>,
                ),
                Res<AssetServer>,
                ResMut<Assets<MeshletMesh>>,
                MessageReader<AssetEvent<MeshletMesh>>,
            )>,
        >,
    >,
    mut times: ResMut<MeshletCpuTimes>,
) {
    let _timer = CpuTimer::start(&mut times.extract);
    // Get instances query
    if system_state.is_none() {
        *system_state = Some(SystemState::new(&mut main_world));
    }
    let system_state = system_state.as_mut().unwrap();
    let (
        changed_query,
        instances_query,
        (mut removed_meshes, mut removed_layers, mut removed_receivers, mut removed_casters),
        asset_server,
        mut assets,
        mut asset_events,
    ) = system_state.get_mut(&mut main_world).unwrap();
    let instance_manager = &mut *instance_manager;

    // Material IDs only grow while kept; start them afresh before they outgrow the material depth target
    if instance_manager.next_material_id > MATERIAL_ID_LIMIT {
        instance_manager.next_material_id = 0;
        instance_manager.material_id_lookup.clear();
        // u32::MAX: not yet known, so the queue system rewrites every instance's material ID
        for (_, id, _) in instance_manager.material_users.values_mut() {
            *id = u32::MAX;
        }
    }

    // Free GPU buffer space for any modified or dropped MeshletMesh assets, and look again at their instances
    let mut reloaded = HashSet::new();
    for asset_event in asset_events.read() {
        if let AssetEvent::Unused { id } | AssetEvent::Modified { id } = asset_event {
            meshlet_mesh_manager.remove(id);
            reloaded.insert(*id);
        }
    }
    if !reloaded.is_empty() {
        for (i, mesh) in instance_manager.meshes.iter().enumerate() {
            if reloaded.contains(mesh) {
                instance_manager
                    .waiting
                    .insert(instance_manager.instances[i].0.id());
            }
        }
    }

    for entity in removed_meshes.read() {
        instance_manager.remove_instance(entity);
    }
    for entity in removed_layers
        .read()
        .chain(removed_receivers.read())
        .chain(removed_casters.read())
    {
        instance_manager.waiting.insert(entity);
    }

    let mut upsert = |instance_manager: &mut InstanceManager,
                      (
        instance,
        meshlet_mesh,
        transform,
        previous_transform,
        render_layers,
        not_shadow_receiver,
        not_shadow_caster,
    ): (
        Entity,
        &MeshletMesh3d,
        &GlobalTransform,
        Option<&PreviousGlobalTransform>,
        Option<&RenderLayers>,
        bool,
        bool,
    )| {
        // Skip instances with an unloaded MeshletMesh asset, and look at them again next frame
        // TODO: This is a semi-expensive check
        if asset_server.is_managed(meshlet_mesh.id())
            && !asset_server.is_loaded_with_dependencies(meshlet_mesh.id())
        {
            instance_manager.remove_instance(instance);
            instance_manager.waiting.insert(instance);
            return;
        }

        // Upload the instance's MeshletMesh asset data if not done already done
        let (root_bvh_node, aabb, bvh_depth) =
            meshlet_mesh_manager.queue_upload_if_needed(meshlet_mesh.id(), &mut assets);

        instance_manager.upsert_instance(
            instance,
            meshlet_mesh.id(),
            root_bvh_node,
            aabb,
            bvh_depth,
            transform,
            previous_transform,
            render_layers,
            not_shadow_receiver,
            not_shadow_caster,
        );
    };

    for item in &changed_query {
        instance_manager.waiting.remove(&item.0);
        upsert(instance_manager, item);
    }
    let waiting: Vec<Entity> = instance_manager.waiting.drain().collect();
    for entity in waiting {
        if let Ok(item) = instances_query.get(entity) {
            upsert(instance_manager, item);
        } else {
            instance_manager.remove_instance(entity);
        }
    }

    if instance_manager.max_bvh_depth_stale {
        instance_manager.max_bvh_depth_stale = false;
        instance_manager.max_bvh_depth = instance_manager.bvh_depths.iter().copied().max().unwrap_or(0);
    }
}

type InstanceData = (
    Entity,
    &'static MeshletMesh3d,
    &'static GlobalTransform,
    Option<&'static PreviousGlobalTransform>,
    Option<&'static RenderLayers>,
    Has<NotShadowReceiver>,
    Has<NotShadowCaster>,
);

/// Records what material ID its material was assigned in the `prepare_material_meshlet_meshes` systems for each
/// entity whose material changed, and which material IDs are used by at least one entity in the scene.
pub fn queue_material_meshlet_meshes(
    mut instance_manager: ResMut<InstanceManager>,
    mut render_material_instances: ResMut<RenderMaterialInstances>,
    render_material_bindings: Res<RenderMaterialBindings>,
    mut times: ResMut<MeshletCpuTimes>,
) {
    let _timer = CpuTimer::start(&mut times.queue);
    let instance_manager = &mut *instance_manager;

    // Entities whose material entry changed, and new instances
    let changes = render_material_instances
        .meshlet_changes
        .replace(Vec::new())
        .unwrap_or_default();
    let pending = core::mem::take(&mut instance_manager.material_pending);
    for entity in changes.into_iter().chain(pending) {
        if let Some(&i) = instance_manager.slots.get(&entity.id()) {
            let material = render_material_instances.mesh_material(entity);
            instance_manager.set_material(i as usize, material, &render_material_bindings);
        }
    }

    // Materials whose ID or slot changed: rewrite their instances
    let mut changed: HashMap<UntypedAssetId, (u32, MaterialBindGroupSlot)> = HashMap::default();
    for (material, (_, id, slot)) in instance_manager.material_users.iter_mut() {
        let new_id = instance_manager
            .material_id_lookup
            .get(material)
            .copied()
            .unwrap_or(0);
        let new_slot = render_material_bindings
            .get(material)
            .map(|binding| binding.slot)
            .unwrap_or_default();
        if (*id, *slot) != (new_id, new_slot) {
            (*id, *slot) = (new_id, new_slot);
            changed.insert(*material, (new_id, new_slot));
        }
    }
    if !changed.is_empty() {
        for i in 0..instance_manager.materials.len() {
            if let Some(&(id, slot)) = instance_manager.materials[i]
                .as_ref()
                .and_then(|material| changed.get(material))
            {
                instance_manager.write_material(i, id, slot);
            }
        }
    }

    instance_manager.material_ids_present_in_scene.clear();
    instance_manager.material_ids_present_in_scene.extend(
        instance_manager
            .material_users
            .values()
            .filter(|(_, id, _)| *id != 0)
            .map(|(_, id, _)| *id),
    );
}

/// Tessera: a storage buffer whose values are kept on the CPU and of which only the changed range is uploaded.
pub struct GpuArray<T> {
    values: Vec<T>,
    buffer: Option<Buffer>,
    capacity: usize,
    /// Indices changed since the last upload (may repeat, may be past the end after removals).
    dirty: Vec<u32>,
    /// Everything is uploaded next time.
    all_dirty: bool,
    label: &'static str,
}

/// Changed elements closer than this are uploaded as one write: fewer, larger writes.
const GPU_ARRAY_MERGE_GAP: u32 = 64;

impl<T: ShaderType + ShaderSize + WriteInto + Clone> GpuArray<T> {
    fn new(label: &'static str) -> Self {
        Self {
            values: Vec::new(),
            buffer: None,
            capacity: 0,
            dirty: Vec::new(),
            all_dirty: false,
            label,
        }
    }

    fn mark(&mut self, i: usize) {
        if !self.all_dirty {
            self.dirty.push(i as u32);
            // past a quarter of the elements, one write of everything is cheaper
            if self.dirty.len() > self.values.len() / 4 + 64 {
                self.all_dirty = true;
                self.dirty.clear();
            }
        }
    }

    fn push(&mut self, value: T) {
        self.values.push(value);
        self.mark(self.values.len() - 1);
    }

    fn set(&mut self, i: usize, value: T) {
        self.values[i] = value;
        self.mark(i);
    }

    fn get_mut(&mut self, i: usize) -> &mut T {
        self.mark(i);
        &mut self.values[i]
    }

    fn swap_remove(&mut self, i: usize) {
        self.values.swap_remove(i);
        if i < self.values.len() {
            self.mark(i);
        }
    }

    /// Uploads what changed; returns true when the buffer was made anew (so bind groups using it are stale).
    pub fn upload(&mut self, device: &RenderDevice, queue: &RenderQueue) -> bool {
        let mut remade = false;
        if self.buffer.is_none() || self.values.len() > self.capacity {
            self.capacity = self.values.len().next_power_of_two().max(64);
            self.buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some(self.label),
                size: self.capacity as u64 * T::min_size().get(),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.all_dirty = true;
            remade = true;
        }
        let Some(buffer) = &self.buffer else {
            return remade;
        };
        let len = self.values.len() as u32;
        let mut runs: Vec<Range<u32>> = Vec::new();
        if self.all_dirty {
            runs.push(0..len);
        } else {
            self.dirty.sort_unstable();
            for &i in self.dirty.iter().filter(|&&i| i < len) {
                match runs.last_mut() {
                    Some(run) if i <= run.end + GPU_ARRAY_MERGE_GAP => run.end = run.end.max(i + 1),
                    _ => runs.push(i..i + 1),
                }
            }
        }
        self.all_dirty = false;
        self.dirty.clear();
        for run in runs.into_iter().filter(|run| !run.is_empty()) {
            let mut bytes = EncaseBuffer::new(Vec::new());
            bytes
                .write(&self.values[run.start as usize..run.end as usize].to_vec())
                .unwrap();
            queue.write_buffer(
                buffer,
                u64::from(run.start) * T::min_size().get(),
                bytes.as_ref(),
            );
        }
        remade
    }

    pub fn buffer(&self) -> Option<&Buffer> {
        self.buffer.as_ref()
    }

    pub fn binding(&self) -> Option<BindingResource<'_>> {
        self.buffer.as_ref().map(|buffer| buffer.as_entire_binding())
    }
}
