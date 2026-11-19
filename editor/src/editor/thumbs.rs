//! Thumbnails of the user's pictures for the Assets panel: each is worked out once in the background, kept in the
//! bake cache (never in the project) and shown as a small picture beside the file's name. They are the user's own
//! content, not part of the editor's drawn interface.
use crate::bake::{self, Bakes};
use super::preview::Asked;
use bevy_egui::egui::{self, TextureHandle};
use std::collections::HashMap;
use std::path::Path;

/// The longest side of a thumbnail (pixels): twice what is shown, so it stays sharp on a dense screen.
const SIZE: u32 = 64;

#[derive(Default)]
pub struct Thumbs {
    /// By project path: the picture once it is ready (None: the file isn't a picture that could be read).
    ready: HashMap<String, Option<TextureHandle>>,
    /// Models asked of the preview stage (`preview.rs`), by path.
    pub asked: Asked,
    waiting: std::collections::HashSet<String>,
}

impl Thumbs {
    /// The thumbnail of the picture at `path` in the project, or None while it is still being made (it is asked for
    /// from the bake cache the first time).
    pub fn get(&mut self, ctx: &egui::Context, dir: &Path, path: &str, bakes: &Bakes) -> Option<egui::TextureId> {
        if let Some(t) = self.ready.get(path) { return t.as_ref().map(|t| t.id()); }
        let file = dir.join(path);
        let meta = std::fs::metadata(&file).ok()?;
        let stamp = meta.modified().ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
        let key = bake::key("thumb", &[path.as_bytes(), &meta.len().to_le_bytes(), &stamp.to_le_bytes(), &SIZE.to_le_bytes()]);
        // a model is photographed by the preview stage; a picture is shrunk in the background
        if crate::models::is_model(path) && bakes.get("thumb", &key).is_none() {
            if self.waiting.insert(path.to_string()) { if let Ok(mut q) = self.asked.0.lock() { q.push((path.to_string(), key)); } }
            return None;
        }
        let Some(png) = bakes.get("thumb", &key) else {
            let name = path.rsplit('/').next().unwrap_or(path).to_string();
            bakes.request("thumb", &key, &name, move |_| make(&file));
            return None;
        };
        let tex = decode(&png).map(|img| ctx.load_texture(format!("thumb {path}"), img, egui::TextureOptions::LINEAR));
        let id = tex.as_ref().map(|t| t.id());
        self.ready.insert(path.to_string(), tex);
        id
    }

    /// Forget what was made (a picture changed, or another project opened).
    pub fn clear(&mut self) { self.ready.clear(); self.waiting.clear(); }
}

/// The thumbnail's PNG bytes, made from a picture file.
fn make(file: &Path) -> Result<Vec<u8>, String> {
    let img = image::open(file).map_err(|e| e.to_string())?;
    let mut out = std::io::Cursor::new(vec![]);
    img.thumbnail(SIZE, SIZE).write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

fn decode(png: &[u8]) -> Option<egui::ColorImage> {
    let img = image::load_from_memory(png).ok()?.to_rgba8();
    Some(egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_becomes_a_small_png_and_back() {
        let dir = std::env::temp_dir().join(format!("beverly-thumb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("big.png");
        image::RgbaImage::from_pixel(300, 150, image::Rgba([200, 30, 30, 255])).save(&file).unwrap();
        let png = make(&file).unwrap();
        let img = decode(&png).unwrap();
        // the longest side is SIZE and the proportions stay
        assert_eq!(img.size, [64, 32]);
        assert_eq!(img.pixels[0].r(), 200);
        assert!(make(&dir.join("missing.png")).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
