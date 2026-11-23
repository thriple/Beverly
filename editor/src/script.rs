//! Script parts at play time: buttons (Interact), wires, and parts that move. Runs in the editor's test mode and
//! in the game player.
use crate::level::{beyond, frame, within, Level, Mover, Shape, Wire};
use crate::physics::PartBody;
use crate::play::Play;
use crate::world::part_transform;
use avian3d::prelude::{AngularVelocity, LinearVelocity, Position, Rotation};
use bevy::prelude::*;

/// How close (metres, feet to the button's side) the player must be to press a button.
pub const REACH: f32 = 1.5;

struct Moving {
    id: u64,
    /// Where it was placed.
    home: Transform,
    to: Vec3,
    /// How far it turns (radians about x, y, z) by the end.
    turn: Vec3,
    /// How fast it turns all the time (radians a second about x, y, z), and for how long it has.
    spin: Vec3,
    /// How fast it travels all the time (metres a second along x, y, z).
    drift: Vec3,
    age: f32,
    secs: f32,
    /// 0 at home, 1 at the end.
    t: f32,
    /// Where it is heading: 0 or 1.
    goal: f32,
    /// Where it was and which way it faced at the last step (the walker standing on it is carried round by the change).
    last: (Vec3, Quat),
}

impl Moving {
    /// How far from home it is now (eased).
    fn offset(&self) -> Vec3 { self.to * self.eased() + self.drift * self.age }
    fn eased(&self) -> f32 { self.t * self.t * (3.0 - 2.0 * self.t) }
    /// Where it is and which way it faces now (the turn is about its own middle, in the world's axes).
    fn pose(&self) -> (Vec3, Quat) {
        let k = self.eased();
        let turn = Quat::from_euler(EulerRot::XYZ, self.turn.x * k, self.turn.y * k, self.turn.z * k);
        let spin = Quat::from_euler(EulerRot::XYZ, self.spin.x * self.age, self.spin.y * self.age, self.spin.z * self.age);
        (self.home.translation + self.to * k + self.drift * self.age, spin * turn * self.home.rotation)
    }
}

/// A part carried by a moving part (`Level::carrier`): where it is in that part's frame.
struct Carried {
    carrier: u64,
    local: (Vec3, Quat),
}

/// Something that happened while playing, for sounds (and later effects) to follow; they never steer the game.
/// A part's events are what it sends along wires, Starts and Hit (see `Part::events`); the walker's (no part) are
/// Step, Jump and Land, with `on` the part underfoot. A game's own player character pushes its Step, Jump and Land
/// into `Script::happened` the same way.
#[derive(Clone, Debug, PartialEq)]
pub struct Happened {
    pub what: String,
    pub part: Option<u64>,
    /// Where (None: where the part is).
    pub at: Option<Vec3>,
    /// The part the walker stands on.
    pub on: Option<u64>,
}

/// Where the level's buttons and moving parts are and how they are wired.
#[derive(Default)]
pub struct Script {
    movers: Vec<Moving>,
    /// The parts attached to the moving ones, by id.
    carried: std::collections::HashMap<u64, Carried>,
    /// Buttons: id, centre, radius.
    buttons: Vec<(u64, Vec3, f32)>,
    wires: Vec<Wire>,
    /// The wires that fired this frame (indices into the level's wires), for the editor to show.
    pub fired: Vec<usize>,
    /// What the wires sent (part, input) since the sounds last took it (see `sound.rs`).
    pub inputs: Vec<(u64, String)>,
    /// What happened since the sounds last took it.
    pub happened: Vec<Happened>,
}

impl Script {
    pub fn new(level: &Level) -> Self {
        let movers = level
            .parts
            .iter()
            .filter_map(|p| {
                let m: &Mover = p.mover.as_ref()?;
                Some(Moving { id: p.id, home: part_transform(p), to: Vec3::from(m.to), turn: Vec3::from(m.turn) * std::f32::consts::PI / 180.0, spin: Vec3::from(m.spin) * std::f32::consts::PI / 180.0, drift: Vec3::from(m.drift), age: 0.0, secs: m.secs.max(0.05), t: 0.0, goal: 0.0, last: (part_transform(p).translation, part_transform(p).rotation) })
            })
            .collect();
        let carried = level.parts.iter().filter_map(|p| {
            let carrier = level.carrier(p.id)?;
            Some((p.id, Carried { carrier, local: within(frame(level.get(carrier)?), frame(p)) }))
        }).collect();
        let buttons = level.parts.iter().filter(|p| p.shape == Shape::Interact).map(|p| (p.id, Vec3::from(p.pos), p.size[0].max(p.size[2]) * 0.5)).collect();
        Self { movers, carried, buttons, wires: (*level.wires).clone(), fired: vec![], inputs: vec![], happened: vec![] }
    }

