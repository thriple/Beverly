//! Playing a level: the plain walker (a stand-in character until the real characters exist), a floating character
//! (bevy_tnua) in the physics world (`physics.rs`): it walks up steps and slopes and rides moving parts. Used by the
//! editor's test mode (`) and by the game player.
use crate::level::{Level, Tag};
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

const RADIUS: f32 = 0.4;
/// The walker's body: a capsule whose middle floats this high above the ground (its bottom 0.45 m up, so it walks
/// up steps that high).
const FLOAT: f32 = 1.1;
const LENGTH: f32 = 0.5;
const EYE: f32 = 1.6;
const GRAVITY: f32 = 22.0;
const JUMP: f32 = 1.45;
const WALK: f32 = 5.0;
const RUN: f32 = 9.0;

/// What the walker can do: walk (always), and jump.
#[derive(TnuaScheme)]
#[scheme(basis = TnuaBuiltinWalk)]
pub enum Moves {
    Jump(TnuaBuiltinJump),
}

/// The walker as the rest of Beverly sees it: feet, velocity, facing, the camera's yaw / pitch, whether it stands on
/// something and on which part. Read from its body each step.
#[derive(Default, Clone)]
pub struct Walker { pub pos: Vec3, pub vel: Vec3, pub facing: f32, pub yaw: f32, pub pitch: f32, pub ground: bool, on: Option<u64> }

impl Walker {
    /// The part it stands on.
    pub fn standing_on(&self) -> Option<u64> { self.on }
}

/// The walker's physics body.
#[derive(Component)]
struct WalkerBody;

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
    /// The physics bodies made for this run (see `physics.rs`).
    bodies: Vec<Entity>,
    /// Where the moving and loose parts are drawn when not playing.
    homes: HashMap<u64, Transform>,
    /// Slippery parts (surface tag).
    slippery: HashSet<u64>,
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
}

#[derive(Component)]
pub struct PlayCamera;

pub struct PlayPlugin;

impl Plugin for PlayPlugin {
    fn build(&self, app: &mut App) {
        let playing = |p: Res<Play>| p.on;
        let walking = |p: Res<Play>| p.on && p.custom.is_none();
        app.init_resource::<Play>()
            .add_plugins((crate::physics::PhysicsPlugin, TnuaControllerPlugin::<Moves>::new(PhysicsSchedule), TnuaAvian3dPlugin::new(PhysicsSchedule)))
            .add_systems(Startup, feel)
            .add_systems(Update, (keys.run_if(playing), follow.run_if(walking)))
            .add_systems(FixedUpdate, (sense.run_if(walking), crate::script::run.run_if(playing), controls.run_if(walking)).chain());
    }
}

/// The walker's two ways of moving: with grip, and on slippery ground (slow to speed up and to stop).
fn feel(mut play: ResMut<Play>, mut configs: ResMut<Assets<MovesConfig>>) {
    let walk = TnuaBuiltinWalkConfig { speed: 1.0, float_height: FLOAT, cling_distance: 0.3, ..default() };
    let config = |walk: TnuaBuiltinWalkConfig| MovesConfig { basis: walk, jump: TnuaBuiltinJumpConfig { height: JUMP, ..default() } };
    let slippery = TnuaBuiltinWalkConfig { acceleration: 4.0, air_acceleration: 4.0, ..walk.clone() };
    play.feel = Some(Feel { grip: configs.add(config(walk)), slippery: configs.add(config(slippery)) });
}

