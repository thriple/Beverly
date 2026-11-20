//! What a part can be: its shape, its mover, its light and the camera a camera part carries.
use super::*;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug, Reflect)]
pub enum Shape {
    Box,
    Ramp,
    Stairs,
    Arch,
    Cylinder,
    Sphere,
    Plane,
    /// An imported glTF model (`Part::model` is its path in the project); `size` is its scale.
    Model,
    /// Where the player character starts. Shown in the editor only.
    PlayerStart,
    /// A button the player presses (E when close). Output: Pressed.
    Interact,
    /// Light in every direction, out to its reach (`Part::light`). Shown in the editor only; never collides.
    PointLight,
    /// Light in a cone along the part's up (+Y): placed on a ceiling it shines down.
    SpotLight,
    /// A camera that stays where it is put, looking along its -Z (`Part::camera`). Editor only; never collides.
    FixedCamera,
    /// A camera that circles its target (`Camera::target`) once a lap, always looking at it.
    OrbitCamera,
    /// A camera that keeps its offset from the player start as the player moves, looking the way it was aimed.
    FollowCamera,
    /// A speaker playing a sound file (`Part::sound`), heard in 3D. Shown in the editor only; never collides.
    /// Inputs: Play, Stop.
    Sound,
}

impl Shape {
    pub const ALL: [Shape; 7] = [Shape::Box, Shape::Ramp, Shape::Stairs, Shape::Arch, Shape::Cylinder, Shape::Sphere, Shape::Plane];
    pub const LIGHTS: [Shape; 2] = [Shape::PointLight, Shape::SpotLight];
    pub fn is_light(self) -> bool { Shape::LIGHTS.contains(&self) }
    pub const CAMERAS: [Shape; 3] = [Shape::FixedCamera, Shape::OrbitCamera, Shape::FollowCamera];
    pub fn is_camera(self) -> bool { Shape::CAMERAS.contains(&self) }
    /// Kinds that are shown in the editor only and never collide: lights, cameras and sounds.
    pub fn is_marker(self) -> bool { self.is_light() || self.is_camera() || self == Shape::Sound }
    /// Every kind with triangles of its own (all but models).
    pub fn built() -> impl Iterator<Item = Shape> { Shape::ALL.into_iter().chain([Shape::PlayerStart, Shape::Interact]).chain(Shape::LIGHTS).chain(Shape::CAMERAS).chain([Shape::Sound]) }
    pub fn label(self) -> &'static str {
        match self {
            Shape::Box => "Box",
            Shape::Ramp => "Ramp",
            Shape::Stairs => "Stairs",
            Shape::Arch => "Arch",
            Shape::Cylinder => "Cylinder",
            Shape::Sphere => "Sphere",
            Shape::Plane => "Plane",
            Shape::Model => "Model",
            Shape::PlayerStart => "Player start",
            Shape::Interact => "Interact",
            Shape::PointLight => "Point light",
            Shape::SpotLight => "Spot light",
            Shape::FixedCamera => "Fixed camera",
            Shape::OrbitCamera => "Orbit camera",
            Shape::FollowCamera => "Follow camera",
            Shape::Sound => "Sound",
        }
    }
    pub fn glyph(self) -> &'static str {
        match self {
            Shape::Box => "▢",
            Shape::Ramp => "◺",
            Shape::Stairs => "▤",
            Shape::Arch => "∩",
            Shape::Cylinder => "◯",
            Shape::Sphere => "●",
            Shape::Plane => "▭",
            Shape::Model => "◆",
            Shape::PlayerStart => "☺",
            Shape::Interact => "☝",
            Shape::PointLight => "☼",
            Shape::SpotLight => "▽",
            Shape::FixedCamera => "▣",
            Shape::OrbitCamera => "↻",
            Shape::FollowCamera => "⇢",
            Shape::Sound => "♪",
        }
    }
    /// One line for the parts pop-up.
    pub fn description(self) -> String {
        let [x, y, z] = self.default_size();
        let what = match self {
            Shape::Box => "a block for walls, floors and cover",
            Shape::Ramp => "a slope rising toward +X",
            Shape::Stairs => "six steps rising toward +X",
            Shape::Arch => "a block with a round-topped opening",
            Shape::Cylinder => "a round column",
            Shape::Sphere => "a ball",
            Shape::Plane => "a thin slab for floors and platforms",
            Shape::Model => return "an imported glTF model".into(),
            Shape::PlayerStart => return "where the player character starts (Shift+` tests from here)".into(),
            Shape::Interact => return "a button the player presses with E; wire Pressed to what it starts".into(),
            Shape::PointLight => return "light in every direction, out to its reach".into(),
            Shape::SpotLight => return "a cone of light, shining away from the surface it is placed on".into(),
            Shape::FixedCamera => return "a view that stays put; Look through to aim it".into(),
            Shape::OrbitCamera => return "a view circling a target you drag, once a lap".into(),
            Shape::FollowCamera => return "a view that keeps its offset from the player as they move".into(),
            Shape::Sound => return "a speaker playing a sound, on start or when wired; heard in 3D".into(),
        };
        format!("{what} · {x} × {y} × {z} m")
    }

    /// Size in metres (x wide, y tall, z deep) when dropped.
    pub fn default_size(self) -> [f32; 3] {
        match self {
            Shape::Box => [2.0, 2.0, 2.0],
            Shape::Ramp => [4.0, 2.0, 3.0],
            Shape::Stairs => [4.0, 2.0, 2.0],
            Shape::Arch => [4.0, 3.0, 1.0],
            Shape::Cylinder => [1.5, 3.0, 1.5],
            Shape::Sphere => [2.0, 2.0, 2.0],
            Shape::Plane => [4.0, 0.1, 4.0],
            Shape::Model => [1.0, 1.0, 1.0],
            Shape::PlayerStart => [0.8, 1.8, 0.8],
            Shape::Interact => [0.5, 1.0, 0.5],
            Shape::PointLight | Shape::SpotLight => [0.3, 0.3, 0.3],
            Shape::FixedCamera | Shape::OrbitCamera | Shape::FollowCamera => [0.4, 0.3, 0.5],
            Shape::Sound => [0.4, 0.4, 0.4],
        }
    }

    /// Events this part sends out along wires.
    pub fn outputs(self) -> &'static [&'static str] {
        match self {
            Shape::Interact => &["Pressed"],
            _ => &[],
        }
    }
}