    /// The button the player at `feet` could press now.
    pub fn button_near(&self, feet: Vec3) -> Option<u64> {
        self.buttons
            .iter()
            .map(|&(id, c, r)| (id, (feet - c).with_y(0.0).length() - r, (feet.y - c.y).abs()))
            .filter(|&(_, d, dy)| d < REACH && dy < 2.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, ..)| id)
    }

    /// Start the moving parts over from where they were placed.
    pub fn reset(&mut self) {
        for m in &mut self.movers {
            m.t = 0.0;
            m.goal = 0.0;
            m.age = 0.0;
            m.last = (m.home.translation, m.home.rotation);
        }
        self.fired.clear();
        self.inputs.clear();
        self.happened.clear();
    }

    /// The parts carried by moving parts: (part, the moving part).
    pub fn carried(&self) -> impl Iterator<Item = (u64, u64)> + '_ { self.carried.iter().map(|(id, c)| (*id, c.carrier)) }

    /// How far moving part `id` is from where it was placed.
    pub fn offset_of(&self, id: u64) -> Option<Vec3> { self.movers.iter().find(|m| m.id == id).map(Moving::offset) }

    /// Move the moving parts on by `dt` seconds; those that get where they were going send Arrived / Returned.
    pub fn tick(&mut self, dt: f32) {
        let mut done = vec![];
        for m in &mut self.movers {
            m.age += dt;
            if m.t != m.goal {
                let step = dt / m.secs;
                m.t = if m.goal > m.t { (m.t + step).min(m.goal) } else { (m.t - step).max(m.goal) };
                if m.t == m.goal { done.push((m.id, if m.goal > 0.5 { "Arrived" } else { "Returned" })); }
            }
        }
        for (id, out) in done { self.send(id, out); }
    }

    /// Send `out` from part `from` along its wires.
    pub fn send(&mut self, from: u64, out: &str) {
        self.happened.push(Happened { what: out.into(), part: Some(from), at: None, on: None });
        for (i, w) in self.wires.iter().enumerate() {
            if w.from != from || w.out != out { continue; }
            self.fired.push(i);
            self.inputs.push((w.to, w.input.clone()));
            if let Some(m) = self.movers.iter_mut().find(|m| m.id == w.to) {
                let was = m.goal;
                match w.input.as_str() {
                    "Go" => m.goal = 1.0,
                    "Back" => m.goal = 0.0,
                    "Toggle" => m.goal = if m.goal > 0.5 { 0.0 } else { 1.0 },
                    _ => {}
                }
                if m.goal != was && m.t != m.goal { self.happened.push(Happened { what: "Starts".into(), part: Some(m.id), at: None, on: None }); }
            }
        }
    }
}

/// One fixed step of the script: buttons, then wires, then moving parts, whose bodies are given the speed that
/// takes them where they should be by the end of the step (so what stands on them rides along).
pub fn run(mut play: ResMut<Play>, time: Res<Time>, mut bodies: Query<(&PartBody, &Position, &mut Rotation, &mut LinearVelocity, &mut AngularVelocity)>, mut walker: Query<&mut Position, (With<crate::play::WalkerBody>, Without<PartBody>)>) {
    let play = &mut *play;
    play.script.fired.clear();
    if std::mem::take(&mut play.press) {
        if let Some(id) = play.script.button_near(play.feet) { play.script.send(id, "Pressed"); }
    }
    let dt = time.delta_secs();
    play.script.tick(dt);
    if dt <= 0.0 { return; }
    // the walker on a part that turns is carried round with it (the slide the body's speed already gives it), and
    // looks round by the same turn
    let on = play.walker.standing_on().and_then(|id| if play.script.movers.iter().any(|m| m.id == id) { Some(id) } else { play.script.carried.get(&id).map(|c| c.carrier) });
    for m in &mut play.script.movers {
        let (to, facing) = m.pose();
        let (was, turned) = (m.last, facing * m.last.1.inverse());
        m.last = (to, facing);
        if on != Some(m.id) || turned.dot(Quat::IDENTITY).abs() > 1.0 - 1e-9 { continue; }
        if let Ok(mut at) = walker.single_mut() {
            let r = at.0 - was.0;
            at.0 += turned * r - r;
            play.walker.yaw += turned.to_euler(EulerRot::YXZ).0;
        }
    }
    for (b, at, mut rot, mut v, mut w) in &mut bodies {
        if let Some(m) = play.script.movers.iter().find(|m| m.id == b.0) {
            let (to, facing) = m.pose();
            v.0 = (to - at.0) / dt;
            // it turns by being put at its facing, not by turning speed: what stands on it is carried round above, and
            // the contact would drag it round a second time
            rot.0 = facing;
            w.0 = Vec3::ZERO;
        }
    }
}

