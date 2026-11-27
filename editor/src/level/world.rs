//! The level's own settings: its lighting and sun, and its navigation.
use super::*;

/// The level panel's sections (shown when nothing is selected).
pub const LIGHTING: &str = "LIGHTING";
pub const SUN: &str = "SUN";

/// The level's light: the sun, or Interior (no sun), and the fill light everywhere.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Lighting {
    #[reflect(@Setting::toggle("Interior").section(LIGHTING).help("No sun: only the fill light and your own lights"))]
    pub interior: bool,
    /// The flat light from every side (sky light outdoors).
    #[reflect(@Setting::feel("Fill", &["Dark", "Dim", "Soft", "Bright"], 10.0, 5000.0, "").log().section(LIGHTING).help("Flat light from every side, so shade is never black"))]
    pub fill: f32,
    /// sRGB, 0..1.
    #[reflect(@Setting::colour("Fill colour").section(LIGHTING))]
    pub fill_colour: [f32; 3],
    /// The compass direction the sunlight comes from, in degrees.
    #[reflect(@Setting::number("Direction", "°", 1.0, 0.0, 360.0).section(SUN).help("Which way the sun is, round the level"))]
    pub direction: f32,
    /// How high the sun is above the horizon, in degrees.
    #[reflect(@Setting::feel("Height", &["Horizon", "Low", "High", "Overhead"], 2.0, 90.0, "°").section(SUN).help("Low: long shadows"))]
    pub height: f32,
    /// sRGB, 0..1.
    #[reflect(@Setting::colour("Colour").section(SUN))]
    pub colour: [f32; 3],
    /// Lux.
    #[reflect(@Setting::feel("Brightness", &["Dusk", "Overcast", "Hazy", "Full sun"], 500.0, 100000.0, "lx").log().section(SUN))]
    pub brightness: f32,
    #[reflect(@Setting::toggle("Shadows").section(SUN))]
    pub shadows: bool,
}

impl Default for Lighting {
    fn default() -> Self {
        Lighting { interior: false, fill: 500.0, fill_colour: [0.85, 0.88, 1.0], direction: 40.0, height: 54.0, colour: [1.0, 1.0, 1.0], brightness: 9000.0, shadows: true }
    }
}

pub const NAVIGATION: &str = "NAVIGATION";

/// The level's walker profile ([`Level::profile`]): the size of the characters that walk it (the walker you play, and
/// those that walk on their own along the navigation area) and what they climb.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Navigation {
    #[reflect(@Setting::number("Width", "m", 0.05, 0.1, 10.0).section(NAVIGATION).help("How wide the characters are, the walker you play too: they keep this far from walls"))]
    pub width: f32,
    #[reflect(@Setting::number("Height", "m", 0.05, 0.2, 20.0).section(NAVIGATION).help("How tall they are: they don't go where it is lower"))]
    pub height: f32,
    #[reflect(@Setting::number("Step", "m", 0.05, 0.0, 5.0).section(NAVIGATION).help("The highest step they walk up (and the pen's Ground lines climb, and Bucket's Floor reaches)"))]
    pub step: f32,
    #[reflect(@Setting::number("Slope", "°", 1.0, 0.0, 89.0).section(NAVIGATION).help("The steepest slope they walk up (and Bucket's Floor paints); steeper, the walker slides"))]
    pub slope: f32,
}

impl Default for Navigation {
    fn default() -> Self { Navigation { width: 0.8, height: 1.8, step: 0.45, slope: 45.0 } }
}

impl Navigation {
    pub(super) fn is_default(&self) -> bool { *self == Navigation::default() }
    /// The least a surface's normal points up and still counts as walkable.
    pub fn up(&self) -> f32 { self.slope.clamp(0.0, 89.0).to_radians().cos() }
    /// How high a line laid on the ground climbs onto the next surface: a step, and touching over it so a step of
    /// exactly that is met.
    pub fn climb(&self, precision: Precision) -> f32 { self.step + precision.touching() }
}

impl Lighting {
    pub(super) fn is_default(&self) -> bool { *self == Lighting::default() }
    /// Which way the sunlight shines (down from the sun).
    pub fn sun_rotation(&self) -> Quat { Quat::from_euler(EulerRot::YXZ, self.direction.to_radians(), -self.height.to_radians(), 0.0) }
}
