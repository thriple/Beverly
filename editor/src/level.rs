//! What gets saved: a project is a folder of plain text files.
//!
//! ```text
//! My Game/
//!   project.ron          name, levels
//!   levels/main.ron      the parts placed in a level
//!   assets/textures/ ... files dropped onto the Assets panel are copied here
//!   assets/models/ assets/sounds/ assets/other/
//! ```
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Shape { Box, Ramp, Stairs, Arch, Cylinder, Sphere, Plane }

impl Shape {
    pub const ALL: [Shape; 7] = [Shape::Box, Shape::Ramp, Shape::Stairs, Shape::Arch, Shape::Cylinder, Shape::Sphere, Shape::Plane];
    pub fn label(self) -> &'static str {
        match self {
            Shape::Box => "Box",
            Shape::Ramp => "Ramp",
            Shape::Stairs => "Stairs",
            Shape::Arch => "Arch",
            Shape::Cylinder => "Cylinder",
            Shape::Sphere => "Sphere",
            Shape::Plane => "Plane",
        }
    }
    pub fn glyph(self) -> &'static str {
        match self {
            Shape::Box => "▢",
            Shape::Ramp => "◺",
            Shape::Stairs => "▤",
            Shape::Arch => "∩",
            Shape::Cylinder => "◯",
            Shape::Sphere => "●",
            Shape::Plane => "▭",
        }
    }
    /// One line for the parts pop-up.
    pub fn description(self) -> String {
        let [x, y, z] = self.default_size();
        let what = match self {
            Shape::Box => "a block for walls, floors and cover",
            Shape::Ramp => "a slope rising toward +X",
            Shape::Stairs => "six steps rising toward +X",
            Shape::Arch => "a block with a round-topped opening",
            Shape::Cylinder => "a round column",
            Shape::Sphere => "a ball",
            Shape::Plane => "a thin slab for floors and platforms",
        };
        format!("{what} · {x} × {y} × {z} m")
    }

    /// Size in metres (x wide, y tall, z deep) when dropped.
    pub fn default_size(self) -> [f32; 3] {
        match self {
            Shape::Box => [2.0, 2.0, 2.0],
            Shape::Ramp => [4.0, 2.0, 3.0],
            Shape::Stairs => [4.0, 2.0, 2.0],
            Shape::Arch => [4.0, 3.0, 1.0],
            Shape::Cylinder => [1.5, 3.0, 1.5],
            Shape::Sphere => [2.0, 2.0, 2.0],
            Shape::Plane => [4.0, 0.1, 4.0],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Collision { Solid, WalkThrough, Trigger }

impl Collision {
    pub const ALL: [Collision; 3] = [Collision::Solid, Collision::WalkThrough, Collision::Trigger];
    pub fn label(self) -> &'static str {
        match self {
            Collision::Solid => "Solid",
            Collision::WalkThrough => "Walk-through",
            Collision::Trigger => "Trigger",
        }
    }
}

/// One basic shape placed in a level. With `visible` off it is only a collider.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Part {
    pub id: u64,
    pub name: String,
    pub shape: Shape,
    pub pos: [f32; 3],
    /// Rotation as a quaternion (x, y, z, w).
    pub rot: [f32; 4],
    pub size: [f32; 3],
    /// sRGB, 0..1.
    pub color: [f32; 3],
    pub visible: bool,
    pub collision: Collision,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct Level {
    pub name: String,
    pub parts: Vec<Part>,
    pub next_id: u64,
}

impl Level {
    pub fn get(&self, id: u64) -> Option<&Part> { self.parts.iter().find(|p| p.id == id) }
    pub fn get_mut(&mut self, id: u64) -> Option<&mut Part> { self.parts.iter_mut().find(|p| p.id == id) }

    /// A new name like "Box 3": the lowest number not used by another part of that shape.
    pub fn fresh_name(&self, base: &str) -> String {
        (1..).map(|n| format!("{base} {n}")).find(|n| !self.parts.iter().any(|p| &p.name == n)).unwrap()
    }

    pub fn add(&mut self, mut part: Part) -> u64 {
        self.next_id += 1;
        part.id = self.next_id;
        self.parts.push(part);
        self.next_id
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjectFile {
    pub name: String,
    pub levels: Vec<String>,
}

pub const ASSET_FOLDERS: [&str; 4] = ["textures", "models", "sounds", "other"];

/// Which assets folder a dropped file goes into, by its extension.
pub fn folder_for(path: &Path) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "tga" | "bmp" | "ktx2" | "dds" | "webp" | "hdr" | "exr" => "textures",
        "glb" | "gltf" | "obj" | "fbx" | "dae" => "models",
        "wav" | "ogg" | "mp3" | "flac" => "sounds",
        _ => "other",
    }
}

/// The default place for new projects: Documents/Beverly (or the home folder's Beverly).
pub fn projects_root() -> PathBuf {
    let home = std::env::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let docs = home.join("Documents");
    if docs.is_dir() { docs.join("Beverly") } else { home.join("Beverly") }
}

/// Create a fresh project folder "Untitled N" under [`projects_root`].
pub fn new_project_dir() -> std::io::Result<PathBuf> {
    let root = projects_root();
    std::fs::create_dir_all(&root)?;
    let dir = (1..).map(|n| root.join(format!("Untitled {n}"))).find(|d| !d.exists()).unwrap();
    make_folders(&dir)?;
    Ok(dir)
}

pub fn make_folders(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir.join("levels"))?;
    for f in ASSET_FOLDERS {
        std::fs::create_dir_all(dir.join("assets").join(f))?;
    }
    Ok(())
}

pub fn save(dir: &Path, name: &str, level: &Level) -> Result<(), String> {
    make_folders(dir).map_err(|e| e.to_string())?;
    let cfg = ron::ser::PrettyConfig::default();
    let pf = ProjectFile { name: name.to_owned(), levels: vec!["main".into()] };
    let p = ron::ser::to_string_pretty(&pf, cfg.clone()).map_err(|e| e.to_string())?;
    let l = ron::ser::to_string_pretty(level, cfg).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("project.ron"), p).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("levels").join("main.ron"), l).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load(dir: &Path) -> Result<(String, Level), String> {
    let p = std::fs::read_to_string(dir.join("project.ron")).map_err(|_| "This folder has no project.ron".to_string())?;
    let pf: ProjectFile = ron::from_str(&p).map_err(|e| format!("project.ron: {e}"))?;
    let first = pf.levels.first().cloned().unwrap_or_else(|| "main".into());
    let l = std::fs::read_to_string(dir.join("levels").join(format!("{first}.ron"))).map_err(|e| e.to_string())?;
    let level: Level = ron::from_str(&l).map_err(|e| format!("levels/{first}.ron: {e}"))?;
    Ok((pf.name, level))
}

/// Copy a dropped file into the project's assets; returns the folder it went into.
pub fn import(dir: &Path, src: &Path) -> std::io::Result<&'static str> {
    let folder = folder_for(src);
    let dest_dir = dir.join("assets").join(folder);
    std::fs::create_dir_all(&dest_dir)?;
    let name = src.file_name().unwrap_or_default();
    std::fs::copy(src, dest_dir.join(name))?;
    Ok(folder)
}

pub fn list_assets(dir: &Path, folder: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir.join("assets").join(folder))
        .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    v.sort();
    v
}
