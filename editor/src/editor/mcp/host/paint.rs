//! The brush and the pen: strokes and curves.
use super::*;

pub(super) fn paint_tools(ed: &mut Editor, name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "paint_stroke" => {
            editable(ed)?;
            let p = part_of(ed, args)?;
            let (id, name) = (p.id, p.name.clone());
            if !crate::paint::paintable(p) { return Err(format!("{name} can't be painted (the brush paints visible basic shapes)")); }
            if args.get("clear").and_then(Value::as_bool) == Some(true) {
                ed.level.get_mut(id).expect("found above").strokes.clear();
                ed.commit();
                return Ok(json!({"part": name, "strokes": 0}));
            }
            let erase = args.get("erase").and_then(Value::as_bool) == Some(true);
            let material = match (erase, args.get("material").and_then(Value::as_str)) {
                (true, _) => String::new(),
                (false, Some(m)) => material_ref(ed, m)?,
                (false, None) => return Err("Give the material to paint (or erase: true)".into()),
            };
            let points: Vec<Vec3> = args.get("points").and_then(Value::as_array).map(|a| a.iter().filter_map(vec3_of).collect()).unwrap_or_default();
            if points.is_empty() { return Err("Give points: [[x, y, z], ...] on its surface, in the world".into()); }
            let num = |k: &str, d: f32, lo: f32, hi: f32| args.get(k).and_then(Value::as_f64).map_or(d, |v| (v as f32).clamp(lo, hi));
            let b = ed.brush.clone();
            let lays = match args.get("lays").and_then(Value::as_str) {
                None => b.lays,
                Some("add") => crate::level::Lays::Add,
                Some("subtract") => crate::level::Lays::Subtract,
                Some("set") => crate::level::Lays::Set,
                Some(o) => return Err(format!("lays is add, subtract or set, not {o}")),
            };
            if erase && lays != crate::level::Lays::Add { return Err("Erasing takes every material away; give lays only when painting".into()); }
            let rules = match args.get("rules") { Some(r) => rules_from(r, b.rules)?, None => b.rules };
            if let Some(t) = rules.on {
                if !ed.level.get(id).expect("found above").tags.contains(&t) { return Err(format!("{name} isn't tagged {t:?}, and the rules paint only on parts that are")); }
            }
            // the ground under a Mix's plants, when it isn't the Mix's own: a material, or "none"
            let ground = match args.get("ground").and_then(Value::as_str) {
                None => b.ground.clone(),
                Some("none") | Some("") => Some(String::new()),
                Some(g) => Some(material_ref(ed, g)?),
            };
            let look = match (args.get("ground_width").and_then(Value::as_f64), args.get("ground_strength").and_then(Value::as_f64)) {
                (None, None) => (b.ground_width >= 1.0 || b.ground_strength < 1.0).then(|| crate::level::GroundLook { width: (b.ground_width >= 1.0).then_some(b.ground_width), strength: b.ground_strength }),
                (w, st) => Some(crate::level::GroundLook { width: w.map(|w| (w as f32).clamp(1.0, 3.0)), strength: st.map_or(1.0, |v| (v as f32).clamp(0.0, 1.0)) }),
            };
            let p = ed.level.get_mut(id).expect("found above");
            let local = points.iter().map(|w| p.to_local(*w).to_array()).collect();
            p.strokes.push(crate::level::Stroke { material, size: num("size", b.size, 0.05, 500.0), hardness: num("hardness", b.hardness, 0.0, 1.0), strength: num("strength", b.strength, 0.0, 1.0), points: local, lays, rules, curve: None, fill: false, faces: vec![], bucket: None, ground: if erase { None } else { ground }, ground_look: look.filter(|_| !erase) });
            settle_instances(ed, args)?;
            ed.commit();
            Ok(describe(ed, ed.level.get(id).expect("found above")))
        }
        "list_shapes" => Ok(Value::Array(ed.level.curves.iter().map(describe_curve).collect())),
        "draw_shape" => {
            editable(ed)?;
            let existing = match args.get("shape") {
                None | Some(Value::Null) => None,
                Some(v) => Some(ed.level.curves.iter().find(|c| v.as_u64() == Some(c.id) || v.as_str() == Some(c.name.as_str())).map(|c| c.id).ok_or_else(|| format!("No curve {v} (list_shapes shows them)"))?),
            };
            if args.get("delete").and_then(Value::as_bool) == Some(true) {
                let id = existing.ok_or("Give the shape to delete")?;
                ed.level.curves.retain(|c| c.id != id);
                if ed.pen.curve == Some(id) { ed.pen.curve = None; ed.pen.drawing = false; }
                crate::editor::pen::strips(ed);
                crate::editor::scatter::scatters(ed);
                ed.commit();
                return Ok(json!({"deleted": id}));
            }
            let given = args.get("points").and_then(Value::as_array);
            if given.is_none() && existing.is_none() { return Err("Give points: [[x, y, z], ...] (or {at, handle_in, handle_out} for a smooth point)".into()); }
            let points: Option<Vec<crate::curve::PenPoint>> = match given {
                None => None,
                Some(a) => {
                    let mut v = vec![];
                    for (i, p) in a.iter().enumerate() {
                        let v3 = |k: &str| p.get(k).map(|h| vec3_of(h).ok_or_else(|| format!("point {i}: {k} is [x, y, z]"))).transpose();
                        v.push(match vec3_of(p) {
                            Some(at) => crate::curve::PenPoint::corner(at),
                            None if p.is_object() => crate::curve::PenPoint {
                                at: v3("at")?.ok_or_else(|| format!("point {i}: give at: [x, y, z]"))?.to_array(),
                                into: v3("handle_in")?.unwrap_or_default().to_array(),
                                out: v3("handle_out")?.unwrap_or_default().to_array(),
                                stretch: match p.get("stretch") {
                                    None | Some(Value::Null) => None,
                                    Some(w) => Some(stretch_of(w).ok_or_else(|| format!("point {i}: stretch is ground, over, air or fall (or null: as the path says)"))?),
                                },
                                sag: match p.get("sag") { None | Some(Value::Null) => 0.0, Some(n) => (n.as_f64().ok_or_else(|| format!("point {i}: sag is metres"))? as f32).clamp(0.0, 10.0) },
                            },
                            None => return Err(format!("point {i} is [x, y, z] or {{at, handle_in, handle_out}}")),
                        });
                    }
                    if v.len() < 2 { return Err("A curve needs at least two points".into()); }
                    Some(v)
                }
            };
            let closed = args.get("closed").and_then(Value::as_bool);
            // the path: how its lines go unless a point says otherwise, and the steepest ground
            let lines = match args.get("stretch") { None | Some(Value::Null) => None, Some(w) => Some(stretch_of(w).ok_or("stretch is ground, over, air or fall")?) };
            let steepest = match args.get("steepest") { None | Some(Value::Null) => None, Some(n) => Some((n.as_f64().ok_or("steepest is degrees")? as f32).clamp(0.0, 89.0)) };
            // a strip: {material, width, edge, strength}, or null to take it away
            let strip: Option<Option<crate::curve::Strip>> = match args.get("strip") {
                None => None,
                Some(Value::Null) => Some(None),
                Some(v) => {
                    let old = existing.and_then(|id| ed.level.curve(id)).and_then(|c| c.strip.clone());
                    let mut st = old.unwrap_or_default();
                    let num = |k: &str, lo: f32, hi: f32, cur: f32| v.get(k).map_or(Ok(cur), |n| n.as_f64().map(|n| (n as f32).clamp(lo, hi)).ok_or_else(|| format!("strip {k} is a number")));
                    if let Some(m) = v.get("material").and_then(Value::as_str) { st.material = material_ref(ed, m)?; }
                    if st.material.is_empty() { st.material = ed.level.materials.keys().next().cloned().ok_or("No materials yet: new_material makes one")?; }
                    (st.size, st.hardness, st.strength) = (num("width", 0.1, 50.0, st.size)?, num("edge", 0.0, 1.0, st.hardness)?, num("strength", 0.05, 1.0, st.strength)?);
                    Some(Some(st))
                }
            };
            // a fill: {material, feather, strength}, or null to take it away
            let fill: Option<Option<crate::curve::Fill>> = match args.get("fill") {
                None => None,
                Some(Value::Null) => Some(None),
                Some(v) => {
                    let old = existing.and_then(|id| ed.level.curve(id)).and_then(|c| c.fill.clone());
                    let mut f = old.unwrap_or_default();
                    let num = |k: &str, lo: f32, hi: f32, cur: f32| v.get(k).map_or(Ok(cur), |n| n.as_f64().map(|n| (n as f32).clamp(lo, hi)).ok_or_else(|| format!("fill {k} is a number")));
                    if let Some(m) = v.get("material").and_then(Value::as_str) { f.material = material_ref(ed, m)?; }
                    if f.material.is_empty() { f.material = ed.level.materials.keys().next().cloned().ok_or("No materials yet: new_material makes one")?; }
                    (f.feather, f.strength) = (num("feather", 0.0, 4.0, f.feather)?, num("strength", 0.05, 1.0, f.strength)?);
                    Some(Some(f))
                }
            };
            // a scatter: {settings: {Where, Spacing, Band, Size variation, Tilt, Sink, Volume height, Falloff, Spin, Drift, Keep clear, Seed}, rules: {slope, band, on}, items: [{name, shape, settings: {Size, Colour, Share, On its side}}]}, or null
            let scatter: Option<Option<crate::curve::Scatter>> = match args.get("scatter") {
                None => None,
                Some(Value::Null) => Some(None),
                Some(v) => {
                    let mut sc = existing.and_then(|id| ed.level.curve(id)).and_then(|c| c.scatter.clone()).unwrap_or_default();
                    if let Some(set) = v.get("settings") { settings::apply(&mut sc, set)?; }
                    if let Some(r) = v.get("rules") { sc.rules = rules_from(r, sc.rules)?; }
                    if let Some(items) = v.get("items").and_then(Value::as_array) {
                        sc.items = items.iter().enumerate().map(|(k, i)| {
                            let mut it = crate::curve::Item { name: format!("Item {}", k + 1), ..Default::default() };
                            if let Some(n) = i.get("name").and_then(Value::as_str) { it.name = n.to_string(); }
                            if let Some(w) = i.get("shape").and_then(Value::as_str) {
                                it.shape = crate::level::Shape::ALL.into_iter().find(|s| s.label().eq_ignore_ascii_case(w)).ok_or_else(|| format!("No shape {w:?} (Box, Ramp, Stairs, Arch, Cylinder, Sphere, Plane)"))?;
                            }
                            if let Some(set) = i.get("settings") { settings::apply(&mut it, set)?; }
                            Ok(it)
                        }).collect::<Result<_, String>>()?;
                    }
                    Some(Some(sc))
                }
            };
            let id = match existing {
                Some(id) => {
                    let c = ed.level.curve_mut(id).expect("found above");
                    if let Some(p) = points { c.points = p; }
                    if let Some(k) = closed { c.closed = k; }
                    if c.points.len() < 3 { c.closed = false; }
                    if let Some(s) = lines { c.path.stretch = s; }
                    if let Some(s) = steepest { c.path.slope = s; }
                    if let Some(st) = strip { c.strip = st; }
                    if let Some(f) = fill { c.fill = f; }
                    if let Some(sc) = scatter.clone() { c.scatter = sc; }
                    id
                }
                None => {
                    let id = ed.level.add_curve(points.unwrap_or_default(), closed.unwrap_or(false));
                    let c = ed.level.curve_mut(id).expect("just added");
                    if c.points.len() < 3 { c.closed = false; }
                    if let Some(s) = lines { c.path.stretch = s; }
                    if let Some(s) = steepest { c.path.slope = s; }
                    if let Some(n) = args.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()) { c.name = n.to_string(); }
                    if let Some(st) = strip { c.strip = st; }
                    if let Some(f) = fill { c.fill = f; }
                    if let Some(sc) = scatter.clone() { c.scatter = sc; }
                    id
                }
            };
            // the strip's and fill's strokes are made now, so the curve and they are one undo step
            crate::editor::pen::lay(ed);
            crate::editor::pen::strips(ed);
            crate::editor::scatter::scatters(ed);
            ed.commit();
            Ok(describe_curve(ed.level.curve(id).expect("found")))
        }
        "turn_into_items" => {
            editable(ed)?;
            let v = args.get("shape").ok_or("Give the shape (curve id or name) whose scattered items to turn into parts")?;
            let id = ed.level.curves.iter().find(|c| v.as_u64() == Some(c.id) || v.as_str() == Some(c.name.as_str())).map(|c| c.id).ok_or_else(|| format!("No curve {v} (list_shapes shows them)"))?;
            let n = crate::editor::scatter::turn_into_items(ed, id);
            if n == 0 && ed.level.curve(id).is_some_and(|c| c.scatter.is_none()) { return Err("That curve has no scatter".into()); }
            ed.commit();
            Ok(json!({"shape": id, "items": n}))
        }
        "list_fills" => Ok(Value::Array(ed.level.buckets.iter().map(|b| describe_fill(ed, b)).collect())),
        "bucket_fill" => {
            editable(ed)?;
            if let Some(id) = args.get("delete").and_then(Value::as_u64) {
                if !ed.level.buckets.iter().any(|b| b.id == id) { return Err(format!("No fill {id} (list_fills shows them)")); }
                ed.level.buckets.retain(|b| b.id != id);
                crate::editor::bucket::fills(ed);
                ed.commit();
                return Ok(json!({"deleted": id}));
            }
            let p = part_of(ed, args)?.clone();
            if !crate::paint::paintable(&p) { return Err(format!("{} can't be filled (the bucket fills visible basic shapes)", p.name)); }
            let names = crate::shapes::build(p.shape).face_names();
            let point = args.get("point").map(|v| vec3_of(v).ok_or("point is [x, y, z]")).transpose()?;
            // where it starts: the face named, else the face under the point (looking down onto it), else the top
            let face: Option<u8> = match args.get("face").and_then(Value::as_str) {
                Some(f) => Some(names.iter().position(|n| n.eq_ignore_ascii_case(f)).ok_or_else(|| format!("{} has no face {f} (its faces: {})", p.name, names.join(", ")))? as u8),
                None => {
                    let from = point.map_or(Vec3::from(p.pos) + Vec3::Y * (Vec3::from(p.size).length() + 1.0), |q| q + Vec3::Y * 0.5);
                    let ray = Ray3d::new(from, Dir3::NEG_Y);
                    crate::world::raycast(std::slice::from_ref(&p), &ed.geo, ray, None, |_| true).and_then(|h| h.face)
                }
            };
            let at = point.unwrap_or_else(|| Vec3::from(p.pos) + Vec3::Y * p.size[1] * 0.5);
            let fills = match args.get("fills").and_then(Value::as_str) {
                None => ed.bucket.fills,
                Some(w) => crate::level::Fills::ALL.into_iter().find(|f| f.word() == w.to_lowercase()).ok_or("fills is material, colour, tag, part or floor")?,
            };
            let tag = match args.get("tag").and_then(Value::as_str) {
                None => None,
                Some(v) => {
                    let want = v.to_lowercase().replace([' ', '_'], "");
                    Some(crate::level::Tag::ALL.into_iter().find(|t| format!("{t:?}").to_lowercase() == want).ok_or_else(|| format!("No surface tag called {v}"))?)
                }
            };
            let erase = args.get("erase").and_then(Value::as_bool) == Some(true) || args.get("material").is_some_and(Value::is_null);
            let material = match (erase, args.get("material").and_then(Value::as_str)) {
                (true, _) => String::new(),
                (false, Some(m)) => material_ref(ed, m)?,
                (false, None) if ed.level.materials.contains_key(&ed.brush.material) => ed.brush.material.clone(),
                (false, None) => return Err("Give the material to fill with (or erase: true)".into()),
            };
            let lays = match args.get("lays").and_then(Value::as_str) {
                None => if erase { Default::default() } else { ed.brush.lays },
                Some("add") => crate::level::Lays::Add,
                Some("subtract") => crate::level::Lays::Subtract,
                Some("set") => crate::level::Lays::Set,
                Some(o) => return Err(format!("lays is add, subtract or set, not {o}")),
            };
            let num = |k: &str, d: f32| args.get(k).and_then(Value::as_f64).map_or(d, |v| (v as f32).clamp(0.0, 1.0));
            let b = crate::level::Bucket { id: 0, part: p.id, face, at: p.to_local(at).to_array(), fills, tolerance: num("tolerance", ed.bucket.tolerance), tag, connected: args.get("connected").and_then(Value::as_bool).unwrap_or(false), material, strength: num("strength", ed.brush.strength), lays };
            let id = crate::editor::bucket::add(ed, b);
            ed.commit();
            Ok(describe_fill(ed, ed.level.buckets.iter().find(|b| b.id == id).expect("just added")))
        }
        _ => unreachable!("routed in `call`"),
    }
}

