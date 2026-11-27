//! Playing a level: the plain walker (a stand-in character until the real characters exist), a floating character
//! (bevy_tnua) in the physics world (`physics.rs`): it walks up steps and slopes and rides moving parts. Used by the
//! editor's test mode (`) and by the game player.
use crate::input::{Actions, Controls};
use crate::level::{Level, Navigation, Tag};
use crate::physics::PartBody;
use crate::script::Script;
use crate::world::{Geometry, PartId, part_transform};
use avian3d::prelude::*;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy_tnua::prelude::*;
use bevy_tnua::builtins::{TnuaBuiltinJumpConfig, TnuaBuiltinWalkConfig};
use bevy_tnua_avian3d::prelude::*;
use std::collections::{HashMap, HashSet};

/// The walker's body, from the level's walker profile ([`Level::profile`]): a capsule as wide as the profile,
/// floating a step above the ground (so it walks up steps that high, and no higher) with its top at the profile's
/// height, and the steepest slope it stands on (steeper, it slides).
#[derive(Clone, Copy, PartialEq, Debug)]
struct Body {
    radius: f32,
    length: f32,
    /// How high the capsule's middle floats above the ground.
    float: f32,
    height: f32,
    /// Radians.
    slope: f32,
}

impl Body {
    fn of(p: &Navigation) -> Body {
        let radius = (p.width * 0.5).max(0.05);
        let length = (p.height - p.step - 2.0 * radius).max(0.0);
        Body { radius, length, float: p.step + radius + length * 0.5, height: p.height.max(2.0 * radius), slope: p.slope.clamp(0.0, 89.0).to_radians() }
    }
}

impl Default for Body {
    fn default() -> Self { Body::of(&Navigation::default()) }
}
const EYE: f32 = 1.6;
pub const GRAVITY: f32 = 22.0;
const JUMP: f32 = 1.45;
const WALK: f32 = 5.0;
const RUN: f32 = 9.0;
/// How far the walker goes between steps (each one a Step for its sound).
const STRIDE: f32 = 1.8;
/// How sharply a loose part's speed must change in one step to be a Hit (m/s), and how soon it can be another.
const KNOCK: f32 = 3.0;
const KNOCK_AGAIN: f32 = 0.2;

/// What the walker can do: walk (always), and jump.
#[derive(TnuaScheme)]
#[scheme(basis = TnuaBuiltinWalk)]
pub enum Moves {
    Jump(TnuaBuiltinJump),
}

/// The walker as the rest of Beverly sees it: feet, velocity, facing, the camera's yaw / pitch, whether it stands on
/// something and on which part. Read from its body each step.
#[derive(Default, Clone)]
pub struct Walker { pub pos: Vec3, pub vel: Vec3, pub facing: f32, pub yaw: f32, pub pitch: f32, pub ground: bool, on: Option<u64>, stride: f32 }

impl Walker {
    /// The part it stands on.
    pub fn standing_on(&self) -> Option<u64> { self.on }
}

/// The walker's physics body.
#[derive(Component)]
pub struct WalkerBody;

/// How the walker moves on ordinary and on slippery ground.
struct Feel { grip: Handle<MovesConfig>, slippery: Handle<MovesConfig> }

/// Test mode / game state.
///
/// A game's own player character collides through the same physics world (Avian): as a body of its own (the walker
/// is a bevy_tnua character, a good start), or by asking Avian's `SpatialQuery` / `MoveAndSlide`. Moving parts are
/// kinematic bodies ([`PartBody`]) whose `LinearVelocity` carries what stands on them.
#[derive(Resource, Default)]
pub struct Play {
    pub on: bool,
    pub walker: Walker,
    /// What is drawn of the walker, and its body.
    shown: Option<Entity>,
    body: Option<Entity>,
    feel: Option<Feel>,
    /// The walker's body for this run (from the level's walker profile).
    size: Body,
    /// The physics bodies made for this run (see `physics.rs`).
    bodies: Vec<Entity>,
    /// Where the moving and loose parts are drawn when not playing.
    homes: HashMap<u64, Transform>,
    /// Slippery parts (surface tag).
    slippery: HashSet<u64>,
    /// Loose parts: their speed last step, and when each last made a Hit.
    pub(crate) loose: HashMap<u64, (Vec3, f32)>,
    clock: f32,
    /// E pressed since the last step (keys are read every frame, play moves on the fixed step).
    pub(crate) press: bool,
    /// A game's own player character plays instead of the walker: its plugin sets this to its controls hint,
    /// then runs while `on`, starting afresh whenever `run` changes, from `start`, and moves the `PlayCamera`.
    pub custom: Option<&'static str>,
    /// Counts the starts.
    pub run: u32,
    /// This run's start: feet, facing (yaw).
    pub start: (Vec3, f32),
    /// Where the player's feet are (a custom character sets this each frame; buttons are pressed from here).
    pub feet: Vec3,
    /// Buttons, wires and moving parts.
    pub script: Script,
    /// Finds paths on the level's navigation area (worked out when play starts) for characters that walk on their own.
    pub nav: Option<crate::nav::Finder>,
    /// The game's controls for this run (the project's, see `input.rs`).
    pub controls: Controls,
}

