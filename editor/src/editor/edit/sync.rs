//! Keeping the 3D entities, sounds, brush strokes, navigation overlay and lighting in step with the level.
use super::*;

/// The sounds: heard from the camera, while editing and in test mode.
pub fn hear(mut sounds: ResMut<crate::sound::Sounds>, mut ed: ResMut<Editor>, mut play: ResMut<Play>, ear: Single<&GlobalTransform, With<crate::play::PlayCamera>>) {
    if let Some(p) = ed.bypass_change_detection().preview_sound.take() { sounds.preview = Some(p); }
    let run = play.on.then_some(play.run);
    let inputs = std::mem::take(&mut play.script.inputs);
    let happened = std::mem::take(&mut play.script.happened);
    let script = &play.script;
    // the Audio tab: its stage instead of the level, and the open sound's recordings read for drawing
    if ed.tab == crate::editor::audio::AUDIO {
        if let Some(s) = ed.sound_open.as_ref().and_then(|p| ed.level.sounds.get(p)) { sounds.read_waves(&ed.dir, s.all_clips()); }
        sounds.step(&ed.level, &crate::editor::audio::speakers(&ed), &ed.dir, None, inputs, happened, &ear, |id| crate::editor::audio::at(&ed, id));
        return;
    }
    sounds.step(&ed.level, &crate::sound::speakers(&ed.level), &ed.dir, run, inputs, happened, &ear, |id| ed.level.get(id).map(|p| Vec3::from(p.pos) + script.offset_of(id).unwrap_or_default()));
}

/// The brush strokes drawn on the parts.
pub fn paint(ed: Res<Editor>, mut paint: crate::paint::PaintSync) { paint.run(&ed.level); }

/// The grass carpet.
pub fn grass(ed: Res<Editor>, mut grass: crate::grass::GrassSync) { grass.run(&ed.level); }

#[derive(Component)]
pub struct PartView { id: u64, applied: Part }

/// Make the 3D entities match the level (spawn new parts, update moved ones, rebuild changed ones, remove deleted ones).
#[allow(clippy::too_many_arguments)]
pub fn sync(
    mut commands: Commands,
    ed: Res<Editor>,
    lib: Res<ShapeLib>,
    mut models: ResMut<ModelLib>,
    server: Res<AssetServer>,
    mut mats: ResMut<Mats>,
    mut assets: ResMut<Assets<StandardMaterial>>,
    mut shaded: ResMut<Assets<crate::shader::ShaderMaterial>>,
    mut shaders: ResMut<crate::shader::Shaders>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut views: Query<(Entity, &mut PartView, &mut Transform)>,
    mut materials: Local<std::collections::BTreeMap<String, level::Material>>,
) {
    if !ed.is_changed() { return; }
    // a textured part's mesh is laid out for its size, and its look follows its materials
    let mats_changed = *materials != *ed.level.materials;
    if mats_changed { *materials = (*ed.level.materials).clone(); }
    let same_look = |a: &Part, b: &Part| a.shape == b.shape && a.model == b.model && a.visible == b.visible && a.color == b.color && a.light == b.light
        && a.surfaces == b.surfaces && (a.surfaces.is_empty() || (a.size == b.size && !mats_changed));
    let mut seen = std::collections::HashSet::new();
    for (e, mut v, mut t) in &mut views {
        match ed.level.get(v.id) {
            Some(p) if same_look(&v.applied, p) => {
                seen.insert(p.id);
                if v.applied != *p {
                    *t = world::part_transform(p);
                    v.applied = p.clone();
                }
            }
            _ => commands.entity(e).despawn(),
        }
    }
    for p in ed.level.parts.iter().filter(|p| !seen.contains(&p.id)) {
        let mut looks = world::Looks { lib: &lib, models: &mut models, server: &server, mats: &mut mats, assets: &mut assets, shaded: &mut shaded, shaders: &mut shaders, meshes: &mut meshes, materials: &ed.level.materials, shadows: std::env::var_os("BEVERLY_LITE").is_none() };
        if let Some(e) = world::spawn_visual(&mut commands, p, &mut looks, true) {
            commands.entity(e).insert(PartView { id: p.id, applied: p.clone() });
        }
    }
}

/// The navigation area drawn over the level (green; orange once the level has changed since it was built).
pub fn nav_overlay(ed: Res<Editor>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, mut drawn: Local<Option<(Entity, (u32, bool, bool))>>) {
    let want = (ed.nav_builds, ed.nav_out_of_date(), ed.nav_shown && ed.testing.is_none());
    if drawn.as_ref().map(|d| d.1) == Some(want) { return; }
    if let Some((e, _)) = drawn.take() { commands.entity(e).despawn(); }
    let Some((m, _)) = ed.nav.as_ref().filter(|_| want.2) else {
        *drawn = Some((commands.spawn(Transform::default()).id(), want));
        return;
    };
    // a little above the ground it covers
    let pos: Vec<[f32; 3]> = crate::nav::surface(m).iter().flatten().map(|v| (*v + Vec3::Y * 0.04).to_array()).collect();
    let mut mesh = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; pos.len()]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    let colour = if want.1 { Color::srgba(0.94, 0.55, 0.26, 0.35) } else { Color::srgba(0.28, 0.75, 0.55, 0.35) };
    let mat = mats.add(StandardMaterial { base_color: colour, unlit: true, alpha_mode: AlphaMode::Blend, cull_mode: None, double_sided: true, ..default() });
    *drawn = Some((commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mat), Transform::default(), world::EDITOR_ONLY)).id(), want));
}

/// The sun and fill light follow the level's Lighting, as the zones the camera is in change it.
pub fn stage(ed: Res<Editor>, mut applied: Local<Option<level::Lighting>>, mut sun: Query<(&mut DirectionalLight, &mut Transform), With<world::Sun>>, mut fill: ResMut<GlobalAmbientLight>, cam: Query<&GlobalTransform, With<crate::play::PlayCamera>>) {
    let at = cam.iter().next().map_or(Vec3::ZERO, |t| t.translation());
    let l = ed.level.lighting_at(at);
    if applied.as_ref() == Some(&l) { return; }
    world::light_stage(&l, std::env::var_os("BEVERLY_LITE").is_none(), &mut sun, &mut fill);
    *applied = Some(l);
}
