//! Zone parts: a box that sends Entered and Left along wires as the player (or anything you choose) goes in and out,
//! and layers that change things inside it, blending over its edge. Each layer is optional (none: the level's own).
//! A feature that can differ by place adds its layer here: today Lighting and Gravity; weather, wind, sound, water
//! and hazards add theirs as they are built.
use super::*;

/// The zone's own section, and its layers'.
pub const ZONE: &str = "ZONE";
pub const ZONE_LIGHTING: &str = "ZONE LIGHTING";
pub const ZONE_GRAVITY: &str = "ZONE GRAVITY";

/// What sets a zone off (its Entered and Left).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default, Reflect)]
pub enum Who {
    #[default]
    Player,
    /// The player or any loose part.
    Anything,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Zone {
    #[reflect(@Setting::choice("Sets off").section(ZONE).help("Who sends Entered and Left: the player, or the player and any loose part"))]
    pub who: Who,
    /// How far in from its sides (metres) its layers take full effect: they blend in over this.
    #[reflect(@Setting::feel("Edge", &["Sharp", "Soft", "Wide"], 0.0, 10.0, "m").section(ZONE).help("Its layers blend in over this distance from its sides"))]
    pub edge: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[reflect(@Setting::optional("Lighting", "Level's lighting", "Own lighting").section(ZONE_LIGHTING).help("Its own light while the camera is inside: an Interior with no sun, or a different fill"))]
    pub lighting: Option<ZoneLighting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[reflect(@Setting::optional("Gravity", "Level's gravity", "Own gravity").section(ZONE_GRAVITY).help("How strongly things fall inside it: the player and loose parts"))]
    pub gravity: Option<ZoneGravity>,
}

impl Default for Zone {
    fn default() -> Self { Zone { who: Who::Player, edge: 1.0, lighting: None, gravity: None } }
}

impl Zone {
    pub const OUTPUTS: [&str; 2] = ["Entered", "Left"];
}

/// A zone's light: no sun inside (Interior), and the fill light.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct ZoneLighting {
    #[reflect(@Setting::toggle("Interior").section(ZONE_LIGHTING).help("No sun inside: only the fill light and your own lights"))]
    pub interior: bool,
    #[reflect(@Setting::feel("Fill", &["Dark", "Dim", "Soft", "Bright"], 0.0, 3000.0, "").section(ZONE_LIGHTING).help("The light everywhere inside, from no direction"))]
    pub fill: f32,
    #[reflect(@Setting::colour("Fill colour").section(ZONE_LIGHTING))]
    pub fill_colour: [f32; 3],
}

impl Default for ZoneLighting {
    fn default() -> Self { ZoneLighting { interior: true, fill: 300.0, fill_colour: [1.0, 0.92, 0.8] } }
}

/// A zone's gravity, as a share of the level's.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct ZoneGravity {
    #[reflect(@Setting::feel("Strength", &["None", "Moon", "Normal", "Heavy"], 0.0, 2.0, "×").section(ZONE_GRAVITY).help("0: things float; 1: the level's gravity"))]
    pub strength: f32,
}

impl Default for ZoneGravity {
    fn default() -> Self { ZoneGravity { strength: 0.3 } }
}

/// How much of a zone part's layers reach `at`: 0 outside it, rising to 1 at its edge's width inside its sides.
pub fn zone_weight(p: &Part, edge: f32, at: Vec3) -> f32 {
    let local = Quat::from_array(p.rot).inverse() * (at - Vec3::from(p.pos));
    let inside = (Vec3::from(p.size) * 0.5 - local.abs()).min_element();
    if inside < 0.0 { 0.0 } else if edge <= 0.0 { 1.0 } else { (inside / edge).min(1.0) }
}

/// Whether `at` is inside a zone part's box.
pub fn in_zone(p: &Part, at: Vec3) -> bool {
    let local = Quat::from_array(p.rot).inverse() * (at - Vec3::from(p.pos));
    (Vec3::from(p.size) * 0.5 - local.abs()).min_element() >= 0.0
}