#[derive(Component)]
pub struct PlayCamera;

pub struct PlayPlugin;

impl Plugin for PlayPlugin {
    fn build(&self, app: &mut App) {
        let playing = |p: Res<Play>| p.on;
        let walking = |p: Res<Play>| p.on && p.custom.is_none();
        app.init_resource::<Play>()
            .add_systems(PreUpdate, hand_controls.before(crate::input::read))
            .add_plugins((crate::input::InputMapPlugin, crate::physics::PhysicsPlugin, TnuaControllerPlugin::<Moves>::new(PhysicsSchedule), TnuaAvian3dPlugin::new(PhysicsSchedule)))
            .add_systems(Startup, feel)
            .add_systems(Update, (keys.run_if(playing), follow.run_if(walking)))
            .add_systems(FixedUpdate, (fit.run_if(walking), sense.run_if(walking), knocks.run_if(playing), crate::script::run.run_if(playing), crate::script::gravity.run_if(playing), controls.run_if(walking)).chain());
    }
}

/// The walker's two ways of moving: with grip, and on slippery ground (slow to speed up and to stop).
fn feel(mut play: ResMut<Play>, mut configs: ResMut<Assets<MovesConfig>>) {
    let (grip, slippery) = moves(play.size);
    play.feel = Some(Feel { grip: configs.add(grip), slippery: configs.add(slippery) });
}

/// The two ways of moving, for a body of this size.
fn moves(b: Body) -> (MovesConfig, MovesConfig) {
    let walk = TnuaBuiltinWalkConfig { speed: 1.0, float_height: b.float, cling_distance: 0.3, max_slope: b.slope, ..default() };
    let config = |walk: TnuaBuiltinWalkConfig| MovesConfig { basis: walk, jump: TnuaBuiltinJumpConfig { height: JUMP, ..default() } };
    let slippery = TnuaBuiltinWalkConfig { acceleration: 4.0, air_acceleration: 4.0, ..walk.clone() };
    (config(walk), config(slippery))
}

/// The ways of moving follow the walker's body (a level with its own walker profile).
fn fit(play: Res<Play>, mut configs: ResMut<Assets<MovesConfig>>, mut fitted: Local<Option<Body>>) {
    let Some(feel) = play.feel.as_ref() else { return };
    if *fitted == Some(play.size) { return; }
    let (grip, slippery) = moves(play.size);
    if let Some(mut c) = configs.get_mut(&feel.grip) { *c = grip; }
    if let Some(mut c) = configs.get_mut(&feel.slippery) { *c = slippery; }
    *fitted = Some(play.size);
}

impl Play {
    /// Start playing `level` with the walker standing at `feet`, facing `yaw`.
    #[allow(clippy::too_many_arguments)]
    pub fn start(&mut self, commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, level: &Level, geo: &Geometry, feet: Vec3, yaw: f32) {
        self.stop(commands);
        self.script = Script::new(level);
        self.controls = (*level.controls).clone();
        self.bodies = crate::physics::spawn(commands, level, geo);
        self.nav = crate::nav::build(level, geo).ok().and_then(|m| crate::nav::Finder::new(&m, &level.navigation).ok());
        self.homes = level.parts.iter().filter(|p| crate::physics::kind(p).is_some_and(|k| !k.is_static()) || level.carrier(p.id).is_some()).map(|p| (p.id, part_transform(p))).collect();
        self.slippery = level.parts.iter().filter(|p| p.tags.contains(&Tag::Slippery)).map(|p| p.id).collect();
        self.loose = level.parts.iter().filter(|p| p.loose.is_some()).map(|p| (p.id, (Vec3::ZERO, -1.0))).collect();
        self.clock = 0.0;
        self.press = false;
        self.feet = feet;
        self.run += 1;
        self.start = (feet, yaw);
        self.on = true;
        if self.custom.is_some() { return; }
        self.size = Body::of(level.profile());
        let size = self.size;
        self.walker = Walker { pos: feet, facing: yaw, yaw, pitch: -0.25, ..default() };
        let grip = self.feel.as_ref().map(|f| f.grip.clone()).unwrap_or_default();
        self.body = Some(commands.spawn((
            WalkerBody,
            RigidBody::Dynamic,
            Collider::capsule(size.radius, size.length),
            LockedAxes::ROTATION_LOCKED,
            GravityScale(GRAVITY / 9.81),
            Mass(80.0),
            TnuaController::<Moves>::default(),
            TnuaConfig::<Moves>(grip),
            TnuaAvian3dSensorShape(Collider::cylinder(size.radius - 0.01, 0.0)),
            Transform::from_translation(feet + Vec3::Y * size.float),
            TransformInterpolation,
        )).id());
        let body_mat = mats.add(StandardMaterial { base_color: Color::srgb(0.85, 0.82, 0.78), perceptual_roughness: 0.6, ..default() });
        let visor = mats.add(StandardMaterial { base_color: Color::srgb(0.96, 0.86, 0.29), emissive: LinearRgba::rgb(0.6, 0.5, 0.1), ..default() });
        let shown = commands
            .spawn((Transform::from_translation(feet), Visibility::Visible))
            .with_children(|b| {
                b.spawn((Mesh3d(meshes.add(Capsule3d::new(size.radius, size.height - 2.0 * size.radius))), MeshMaterial3d(body_mat), Transform::from_xyz(0.0, size.height * 0.5, 0.0)));
                b.spawn((Mesh3d(meshes.add(Cuboid::new(0.5, 0.14, 0.12))), MeshMaterial3d(visor), Transform::from_xyz(0.0, 1.45, -0.36)));
            })
            .id();
        self.shown = Some(shown);
    }

