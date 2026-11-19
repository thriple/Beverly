//! Pictures of models for the Assets panel: each model is shown for a moment on its own, far below the level and lit
//! by a light of its own, photographed into a small image and kept in the bake cache (like every thumbnail, see
//! `thumbs.rs`). One model at a time, a few frames each.
use crate::bake::Bakes;
use crate::models::{ModelLib, ModelRoot, UNTINTED};
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use std::sync::{Arc, Mutex};

/// Where the picture is taken: nothing of the level is anywhere near.
const STAGE: Vec3 = Vec3::new(0.0, -9000.0, 0.0);
/// The picture's sides (pixels), and what is kept (the longest side).
const SIZE: u32 = 128;
const KEPT: u32 = 64;
/// Frames to wait for the model to show before the picture, and for it to load before giving up.
const SETTLE: u32 = 10;
const GIVE_UP: u32 = 600;

/// The models asked for (path, bake key), shared with the Assets panel.
#[derive(Clone, Default)]
pub struct Asked(pub Arc<Mutex<Vec<(String, String)>>>);

enum Job {
    Loading { path: String, key: String, waited: u32 },
    Shooting { key: String, image: Handle<Image>, entities: Vec<Entity>, frames: u32 },
}

#[derive(Resource, Default)]
pub struct Previews { asked: Asked, job: Option<Job> }

impl Previews { pub fn new(asked: Asked) -> Self { Previews { asked, job: None } } }

pub fn run(mut commands: Commands, mut p: ResMut<Previews>, mut lib: ResMut<ModelLib>, server: Res<AssetServer>, bakes: Res<Bakes>, mut images: ResMut<Assets<Image>>) {
    match p.job.take() {
        None => {
            let next = p.asked.0.lock().ok().and_then(|mut q| if q.is_empty() { None } else { Some(q.remove(0)) });
            if let Some((path, key)) = next {
                lib.request(&server, &path);
                p.job = Some(Job::Loading { path, key, waited: 0 });
            }
        }
        Some(Job::Loading { path, key, waited }) => {
            let Some(geo) = lib.geo(&path) else {
                if waited < GIVE_UP { p.job = Some(Job::Loading { path, key, waited: waited + 1 }); }
                return;
            };
            let (lo, hi) = geo.bounds();
            let centre = (lo + hi) * 0.5;
            let radius = ((hi - lo).length() * 0.5).max(0.05);
            let dir = Vec3::new(1.0, 0.75, 1.2).normalize();
            let dist = radius * 2.8;
            let image = images.add(Image::new_target_texture(SIZE, SIZE, TextureFormat::Rgba8UnormSrgb, None));
            let model = commands.spawn((Transform::from_translation(STAGE - centre), Visibility::default(), ModelRoot { path, tint: UNTINTED })).id();
            let camera = commands.spawn((
                Camera3d::default(),
                Camera { order: -5, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
                RenderTarget::Image(image.clone().into()),
                Msaa::Off,
                AmbientLight { color: Color::srgb(0.85, 0.88, 1.0), brightness: 400.0, ..default() },
                Transform::from_translation(STAGE + dir * dist).looking_at(STAGE, Vec3::Y),
            )).id();
            // a point light reaches only what is near it, so the level is left as it was
            let light = commands.spawn((
                PointLight { intensity: 6000.0 * 4.0 * std::f32::consts::PI * (dist * 1.2).powi(2), range: dist * 6.0, shadow_maps_enabled: false, ..default() },
                Transform::from_translation(STAGE + Vec3::new(1.0, 1.4, 0.6).normalize() * dist * 1.2),
            )).id();
            p.job = Some(Job::Shooting { key, image, entities: vec![model, camera, light], frames: 0 });
        }
        Some(Job::Shooting { key, image, entities, frames }) => {
            if frames == SETTLE {
                let bakes = bakes.clone();
                let key2 = key.clone();
                commands.spawn(Screenshot::image(image.clone())).observe(move |ev: On<ScreenshotCaptured>| {
                    if let Some(png) = kept(&ev.image) { let _ = bakes.put("thumb", &key2, &png); }
                });
            }
            // the picture is taken the frame after it is asked for; then the stage is cleared
            if frames > SETTLE + 3 {
                for e in entities { commands.entity(e).despawn(); }
                return;
            }
            p.job = Some(Job::Shooting { key, image, entities, frames: frames + 1 });
        }
    }
}

/// The picture as the small PNG that is kept: cut to what the model fills (a little margin, square), so it shows as
/// large as it can.
fn kept(img: &Image) -> Option<Vec<u8>> {
    let rgba = img.clone().try_into_dynamic().ok()?.to_rgba8();
    let (w, h) = rgba.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (x, y, p) in rgba.enumerate_pixels() {
        if p[3] > 8 { x0 = x0.min(x); y0 = y0.min(y); x1 = x1.max(x); y1 = y1.max(y); }
    }
    let cropped = if x1 > x0 && y1 > y0 {
        let side = ((x1 - x0).max(y1 - y0) as f32 * 1.1).ceil() as i64;
        let (cx, cy) = ((x0 + x1) as i64 / 2, (y0 + y1) as i64 / 2);
        let left = (cx - side / 2).clamp(0, (w as i64 - side).max(0));
        let top = (cy - side / 2).clamp(0, (h as i64 - side).max(0));
        let side = side.min(w as i64).min(h as i64) as u32;
        image::imageops::crop_imm(&rgba, left as u32, top as u32, side, side).to_image()
    } else { rgba };
    let mut out = std::io::Cursor::new(vec![]);
    image::DynamicImage::ImageRgba8(cropped).thumbnail(KEPT, KEPT).write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}
