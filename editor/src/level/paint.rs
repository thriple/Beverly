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
    /// A fill, not a stroke: `points` are the outline of the area, in the part's frame as a stroke's are (seen from above in
    /// that frame), `size` is the width of its feathered edge, and it paints everything inside (as the paint shader
    /// works out). So it holds when the part moves, turns or the world shifts.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fill: bool,
    /// Whole faces, not a stroke: it covers these faces of the part all over (`points` and `size` aren't used). A bucket
    /// fill's strokes are these.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faces: Vec<u8>,
    /// The bucket fill that made this stroke (`Level::buckets`, `bucket.rs`): it is made again when the level changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket: Option<u64>,
}

/// A bucket fill: the place clicked and a rule for what else counts as the same, with the layer it lays there. It stays
/// a rule, not the area it found: change what was clicked (recolour it, lay another material on it) and the fill
/// follows. The strokes it makes on each part it reaches are made again whenever the level changes (`bucket.rs`).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Bucket {
    pub id: u64,
    /// The part clicked, its face, and where on it (in the part's own frame).
    pub part: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<u8>,
    pub at: [f32; 3],
    /// What else it fills.
    pub fills: Fills,
    /// How far a colour may be from the clicked one and still count (0: exactly, 1: any).
    #[serde(default, skip_serializing_if = "is_default")]
    pub tolerance: f32,
    /// The surface tag it fills (Same tag), when not the clicked part's first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<Tag>,
    /// Only what touches the clicked part, part to part: it stops at gaps.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub connected: bool,
    /// The material it lays (empty: it erases), how much, and how.
    pub material: String,
    pub strength: f32,
    #[serde(default, skip_serializing_if = "is_default")]
    pub lays: Lays,
}

/// What a bucket fill fills, starting from what was clicked.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default, Reflect)]
pub enum Fills {
    /// Every face with the same material laid on it (or, with none, the same colour).
    #[default]
    #[reflect(@Label("Material"))]
    Material,
    /// Every face of the same colour, within the tolerance.
    #[reflect(@Label("Colour"))]
    Colour,
    /// Every part with the same surface tag.
    #[reflect(@Label("Tag"))]
    Tag,
    /// All of the part clicked.
    #[reflect(@Label("Part"))]
    Part,
    /// The walkable faces at the height clicked: a storey of a building, the ground of a yard.
    #[reflect(@Label("Floor"))]
    Floor,
}

impl Fills {
    pub const ALL: [Fills; 5] = [Fills::Material, Fills::Colour, Fills::Tag, Fills::Part, Fills::Floor];
    pub fn word(self) -> &'static str {
        match self { Fills::Material => "material", Fills::Colour => "colour", Fills::Tag => "tag", Fills::Part => "part", Fills::Floor => "floor" }
    }
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
    /// A point in the part's own frame in the world.
    pub fn to_world(&self, local: Vec3) -> Vec3 { Quat::from_array(self.rot) * local + Vec3::from(self.pos) }
}