/// A part that moves: from where it was placed to `pos + to` over `secs`, eased, when told to.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
pub struct Mover {
    /// The end position, relative to the start (metres).
    #[reflect(@Setting::number("By", "m", 0.05, -100000.0, 100000.0).section(MOVES).help("Where it moves to, from where it was placed"))]
    pub to: [f32; 3],
    /// How far it turns about each axis (degrees) on the way, about its own middle (a door on a hinge hangs on a moving
    /// part at the hinge).
    #[serde(default)]
    #[reflect(@Setting::number("Turn by", "°", 1.0, -3600.0, 3600.0).section(MOVES).help("How far it turns about each axis on the way, about its own middle"))]
    pub turn: [f32; 3],
    /// How fast it turns all the time while playing (degrees a second about each axis): rotors, slowly spinning debris.
    #[serde(default)]
    #[reflect(@Setting::number("Spins", "°/s", 1.0, -3600.0, 3600.0).section(MOVES).help("How fast it turns all the time while playing, about each axis"))]
    pub spin: [f32; 3],
    #[reflect(@Setting::feel("Speed", &["Slow", "Steady", "Brisk", "Quick"], 20.0, 0.2, "s").log().section(MOVES).help("How long it takes to get there"))]
    pub secs: f32,
}

impl Default for Mover {
    fn default() -> Self { Mover { to: [0.0, 3.0, 0.0], turn: [0.0; 3], spin: [0.0; 3], secs: 2.0 } }
}

