//! What gets saved: a project is a folder of plain text files.
//!
//! ```text
//! My Game/
//!   project.bev          name, levels
//!   levels/main.bev      the parts placed in a level
//!   assets/textures/ ... files dropped onto the Assets panel are copied here
//!   assets/materials/    a texture and its settings, shared by every surface that uses it
//!   assets/models/ assets/sounds/ assets/other/
//! ```
//!
//! `.bev` files are RON text (characters/*.bev will join them). Each starts with `format`, the [`FORMAT`] that wrote
//! it, so a newer Beverly can upgrade older files as it opens them and an older one refuses newer files instead of
//! misreading them. Projects from before `.bev` (project.ron, levels/*.ron, format 0) still open; saving one writes
//! .bev files in place of its .ron ones.
use crate::settings::{Label, Ranges, Setting};
use bevy::reflect::{std_traits::ReflectDefault, Reflect};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug, Reflect)]
pub enum Shape {
    Box,
    Ramp,
    Stairs,
    Arch,
    Cylinder,
    Sphere,
    Plane,
    /// An imported glTF model (`Part::model` is its path in the project); `size` is its scale.
    Model,
    /// Where the player character starts. Shown in the editor only.
    PlayerStart,
    /// A button the player presses (E when close). Output: Pressed.
    Interact,
    /// Light in every direction, out to its reach (`Part::light`). Shown in the editor only; never collides.
    PointLight,
    /// Light in a cone along the part's up (+Y): placed on a ceiling it shines down.
    SpotLight,
}

impl Shape {
    pub const ALL: [Shape; 7] = [Shape::Box, Shape::Ramp, Shape::Stairs, Shape::Arch, Shape::Cylinder, Shape::Sphere, Shape::Plane];
    pub const LIGHTS: [Shape; 2] = [Shape::PointLight, Shape::SpotLight];
    pub fn is_light(self) -> bool { Shape::LIGHTS.contains(&self) }
    /// Every kind with triangles of its own (all but models).
    pub fn built() -> impl Iterator<Item = Shape> { Shape::ALL.into_iter().chain([Shape::PlayerStart, Shape::Interact]).chain(Shape::LIGHTS) }
    pub fn label(self) -> &'static str {
        match self {
            Shape::Box => "Box",
            Shape::Ramp => "Ramp",
            Shape::Stairs => "Stairs",
            Shape::Arch => "Arch",
            Shape::Cylinder => "Cylinder",
            Shape::Sphere => "Sphere",
            Shape::Plane => "Plane",
            Shape::Model => "Model",
            Shape::PlayerStart => "Player start",
            Shape::Interact => "Interact",
            Shape::PointLight => "Point light",
            Shape::SpotLight => "Spot light",
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
            Shape::Model => "◆",
            Shape::PlayerStart => "☺",
            Shape::Interact => "☝",
            Shape::PointLight => "☼",
            Shape::SpotLight => "▽",
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
            Shape::Model => return "an imported glTF model".into(),
            Shape::PlayerStart => return "where the player character starts (Shift+` tests from here)".into(),
            Shape::Interact => return "a button the player presses with E; wire Pressed to what it starts".into(),
            Shape::PointLight => return "light in every direction, out to its reach".into(),
            Shape::SpotLight => return "a cone of light, shining away from the surface it is placed on".into(),
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
            Shape::Model => [1.0, 1.0, 1.0],
            Shape::PlayerStart => [0.8, 1.8, 0.8],
            Shape::Interact => [0.5, 1.0, 0.5],
            Shape::PointLight | Shape::SpotLight => [0.3, 0.3, 0.3],
        }
    }

    /// Events this part sends out along wires.
    pub fn outputs(self) -> &'static [&'static str] {
        match self {
            Shape::Interact => &["Pressed"],
            _ => &[],
        }
    }
}