/// The parts carried by moving parts are drawn where their carrier is (the carrier is drawn smoothed between steps).
pub fn carry(play: Res<Play>, mut shown: Query<(&crate::world::PartId, &mut Transform)>) {
    if play.script.carried.is_empty() { return; }
    let carriers: std::collections::HashMap<u64, (Vec3, Quat)> = shown.iter()
        .filter(|(id, _)| play.script.carried.values().any(|c| c.carrier == id.0))
        .map(|(id, t)| (id.0, (t.translation, t.rotation))).collect();
    for (id, mut t) in &mut shown {
        let Some(c) = play.script.carried.get(&id.0) else { continue };
        let Some(&at) = carriers.get(&c.carrier) else { continue };
        let (pos, rot) = beyond(at, c.local);
        if t.translation != pos || t.rotation != rot { t.translation = pos; t.rotation = rot; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{Collision, Part};

    fn part(id: u64, shape: Shape, pos: [f32; 3], size: [f32; 3]) -> Part {
        Part { id, name: format!("p{id}"), shape, pos, rot: [0.0, 0.0, 0.0, 1.0], size, color: [0.5; 3], visible: true, collision: Collision::Solid, model: None, mover: None, surfaces: vec![], unique: false, tags: vec![], loose: None, light: None, camera: None, sound: None, sounds: vec![], sounds_like: None, strokes: vec![], parent: None, scatter: None }
    }

    #[test]
    fn a_button_sends_a_pad_up_and_it_reports_arriving() {
        let mut pad = part(2, Shape::Plane, [5.0, 0.05, 0.0], [4.0, 0.1, 4.0]);
        pad.mover = Some(Mover { to: [0.0, 6.0, 0.0], secs: 2.0, ..default() });
        let level = Level {
            name: "t".into(),
            parts: vec![part(1, Shape::Interact, [0.0, 0.5, 0.0], [0.5, 1.0, 0.5]), pad].into(),
            next_id: 2,
            wires: vec![
                Wire { from: 1, out: "Pressed".into(), to: 2, input: "Toggle".into() },
                Wire { from: 2, out: "Arrived".into(), to: 2, input: "Back".into() },
            ].into(),
            ..Default::default()
        };
        let mut s = Script::new(&level);
        assert_eq!(s.button_near(Vec3::new(1.0, 0.0, 0.0)), Some(1));
        assert_eq!(s.button_near(Vec3::new(4.0, 0.0, 0.0)), None);
        s.send(1, "Pressed");
        let up = |s: &Script| s.offset_of(2).unwrap().y;
        s.tick(1.0);
        assert!((up(&s) - 3.0).abs() < 1e-4, "halfway up after half the time");
        s.tick(1.0);
        assert!((up(&s) - 6.0).abs() < 1e-4);
        // Arrived is wired to its own Back: it heads home again
        s.tick(2.0);
        assert!(up(&s).abs() < 1e-4);
    }

    #[test]
    fn a_mover_turns_as_well_as_slides_and_what_is_attached_to_it_is_carried() {
        let mut deck = part(1, Shape::Box, [0.0, 0.5, 0.0], [4.0, 0.4, 4.0]);
        deck.mover = Some(Mover { to: [0.0, 3.0, 0.0], turn: [0.0, 90.0, 0.0], secs: 2.0, ..Default::default() });
        let mut crate_ = part(2, Shape::Box, [1.5, 0.9, 0.0], [1.0; 3]);
        crate_.parent = Some(1);
        // a moving part attached to a moving part is not carried (it moves by its own motion)
        let mut other = part(3, Shape::Box, [0.0, 0.0, 5.0], [1.0; 3]);
        other.mover = Some(Mover::default());
        other.parent = Some(1);
        let level = Level { name: "t".into(), parts: vec![deck, crate_, other].into(), next_id: 3, ..Default::default() };
        let mut s = Script::new(&level);
        assert_eq!(s.carried().collect::<Vec<_>>(), vec![(2, 1)]);
        s.movers[0].goal = 1.0;
        for _ in 0..200 { s.tick(0.01); }
        let (pos, rot) = s.movers[0].pose();
        assert!((pos - Vec3::new(0.0, 3.5, 0.0)).length() < 1e-4);
        assert!((rot * Vec3::X - Vec3::NEG_Z).length() < 1e-4, "a quarter turn about Y takes x to -z");
        // spinning goes on without being told: 90 degrees a second about Y for a second more is another quarter turn
        s.movers[0].spin = Vec3::new(0.0, std::f32::consts::FRAC_PI_2, 0.0);
        s.movers[0].age = 0.0;
        for _ in 0..100 { s.tick(0.01); }
        let (_, spun) = s.movers[0].pose();
        assert!((spun * Vec3::X - Vec3::NEG_X).length() < 1e-3, "half a turn in all, {:?}", spun * Vec3::X);
        s.movers[0].spin = Vec3::ZERO;
        // drifting goes on too: 2 m a second along x for a second and a half
        s.movers[0].drift = Vec3::new(2.0, 0.0, 0.0);
        s.movers[0].age = 0.0;
        for _ in 0..150 { s.tick(0.01); }
        assert!((s.movers[0].offset() - Vec3::new(3.0, 3.0, 0.0)).length() < 1e-3 && (s.movers[0].pose().0.x - 3.0).abs() < 1e-3, "{:?}", s.movers[0].offset());
        s.movers[0].drift = Vec3::ZERO;
        s.movers[0].age = 0.0;
        // the crate, where its carrier puts it
        let c = &s.carried[&2];
        let at = beyond((pos, rot), c.local);
        assert!((at.0 - Vec3::new(0.0, 3.9, -1.5)).length() < 1e-4, "{:?}", at.0);
    }
}