    pub fn stop(&mut self, commands: &mut Commands) {
        for e in [self.shown.take(), self.body.take()].into_iter().flatten() { commands.entity(e).despawn(); }
        for e in self.bodies.drain(..) { commands.entity(e).try_despawn(); }
        // what moved goes back where the level has it
        let homes = std::mem::take(&mut self.homes);
        if !homes.is_empty() {
            commands.queue(move |w: &mut World| {
                let mut shown = w.query_filtered::<(&PartId, &mut Transform), Without<crate::physics::PartBody>>();
                for (id, mut t) in shown.iter_mut(w) {
                    if let Some(h) = homes.get(&id.0) { *t = *h; }
                }
            });
        }
        self.script.reset();
        self.on = false;
    }
}

/// The input map reads the controls of this run.
fn hand_controls(play: Res<Play>, mut actions: ResMut<Actions>) {
    if play.is_changed() && actions.controls != play.controls { actions.controls = play.controls.clone(); }
}

/// How fast the look actions (a gamepad's right stick) turn the view, in radians a second when fully over.
const LOOK: f32 = 2.6;

/// Actions that act once (Use) are caught every frame and used on the next step; the mouse and the look actions turn
/// the view.
fn keys(mut play: ResMut<Play>, actions: Res<Actions>, motion: Res<AccumulatedMouseMotion>, time: Res<Time>) {
    if actions.just_pressed("Use") { play.press = true; }
    let look = actions.stick("Look left", "Look right", "Look down", "Look up") * LOOK * time.delta_secs();
    if play.custom.is_none() && (motion.delta != Vec2::ZERO || look != Vec2::ZERO) {
        let w = &mut play.walker;
        w.yaw -= motion.delta.x * 0.0025 + look.x;
        w.pitch = (w.pitch - motion.delta.y * 0.0025 + look.y).clamp(-1.2, 0.9);
    }
}

/// Where the move actions point, on the ground, from the camera's yaw: as long as the stick leans (keys are all the way).
fn walk_dir(actions: &Actions, yaw: f32) -> Vec3 {
    let s = actions.stick("Left", "Right", "Back", "Forward");
    Quat::from_rotation_y(yaw) * Vec3::new(s.x, 0.0, -s.y)
}

/// At the start of each step: where the walker's body got to and what it stands on; a Step each stride on the
/// ground, Jump on leaving it upward and Land on coming down.
fn sense(mut play: ResMut<Play>, time: Res<Time>, body: Query<(&Position, &LinearVelocity, &TnuaController<Moves>), With<WalkerBody>>, parts: Query<&PartBody>) {
    let Ok((at, v, c)) = body.single() else { return };
    let play = &mut *play;
    let w = &mut play.walker;
    let (was_ground, was_vel) = (w.ground, w.vel);
    w.pos = at.0 - Vec3::Y * play.size.float;
    w.vel = v.0;
    w.ground = c.is_airborne().is_ok_and(|air| !air);
    w.on = c.basis_memory.standing_on_entity().and_then(|e| parts.get(e).ok()).map(|b| b.0);
    play.feet = w.pos;
    let (pos, on) = (w.pos, w.on);
    let mut happen = |what: &str| play.script.happened.push(crate::script::Happened { what: what.into(), part: None, at: Some(pos), on });
    if w.ground && !was_ground && was_vel.y < -3.0 { happen("Land"); w.stride = 0.0; }
    if !w.ground && was_ground && w.vel.y > 1.5 { happen("Jump"); }
    if w.ground {
        w.stride += w.vel.with_y(0.0).length() * time.delta_secs();
        if w.stride >= STRIDE { w.stride -= STRIDE; happen("Step"); }
    } else {
        // the first step after landing comes soon
        w.stride = STRIDE * 0.6;
    }
}

