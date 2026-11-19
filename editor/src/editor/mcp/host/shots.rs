//! Screenshots: asked for on one frame, taken a couple of frames later, sent back as PNG.
use super::*;

/// A screenshot on its way: taken a couple of frames after it is asked for, so the last changes are drawn.
pub struct Shot {
    pub(super) reply: std::sync::mpsc::Sender<Result<Output, String>>,
    /// The camera made for a view from elsewhere (none: the window).
    pub(super) cam: Option<(Entity, Handle<Image>)>,
    pub(super) width: u32,
    pub(super) frames: u8,
}

/// Start a screenshot: of the window, or from a camera made for it. Returns the camera and the width.
pub(super) fn shot(ed: &Editor, view: &View, args: &Value, commands: &mut Commands, images: &mut Assets<Image>) -> Result<(Option<(Entity, Handle<Image>)>, u32), String> {
    let width = args.get("width").and_then(Value::as_u64).unwrap_or(1024).clamp(256, 1920) as u32;
    let (w, h) = (width, width * 9 / 16);
    use bevy::render::render_resource::TextureFormat;
    let texture = |images: &mut Assets<Image>| images.add(Image::new_target_texture(w, h, TextureFormat::Rgba8UnormSrgb, None));
    // through a camera part, `at` seconds into its move (a Follow camera's player standing at the player start)
    if let Some(c) = args.get("camera") {
        let p = ed.level.get(part_ref(ed, Some(c))?).filter(|p| p.shape.is_camera()).ok_or("That isn't a camera part")?;
        let at = args.get("at").and_then(Value::as_f64).unwrap_or(0.0) as f32;
        let start = ed.level.parts.iter().find(|q| q.shape == Shape::PlayerStart).map_or(Vec3::ZERO, |q| Vec3::from(q.pos));
        let (pos, rot) = crate::level::camera_view(p, at, start, start);
        let fov = p.camera.as_ref().map_or(45.0, |c| c.fov).to_radians();
        let image = texture(images);
        let cam = commands.spawn((
            Camera3d::default(),
            Camera { order: -1, ..default() },
            bevy::camera::RenderTarget::Image(image.clone().into()),
            Projection::Perspective(PerspectiveProjection { fov, ..default() }),
            Msaa::Off,
            Transform::from_translation(pos).with_rotation(rot),
        )).id();
        return Ok((Some((cam, image)), width));
    }
    let Some(from) = args.get("from") else { return Ok((None, width)) };
    let from = vec3_of(from).ok_or("from is [x, y, z]")?;
    let target = match args.get("look_at") {
        Some(t) => vec3_of(t).ok_or("look_at is [x, y, z]")?,
        None => match ed.selected.and_then(|id| ed.level.get(id)) {
            Some(p) => Vec3::from(p.pos),
            None if !ed.level.parts.is_empty() => ed.level.parts.iter().map(|p| Vec3::from(p.pos)).sum::<Vec3>() / ed.level.parts.len() as f32,
            None => from + view.forward(),
        },
    };
    if (target - from).length() < 1e-3 { return Err("from and look_at are the same point".into()); }
    let up = if (target - from).normalize().y.abs() > 0.999 { Vec3::Z } else { Vec3::Y };
    let image = texture(images);
    let cam = commands.spawn((
        Camera3d::default(),
        Camera { order: -1, ..default() },
        bevy::camera::RenderTarget::Image(image.clone().into()),
        Msaa::Off,
        Transform::from_translation(from).looking_at(target, up),
    )).id();
    Ok((Some((cam, image)), width))
}

/// Take the screenshots whose frames are up; each answers its call when the picture arrives.
pub(super) fn take_shots(commands: &mut Commands, shots: &mut Vec<Shot>) {
    use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
    for s in shots.iter_mut() { s.frames = s.frames.saturating_sub(1); }
    let (ready, waiting): (Vec<Shot>, Vec<Shot>) = std::mem::take(shots).into_iter().partition(|s| s.frames == 0);
    *shots = waiting;
    for s in ready {
        let what = match &s.cam { Some((_, img)) => Screenshot::image(img.clone()), None => Screenshot::primary_window() };
        let (reply, cam, width) = (s.reply, s.cam.map(|c| c.0), s.width);
        commands.spawn(what).observe(move |ev: On<ScreenshotCaptured>, mut commands: Commands| {
            let _ = reply.send(png_of(&ev.image, width));
            if let Some(c) = cam { commands.entity(c).despawn(); }
        });
    }
}

/// The picture as a PNG at most `width` pixels wide.
pub(super) fn png_of(img: &Image, width: u32) -> Result<Output, String> {
    let rgba = img.clone().try_into_dynamic().map_err(|e| format!("Couldn't read the picture: {e}"))?.to_rgba8();
    let (w, h) = rgba.dimensions();
    let rgba = if w > width { image::imageops::resize(&rgba, width, (h * width / w).max(1), image::imageops::FilterType::Triangle) } else { rgba };
    let (w, h) = rgba.dimensions();
    let mut png = vec![];
    image::DynamicImage::ImageRgba8(rgba).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(Output::Image { png, text: format!("{w} × {h}") })
}
