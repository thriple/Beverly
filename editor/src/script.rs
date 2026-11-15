//! Script parts at play time: buttons (Interact), wires, and parts that move. Runs in the editor's test mode and
//! in the game player.
use crate::level::{Level, Mover, Shape, Wire};
use crate::play::{CollisionWorld, Play};
use crate::world::{PartId, part_transform};
use bevy::prelude::*;

/// How close (metres, feet to the button's side) the player must be to press a button.
pub const REACH: f32 = 1.5;

struct Moving {
    id: u64,
    piece: Option<usize>,
    /// Where it was placed.
    home: Transform,
    to: Vec3,
    secs: f32,
    /// 0 at home, 1 at the end.
    t: f32,
    /// Where it is heading: 0 or 1.
    goal: f32,
    entity: Option<Entity>,
}

impl Moving {
    /// How far from home it is now (eased).
    fn offset(&self) -> Vec3 { self.to * (self.t * self.t * (3.0 - 2.0 * self.t)) }
}

/// Where the level's buttons and moving parts are and how they are wired.
#[derive(Default)]
pub struct Script {
    movers: Vec<Moving>,
    /// Buttons: id, centre, radius.
    buttons: Vec<(u64, Vec3, f32)>,
    wires: Vec<Wire>,
    /// The wires that fired this frame (indices into the level's wires), for the editor to show.
    pub fired: Vec<usize>,
}

impl Script {
    pub fn new(level: &Level, world: &CollisionWorld) -> Self {
        let movers = level
            .parts
            .iter()
            .filter_map(|p| {
                let m: &Mover = p.mover.as_ref()?;
                Some(Moving { id: p.id, piece: world.piece_of(p.id), home: part_transform(p), to: Vec3::from(m.to), secs: m.secs.max(0.05), t: 0.0, goal: 0.0, entity: None })
            })
            .collect();
        let buttons = level.parts.iter().filter(|p| p.shape == Shape::Interact).map(|p| (p.id, Vec3::from(p.pos), p.size[0].max(p.size[2]) * 0.5)).collect();
        Self { movers, buttons, wires: level.wires.clone(), fired: vec![] }
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

    /// Put moved parts back where they were placed.
    pub fn reset(&mut self, commands: &mut Commands) {
        for m in &mut self.movers {
            if let Some(e) = m.entity { commands.entity(e).try_insert(m.home); }
            m.t = 0.0;
            m.goal = 0.0;
        }
        self.fired.clear();
    }

    /// Move the moving parts on by `dt` seconds (their solid pieces in `world` too); those that get where they
    /// were going send Arrived / Returned.
    pub fn tick(&mut self, world: &mut CollisionWorld, dt: f32) {
        let mut done = vec![];
        for m in &mut self.movers {
            if m.t != m.goal {
                let step = dt / m.secs;
                m.t = if m.goal > m.t { (m.t + step).min(m.goal) } else { (m.t - step).max(m.goal) };
                if m.t == m.goal { done.push((m.id, if m.goal > 0.5 { "Arrived" } else { "Returned" })); }
            }
            if let Some(i) = m.piece { world.set_offset(i, m.offset()); }
        }
        for (id, out) in done { self.send(id, out); }
    }

    /// Send `out` from part `from` along its wires.
    pub fn send(&mut self, from: u64, out: &str) {
        for (i, w) in self.wires.iter().enumerate() {
            if w.from != from || w.out != out { continue; }
            self.fired.push(i);
            if let Some(m) = self.movers.iter_mut().find(|m| m.id == w.to) {
                match w.input.as_str() {
                    "Go" => m.goal = 1.0,
                    "Back" => m.goal = 0.0,
                    "Toggle" => m.goal = if m.goal > 0.5 { 0.0 } else { 1.0 },
                    _ => {}
                }
            }
        }
    }
}

/// One frame of the script: buttons, then wires, then moving parts.
pub fn run(mut play: ResMut<Play>, keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, mut parts: Query<(Entity, &PartId, &mut Transform)>) {
    let play = &mut *play;
    play.world.clear_deltas();
    play.script.fired.clear();
    if keys.just_pressed(KeyCode::KeyE) {
        if let Some(id) = play.script.button_near(play.feet) { play.script.send(id, "Pressed"); }
    }
    play.script.tick(&mut play.world, time.delta_secs().min(0.1));
    for m in &mut play.script.movers {
        if m.entity.is_none() { m.entity = parts.iter().find(|(_, id, _)| id.0 == m.id).map(|(e, ..)| e); }
        if let Some(Ok((_, _, mut tf))) = m.entity.map(|e| parts.get_mut(e)) { tf.translation = m.home.translation + m.offset(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{Collision, Part};
    use crate::world::Geometry;

    fn part(id: u64, shape: Shape, pos: [f32; 3], size: [f32; 3]) -> Part {
        Part { id, name: format!("p{id}"), shape, pos, rot: [0.0, 0.0, 0.0, 1.0], size, color: [0.5; 3], visible: true, collision: Collision::Solid, model: None, mover: None }
    }

    #[test]
    fn a_button_sends_a_pad_up_and_it_reports_arriving() {
        let mut pad = part(2, Shape::Plane, [5.0, 0.05, 0.0], [4.0, 0.1, 4.0]);
        pad.mover = Some(Mover { to: [0.0, 6.0, 0.0], secs: 2.0 });
        let level = Level {
            name: "t".into(),
            parts: vec![part(1, Shape::Interact, [0.0, 0.5, 0.0], [0.5, 1.0, 0.5]), pad],
            next_id: 2,
            wires: vec![
                Wire { from: 1, out: "Pressed".into(), to: 2, input: "Toggle".into() },
                Wire { from: 2, out: "Arrived".into(), to: 2, input: "Back".into() },
            ],
            ..Default::default()
        };
        let mut world = CollisionWorld::build(&level, &Geometry::default());
        let mut s = Script::new(&level, &world);
        assert_eq!(s.button_near(Vec3::new(1.0, 0.0, 0.0)), Some(1));
        assert_eq!(s.button_near(Vec3::new(4.0, 0.0, 0.0)), None);
        s.send(1, "Pressed");
        let i = world.piece_of(2).unwrap();
        s.tick(&mut world, 1.0);
        assert!((world.offsets().nth(i).unwrap().y - 3.0).abs() < 1e-4, "halfway up after half the time");
        s.tick(&mut world, 1.0);
        assert!((world.offsets().nth(i).unwrap().y - 6.0).abs() < 1e-4);
        // Arrived is wired to its own Back: it heads home again
        s.tick(&mut world, 2.0);
        assert!(world.offsets().nth(i).unwrap().y.abs() < 1e-4);
    }
}