/// A part that moves: from where it was placed to `pos + to` over `secs`, eased, when told to.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
pub struct Mover {
    /// The end position, relative to the start (metres).
    #[reflect(@Setting::number("By", "m", 0.05, -100000.0, 100000.0).section(MOVES).help("Where it moves to, from where it was placed"))]
    pub to: [f32; 3],
    #[reflect(@Setting::feel("Speed", ("Slow", "Quick"), 20.0, 0.2, "s").log().section(MOVES).help("How long it takes to get there"))]
    pub secs: f32,
}

impl Default for Mover {
    fn default() -> Self { Mover { to: [0.0, 3.0, 0.0], secs: 2.0 } }
}

/// A light part's light: Point light and Spot light (the cone is a spot's only).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Light {
    /// sRGB, 0..1.
    #[reflect(@Setting::colour("Colour").section(LIGHT))]
    pub colour: [f32; 3],
    #[reflect(@Setting::toggle("Shadows").section(LIGHT).help("Off is cheaper: light passes through parts"))]
    pub shadows: bool,
    /// Lumens.
    #[reflect(@Setting::feel("Brightness", ("Dim", "Bright"), 1000.0, 10000000.0, "lm").log().section(LIGHT))]
    pub brightness: f32,
    /// How far the light reaches, in metres (the sphere's or cone's handle).
    #[reflect(@Setting::feel("Reach", ("Near", "Far"), 1.0, 100.0, "m").log().section(LIGHT).help("Nothing past this is lit"))]
    pub reach: f32,
    /// The spot's cone, edge to edge, in degrees.
    #[reflect(@Setting::feel("Cone", ("Narrow", "Wide"), 5.0, 150.0, "°").section(CONE).help("Spot lights only: how wide the cone is"))]
    pub cone: f32,
}

impl Default for Light {
    fn default() -> Self { Light { colour: [1.0, 0.93, 0.82], shadows: true, brightness: 800000.0, reach: 10.0, cone: 50.0 } }
}

/// The part panel's sections.
pub const PART: &str = "PART";
pub const PLACE: &str = "POSITION  ·  ROTATION";
pub const LOOK: &str = "SIZE  ·  LOOK";
pub const MOVES: &str = "MOVES";
pub const LIGHT: &str = "LIGHT";
pub const CONE: &str = "CONE";

impl Mover {
    pub const INPUTS: [&str; 3] = ["Go", "Back", "Toggle"];
    pub const OUTPUTS: [&str; 2] = ["Arrived", "Returned"];
}

/// A material laid on a part, all over or on one face, through its file in assets/materials. Where it sits is the
/// surface's own (two parts can share a material and lay it differently).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
pub struct Surface {
    /// The face (see `shapes::Tris::faces`), or `None` for all of the part. A face's own surface wins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<u8>,
    /// The material file, e.g. "assets/materials/brick.bev".
    pub material: String,
    /// How big one copy of the texture is on the surface, in metres (across, down).
    pub tile: [f32; 2],
    /// How far the texture is slid along the surface, in metres (across, down).
    #[serde(default)]
    pub offset: [f32; 2],
}

impl Part {
    /// The surface that covers `face`: its own, else the all-over one.
    pub fn surface(&self, face: u8) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.face == Some(face)).or_else(|| self.surfaces.iter().find(|s| s.face.is_none()))
    }
    /// Lay `s` on the part: on its face (or all over), replacing what was there. All over clears the faces' own.
    pub fn lay(&mut self, s: Surface) {
        if s.face.is_none() { self.surfaces.clear(); }
        self.surfaces.retain(|o| o.face != s.face);
        self.surfaces.push(s);
    }
}

/// The material panel's section.
pub const MATERIAL: &str = "MATERIAL";

