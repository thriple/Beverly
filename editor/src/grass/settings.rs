//! Look + cost knobs. Defaults are the original game's values.
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// sRGB 0..1, written as `srgb_hex(0x41786e)`.
pub type Srgb = [f32; 3];
pub const fn srgb_hex(h: u32) -> Srgb {
    [((h >> 16) & 255) as f32 / 255.0, ((h >> 8) & 255) as f32 / 255.0, (h & 255) as f32 / 255.0]
}

/// Blades per tile side. One tile = one instanced batch entity; do not change.
pub const TILE: u32 = 64;

#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct GrassSettings {
    // ---- the sphere (cost knobs) ----
    /// Metres from the focus out to where the carpet ends.
    pub radius: f32,
    /// Fraction of the radius over which blades thin out to `min_scale`.
    pub edge: f32,
    /// Width scale a blade keeps at the rim.
    pub min_scale: f32,
    /// Lattice size in tiles per side (`tiles * 64` cells). THE cost knob: 8 ~ mobile, 10 ~ desktop.
    pub tiles: u32,
    /// Target blades per m2 scale. Divides out lattice size and radius, so changing `radius` or `tiles`
    /// keeps the look the same until the lattice caps (see `GrassStats`).
    pub density: f32,
    /// Lattice size/radius `density` was tuned at (the original: 384 cells at 80 m).
    pub tuned_at: f32,
    pub tuned_radius: f32,
    // ---- blade ----
    pub height: f32,
    pub height_var: f32,
    pub width: f32,
    pub min_width: f32,
    pub bend: f32,
    pub jitter: f32,
    pub clump: f32,
    pub clump_size: f32,
    // ---- colour ----
    /// 0 = fully Lambert lit, 1 = fully flat (unlit, `GrassLighting::flat_scale`).
    pub flat: f32,
    /// Metres per octave of the colour patch noise.
    pub patch: f32,
    /// How much of `ground_color` bleeds into the blade.
    pub ground: f32,
    pub tip: f32,
    pub dark: Srgb,
    pub mid: Srgb,
    pub light: Srgb,
    /// Unused here: each blade's root takes the colour of the surface under it.
    pub ground_color: Srgb,
    // ---- wind ----
    pub sway: f32,
    pub gust: f32,
    // ---- heads / pads ----
    pub head_radius: f32,
    pub head_var: f32,
    pub head_var_hi: f32,
    pub plume_at: f32,
    pub pad_lift: f32,
    // ---- flat-orient (ivy) shading ----
    pub ivy_normal_bias: f32,
    pub ivy_direct_mul: f32,
    pub ivy_ground_tint: f32,
    // ---- reed gradient atlas ----
    pub gradient_amount: f32,
    pub gradient_alt: f32,
    pub gradient_lo: f32,
    pub gradient_hi: f32,
    pub gradient_pow: f32,
}

impl Default for GrassSettings {
    fn default() -> Self {
        Self {
            // the original's look scaled to a person's size (its grass was 1.6 m): half the reach at twice the
            // lattice's fineness, and four times the blades per square metre
            radius: 40.0, edge: 0.36, min_scale: 0.05, tiles: 10, density: 10.0,
            tuned_at: 384.0, tuned_radius: 80.0,
            height: 0.5, height_var: 0.59, width: 0.045, min_width: 0.018, bend: 0.16,
            jitter: 0.7, clump: 0.0, clump_size: 1.3,
            flat: 0.72, patch: 6.0, ground: 0.25, tip: 0.40,
            dark: srgb_hex(0x41786e), mid: srgb_hex(0x79a857), light: srgb_hex(0xc6d47c),
            ground_color: srgb_hex(0xA0B364),
            sway: 1.0, gust: 1.0,
            head_radius: 0.25, head_var: 0.12, head_var_hi: 0.25, plume_at: 0.72, pad_lift: 0.02,
            ivy_normal_bias: 0.7, ivy_direct_mul: 0.4, ivy_ground_tint: 0.35,
            gradient_amount: 1.0, gradient_alt: 0.28, gradient_lo: 0.0, gradient_hi: 1.0, gradient_pow: 1.0,
        }
    }
}

impl GrassSettings {
    pub fn lattice(&self) -> u32 { self.tiles * TILE }
    /// Metres between lattice cells.
    pub fn spacing(&self) -> f32 { self.radius * 2.0 / self.lattice() as f32 }
    /// Fraction of lattice cells kept alive before painted density is applied.
    pub fn keep(&self) -> f32 {
        (self.density * (self.tuned_at / self.lattice() as f32).powi(2)
            * (self.radius / self.tuned_radius).powi(2)).min(1.0)
    }
}

/// Wind. Same mapping as the original: `strength = sway * min(1, 0.22 + speed * 0.052) * 0.08`.
#[derive(Resource, Clone, Debug)]
pub struct GrassWind {
    pub speed: f32,
    /// Radians in the XZ plane, `(cos, sin)` = (x, z).
    pub direction: f32,
}
impl Default for GrassWind {
    fn default() -> Self { Self { speed: 6.0, direction: 0.6 } }
}

/// Lighting for the carpet. The original's time of day drove `flat_scale` (its `grassLit`);
/// drive it from yours, or leave at 1.
#[derive(Resource, Clone, Debug)]
pub struct GrassLighting {
    /// Copy direction + colour from the first `DirectionalLight` each frame.
    pub sync_directional_light: bool,
    /// Unit vector pointing TOWARD the sun.
    pub sun_dir: Vec3,
    pub sun_color: Vec3,
    pub ambient: Vec3,
    /// Multiplier on the flat (unlit) share. Lower it at night.
    pub flat_scale: Vec3,
}
impl Default for GrassLighting {
    fn default() -> Self {
        Self {
            sync_directional_light: true,
            sun_dir: Vec3::new(0.4, 0.8, 0.3).normalize(),
            sun_color: Vec3::splat(0.9),
            ambient: Vec3::splat(0.35),
            flat_scale: Vec3::ONE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lattice_numbers() {
        let s = GrassSettings::default();
        assert_eq!(s.lattice(), 640);
        assert!((s.spacing() - 0.125).abs() < 1e-6);                         // 80 m / 640
        assert!((s.keep() - 10.0 * (384.0f32 / 640.0).powi(2) * 0.25).abs() < 1e-6);
        let doubled = GrassSettings { radius: 20.0, ..s.clone() };   // (above 42 m the lattice caps at keep = 1)
        assert!(((doubled.keep() / doubled.spacing().powi(2)) - (s.keep() / s.spacing().powi(2))).abs() < 1e-3,
            "blades per m2 stays constant when the radius changes");
    }
}
