//! Playing through the MCP server: keys the assistant holds, calls answered after a wait, the play state.
use super::*;

/// Keys the assistant holds down (until a time on the real clock), and a turn of the view to make.
#[derive(Resource, Default)]
pub struct Injected {
    pub(super) held: Vec<(Input, f64)>,
    pub(super) look: Vec2,
}

/// Press the assistant's keys after the real input is read (as the scripted tests do), and let go when their time is up.
pub fn inject(mut inj: ResMut<Injected>, time: Res<Time<Real>>, mut keys: ResMut<ButtonInput<KeyCode>>, mut mouse: ResMut<ButtonInput<MouseButton>>, mut motion: ResMut<AccumulatedMouseMotion>) {
    let now = time.elapsed_secs_f64();
    for (i, until) in &inj.held {
        let up = *until <= now;
        match (*i, up) {
            (Input::Key(k), false) => keys.press(k),
            (Input::Key(k), true) => keys.release(k),
            (Input::Mouse(b), false) => mouse.press(b),
            (Input::Mouse(b), true) => mouse.release(b),
        }
    }
    inj.held.retain(|(_, until)| *until > now);
    if inj.look != Vec2::ZERO {
        motion.delta += inj.look;
        inj.look = Vec2::ZERO;
    }
}

/// A call answered with the play state once its time comes (after keys are let go, or once test mode has started).
pub struct Waiting {
    pub(super) reply: std::sync::mpsc::Sender<Result<Output, String>>,
    pub(super) until: f64,
    pub(super) frames: u8,
}

/// What `serve` reads while playing.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Playing<'w, 's> {
    pub(super) play: Res<'w, Play>,
    pub(super) time: Res<'w, Time<Real>>,
    pub(super) injected: ResMut<'w, Injected>,
    pub(super) parts: Query<'w, 's, (&'static PartId, &'static Transform)>,
    pub(super) asleep: Query<'w, 's, (&'static crate::physics::PartBody, Has<avian3d::prelude::Sleeping>)>,
    pub(super) blasts: MessageWriter<'w, crate::physics::Blast>,
    pub(super) pulls: MessageWriter<'w, crate::physics::Pull>,
}

/// Start or stop test mode, or hold keys: returns when (on the real clock) to answer with the play state.
pub(super) fn playing(ed: &mut Editor, view: &View, pl: &mut Playing, name: &str, args: &Value, now: f64) -> Result<f64, String> {
    match name {
        "start_test" => {
            if ed.testing.is_some() { return Err("Already testing (stop_test first, or get_play to see it)".into()); }
            if ed.pending.is_some() { return Err("The unsaved-changes question is open".into()); }
            if ed.drag.is_some() { return Err("The user is dragging something; try again in a moment".into()); }
            let start = ed.level.parts.iter().find(|p| p.shape == Shape::PlayerStart).map(|p| Vec3::from(p.pos) - Vec3::Y * p.size[1] * 0.5);
            let feet = match args.get("from") {
                Some(Value::Array(_)) => vec3_of(&args["from"]).ok_or("from is \"player_start\", \"view\" or [x, y, z]")?,
                Some(Value::String(s)) if s == "view" => {
                    let ray = Ray3d::new(view.pos, Dir3::new(view.forward()).unwrap_or(Dir3::NEG_Z));
                    ed.hit(ray).map(|h| h.point).unwrap_or(view.pos.with_y(0.0))
                }
                None | Some(Value::String(_)) => start.ok_or("There's no Player start (place one with add_part, or give from: \"view\" or [x, y, z])")?,
                _ => return Err("from is \"player_start\", \"view\" or [x, y, z]".into()),
            };
            ed.test_request = Some(TestRequest::Start(feet));
            ed.parts_open = false;
            Ok(now)
        }
        "stop_test" => {
            if ed.testing.is_none() { return Err("Not testing".into()); }
            ed.test_request = Some(TestRequest::Stop);
            pl.injected.held.clear();
            Ok(now)
        }
        "press_keys" => {
            if ed.testing.is_none() { return Err("Not testing (start_test first): keys only go to the game".into()); }
            let secs = args.get("secs").and_then(Value::as_f64).unwrap_or(0.1).clamp(0.02, 30.0);
            let names: Vec<&str> = args.get("keys").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            let mut held = vec![];
            for n in &names {
                let i = input(n).ok_or(format!("No key called {n} (letters, digits, Space, Shift, Ctrl, Alt, Tab, Enter, Escape, Up, Down, Left, Right, MouseLeft, MouseRight)"))?;
                held.push((i, now + secs));
            }
            if let Some(l) = args.get("look") {
                let a = l.as_array().filter(|a| a.len() == 2).and_then(|a| Some(Vec2::new(a[0].as_f64()? as f32, a[1].as_f64()? as f32)));
                pl.injected.look += a.ok_or("look is [dx, dy] in mouse pixels")?;
            }
            pl.injected.held.extend(held);
            Ok(now + secs)
        }
        "blast" | "pull" => {
            if ed.testing.is_none() { return Err("Not testing (start_test first): blasts and pulls happen in the game".into()); }
            let secs = args.get("secs").and_then(Value::as_f64).unwrap_or(1.0).clamp(0.02, 30.0);
            if name == "blast" {
                let at = args.get("at").and_then(vec3_of).ok_or("at is [x, y, z]")?;
                let radius = args.get("radius").and_then(Value::as_f64).unwrap_or(4.0) as f32;
                let strength = args.get("strength").and_then(Value::as_f64).unwrap_or(200.0) as f32;
                pl.blasts.write(crate::physics::Blast { at, radius, strength });
            } else {
                let part = part_ref(ed, args.get("part"))?;
                if !ed.level.get(part).is_some_and(|p| crate::physics::kind(p) == Some(avian3d::prelude::RigidBody::Dynamic)) { return Err("That part isn't loose".into()); }
                let to = args.get("toward").and_then(vec3_of).ok_or("toward is [x, y, z]")?;
                let force = args.get("force").and_then(Value::as_f64).unwrap_or(500.0) as f32;
                pl.pulls.write(crate::physics::Pull { part, to, force, secs: secs as f32 });
            }
            Ok(now + secs)
        }
        _ => unreachable!(),
    }
}