/// A material file (assets/materials/NAME.bev): what every surface using it looks like.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
#[reflect(Default)]
#[serde(default)]
pub struct Material {
    pub format: u32,
    /// The texture, e.g. "assets/textures/brick.png" (none: plain, in the tint).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub texture: Option<String>,
    /// sRGB, 0..1; multiplies the texture.
    #[reflect(@Setting::colour("Tint").section(MATERIAL).help("Colours the texture (white leaves it as it is)"))]
    pub tint: [f32; 3],
    #[reflect(@Setting::feel("Opacity", ("Clear", "Solid"), 0.0, 1.0, "").section(MATERIAL).help("See-through below Solid, like glass"))]
    pub opacity: f32,
    #[reflect(@Setting::feel("Metal", ("Not metal", "Metal"), 0.0, 1.0, "").section(MATERIAL))]
    pub metal: f32,
    #[reflect(@Setting::feel("Roughness", ("Glossy", "Rough"), 0.0, 1.0, "").section(MATERIAL))]
    pub roughness: f32,
    #[reflect(@Setting::feel("Glow", ("None", "Bright"), 0.0, 10.0, "").section(MATERIAL).help("Gives off its own light, for screens, signs and light strips"))]
    pub glow: f32,
}

impl Default for Material {
    fn default() -> Self { Material { format: 0, texture: None, tint: [1.0; 3], opacity: 1.0, metal: 0.0, roughness: 0.75, glow: 0.0 } }
}

pub fn load_material(dir: &Path, path: &str) -> Result<Material, String> {
    let text = std::fs::read_to_string(dir.join(path)).map_err(|e| format!("{path}: {e}"))?;
    let m: Material = ron::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    if m.format > FORMAT { return Err(format!("{path} was saved by a newer Beverly")); }
    Ok(m)
}

pub fn save_material(dir: &Path, path: &str, m: &Material) -> Result<(), String> {
    let text = ron::ser::to_string_pretty(&Material { format: FORMAT, ..m.clone() }, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir.join("assets/materials")).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(path), text).map_err(|e| e.to_string())
}

/// Every material file in the project, by path.
pub fn load_materials(dir: &Path) -> std::collections::BTreeMap<String, Material> {
    list_assets(dir, "materials").into_iter().filter(|f| f.ends_with(".bev")).filter_map(|f| {
        let path = format!("assets/materials/{f}");
        load_material(dir, &path).ok().map(|m| (path, m))
    }).collect()
}

impl Level {
    /// The material for a texture: the one already made for it, or a new "assets/materials/<texture name>.bev".
    pub fn material_for_texture(&mut self, texture: &str) -> String {
        let stem = Path::new(texture).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "material".into());
        for n in 1.. {
            let path = if n == 1 { format!("assets/materials/{stem}.bev") } else { format!("assets/materials/{stem} {n}.bev") };
            match self.materials.get(&path) {
                Some(m) if m.texture.as_deref() == Some(texture) => return path,
                Some(_) => continue,
                None => {
                    self.materials.insert(path.clone(), Material { texture: Some(texture.into()), ..Default::default() });
                    return path;
                }
            }
        }
        unreachable!()
    }
    /// A new plain material, "assets/materials/<base> N.bev".
    pub fn new_material(&mut self, base: &str, m: Material) -> String {
        let path = (1..).map(|n| format!("assets/materials/{base} {n}.bev")).find(|p| !self.materials.contains_key(p)).unwrap();
        self.materials.insert(path.clone(), m);
        path
    }
    /// How many surfaces use a material, on how many parts.
    pub fn material_users(&self, path: &str) -> (usize, usize) {
        let per = self.parts.iter().map(|p| p.surfaces.iter().filter(|s| s.material == path).count());
        per.clone().fold((0, 0), |(s, p), n| (s + n, p + (n > 0) as usize))
    }
}

/// A wire: when part `from` sends `out`, part `to` gets `input`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Wire {
    pub from: u64,
    pub out: String,
    pub to: u64,
    pub input: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Reflect)]
