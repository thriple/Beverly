//! The level's foliage: how far and how finely the grass carpet grows, and the grass species the level changed or added
//! (the built-in ones are in `grass/defaults.rs`; a species here with the same name replaces it).
use super::*;
use crate::grass::GrassSpecies;

/// The Foliage topic's carpet section.
pub const CARPET: &str = "CARPET";

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Foliage {
    #[reflect(@Setting::feel("Reach", &["Near", "Far", "Very far"], 10.0, 120.0, "m").log().section(CARPET).help("How far from the camera the grass grows"))]
    pub reach: f32,
    /// Tiles of blades per side of the carpet around the camera (the carpet's cost): 8 for a light card, 10 for a desktop.
    #[reflect(@Setting::feel("Quality", &["Low", "Medium", "High"], 6.0, 14.0, "").section(CARPET).help("How finely the carpet is laid: more is prettier and costs the card more"))]
    pub quality: f32,
    /// The species this level changed or added.
    #[reflect(ignore)]
    pub species: Vec<GrassSpecies>,
}

impl Default for Foliage {
    fn default() -> Self { let s = crate::grass::GrassSettings::default(); Foliage { reach: s.radius, quality: s.tiles as f32, species: vec![] } }
}

impl Foliage {
    pub fn is_default(&self) -> bool { *self == Foliage::default() }
    /// Keep a species in the level when it differs from the built-in one of its name (or is new), else drop the level's own.
    pub fn keep(&mut self, s: GrassSpecies) {
        let built_in = crate::grass::GrassSpeciesRegistry::default().species.into_iter().find(|b| b.name == s.name);
        self.species.retain(|o| o.name != s.name);
        if built_in.as_ref() != Some(&s) { self.species.push(s); }
    }
    /// The carpet's lattice, in tiles per side.
    pub fn tiles(&self) -> u32 { self.quality.round().clamp(6.0, 14.0) as u32 }
}
