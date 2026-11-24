//! The project folder: saving, loading, importing assets, exporting the game.
use super::*;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjectFile {
    #[serde(default)]
    pub format: u32,
    pub name: String,
    pub levels: Vec<String>,
    /// The game's controls, when they differ from Beverly's defaults (`input.rs`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controls: Option<crate::input::Controls>,
}

pub const ASSET_FOLDERS: [&str; 6] = ["textures", "materials", "models", "sounds", "shaders", "other"];

/// Which assets folder a dropped file goes into, by its extension.
pub fn folder_for(path: &Path) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "tga" | "bmp" | "ktx2" | "dds" | "webp" | "hdr" | "exr" => "textures",
        "glb" | "gltf" | "obj" | "fbx" | "dae" => "models",
        "wav" | "ogg" | "mp3" | "flac" => "sounds",
        "bev" => "materials",
        "wesl" => "shaders",
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

/// The file format Beverly writes. Raise it when a saved field is renamed or changes meaning, and teach [`load`] to
/// upgrade files from before (a new field with a default needs neither).
pub const FORMAT: u32 = 1;
/// The newest format a level file can be in: 2 when parts have parents (their positions are offsets from them), which
/// a Beverly that doesn't know parents must not read as the world's. Levels without parents are still written as 1.
pub const LEVEL_FORMAT: u32 = 2;

/// The project file in `dir`: project.bev, or project.ron in projects from before .bev files.
pub fn project_file(dir: &Path) -> Option<PathBuf> {
    ["bev", "ron"].map(|e| dir.join(format!("project.{e}"))).into_iter().find(|p| p.is_file())
}

/// The project a path given to Beverly belongs to: a project folder, or any file or folder inside one (project.bev,
/// levels/main.bev), so a .bev file opened from Explorer opens its project.
pub fn project_dir_of(path: &Path) -> PathBuf {
    path.ancestors().find(|d| project_file(d).is_some()).unwrap_or(path).to_path_buf()
}

/// A project saved by a Beverly from before .bev files; saving it upgrades it.
pub fn is_old(dir: &Path) -> bool { project_file(dir).is_some_and(|p| p.extension().is_some_and(|e| e == "ron")) }

pub fn save(dir: &Path, name: &str, level: &Level) -> Result<(), String> {
    make_folders(dir).map_err(|e| e.to_string())?;
    let cfg = ron::ser::PrettyConfig::default();
    let controls = (!level.controls.is_default()).then(|| (*level.controls).clone());
    let pf = ProjectFile { format: FORMAT, name: name.to_owned(), levels: vec!["main".into()], controls };
    let p = ron::ser::to_string_pretty(&pf, cfg.clone()).map_err(|e| e.to_string())?;
    let format = if level.parts.iter().any(|p| p.parent.is_some()) { LEVEL_FORMAT } else { FORMAT };
    let l = ron::ser::to_string_pretty(&Level { format, ..level.to_file() }, cfg).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("project.bev"), p).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("levels").join("main.bev"), l).map_err(|e| e.to_string())?;
    for (path, m) in &level.materials {
        if load_material(dir, path).ok().as_ref() != Some(&Material { format: FORMAT, ..m.clone() }) { save_material(dir, path, m)?; }
    }
    for (path, m) in &level.sounds {
        if load_sound(dir, path).ok().as_ref() != Some(&Sound { format: FORMAT, ..m.clone() }) { save_sound(dir, path, m)?; }
    }
    // the .ron files these replace, in a project from before .bev files
    for old in [dir.join("project.ron"), dir.join("levels").join("main.ron")] {
        if old.is_file() { std::fs::remove_file(old).map_err(|e| e.to_string())?; }
    }
    Ok(())
}

pub fn load(dir: &Path) -> Result<(String, Level), String> {
    let file = project_file(dir).ok_or("This folder has no project.bev")?;
    let ext = file.extension().unwrap_or_default().to_string_lossy().into_owned();
    let p = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
    let pf: ProjectFile = ron::from_str(&p).map_err(|e| format!("project.{ext}: {e}"))?;
    let first = pf.levels.first().cloned().unwrap_or_else(|| "main".into());
    let l = std::fs::read_to_string(dir.join("levels").join(format!("{first}.{ext}"))).map_err(|e| e.to_string())?;
    let mut level: Level = ron::from_str(&l).map_err(|e| format!("levels/{first}.{ext}: {e}"))?;
    if let Some(n) = [(pf.format, FORMAT), (level.format, LEVEL_FORMAT)].into_iter().find(|&(n, max)| n > max).map(|(n, _)| n) {
        return Err(format!("It was saved by a newer Beverly (file format {n}; this one reads up to {LEVEL_FORMAT}): update Beverly to open it"));
    }
    // format 0 (.ron) to 1 (.bev) changed nothing inside the files; 2 keeps child parts as offsets from their parents
    level.from_file();
    level.format = FORMAT;
    level.materials = load_materials(dir).into_iter().map(|(k, m)| (k, Material { format: 0, ..m })).collect();
    level.sounds = load_sounds(dir).into_iter().map(|(k, m)| (k, Sound { format: 0, ..m })).collect();
    level.controls = pf.controls.map(|c| c.with_defaults()).unwrap_or_default().into();
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

/// Write a playable Windows game: `<name>/<name>.exe` (a copy of the game player) and the project's files beside
/// it, zipped. Returns how many files went in.
pub fn export_zip(dir: &Path, name: &str, player: &Path, bakes: &[(String, Vec<u8>)], out: &Path) -> std::io::Result<usize> {
    use std::io::Write;
    let mut z = zip::ZipWriter::new(std::fs::File::create(out)?);
    let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let ext = if cfg!(windows) { ".exe" } else { "" };
    z.start_file(format!("{name}/{name}{ext}"), opt.unix_permissions(0o755)).map_err(std::io::Error::other)?;
    z.write_all(&std::fs::read(player)?)?;
    let mut n = 1;
    let mut stack = vec![dir.join("project.bev"), dir.join("project.ron"), dir.join("levels"), dir.join("assets")];
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            for e in std::fs::read_dir(&p)?.flatten() { stack.push(e.path()); }
        } else if p.is_file() {
            let rel = p.strip_prefix(dir).unwrap_or(&p).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
            z.start_file(format!("{name}/{rel}"), opt).map_err(std::io::Error::other)?;
            z.write_all(&std::fs::read(&p)?)?;
            n += 1;
        }
    }
    // the bakes the level used (bake.rs), so the game doesn't build them again
    for (rel, bytes) in bakes {
        z.start_file(format!("{name}/{rel}"), opt).map_err(std::io::Error::other)?;
        z.write_all(bytes)?;
        n += 1;
    }
    z.finish().map_err(std::io::Error::other)?;
    Ok(n)
}