pub enum Collision {
    Solid,
    #[reflect(@Label("Walk-through"))]
    WalkThrough,
    Trigger,
}

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
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Reflect)]
pub struct Part {
    pub id: u64,
    #[reflect(@Setting::text("Name").section(PART))]
    pub name: String,
    pub shape: Shape,
    #[reflect(@Setting::number("Position", "m", 0.05, -100000.0, 100000.0).section(PLACE).bare())]
    pub pos: [f32; 3],
    /// Rotation as a quaternion (x, y, z, w).
    #[reflect(@Setting::turn("Turn").section(PLACE))]
    pub rot: [f32; 4],
    #[reflect(@Setting::number("Size", "m", 0.02, 0.02, 100000.0).section(LOOK).bare())]
    pub size: [f32; 3],
    /// sRGB, 0..1.
    #[reflect(@Setting::colour("Colour").section(LOOK))]
    pub color: [f32; 3],
    #[reflect(@Setting::toggle("Visible").section(LOOK).help("Off: an invisible collider (shown hatched in the editor only)"))]
    pub visible: bool,
    #[reflect(@Setting::choice("Collision").section(LOOK).bare())]
    pub collision: Collision,
    /// For `Shape::Model`: the model file, relative to the project folder (e.g. "assets/models/tower.glb").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Textures laid on the part (basic shapes): all over, or face by face.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surfaces: Vec<Surface>,
    /// Off (the default): parts with the same contents (what is inside the box: see [`Part::same_contents`]) are
    /// instances of each other, and changing one asks whether to change them all. On: its contents are its own.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[reflect(@Setting::toggle("Unique").section(PART).help("On: changing its look never changes the parts just like it"))]
    pub unique: bool,
    /// A light part's light (`Shape::PointLight`, `Shape::SpotLight`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[reflect(@Setting::optional("Light", "Off", "On").section(LIGHT))]
    pub light: Option<Light>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[reflect(@Setting::optional("Moves", "Stays put", "Moves when told to").section(MOVES).help("Moves to an end position and back when wired to"))]
    pub mover: Option<Mover>,
}

impl Part {
    /// A part as it is first placed: the default colour, and Walk-through for a player start.
    pub fn placed(shape: Shape, model: Option<String>, name: String, pos: [f32; 3], rot: [f32; 4], size: [f32; 3]) -> Part {
        Part {
            id: 0, name, shape, pos, rot, size,
            color: if shape == Shape::Interact { [0.94, 0.55, 0.26] } else { [0.62, 0.64, 0.70] },
            visible: true,
            collision: if shape == Shape::PlayerStart || shape.is_light() { Collision::WalkThrough } else { Collision::Solid },
            model, mover: None, surfaces: vec![], unique: false,
            light: shape.is_light().then(Light::default),
        }
    }