impl Play {
    /// Start playing `level` with the walker standing at `feet`, facing `yaw`.
    #[allow(clippy::too_many_arguments)]
    pub fn start(&mut self, commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, level: &Level, geo: &Geometry, feet: Vec3, yaw: f32) {
        self.stop(commands);
        self.script = Script::new(level);
        self.bodies = crate::physics::spawn(commands, level, geo);
        self.homes = level.parts.iter().filter(|p| crate::physics::kind(p).is_some_and(|k| !k.is_static())).map(|p| (p.id, part_transform(p))).collect();
        self.slippery = level.parts.iter().filter(|p| p.tags.contains(&Tag::Slippery)).map(|p| p.id).collect();
        self.press = false;
        self.feet = feet;
        self.run += 1;
        self.start = (feet, yaw);
        self.on = true;
        if self.custom.is_some() { return; }
        self.walker = Walker { pos: feet, facing: yaw, yaw, pitch: -0.25, ..default() };
        let grip = self.feel.as_ref().map(|f| f.grip.clone()).unwrap_or_default();
        self.body = Some(commands.spawn((
            WalkerBody,
            RigidBody::Dynamic,
            Collider::capsule(RADIUS, LENGTH),
            LockedAxes::ROTATION_LOCKED,
            GravityScale(GRAVITY / 9.81),
            Mass(80.0),
            TnuaController::<Moves>::default(),
            TnuaConfig::<Moves>(grip),
            TnuaAvian3dSensorShape(Collider::cylinder(RADIUS - 0.01, 0.0)),
            Transform::from_translation(feet + Vec3::Y * FLOAT),
            TransformInterpolation,
        )).id());
        let body_mat = mats.add(StandardMaterial { base_color: Color::srgb(0.85, 0.82, 0.78), perceptual_roughness: 0.6, ..default() });
        let visor = mats.add(StandardMaterial { base_color: Color::srgb(0.96, 0.86, 0.29), emissive: LinearRgba::rgb(0.6, 0.5, 0.1), ..default() });
        let shown = commands
            .spawn((Transform::from_translation(feet), Visibility::Visible))
            .with_children(|b| {
                b.spawn((Mesh3d(meshes.add(Capsule3d::new(RADIUS, 1.0))), MeshMaterial3d(body_mat), Transform::from_xyz(0.0, 0.9, 0.0)));
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

/// Keys that act once (E) are caught every frame and used on the next step; the mouse turns the view.
fn keys(mut play: ResMut<Play>, keys: Res<ButtonInput<KeyCode>>, motion: Res<AccumulatedMouseMotion>) {
    if keys.just_pressed(KeyCode::KeyE) { play.press = true; }
    if play.custom.is_none() && motion.delta != Vec2::ZERO {
        let w = &mut play.walker;
        w.yaw -= motion.delta.x * 0.0025;
        w.pitch = (w.pitch - motion.delta.y * 0.0025).clamp(-1.2, 0.9);
    }
}

/// At the start of each step: where the walker's body got to, and what it stands on.
fn sense(mut play: ResMut<Play>, body: Query<(&Position, &LinearVelocity, &TnuaController<Moves>), With<WalkerBody>>, parts: Query<&PartBody>) {
    let Ok((at, v, c)) = body.single() else { return };
    let w = &mut play.walker;
    w.pos = at.0 - Vec3::Y * FLOAT;
    w.vel = v.0;
    w.ground = c.is_airborne().is_ok_and(|air| !air);
    w.on = c.basis_memory.standing_on_entity().and_then(|e| parts.get(e).ok()).map(|b| b.0);
    play.feet = w.pos;
}

/// The walker's controls for this step: walk where the keys say (Shift runs), jump while Space is held; slippery
/// ground gives less grip.
fn controls(play: Res<Play>, keys: Res<ButtonInput<KeyCode>>, mut body: Query<(&mut TnuaController<Moves>, &mut TnuaConfig<Moves>), With<WalkerBody>>) {
    let Ok((mut c, mut config)) = body.single_mut() else { return };
    let w = &play.walker;
    let rot = Quat::from_rotation_y(w.yaw);
    let (f, r) = (rot * Vec3::NEG_Z, rot * Vec3::X);
    let mut dir = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) { dir += f; }
    if keys.pressed(KeyCode::KeyS) { dir -= f; }
    if keys.pressed(KeyCode::KeyD) { dir += r; }
    if keys.pressed(KeyCode::KeyA) { dir -= r; }
    let dir = dir.normalize_or_zero();
    let speed = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) { RUN } else { WALK };
    c.basis = TnuaBuiltinWalk { desired_motion: dir * speed, desired_forward: None };
    c.initiate_action_feeding();
    if keys.pressed(KeyCode::Space) { c.action(Moves::Jump(default())); }
    if let Some(feel) = play.feel.as_ref() {
        let want = if w.on.is_some_and(|id| play.slippery.contains(&id)) { &feel.slippery } else { &feel.grip };
        if config.0 != *want { config.0 = want.clone(); }
    }
}

fn follow(mut play: ResMut<Play>, keys: Res<ButtonInput<KeyCode>>, ray: SpatialQuery, body: Query<&Transform, (With<WalkerBody>, Without<PlayCamera>)>, mut shown: Query<&mut Transform, (Without<WalkerBody>, Without<PlayCamera>)>, mut cam: Query<&mut Transform, (With<PlayCamera>, Without<WalkerBody>)>) {
    let Some(b) = play.body else { return };
    // the body as drawn (smoothed between steps); facing the way it walks
    let Ok(at) = body.get(b).map(|t| t.translation - Vec3::Y * FLOAT) else { return };
    let w = &mut play.walker;
    let rot = Quat::from_rotation_y(w.yaw);
    let mut dir = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) { dir += rot * Vec3::NEG_Z; }
    if keys.pressed(KeyCode::KeyS) { dir -= rot * Vec3::NEG_Z; }
    if keys.pressed(KeyCode::KeyD) { dir += rot * Vec3::X; }
    if keys.pressed(KeyCode::KeyA) { dir -= rot * Vec3::X; }
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