impl Level {
    /// The zone parts and their zones, in the level's order (a later one wins where they overlap).
    pub fn zones(&self) -> impl Iterator<Item = (&Part, &Zone)> { self.parts.iter().filter_map(|p| Some((p, p.zone.as_ref()?))) }

    /// The level's lighting as seen from `at`: each zone's own lighting blended in by how far inside it `at` is.
    pub fn lighting_at(&self, at: Vec3) -> Lighting {
        let mut l = (*self.lighting).clone();
        let mut sun = if l.interior { 0.0 } else { 1.0 };
        for (p, z) in self.zones() {
            let Some(zl) = &z.lighting else { continue };
            let w = zone_weight(p, z.edge, at);
            if w <= 0.0 { continue; }
            let own = if zl.interior { 0.0 } else { 1.0 };
            sun += (own - sun) * w;
            l.fill += (zl.fill - l.fill) * w;
            for k in 0..3 { l.fill_colour[k] += (zl.fill_colour[k] - l.fill_colour[k]) * w; }
        }
        // the sun dims rather than switching off; its shadows go once it is mostly gone
        l.brightness *= sun;
        l.interior = sun <= 0.0;
        l.shadows &= sun > 0.5;
        l
    }

    /// How strongly things fall at `at`, as a share of the level's gravity.
    pub fn gravity_at(&self, at: Vec3) -> f32 { gravity_among(self.zones(), at) }
}

/// How strongly things fall at `at` among these zone parts, as a share of the level's gravity.
pub fn gravity_among<'a>(zones: impl Iterator<Item = (&'a Part, &'a Zone)>, at: Vec3) -> f32 {
    let mut g = 1.0;
    for (p, z) in zones {
        let Some(zg) = &z.gravity else { continue };
        g += (zg.strength - g) * zone_weight(p, z.edge, at);
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(pos: [f32; 3], size: [f32; 3], z: Zone) -> Part {
        let mut p = Part::placed(Shape::Zone, None, "Z".into(), pos, [0.0, 0.0, 0.0, 1.0], size);
        p.zone = Some(z);
        p
    }

    #[test]
    fn layers_blend_in_over_the_edge_and_a_later_zone_wins() {
        let mut l = Level::default();
        l.next_id = 2;
        let mut a = zone([0.0, 2.0, 0.0], [10.0, 4.0, 10.0], Zone { edge: 2.0, lighting: Some(ZoneLighting::default()), gravity: Some(ZoneGravity { strength: 0.0 }), ..Zone::default() });
        a.id = 1;
        l.parts.push(a);
        // outside: the level's own
        assert_eq!(l.gravity_at(Vec3::new(8.0, 1.0, 0.0)), 1.0);
        assert_eq!(l.lighting_at(Vec3::new(8.0, 1.0, 0.0)).brightness, l.lighting.brightness);
        // a metre in from the side (half the edge): halfway
        assert!((l.gravity_at(Vec3::new(4.0, 2.0, 0.0)) - 0.5).abs() < 1e-5);
        // deep inside: the zone's (no sun, its fill)
        let deep = l.lighting_at(Vec3::new(0.0, 2.0, 0.0));
        assert!(deep.interior && deep.brightness == 0.0 && !deep.shadows && deep.fill == 300.0, "{deep:?}");
        assert_eq!(l.gravity_at(Vec3::new(0.0, 2.0, 0.0)), 0.0);
        // a turned zone: its own frame
        let mut b = zone([20.0, 2.0, 0.0], [10.0, 4.0, 2.0], Zone { edge: 0.0, gravity: Some(ZoneGravity { strength: 2.0 }), ..Zone::default() });
        b.rot = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2).to_array();
        b.id = 2;
        l.parts.push(b);
        assert_eq!(l.gravity_at(Vec3::new(20.0, 2.0, 4.0)), 2.0, "along its length, now along z");
        assert_eq!(l.gravity_at(Vec3::new(24.0, 2.0, 0.0)), 1.0, "across it, now along x");
    }
}