    pub fn outputs(&self) -> Vec<&'static str> {
        let mut v = self.shape.outputs().to_vec();
        if self.mover.is_some() { v.extend(Mover::OUTPUTS); }
        v
    }
    pub fn inputs(&self) -> &'static [&'static str] { if self.mover.is_some() { &Mover::INPUTS } else { &[] } }

    /// Whether two parts hold the same contents: what is inside the box (the shape or model, its colour, its
    /// textures and how they lie, Visible, Collision). The box itself (name, position, turn, size) is each part's
    /// own, as an instance's placement is in SketchUp.
    pub fn same_contents(&self, o: &Part) -> bool {
        self.shape == o.shape && self.model == o.model && self.color == o.color && self.surfaces == o.surfaces && self.visible == o.visible && self.collision == o.collision
    }
    /// Take `from`'s contents, keeping this part's box.
    pub fn set_contents(&mut self, from: &Part) {
        self.shape = from.shape;
        self.model = from.model.clone();
        self.color = from.color;
        self.surfaces = from.surfaces.clone();
        self.visible = from.visible;
        self.collision = from.collision;
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct Level {
    /// The file format that wrote this level (see [`FORMAT`]); 0 for files from before it existed.
    #[serde(default)]
    pub format: u32,
    pub name: String,
    pub parts: Vec<Part>,
    pub next_id: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wires: Vec<Wire>,
    /// Slider range ends changed under More in the panels (see `settings`).
    #[serde(default, skip_serializing_if = "Ranges::is_empty")]
    pub ranges: Ranges,
    /// The project's materials, by path ("assets/materials/brick.bev"). Saved as their own files, not in the level;
    /// kept here so undo and the unsaved-changes dot cover them.
    #[serde(skip)]
    pub materials: std::collections::BTreeMap<String, Material>,
}

impl Level {
    pub fn get(&self, id: u64) -> Option<&Part> { self.parts.iter().find(|p| p.id == id) }
    pub fn get_mut(&mut self, id: u64) -> Option<&mut Part> { self.parts.iter_mut().find(|p| p.id == id) }

    /// A new name like "Box 3": the lowest number not used by another part of that shape.
    pub fn fresh_name(&self, base: &str) -> String {
        (1..).map(|n| format!("{base} {n}")).find(|n| !self.parts.iter().any(|p| &p.name == n)).unwrap()
    }

    /// Whether a part is instanced: only a mesh plus a material (a basic shape or a model), not Unique, with no
    /// behaviour (Moves, wires), which keeps a part individual.
    pub fn instanced(&self, p: &Part) -> bool {
        (Shape::ALL.contains(&p.shape) || p.shape == Shape::Model) && !p.unique && p.mover.is_none()
            && !self.wires.iter().any(|w| w.from == p.id || w.to == p.id)
    }
    /// The other parts that are instances of the same contents as part `id`.
    pub fn instances(&self, id: u64) -> Vec<u64> {
        let Some(p) = self.get(id).filter(|p| self.instanced(p)) else { return vec![] };
        self.parts.iter().filter(|q| q.id != id && self.instanced(q) && q.same_contents(p)).map(|q| q.id).collect()
    }
    /// The part whose contents changed since `before` while its instances there kept theirs: (it, them). Changing
    /// them all, or making it Unique, settles it.
    pub fn split_from(&self, before: &Level) -> Option<(u64, Vec<u64>)> {
        self.parts.iter().find_map(|p| {
            let old = before.get(p.id).filter(|o| !o.same_contents(p) && self.instanced(p))?;
            let them: Vec<u64> = before.instances(p.id).into_iter().filter(|o| self.get(*o).is_some_and(|q| q.same_contents(old) && self.instanced(q))).collect();
            (!them.is_empty()).then_some((p.id, them))
        })
    }
    /// Give `them` part `id`'s contents.
    pub fn change_all(&mut self, id: u64, them: &[u64]) {
        let Some(p) = self.get(id).cloned() else { return };
        for q in self.parts.iter_mut().filter(|q| them.contains(&q.id)) { q.set_contents(&p); }
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
    #[serde(default)]
    pub format: u32,
    pub name: String,
    pub levels: Vec<String>,
}

pub const ASSET_FOLDERS: [&str; 5] = ["textures", "materials", "models", "sounds", "other"];

/// Which assets folder a dropped file goes into, by its extension.
pub fn folder_for(path: &Path) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "tga" | "bmp" | "ktx2" | "dds" | "webp" | "hdr" | "exr" => "textures",
        "glb" | "gltf" | "obj" | "fbx" | "dae" => "models",
        "wav" | "ogg" | "mp3" | "flac" => "sounds",
        "bev" => "materials",
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
    let pf = ProjectFile { format: FORMAT, name: name.to_owned(), levels: vec!["main".into()] };
    let p = ron::ser::to_string_pretty(&pf, cfg.clone()).map_err(|e| e.to_string())?;
    let l = ron::ser::to_string_pretty(&Level { format: FORMAT, ..level.clone() }, cfg).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("project.bev"), p).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("levels").join("main.bev"), l).map_err(|e| e.to_string())?;
    for (path, m) in &level.materials {
        if load_material(dir, path).ok().as_ref() != Some(&Material { format: FORMAT, ..m.clone() }) { save_material(dir, path, m)?; }
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
    if let Some(n) = [pf.format, level.format].into_iter().find(|&n| n > FORMAT) {
        return Err(format!("It was saved by a newer Beverly (file format {n}; this one reads up to {FORMAT}): update Beverly to open it"));
    }
    // format 0 (.ron) to 1 (.bev) changed nothing inside the files
    level.format = FORMAT;
    level.materials = load_materials(dir).into_iter().map(|(k, m)| (k, Material { format: 0, ..m })).collect();
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
pub fn export_zip(dir: &Path, name: &str, player: &Path, out: &Path) -> std::io::Result<usize> {
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
    z.finish().map_err(std::io::Error::other)?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exports_a_zip_with_the_player_and_the_project() {
        let base = std::env::temp_dir().join(format!("beverly-export-{}", std::process::id()));
        let dir = base.join("proj");
        let lvl = Level { name: "main".into(), ..Default::default() };
        save(&dir, "My Game", &lvl).unwrap();
        std::fs::write(dir.join("assets/textures/a.png"), b"png").unwrap();
        let player = base.join("player.bin");
        std::fs::write(&player, b"exe").unwrap();
        let out = base.join("out.zip");
        let n = export_zip(&dir, "My Game", &player, &out).unwrap();
        assert_eq!(n, 4); // player, project.bev, levels/main.bev, the texture
        let mut z = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
        let ext = if cfg!(windows) { ".exe" } else { "" };
        assert!(z.by_name(&format!("My Game/My Game{ext}")).is_ok());
        assert!(z.by_name("My Game/levels/main.bev").is_ok());
        assert!(z.by_name("My Game/assets/textures/a.png").is_ok());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn saves_bev_files_with_their_format_and_still_opens_ron_projects() {
        let dir = std::env::temp_dir().join(format!("beverly-bev-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // a project as older Beverlys saved it
        make_folders(&dir).unwrap();
        std::fs::write(dir.join("project.ron"), r#"(name: "Old", levels: ["main"])"#).unwrap();
        std::fs::write(dir.join("levels/main.ron"), r#"(name: "main", parts: [], next_id: 0)"#).unwrap();
        assert!(is_old(&dir));
        assert_eq!(project_dir_of(&dir.join("levels/main.ron")), dir, "a file inside opens its project");
        let (name, lvl) = load(&dir).unwrap();
        assert_eq!((name.as_str(), lvl.format), ("Old", FORMAT));
        // saving upgrades it: .bev files that start with their format, and no .ron left behind
        save(&dir, &name, &lvl).unwrap();
        assert!(!is_old(&dir) && !dir.join("project.ron").exists() && !dir.join("levels/main.ron").exists());
        let text = std::fs::read_to_string(dir.join("levels/main.bev")).unwrap();
        assert!(text.trim_start_matches('(').trim_start().starts_with(&format!("format: {FORMAT},")), "{text}");
        assert_eq!(load(&dir).unwrap(), (name, lvl));
        // a newer Beverly's files are refused, not misread
        std::fs::write(dir.join("project.bev"), format!(r#"(format: {}, name: "New", levels: ["main"])"#, FORMAT + 1)).unwrap();
        assert!(load(&dir).unwrap_err().contains("newer Beverly"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn surfaces_cover_faces_and_share_one_material_per_texture() {
        let dir = std::env::temp_dir().join(format!("beverly-mat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut lvl = Level { name: "main".into(), ..Default::default() };
        let brick = lvl.material_for_texture("assets/textures/brick.png");
        assert_eq!(brick, "assets/materials/brick.bev");
        assert_eq!(lvl.material_for_texture("assets/textures/brick.png"), brick, "the same texture reuses its material");
        assert_eq!(lvl.material_for_texture("assets/textures/other/brick.png"), "assets/materials/brick 2.bev");
        lvl.materials.get_mut(&brick).unwrap().glow = 2.0;
        let mut p = Part::placed(Shape::Box, None, "Box 1".into(), [0.0; 3], [0.0, 0.0, 0.0, 1.0], [2.0; 3]);
        let s = |face, m: &str| Surface { face, material: m.into(), tile: [1.0, 1.0], offset: [0.0, 0.0] };
        p.lay(s(None, "all"));
        p.lay(s(Some(2), "top"));
        assert_eq!((p.surface(2).unwrap().material.as_str(), p.surface(0).unwrap().material.as_str()), ("top", "all"));
        p.lay(s(Some(2), "top again"));
        assert_eq!(p.surfaces.len(), 2, "a face holds one surface");
        p.lay(s(None, &brick));
        assert_eq!(p.surfaces.len(), 1, "all over replaces the faces' own");
        lvl.add(p);
        assert_eq!(lvl.material_users(&brick), (1, 1));
        // materials are saved as their own files and come back with the level
        save(&dir, "Mats", &lvl).unwrap();
        assert!(dir.join("assets/materials/brick 2.bev").is_file());
        let (_, back) = load(&dir).unwrap();
        assert_eq!(back.materials[&brick].glow, 2.0);
        assert_eq!(back.materials, lvl.materials);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn instances_share_their_contents_and_unique_or_behaviour_keeps_a_part_alone() {
        let mut lvl = Level { name: "main".into(), ..Default::default() };
        let boxed = |x: f32| Part::placed(Shape::Box, None, format!("Box {x}"), [x, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0], [2.0; 3]);
        let a = lvl.add(boxed(0.0));
        let b = lvl.add(Part { size: [1.0, 3.0, 1.0], rot: [0.0, 0.38268343, 0.0, 0.9238795], ..boxed(3.0) });
        let c = lvl.add(boxed(6.0));
        let ball = lvl.add(Part::placed(Shape::Sphere, None, "Ball".into(), [9.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0], [2.0; 3]));
        // the box (place, turn, size) is each part's own; the contents are shared
        assert_eq!(lvl.instances(a), [b, c]);
        assert!(lvl.instances(ball).is_empty());
        // texturing one splits it from the others until they all change or it is made Unique
        let before = lvl.clone();
        lvl.get_mut(a).unwrap().lay(Surface { face: None, material: "assets/materials/brick.bev".into(), tile: [1.0, 1.0], offset: [0.0, 0.0] });
        assert_eq!(lvl.split_from(&before), Some((a, vec![b, c])));
        let mut all = lvl.clone();
        all.change_all(a, &[b, c]);
        assert_eq!(all.split_from(&before), None);
        assert_eq!(all.instances(a), [b, c]);
        assert_eq!(all.get(b).unwrap().size, [1.0, 3.0, 1.0], "their boxes stay theirs");
        // a different colour is different contents (the colour is a plain shape's material)
        assert!(!boxed(0.0).same_contents(&Part { color: [1.0, 0.0, 0.0], ..boxed(0.0) }));
        lvl.get_mut(a).unwrap().unique = true;
        assert_eq!(lvl.split_from(&before), None);
        assert_eq!(lvl.instances(b), [c]);
        // moving or wired parts are individual
        let mut lvl = before.clone();
        lvl.get_mut(c).unwrap().mover = Some(Mover::default());
        assert_eq!(lvl.instances(a), [b]);
        lvl.wires.push(Wire { from: b, out: "x".into(), to: ball, input: "y".into() });
        assert!(lvl.instances(a).is_empty());
        // Unique is saved only when on
        let text = ron::to_string(&Part { unique: true, ..boxed(0.0) }).unwrap();
        assert!(text.contains("unique:true") && !ron::to_string(&boxed(0.0)).unwrap().contains("unique"), "{text}");
    }

    #[test]
    fn light_parts_carry_a_light_are_individual_and_never_collide() {
        let mut lvl = Level { name: "main".into(), ..Default::default() };
        let mut spot = Part::placed(Shape::SpotLight, None, "Spot 1".into(), [0.0, 3.0, 0.0], [1.0, 0.0, 0.0, 0.0], Shape::SpotLight.default_size());
        assert_eq!(spot.light, Some(Light::default()));
        spot.collision = Collision::Solid;
        let a = lvl.add(spot.clone());
        lvl.add(spot);
        assert!(lvl.instances(a).is_empty(), "lights are always individual");
        assert_eq!(crate::play::CollisionWorld::build(&lvl, &crate::world::Geometry::default()).pieces().count(), 0, "not even set Solid");
        let text = ron::to_string(&lvl.parts[0]).unwrap();
        assert_eq!(ron::from_str::<Part>(&text).unwrap(), lvl.parts[0]);
        assert!(!ron::to_string(&Part::placed(Shape::Box, None, "Box".into(), [0.0; 3], [0.0, 0.0, 0.0, 1.0], [1.0; 3])).unwrap().contains("light"));
    }
}
