//! The Audio tab's world: its listening stage (a speaker on a plinth, and a walker that steps around it on the
//! chosen surface), hiding the level while the tab is open, and each tab keeping its own view. Its panels are in
//! `audio_ui.rs`.
use super::edit::PartView;
use super::view::View;
use super::Editor;
use crate::level::{Ground, Plays, Shape, Speaker};
use crate::shapes::ShapeLib;
use crate::sound::Sounds;
use bevy::prelude::*;

/// The tabs (indexes into `theme::TABS`) that are built.
pub const LEVEL: usize = 0;
pub const AUDIO: usize = 4;
pub fn built(tab: usize) -> bool { tab == LEVEL || tab == AUDIO }

/// The Audio tab's topics: name, icon, and when one that isn't built comes.
pub const TOPICS: [(&str, &str, &str); 5] = [
    ("Sounds", "wave", ""),
    ("Music", "music", "Coming soon: music comes with the timeline"),
    ("Ambience", "wind", "Coming soon: ambience comes with zones"),
    ("Mixer", "sliders", "Coming soon: the mixer comes after sounds"),
    ("Voices", "caption", "Coming soon: voices come with dialog"),
];

/// The stage's speaker and walker, as the sounds know them.
pub const SPEAKER: u64 = u64::MAX - 1;
pub const WALKER: u64 = u64::MAX - 2;
/// Where the speaker is (the top of the plinth) and how far from it the walker goes round.
pub const SPEAKER_AT: Vec3 = Vec3::new(0.0, 1.25, 0.0);
const WALK_R: f32 = 3.0;
/// Seconds between the walker's steps, and between plays of a sound that doesn't loop.
const STEP: f32 = 0.45;
const AGAIN: f32 = 1.6;

/// What the stage is doing.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Stage {
    /// The sound plays from the walker's feet (one step each time) instead of the speaker.
    pub walker: bool,
    /// What the walker walks on (None: the sound's usual recordings).
    pub ground: Option<Ground>,
    /// Playing: a loop plays, a sound that doesn't loop plays again and again, the walker walks.
    pub playing: bool,
    /// How far round the walker is (radians).
    pub turn: f32,
}

#[derive(Component)]
pub(super) struct StagePiece { walker: bool }

/// The stage's speaker and walker, as `Sounds::step` takes them: the speaker plays the open sound when it loops.
pub fn speakers(ed: &Editor) -> Vec<(u64, Speaker)> {
    let Some(open) = ed.sound_open.clone().filter(|p| ed.level.sounds.get(p).is_some_and(|s| s.looped)) else { return vec![] };
    if !ed.stage.playing || ed.stage.walker { return vec![]; }
    vec![(SPEAKER, Speaker { sound: open, plays: Plays::OnStart })]
}

/// Where the walker's feet are (it starts at the front left, in sight of the view the tab opens with).
pub fn walker_at(turn: f32) -> Vec3 {
    let a = turn + 2.2;
    Vec3::new(a.cos() * WALK_R, 0.05, a.sin() * WALK_R)
}

/// Where a stage piece is, for the sounds.
pub fn at(ed: &Editor, id: u64) -> Option<Vec3> {
    match id {
        SPEAKER => Some(SPEAKER_AT),
        WALKER => Some(walker_at(ed.stage.turn)),
        _ => None,
    }
}