/// A light part's light: Point light and Spot light (the cone is a spot's only).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Light {
    /// sRGB, 0..1.
    #[reflect(@Setting::colour("Colour").section(LIGHT))]
    pub colour: [f32; 3],
    #[reflect(@Setting::toggle("Shadows").section(LIGHT).help("Off is cheaper: light passes through parts"))]
    pub shadows: bool,
    /// Lumens.
    #[reflect(@Setting::feel("Brightness", &["Dim", "Soft", "Bright", "Blinding"], 1000.0, 10000000.0, "lm").log().section(LIGHT))]
    pub brightness: f32,
    /// How far the light reaches, in metres (the sphere's or cone's handle).
    #[reflect(@Setting::feel("Reach", &["Near", "Room", "Hall", "Far"], 1.0, 100.0, "m").log().section(LIGHT).help("Nothing past this is lit"))]
    pub reach: f32,
    /// The spot's cone, edge to edge, in degrees.
    #[reflect(@Setting::feel("Cone", &["Pin", "Narrow", "Wide", "Flood"], 5.0, 150.0, "°").section(CONE).help("Spot lights only: how wide the cone is"))]
    pub cone: f32,
}

impl Default for Light {
    fn default() -> Self { Light { colour: [1.0, 0.93, 0.82], shadows: true, brightness: 800000.0, reach: 10.0, cone: 50.0 } }
}

/// The part panel's sections.
pub const PART: &str = "PART";
pub const PLACE: &str = "POSITION  ·  ROTATION";
pub const LOOK: &str = "SIZE  ·  LOOK";
pub const MOVES: &str = "MOVES";
pub const LIGHT: &str = "LIGHT";
pub const CONE: &str = "CONE";
pub const TAGS: &str = "TAGS";
pub const CAMERA: &str = "CAMERA";
pub const ORBIT: &str = "ORBIT";

/// A camera part's camera: Fixed, Orbit and Follow cameras (the target and lap are an Orbit camera's only).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Camera {
    /// How much it sees, top to bottom, in degrees.
    #[reflect(@Setting::feel("View", &["Zoomed", "Narrow", "Normal", "Wide", "Fisheye"], 10.0, 120.0, "°").section(CAMERA).help("How much it sees, top to bottom"))]
    pub fov: f32,
    /// The point an Orbit camera circles and looks at.
    #[reflect(@Setting::number("Target", "m", 0.05, -100000.0, 100000.0).section(ORBIT).help("The point it circles and looks at (drag the handle)"))]
    pub target: [f32; 3],
    /// Seconds for one circle.
    #[reflect(@Setting::feel("Lap", &["Quick", "Steady", "Slow", "Crawl"], 3.0, 120.0, "s").log().section(ORBIT).help("How long one circle takes"))]
    pub lap: f32,
}

impl Default for Camera {
    fn default() -> Self { Camera { fov: 45.0, target: [0.0; 3], lap: 20.0 } }
}

/// Where a camera part is and which way it looks (Bevy's: along -Z), `t` seconds into playing, with the player's
/// feet at `feet` (a Follow camera keeps its offset from `start`, the player start's feet).
pub fn camera_view(p: &Part, t: f32, feet: Vec3, start: Vec3) -> (Vec3, Quat) {
    let pos = Vec3::from(p.pos);
    let rot = Quat::from_array(p.rot);
    let c = p.camera.clone().unwrap_or_default();
    match p.shape {
        Shape::OrbitCamera => {
            let target = Vec3::from(c.target);
            let at = target + Quat::from_rotation_y(std::f32::consts::TAU * t / c.lap.max(0.1)) * (pos - target);
            (at, looking(at, target).unwrap_or(rot))
        }
        Shape::FollowCamera => (pos + feet - start, rot),
        _ => (pos, rot),
    }
}

/// The turn (yaw, then pitch: no roll) that looks from `from` at `to`; None when they are the same point.
pub fn looking(from: Vec3, to: Vec3) -> Option<Quat> {
    let d = (to - from).try_normalize()?;
    Some(Quat::from_euler(EulerRot::YXZ, (-d.x).atan2(-d.z), d.y.clamp(-1.0, 1.0).asin(), 0.0))
}

impl Mover {
    pub const INPUTS: [&str; 3] = ["Go", "Back", "Toggle"];
    pub const OUTPUTS: [&str; 2] = ["Arrived", "Returned"];
}
