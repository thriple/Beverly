//! The two ways of moving around, switched with Caps Lock:
//! - Editor movement: free cursor; middle-drag orbits the point under the reticle, hold Space and move the mouse
//!   to pan, the wheel zooms toward the point under the cursor (each notch closes a share of the distance).
//! - Builder movement: no-clip flight; the mouse looks, W/A/S/D move where you look, no speed ramp. Mouse 5 / ]
//!   faster, Mouse 4 / [ slower through 12 speeds. Hold Alt for a cursor.
use super::{Editor, theme};
use bevy::camera::Viewport;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use bevy::camera::{CameraOutputMode, visibility::RenderLayers};
use bevy::render::render_resource::BlendState;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{EguiGlobalSettings, PrimaryEguiContext};

pub const SPEEDS: [f32; 12] = [0.125, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 400.0];
pub const SPEED_LABELS: [&str; 12] = ["⅛", "¼", "½", "1×", "2", "5", "10", "20", "50", "100", "200", "400"];
const BASE_SPEED: f32 = 6.0; // metres per second at 1×

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode { Editor, Builder }

#[derive(Resource)]
pub struct View {
    pub mode: Mode,
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub speed: usize,
    /// When the speed last changed (seconds), to show the speed strip for a moment.
    pub speed_changed: f64,
    /// The 3D viewport in logical window pixels (the area the panels leave free).
    pub rect: Rect,
    /// Cursor in logical window pixels, if it is in the window.
    pub cursor: Option<Vec2>,
    /// The pointer is over a panel or a pop-up, so the 3D view ignores it.
    pub over_ui: bool,
    pub keyboard_in_ui: bool,
    orbit_pivot: Option<Vec3>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            mode: Mode::Editor,
            pos: vec3(14.0, 10.0, 14.0),
            yaw: std::f32::consts::FRAC_PI_4,
            pitch: -0.5,
            speed: 3,
            speed_changed: -10.0,
            rect: Rect::new(0.0, 0.0, 1280.0, 720.0),
            cursor: None,
            over_ui: false,
            keyboard_in_ui: false,
            orbit_pivot: None,
        }
    }
}

impl View {
    pub fn rot(&self) -> Quat { Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0) }
    pub fn forward(&self) -> Vec3 { self.rot() * Vec3::NEG_Z }
    /// Where clicks happen: the cursor in Editor movement, the reticle (viewport centre) in Builder movement.
    pub fn pointer(&self) -> Option<Vec2> {
        match self.mode {
            Mode::Builder => Some(self.rect.center()),
            Mode::Editor => self.cursor.filter(|c| self.rect.contains(*c) && !self.over_ui),
        }
    }
    /// A window point (logical) to a ray through the camera. (Bevy's viewport functions take and give window
    /// coordinates, the viewport's offset included.)
    pub fn ray(&self, cam: &Camera, tf: &GlobalTransform, p: Vec2) -> Option<Ray3d> { cam.viewport_to_world(tf, p).ok() }
    pub fn to_screen(&self, cam: &Camera, tf: &GlobalTransform, w: Vec3) -> Option<Vec2> { cam.world_to_viewport(tf, w).ok() }
}

#[derive(Component)]
pub struct MainCamera;

