//! Material files (assets/materials/NAME.bev) and the level's material lists.
use super::*;

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
    #[reflect(@Setting::colour("Tint").alpha("opacity").section(MATERIAL).help("Colours the texture (white leaves it as it is)"))]
    pub tint: [f32; 3],
    #[reflect(@Setting::feel("Opacity", &["Clear", "Faint", "Frosted", "Solid"], 0.0, 1.0, "").section(MATERIAL).help("See-through below Solid, like glass"))]
    pub opacity: f32,
    #[reflect(@Setting::feel("Metal", &["Not metal", "Worn", "Metal"], 0.0, 1.0, "").section(MATERIAL))]
    pub metal: f32,
    #[reflect(@Setting::feel("Roughness", &["Glossy", "Satin", "Matte"], 0.0, 1.0, "").section(MATERIAL))]
    pub roughness: f32,
    #[reflect(@Setting::feel("Glow", &["None", "Faint", "Glowing", "Bright"], 0.0, 10.0, "").section(MATERIAL).help("Gives off its own light, for screens, signs and light strips"))]
    pub glow: f32,
    /// What it sounds like underfoot (None: what the part says).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sounds_like: Option<Ground>,
    /// A custom shader that draws it (shader.rs), e.g. "assets/shaders/waves.wesl", and the values it was given by
    /// the shader's setting names (a setting not here has the shader's default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shader: Option<String>,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub shader_values: std::collections::BTreeMap<String, Vec<f32>>,
}

impl Default for Material {
    fn default() -> Self { Material { format: 0, texture: None, tint: [1.0; 3], opacity: 1.0, metal: 0.0, roughness: 0.75, glow: 0.0, sounds_like: None, shader: None, shader_values: Default::default() } }
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
