//! Grass "types". Data, not code: add, remove or edit entries at runtime and the shader follows.
//! Materials name species by `name`; the shader gets them by index in this list.
use bevy::prelude::*;
use bevy::render::render_resource::ShaderType;
use serde::{Deserialize, Serialize};

pub const MAX_SPECIES: usize = 32;

/// Which shape routine a species uses (the original's classes S / P / F / L).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeciesClass {
    /// Plain tapering blade (grass, reed, rush, ivy).
    Blade,
    /// Blade that swells into a plume head (pampas, poa).
    Plume,
    /// Stem with a flower head; head colour = `tint_tip`.
    Flower,
    /// Flat disc lying on the surface (lily pad).
    Pad,
}

/// Flower head profile, only read by [`SpeciesClass::Flower`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeadKind {
    Disc,
    Sphere,
    Spike,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orient {
    /// Stands up from the ground.
    Upright,
    /// Lies in the terrain tangent plane and reaches across it (ivy).
    Flat,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrassSpecies {
    pub name: String,
    /// Multipliers on the global settings (height / width / ...).
    pub h_scale: f32,
    pub w_scale: f32,
    pub taper: f32,
    pub bend: f32,
    pub sway: f32,
    /// Flower/plume/pad head radius, as a fraction of `GrassSettings::head_radius`.
    pub head_radius: f32,
    pub class: SpeciesClass,
    pub head_kind: HeadKind,
    pub orient: Orient,
    /// 0 = none, 1 or 2 = row of the gradient atlas (reed uses 1; `gradient_alt` flips 1 -> 2 per blade).
    pub gradient: u8,
    /// LINEAR multipliers on the patch colour at the blade root / tip.
    pub tint_base: [f32; 3],
    pub tint_tip: [f32; 3],
    /// LINEAR colour blended in by `abs_amount` (stronger toward the tip). Used for straw/dry looks.
    pub abs_color: [f32; 3],
    pub abs_amount: f32,
}

/// The species list. `index == id` written into the paint map.
#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct GrassSpeciesRegistry {
    pub species: Vec<GrassSpecies>,
}

impl Default for GrassSpeciesRegistry {
    /// The original's 17 species, ids 0..=16, numbers carried over unchanged.
    fn default() -> Self {
        Self { species: super::defaults::default_species() }
    }
}

impl GrassSpeciesRegistry {
    pub fn id_of(&self, name: &str) -> Option<u8> {
        self.species.iter().position(|s| s.name == name).map(|i| i as u8)
    }
}

#[derive(ShaderType, Clone, Copy, Default, PartialEq)]
pub(crate) struct GpuSpecies {
    shape: Vec4,
    shape2: Vec4,
    flags: Vec4,
    tint_a: Vec4,
    tint_b: Vec4,
    abs_c: Vec4,
}

#[derive(ShaderType, Clone, Copy, PartialEq)]
pub(crate) struct SpeciesTable {
    items: [GpuSpecies; MAX_SPECIES],
}

impl Default for SpeciesTable {
    fn default() -> Self {
        Self { items: [GpuSpecies::default(); MAX_SPECIES] }
    }
}

impl GrassSpeciesRegistry {
    pub(crate) fn pack(&self) -> SpeciesTable {
        let mut t = SpeciesTable::default();
        for (g, s) in t.items.iter_mut().zip(self.species.iter().take(MAX_SPECIES)) {
            let v3 = |c: [f32; 3]| Vec4::new(c[0], c[1], c[2], 0.0);
            *g = GpuSpecies {
                shape: Vec4::new(s.h_scale, s.w_scale, s.taper, s.bend),
                shape2: Vec4::new(
                    s.sway,
                    s.head_radius,
                    s.class as u8 as f32,
                    s.head_kind as u8 as f32,
                ),
                flags: Vec4::new(
                    if s.orient == Orient::Flat { 1.0 } else { 0.0 },
                    s.gradient as f32,
                    s.abs_amount,
                    0.0,
                ),
                tint_a: v3(s.tint_base),
                tint_b: v3(s.tint_tip),
                abs_c: v3(s.abs_color),
            };
        }
        t
    }
}