pub fn setup(mut commands: Commands, mut egui_settings: ResMut<EguiGlobalSettings>) {
    let v = View::default();
    // BEVERLY_LITE=1: no anti-aliasing (for slow software renderers, e.g. automated tests)
    let msaa = if std::env::var_os("BEVERLY_LITE").is_some() { Msaa::Off } else { Msaa::Sample4 };
    commands.spawn((Camera3d::default(), MainCamera, crate::play::PlayCamera, Transform::from_translation(v.pos).with_rotation(v.rot()), msaa));
    commands.insert_resource(v);
    // The panels get their own full-window camera, drawn over the 3D view (whose viewport shrinks to the free area).
    egui_settings.auto_create_primary_context = false;
    commands.spawn((
        PrimaryEguiContext,
        Camera2d,
        RenderLayers::none(),
        Camera {
            order: 1,
            output_mode: CameraOutputMode::Write { blend_state: Some(BlendState::ALPHA_BLENDING), clear_color: ClearColorConfig::None },
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
    ));
}

#[allow(clippy::too_many_arguments)]
pub fn input(
    mut view: ResMut<View>,
    mut ed: ResMut<Editor>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time<Real>>,
    egui_in: Res<EguiWantsInput>,
    window: Single<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    cam: Single<(&Camera, &GlobalTransform, &mut Transform), With<MainCamera>>,
    mut caps: ResMut<super::caps::CapsLock>,
) {
    let (window, mut cursor_opts) = window.into_inner();
    let (cam, gtf, mut tf) = cam.into_inner();
    view.cursor = window.cursor_position();
    view.keyboard_in_ui = egui_in.wants_keyboard_input() || ed.dialog_open();
    // only when the pointer is over a panel or pop-up (not merely held down: drags in the 3D view keep going)
    view.over_ui = egui_in.is_pointer_over_area() || egui_in.is_popup_open() || ed.parts_open || ed.dialog_open();
    let dt = time.delta_secs();
    let now = time.elapsed_secs_f64();
    let typing = view.keyboard_in_ui || ed.pending.is_some();

    // Test mode: the walker has the camera and the mouse.
    if ed.testing.is_some() {
        caps.pressed(&keys);
        if cursor_opts.grab_mode != CursorGrabMode::Locked {
            cursor_opts.grab_mode = CursorGrabMode::Locked;
            cursor_opts.visible = false;
        }
        return;
    }

    // Caps Lock switches modes (and nothing else: see caps.rs).
    if caps.pressed(&keys) && !ed.pending.is_some() {
        view.mode = if view.mode == Mode::Editor { Mode::Builder } else { Mode::Editor };
        view.orbit_pivot = None;
        let label = if view.mode == Mode::Builder { "Builder movement" } else { "Editor movement" };
        let sub = if view.mode == Mode::Builder { "WASD fly · Mouse 4 / 5 speed · Alt for cursor" } else { "Middle-drag orbit · Space pan · wheel zoom" };
        ed.island.show("⇪", label, sub, theme::Tone::Accent);
    }

    // Cursor: captured in Builder movement unless Alt is held or a pop-up is open.
    let free = view.mode == Mode::Editor || keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight) || ed.parts_open || ed.pending.is_some();
    let want = if free { CursorGrabMode::None } else { CursorGrabMode::Locked };
    if cursor_opts.grab_mode != want {
        cursor_opts.grab_mode = want;
        cursor_opts.visible = free;
    }

    // Fly speed: Mouse 5 / ] faster, Mouse 4 / [ slower (in both modes; it's shown in the corner).
    let up = mouse.just_pressed(MouseButton::Forward) || (!typing && keys.just_pressed(KeyCode::BracketRight));
    let down = mouse.just_pressed(MouseButton::Back) || (!typing && keys.just_pressed(KeyCode::BracketLeft));
    if up || down {
        let s = if up { (view.speed + 1).min(SPEEDS.len() - 1) } else { view.speed.saturating_sub(1) };
        view.speed = s;
        view.speed_changed = now;
    }

    match view.mode {
        Mode::Builder => {
            if !free {
                let d = motion.delta;
                view.yaw -= d.x * 0.0025;
                view.pitch = (view.pitch - d.y * 0.0025).clamp(-1.55, 1.55);
            }
            if !typing {
                let mut dir = Vec3::ZERO;
                let (f, r) = (view.forward(), view.rot() * Vec3::X);
                if keys.pressed(KeyCode::KeyW) { dir += f; }
                if keys.pressed(KeyCode::KeyS) { dir -= f; }
                if keys.pressed(KeyCode::KeyD) { dir += r; }
                if keys.pressed(KeyCode::KeyA) { dir -= r; }
                let step = dir.normalize_or_zero() * BASE_SPEED * SPEEDS[view.speed] * dt;
                view.pos += step;
            }
        }
        Mode::Editor => {
            let over = view.over_ui;
            let center = view.rect.center();
            // orbit around the point under the reticle
            if mouse.just_pressed(MouseButton::Middle) && !over {
                view.orbit_pivot = view.ray(cam, gtf, center).map(|r| ed.hit(r).map(|h| h.point).unwrap_or(r.origin + *r.direction * 12.0));
            }
            if !mouse.pressed(MouseButton::Middle) { view.orbit_pivot = None; }
            if let Some(pivot) = view.orbit_pivot {
                let d = motion.delta;
                let dist = view.pos.distance(pivot);
                view.yaw -= d.x * 0.006;
                view.pitch = (view.pitch - d.y * 0.006).clamp(-1.5, 1.5);
                view.pos = pivot - view.forward() * dist;
            }
            // hand pan while Space is held
            if keys.pressed(KeyCode::Space) && !typing {
                let depth = view
                    .ray(cam, gtf, center)
                    .and_then(|r| ed.hit(r))
                    .map(|h| h.t)
                    .unwrap_or(15.0)
                    .clamp(0.5, 5000.0);
                let fov = std::f32::consts::FRAC_PI_4;
                let k = 2.0 * (fov / 2.0).tan() * depth / view.rect.height().max(1.0);
                let d = motion.delta;
                let (r, u) = (view.rot() * Vec3::X, view.rot() * Vec3::Y);
                view.pos += (-r * d.x + u * d.y) * k;
            }
            // proximity zoom toward the point under the cursor
            let notches = match scroll.unit {
                MouseScrollUnit::Line => scroll.delta.y,
                MouseScrollUnit::Pixel => scroll.delta.y / 60.0,
            };
            if notches != 0.0 && !over {
                if let Some(ray) = view.cursor.and_then(|c| view.ray(cam, gtf, c)) {
                    match ed.hit(ray) {
                        Some(h) => {
                            let k = 0.82f32.powf(notches);
                            let to = view.pos - h.point;
                            let mut next = h.point + to * k;
                            if next.distance(h.point) < 0.3 { next = h.point + to.normalize_or_zero() * 0.3; }
                            view.pos = next;
                        }
                        None => view.pos += *ray.direction * notches * 2.0,
                    }
                }
            }
        }
    }
    tf.translation = view.pos;
    tf.rotation = view.rot();
}

