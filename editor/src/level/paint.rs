//! What lies on a part's surface: materials laid on faces, brush strokes and their rules.
use super::*;

/// A material laid on a part, all over or on one face, through its file in assets/materials. Where it sits is the
/// surface's own (two parts can share a material and lay it differently).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
pub struct Surface {
    /// The face (see `shapes::Tris::faces`), or `None` for all of the part. A face's own surface wins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<u8>,
    /// The material file, e.g. "assets/materials/brick.bev".
    pub material: String,
    /// How big one copy of the texture is on the surface, in metres (across, down).
    pub tile: [f32; 2],
    /// How far the texture is slid along the surface, in metres (across, down).
    #[serde(default)]
    pub offset: [f32; 2],
}

impl Part {
    /// The surface that covers `face`: its own, else the all-over one.
    pub fn surface(&self, face: u8) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.face == Some(face)).or_else(|| self.surfaces.iter().find(|s| s.face.is_none()))
    }
    /// Lay `s` on the part: on its face (or all over), replacing what was there. All over clears the faces' own.
    pub fn lay(&mut self, s: Surface) {
        if s.face.is_none() { self.surfaces.clear(); }
        self.surfaces.retain(|o| o.face != s.face);
        self.surfaces.push(s);
    }
}

/// A brush stroke on a part: a material (or erasing) laid along points on its surface, with a soft or hard edge.
/// Later strokes cover earlier ones; erasing uncovers. Kept as the stroke, never as an image.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
pub struct Stroke {
    /// The material painted, e.g. "assets/materials/moss.bev"; empty: erasing.
    pub material: String,
    /// Metres across.
    pub size: f32,
    /// 0: soft all the way in, 1: a hard edge.
    pub hardness: f32,
    /// How much it covers, 0..1.
    pub strength: f32,
    /// Where it went, in metres in the part's own frame (from its middle, turned with it), so it stays put when the
    /// part moves, turns or is resized.
    pub points: Vec<[f32; 3]>,
    /// How it lays its material: adding to what is there, taking it away, or setting it to exactly its strength.
    #[serde(default, skip_serializing_if = "is_default")]
    pub lays: Lays,
    /// Where it may land (slope, height), worked out where it shows, so it holds wherever the part goes.
    #[serde(default, skip_serializing_if = "is_default")]
    pub rules: Rules,
    /// The pen curve whose strip made this stroke (`curve.rs`): it is made again when that curve changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve: Option<u64>,
    /// A fill, not a stroke: `points` are the outline of the area, in the world (not the part's frame), `size` is the
    /// width of its feathered edge, and it paints everything inside (as the paint shader works out).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fill: bool,
}

/// How a stroke lays its layer.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default, Reflect)]
pub enum Lays {
    /// Adds a little each pass (as much as its strength), covering other layers as it goes.
    #[default]
    Add,
    /// Takes a little of its own layer away each pass, leaving the others.
    Subtract,
    /// Sets its layer to exactly its strength, however often it goes over a place.
    Set,
}

/// Where a brush (or any tool) may land, whatever it lays. Kept with each stroke.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Rules {
    /// The steepest surface it lands on, in degrees from flat (90: any, walls included).
    #[reflect(@Setting::feel("Slope", &["Flat", "Gentle", "Steep", "Any"], 0.0, 90.0, "°").section(RULES).help("The steepest surface it lands on: Flat only paints tops, Any paints walls too"))]
    pub slope: f32,
    /// Only between these heights in the world (metres), when set.
    pub band: Option<[f32; 2]>,
    /// Only on parts with this surface tag, when set.
    pub on: Option<Tag>,
}

/// The rules' section in the brush's panel.
pub const RULES: &str = "RULES";

impl Default for Rules {
    fn default() -> Self { Rules { slope: 90.0, band: None, on: None } }
}

impl Rules {
    /// How much a surface with this upward share of its normal (1: flat, 0: a wall) at this height lets through.
    pub fn allows(&self, up: f32, y: f32) -> f32 {
        let (c, lo, hi) = self.shader();
        let slope = if c < -1.0 { 1.0 } else { smooth(c - 0.03, c + 0.03, up) };
        slope * smooth(lo - 0.05, lo + 0.05, y) * (1.0 - smooth(hi - 0.05, hi + 0.05, y))
    }
    /// As the shader takes them: the cosine of the steepest slope (below -1: any), and the height band.
    pub fn shader(&self) -> (f32, f32, f32) {
        let c = if self.slope >= 89.5 { -2.0 } else { self.slope.to_radians().cos() };
        let [lo, hi] = self.band.unwrap_or([-1e9, 1e9]);
        (c, lo, hi)
    }
}

/// For serde: leave a field out of the file while it is its default.
fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Part {
    /// The materials painted on it, in the order they were first painted.
    pub fn painted(&self) -> Vec<&str> {
        let mut v: Vec<&str> = vec![];
        for s in &self.strokes { if !s.material.is_empty() && !v.contains(&s.material.as_str()) { v.push(&s.material); } }
        v
    }
    /// A point in the world in the part's own frame (metres from its middle, turned with it).
    pub fn to_local(&self, world: Vec3) -> Vec3 { Quat::from_array(self.rot).inverse() * (world - Vec3::from(self.pos)) }
}