/// Loose parts that knock into something (their speed changes sharply in one step) make a Hit.
fn knocks(mut play: ResMut<Play>, time: Res<Time>, bodies: Query<(&PartBody, &LinearVelocity, &Position)>) {
    let play = &mut *play;
    play.clock += time.delta_secs();
    for (b, v, at) in &bodies {
        let Some((last, hit)) = play.loose.get_mut(&b.0) else { continue };
        if (v.0 - *last).length() > KNOCK && play.clock - *hit > KNOCK_AGAIN {
            *hit = play.clock;
            play.script.happened.push(crate::script::Happened { what: "Hit".into(), part: Some(b.0), at: Some(at.0), on: None });
        }
        *last = v.0;
    }
}

/// The walker's controls for this step: walk where the move actions say (Run runs), jump while Jump is held;
/// slippery ground gives less grip.
fn controls(play: Res<Play>, actions: Res<Actions>, mut body: Query<(&mut TnuaController<Moves>, &mut TnuaConfig<Moves>), With<WalkerBody>>) {
    let Ok((mut c, mut config)) = body.single_mut() else { return };
    let w = &play.walker;
    let dir = walk_dir(&actions, w.yaw);
    let speed = if actions.pressed("Run") { RUN } else { WALK };
    c.basis = TnuaBuiltinWalk { desired_motion: dir * speed, desired_forward: None };
    c.initiate_action_feeding();
    if actions.pressed("Jump") { c.action(Moves::Jump(default())); }
    if let Some(feel) = play.feel.as_ref() {
        let want = if w.on.is_some_and(|id| play.slippery.contains(&id)) { &feel.slippery } else { &feel.grip };
        if config.0 != *want { config.0 = want.clone(); }
    }
}

fn follow(mut play: ResMut<Play>, actions: Res<Actions>, ray: SpatialQuery, body: Query<&Transform, (With<WalkerBody>, Without<PlayCamera>)>, mut shown: Query<&mut Transform, (Without<WalkerBody>, Without<PlayCamera>)>, mut cam: Query<&mut Transform, (With<PlayCamera>, Without<WalkerBody>)>) {
    let Some(b) = play.body else { return };
    // the body as drawn (smoothed between steps); facing the way it walks
    let Ok(at) = body.get(b).map(|t| t.translation - Vec3::Y * play.size.float) else { return };
    let w = &mut play.walker;
    let dir = walk_dir(&actions, w.yaw);
    if dir != Vec3::ZERO { w.facing = (-dir.x).atan2(-dir.z); }
    if let Some(Ok(mut t)) = play.shown.map(|s| shown.get_mut(s)) {
        t.translation = at;
        t.rotation = Quat::from_rotation_y(play.walker.facing);
    }
    let w = &play.walker;
    let rot = Quat::from_euler(EulerRot::YXZ, w.yaw, w.pitch, 0.0);
    let target = at + Vec3::Y * EYE;
    let back = rot * Vec3::Z;
    let not_me = SpatialQueryFilter::from_excluded_entities([b]);
    let dist = Dir3::new(back).ok().and_then(|d| ray.cast_ray(target, d, 5.0, true, &not_me)).map(|h| (h.distance - 0.3).max(0.6)).unwrap_or(5.0);
    for mut t in &mut cam {
        t.translation = target + back * dist;
        t.rotation = rot;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_walkers_body_is_the_walker_profile() {
        let check = |p: &Navigation| {
            let b = Body::of(p);
            let bottom = b.float - b.length * 0.5 - b.radius;
            assert!((bottom - p.step).abs() < 1e-5, "it floats a step up, so it climbs that and no higher: {b:?}");
            assert!((b.float + b.length * 0.5 + b.radius - p.height).abs() < 1e-5, "its top is the profile's height: {b:?}");
            assert!((2.0 * b.radius - p.width).abs() < 1e-5 && (b.slope - p.slope.to_radians()).abs() < 1e-5, "{b:?}");
            let (grip, slippery) = moves(b);
            assert_eq!((grip.basis.float_height, grip.basis.max_slope), (b.float, b.slope));
            assert_eq!((slippery.basis.float_height, slippery.basis.max_slope), (b.float, b.slope));
        };
        check(&Navigation::default());
        check(&Navigation { width: 1.2, height: 2.4, step: 0.3, slope: 30.0 });
    }
}
