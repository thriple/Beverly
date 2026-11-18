//! What the graphics card can do, found before the app starts: `GpuCaps`. Features read it instead of assuming a
//! card (CLAUDE.md, built to spec). The editor and the player pin the renderer to the adapter found here, on Vulkan
//! when its driver has everything virtual geometry needs.
use bevy::prelude::*;
use bevy::render::render_resource::WgpuFeatures;
use bevy::render::settings::{Backends, WgpuSettings};
use bevy::render::RenderPlugin;

/// What Bevy's virtual geometry (meshlets) asks of the card (bevy_pbr's meshlet/mod.rs).
pub const VIRTUAL_GEOMETRY: WgpuFeatures = WgpuFeatures::TEXTURE_INT64_ATOMIC
    .union(WgpuFeatures::TEXTURE_ATOMIC)
    .union(WgpuFeatures::SHADER_INT64)
    .union(WgpuFeatures::SUBGROUP)
    .union(WgpuFeatures::DEPTH_CLIP_CONTROL)
    .union(WgpuFeatures::IMMEDIATES);

/// The graphics card the app renders with and what it can do.
#[derive(Resource, Clone, Debug)]
pub struct GpuCaps {
    pub name: String,
    /// Vulkan, Dx12, Metal or Gl
    pub backend: String,
    /// DiscreteGpu, IntegratedGpu, VirtualGpu, Cpu or Other
    pub kind: String,
    pub features: WgpuFeatures,
}

impl GpuCaps {
    /// Bevy's virtual geometry runs: the features, on Vulkan or Metal.
    pub fn virtual_geometry(&self) -> bool {
        self.features.contains(VIRTUAL_GEOMETRY) && (self.backend == "Vulkan" || self.backend == "Metal")
    }
}

/// Probes the card (`probe`), keeps the caps as a resource and logs them; returns the render settings to add.
pub fn render_plugin(app: &mut App) -> RenderPlugin {
    let (caps, plugin) = probe();
    app.insert_resource(caps).add_systems(Startup, |caps: Res<GpuCaps>| {
        info!("Graphics card: {} ({}, {}), virtual geometry: {}", caps.name, caps.backend, caps.kind, if caps.virtual_geometry() { "yes" } else { "no" });
    });
    plugin
}

/// Finds the card (a real one before a software renderer, a separate one before a built-in one, the way Bevy's
/// high-performance choice goes), takes its Vulkan driver when that has what virtual geometry needs, and returns the
/// caps with render settings pinned to that adapter. `WGPU_BACKEND` limits the search, as it does Bevy's.
pub fn probe() -> (GpuCaps, RenderPlugin) {
    let mut settings = WgpuSettings::default();
    let backends = settings.backends.unwrap_or(Backends::all());
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends, ..wgpu::InstanceDescriptor::new_without_display_handle() });
    let rank = |t: wgpu::DeviceType| match t {
        wgpu::DeviceType::DiscreteGpu => 0,
        wgpu::DeviceType::IntegratedGpu => 1,
        wgpu::DeviceType::VirtualGpu => 2,
        wgpu::DeviceType::Other => 3,
        wgpu::DeviceType::Cpu => 4,
    };
    // enumerated Vulkan first, as wgpu itself prefers; the sort keeps that order within a rank
    let mut found: Vec<(wgpu::AdapterInfo, WgpuFeatures)> =
        bevy::tasks::block_on(instance.enumerate_adapters(backends)).iter().map(|a| (a.get_info(), a.features())).collect();
    found.sort_by_key(|(info, _)| rank(info.device_type));
    let best = found.iter().find(|(info, f)| info.backend == wgpu::Backend::Vulkan && f.contains(VIRTUAL_GEOMETRY) && found.first().is_some_and(|(b, _)| b.name == info.name));
    let caps = match best.or(found.first()) {
        Some((info, features)) => {
            settings.backends = Some(info.backend.into());
            settings.adapter_name = Some(info.name.clone());
            GpuCaps { name: info.name.clone(), backend: format!("{:?}", info.backend), kind: format!("{:?}", info.device_type), features: *features }
        }
        // none found: Bevy says so when it looks for one itself
        None => GpuCaps { name: "none found".into(), backend: String::new(), kind: String::new(), features: WgpuFeatures::empty() },
    };
    (caps, RenderPlugin { render_creation: settings.into(), ..default() })
}