/// Point the camera's viewport at the area the panels leave free.
pub fn apply_viewport(view: Res<View>, window: Single<&Window, With<PrimaryWindow>>, mut cam: Single<&mut Camera, With<MainCamera>>) {
    let s = window.scale_factor();
    let r = view.rect;
    let pos = (r.min * s).as_uvec2();
    let size = (r.size() * s).as_uvec2().max(UVec2::ONE);
    let phys = UVec2::new(window.physical_width(), window.physical_height());
    if pos.x + size.x > phys.x || pos.y + size.y > phys.y || phys.x == 0 { return; }
    let vp = Viewport { physical_position: pos, physical_size: size, ..default() };
    if cam.viewport.as_ref().map(|v| (v.physical_position, v.physical_size)) != Some((pos, size)) {
        cam.viewport = Some(vp);
    }
}

/// Ground grid around the camera: 1 m lines, brighter every 10 m, fading out.
pub fn grid(view: Res<View>, mut g: Gizmos) {
    let c = view.pos;
    let reach = (c.y.abs() * 3.0).clamp(30.0, 400.0);
    let step = if reach > 150.0 { 10.0 } else { 1.0 };
    let n = (reach / step) as i32;
    let (cx, cz) = ((c.x / step).round() * step, (c.z / step).round() * step);
    for i in -n..=n {
        let v = i as f32 * step;
        let major = ((cx + v) / 10.0).round() * 10.0 == (cx + v).round() && ((cx + v) % 10.0).abs() < 0.01;
        let a = if major { 0.16 } else { 0.06 };
        g.line(vec3(cx + v, 0.0, cz - reach), vec3(cx + v, 0.0, cz + reach), Color::srgba(1.0, 1.0, 1.0, a));
        let major = ((cz + v) % 10.0).abs() < 0.01;
        let a = if major { 0.16 } else { 0.06 };
        g.line(vec3(cx - reach, 0.0, cz + v), vec3(cx + reach, 0.0, cz + v), Color::srgba(1.0, 1.0, 1.0, a));
    }
}

/// BEVERLY_LITE=1 also logs key presses and the frame time every 20 frames.
pub fn frame_log(time: Res<Time<Real>>, keys: Res<ButtonInput<KeyCode>>, mut n: Local<(u32, f64)>) {
    if std::env::var_os("BEVERLY_LITE").is_none() { return; }
    for k in keys.get_just_pressed() { info!("key {k:?}"); }
    n.0 += 1;
    if n.0 % 20 == 0 {
        let now = time.elapsed_secs_f64();
        info!("frame {}: {:.0} ms per frame", n.0, (now - n.1) * 1000.0 / 20.0);
        n.1 = now;
    }
}