/// The stage: made once, shown only in the Audio tab; the walker walks while playing, a step's sound on each step,
/// and a sound that doesn't loop plays again from the speaker every so often.
#[allow(clippy::too_many_arguments)]
pub fn stage(
    mut commands: Commands,
    mut ed: ResMut<Editor>,
    mut sounds: ResMut<Sounds>,
    time: Res<Time<Real>>,
    lib: Res<ShapeLib>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut pieces: Query<(&StagePiece, &mut Transform, &mut Visibility)>,
    mut made: Local<bool>,
    mut clock: Local<f32>,
) {
    if !*made {
        *made = true;
        let grey = mats.add(StandardMaterial { base_color: Color::srgb(0.34, 0.34, 0.37), perceptual_roughness: 0.9, ..default() });
        commands.spawn((StagePiece { walker: false }, Mesh3d(meshes.add(Cylinder::new(0.55, 1.0))), MeshMaterial3d(grey), Transform::from_xyz(0.0, 0.5, 0.0), Visibility::Hidden));
        // the speaker shows its side to the view the tab opens with, turned a little
        let blue = mats.add(StandardMaterial { base_color: Color::srgb_u8(0x3D, 0x7B, 0xFF), unlit: true, ..default() });
        commands.spawn((
            StagePiece { walker: false }, Mesh3d(lib.mesh(Shape::Sound)), MeshMaterial3d(blue),
            Transform::from_translation(SPEAKER_AT).with_rotation(Quat::from_rotation_y(0.35)).with_scale(Vec3::splat(0.5)),
            Visibility::Hidden,
        ));
        let body = mats.add(StandardMaterial { base_color: Color::srgb(0.78, 0.78, 0.82), perceptual_roughness: 0.8, ..default() });
        commands.spawn((StagePiece { walker: true }, Mesh3d(meshes.add(Capsule3d::new(0.28, 1.1))), MeshMaterial3d(body), Transform::default(), Visibility::Hidden));
    }
    let open = ed.tab == super::audio::AUDIO;
    let sound = ed.sound_open.clone().filter(|p| ed.level.sounds.contains_key(p));
    let playing = open && ed.stage.playing && sound.is_some();
    if playing {
        let dt = time.delta_secs();
        *clock += dt;
        if ed.stage.walker {
            ed.bypass_change_detection().stage.turn += dt * 1.4 / WALK_R;
            if *clock >= STEP {
                *clock = 0.0;
                sounds.cue(WALKER, sound.clone().unwrap(), ed.stage.ground);
            }
        } else if !ed.level.sounds[sound.as_ref().unwrap()].looped && *clock >= AGAIN {
            *clock = 0.0;
            sounds.cue(SPEAKER, sound.clone().unwrap(), None);
        }
    } else {
        // the first play comes right away
        *clock = f32::MAX / 2.0;
    }
    for (piece, mut tf, mut vis) in &mut pieces {
        let show = open && (!piece.walker || ed.stage.walker);
        *vis = if show { Visibility::Inherited } else { Visibility::Hidden };
        if piece.walker {
            let feet = walker_at(ed.stage.turn);
            // facing along the circle
            *tf = Transform::from_translation(feet + Vec3::Y * 0.83).with_rotation(Quat::from_rotation_y(-ed.stage.turn - 2.2));
        }
    }
}

/// Changing tabs: the level is hidden outside the Level tab, its selection kept for when it comes back, and each
/// tab has its own view (the Audio tab's starts looking at the stage).
pub fn follow_tab(mut ed: ResMut<Editor>, mut view: ResMut<View>, mut parts: Query<&mut Visibility, With<PartView>>, mut last: Local<usize>, mut views: Local<[Option<(Vec3, f32, f32)>; 8]>, mut chosen: Local<Option<u64>>) {
    let tab = ed.tab;
    if tab != *last {
        views[*last] = Some((view.pos, view.yaw, view.pitch));
        let (pos, yaw, pitch) = views[tab].unwrap_or(match tab {
            AUDIO => (Vec3::new(0.0, 2.6, 7.0), 0.0, -0.2),
            _ => (View::default().pos, View::default().yaw, View::default().pitch),
        });
        (view.pos, view.yaw, view.pitch) = (pos, yaw, pitch);
        if *last == LEVEL { *chosen = ed.selected.take(); ed.placing = None; ed.parts_open = false; }
        if tab == LEVEL { ed.selected = chosen.take().filter(|id| ed.level.get(*id).is_some()); }
        *last = tab;
    }
    let show = if tab == LEVEL { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut parts {
        if *v != show { *v = show; }
    }
}

/// The stage's lines: how far the open sound is heard at full volume and how far it carries, round the speaker or
/// the walker.
pub fn draw(ed: &Editor, view: &View, g: &mut Gizmos<super::edit::WidgetGizmos>) {
    let Some(s) = ed.sound_open.as_ref().and_then(|p| ed.level.sounds.get(p)) else { return };
    let c = if ed.stage.walker { walker_at(ed.stage.turn) } else { SPEAKER_AT };
    let col = Color::srgba_u8(0xFF, 0x7A, 0x59, 170);
    for r in [s.near, s.far] {
        g.circle(Isometry3d::new(c.with_y(0.03), Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), r, col).resolution(96);
        g.circle(Isometry3d::new(c, Quat::from_rotation_arc(Vec3::Z, -view.forward())), r, col.with_alpha(0.35)).resolution(96);
    }
}

/// Run only in the Level tab (placing, selecting, test mode).
pub fn in_level(ed: Res<Editor>) -> bool { ed.tab == LEVEL }