/// A stretch from its word (ground, over, air, fall).
fn stretch_of(w: &Value) -> Option<crate::curve::Stretch> {
    crate::curve::Stretch::ALL.into_iter().find(|s| w.as_str().is_some_and(|w| stretch_word(*s) == w.to_lowercase()))
}

/// A bucket fill, as list_fills and bucket_fill say it.
fn describe_fill(ed: &Editor, b: &crate::level::Bucket) -> Value {
    let name = |id: u64| ed.level.get(id).map_or_else(|| format!("#{id} (gone)"), |p| p.name.clone());
    json!({
        "id": b.id, "from": name(b.part), "fills": b.fills.word(), "tolerance": settings::num(b.tolerance), "tag": b.tag.map(|t| format!("{t:?}")), "connected": b.connected,
        "material": b.material, "strength": settings::num(b.strength), "lays": format!("{:?}", b.lays).to_lowercase(),
        "reaches": crate::bucket::reach(&ed.level, b).iter().map(|(id, f)| json!({"part": name(*id), "faces": f.len()})).collect::<Vec<_>>(),
    })
}

/// The rules a call gives ({slope, band: [lowest, highest] or null, on: a surface tag or null}) over the ones it starts from.
fn rules_from(r: &Value, mut rules: crate::level::Rules) -> Result<crate::level::Rules, String> {
    if let Some(v) = r.get("slope").and_then(Value::as_f64) { rules.slope = (v as f32).clamp(0.0, 90.0); }
    match r.get("band") {
        Some(Value::Null) => rules.band = None,
        Some(v) => {
            let lh: Vec<f32> = v.as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect()).unwrap_or_default();
            let [lo, hi] = lh[..] else { return Err("band is [lowest, highest] in metres, or null".into()) };
            rules.band = Some([lo.min(hi), lo.max(hi)]);
        }
        None => {}
    }
    match r.get("on") {
        Some(Value::Null) => rules.on = None,
        Some(v) => {
            let want = v.as_str().unwrap_or_default().to_lowercase().replace([' ', '_'], "");
            rules.on = Some(crate::level::Tag::ALL.into_iter().find(|t| format!("{t:?}").to_lowercase() == want).ok_or_else(|| format!("No surface tag called {v} (Climbable, Diggable, Water, Grind rail, Perch spot, Slippery, Ground, Paths ignore)"))?);
        }
        None => {}
    }
    Ok(rules)
}