/// The player and the moving parts, while testing.
pub(super) fn play_state(ed: &Editor, pl: &Playing) -> Value {
    if ed.testing.is_none() || !pl.play.on { return json!({"testing": false}); }
    let p = &pl.play;
    let mut player = json!({"feet": settings::nums(&p.feet.to_array())});
    if p.custom.is_none() {
        let w = &p.walker;
        player["velocity"] = settings::nums(&w.vel.to_array());
        player["on_ground"] = json!(w.ground);
        player["facing"] = settings::num((w.facing.to_degrees() * 10.0).round() / 10.0);
        // what it stands on, with its surface tags
        if let Some(q) = w.standing_on().and_then(|id| ed.level.get(id)) {
            player["standing_on"] = json!({"id": q.id, "name": q.name, "tags": tags_of(q)});
        }
    } else {
        player["character"] = json!("the game's own");
    }
    // the zones the player is in, and how strongly things fall where they stand
    player["in_zones"] = json!(p.script.zones_in().iter().filter_map(|id| ed.level.get(*id)).map(|q| json!({"id": q.id, "name": q.name})).collect::<Vec<_>>());
    player["gravity"] = settings::num(p.script.gravity_at(p.feet));
    let moving: Vec<Value> = ed.level.parts.iter().filter(|q| q.mover.is_some()).map(|q| {
        let live = pl.parts.iter().find(|(id, _)| id.0 == q.id).map(|(_, t)| t.translation);
        let moved = live.map(|l| l - part_transform(q).translation).unwrap_or(Vec3::ZERO);
        json!({"id": q.id, "name": q.name, "position": settings::nums(&(Vec3::from(q.pos) + moved).to_array()), "moved_by": settings::nums(&moved.to_array())})
    }).collect();
    let loose: Vec<Value> = ed.level.parts.iter().filter(|q| crate::physics::kind(q) == Some(avian3d::prelude::RigidBody::Dynamic)).map(|q| {
        let live = pl.parts.iter().find(|(id, _)| id.0 == q.id).map(|(_, t)| t.translation).unwrap_or(Vec3::from(q.pos));
        let asleep = pl.asleep.iter().any(|(b, s)| b.0 == q.id && s);
        json!({"id": q.id, "name": q.name, "position": settings::nums(&live.to_array().map(|x| (x * 1e3).round() / 1e3)), "moved_by": settings::nums(&(live - Vec3::from(q.pos)).to_array().map(|x| (x * 1e3).round() / 1e3)), "asleep": asleep})
    }).collect();
    json!({"testing": true, "player": player, "moving_parts": moving, "loose_parts": loose, "controls": p.custom.unwrap_or("WASD walk · Shift run · Space jump · E press")})
}
