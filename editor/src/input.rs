//! The input map: play reads named actions (Forward, Jump, Use...), never keys, so a game's controls can be changed
//! without touching what reads them, gamepads work beside the keyboard, and input can later come over a network.
//! A game's controls live in its project file (`controls`, left out while they are Beverly's defaults); each action
//! lists its inputs by name: keys ("W", "Space", "Shift", "F1"), mouse buttons ("MouseLeft") and gamepad buttons and
//! stick directions ("Pad South", "Pad LeftStick Up"). Turning the view with the mouse is not an action.
use bevy::input::InputSystems;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One action and the inputs that do it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub name: String,
    pub inputs: Vec<String>,
}

/// A game's controls: its actions in the order they are shown.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Controls(pub Vec<Action>);

/// Beverly's actions and their default inputs: the walker's. (name, what it does, inputs)
pub const DEFAULTS: [(&str, &str, &[&str]); 11] = [
    ("Forward", "Walk forward", &["W", "Up", "Pad LeftStick Up"]),
    ("Back", "Walk back", &["S", "Down", "Pad LeftStick Down"]),
    ("Left", "Step left", &["A", "Left", "Pad LeftStick Left"]),
    ("Right", "Step right", &["D", "Right", "Pad LeftStick Right"]),
    ("Run", "Run while held", &["Shift", "Pad LeftThumb"]),
    ("Jump", "Jump", &["Space", "Pad South"]),
    ("Use", "Press buttons and use things", &["E", "Pad West"]),
    ("Look left", "Turn the view (the mouse always turns it too)", &["Pad RightStick Left"]),
    ("Look right", "Turn the view", &["Pad RightStick Right"]),
    ("Look up", "Tilt the view", &["Pad RightStick Up"]),
    ("Look down", "Tilt the view", &["Pad RightStick Down"]),
];

impl Default for Controls {
    fn default() -> Self {
        Controls(DEFAULTS.iter().map(|(n, _, i)| Action { name: (*n).into(), inputs: i.iter().map(|s| (*s).into()).collect() }).collect())
    }
}

impl Controls {
    pub fn is_default(&self) -> bool { *self == Controls::default() }
    pub fn get(&self, name: &str) -> Option<&Action> { self.0.iter().find(|a| a.name == name) }
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Action> { self.0.iter_mut().find(|a| a.name == name) }
    /// Beverly's actions a file from before them leaves out, added with their defaults (a file keeps its own order).
    pub fn with_defaults(mut self) -> Self {
        for d in Controls::default().0 {
            if self.get(&d.name).is_none() { self.0.push(d); }
        }
        self
    }
    /// What an action does, for Beverly's own actions.
    pub fn help(name: &str) -> &'static str { DEFAULTS.iter().find(|d| d.0 == name).map_or("", |d| d.1) }
}

/// A physical input.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Input {
    Key(KeyCode),
    Mouse(MouseButton),
    Pad(GamepadButton),
    /// A stick direction: the axis and which way (1 or -1).
    Stick(GamepadAxis, f32),
}

/// The keys an action can use, by the name files and panels show.
fn keys() -> Vec<(String, KeyCode)> {
    use KeyCode::*;
    let mut v: Vec<(String, KeyCode)> = [
        ("Space", Space), ("Shift", ShiftLeft), ("Right shift", ShiftRight), ("Ctrl", ControlLeft), ("Right ctrl", ControlRight),
        ("Alt", AltLeft), ("Right alt", AltRight), ("Tab", Tab), ("Enter", Enter), ("Escape", Escape), ("Backspace", Backspace),
        ("Caps lock", CapsLock), ("Up", ArrowUp), ("Down", ArrowDown), ("Left", ArrowLeft), ("Right", ArrowRight),
        ("Comma", Comma), ("Period", Period), ("Slash", Slash), ("Semicolon", Semicolon), ("Quote", Quote),
        ("Minus", Minus), ("Equal", Equal), ("Backquote", Backquote), ("[", BracketLeft), ("]", BracketRight),
    ].into_iter().map(|(n, k)| (n.to_string(), k)).collect();
    const LETTERS: [KeyCode; 26] = [KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP,
        KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ];
    const DIGITS: [KeyCode; 10] = [Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9];
    const FS: [KeyCode; 12] = [F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12];
    v.extend(LETTERS.iter().enumerate().map(|(i, k)| (((b'A' + i as u8) as char).to_string(), *k)));
    v.extend(DIGITS.iter().enumerate().map(|(i, k)| (i.to_string(), *k)));
    v.extend(FS.iter().enumerate().map(|(i, k)| (format!("F{}", i + 1), *k)));
    v
}

const MOUSE: [(&str, MouseButton); 5] = [("MouseLeft", MouseButton::Left), ("MouseRight", MouseButton::Right),
    ("MouseMiddle", MouseButton::Middle), ("MouseBack", MouseButton::Back), ("MouseForward", MouseButton::Forward)];

const PAD: [(&str, GamepadButton); 16] = {
    use GamepadButton::*;
    [("South", South), ("East", East), ("North", North), ("West", West), ("LeftBumper", LeftTrigger),
        ("RightBumper", RightTrigger), ("LeftTrigger", LeftTrigger2), ("RightTrigger", RightTrigger2), ("Select", Select),
        ("Start", Start), ("LeftThumb", LeftThumb), ("RightThumb", RightThumb), ("DPadUp", DPadUp), ("DPadDown", DPadDown),
        ("DPadLeft", DPadLeft), ("DPadRight", DPadRight)]
};

const STICKS: [(&str, GamepadAxis, f32); 8] = [
    ("LeftStick Up", GamepadAxis::LeftStickY, 1.0), ("LeftStick Down", GamepadAxis::LeftStickY, -1.0),
    ("LeftStick Left", GamepadAxis::LeftStickX, -1.0), ("LeftStick Right", GamepadAxis::LeftStickX, 1.0),
    ("RightStick Up", GamepadAxis::RightStickY, 1.0), ("RightStick Down", GamepadAxis::RightStickY, -1.0),
    ("RightStick Left", GamepadAxis::RightStickX, -1.0), ("RightStick Right", GamepadAxis::RightStickX, 1.0),
];

/// An input from its name ("w" and "W" are the same key).
pub fn parse(name: &str) -> Option<Input> {
    if let Some(p) = name.strip_prefix("Pad ") {
        if let Some((_, b)) = PAD.iter().find(|(n, _)| *n == p) { return Some(Input::Pad(*b)); }
        return STICKS.iter().find(|(n, _, _)| *n == p).map(|(_, a, s)| Input::Stick(*a, *s));
    }
    if let Some((_, b)) = MOUSE.iter().find(|(n, _)| *n == name) { return Some(Input::Mouse(*b)); }
    keys().into_iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, k)| Input::Key(k))
}

/// An input's name, for the inputs [`parse`] knows.
pub fn name(i: Input) -> Option<String> {
    match i {
        Input::Key(k) => keys().into_iter().find(|(_, c)| *c == k).map(|(n, _)| n),
        Input::Mouse(b) => MOUSE.iter().find(|(_, m)| *m == b).map(|(n, _)| n.to_string()),
        Input::Pad(b) => PAD.iter().find(|(_, p)| *p == b).map(|(n, _)| format!("Pad {n}")),
        Input::Stick(a, s) => STICKS.iter().find(|(_, x, y)| *x == a && *y == s).map(|(n, _, _)| format!("Pad {n}")),
    }
}

/// How far a stick has to lean before it counts.
const DEAD: f32 = 0.15;

/// What each action is doing this frame (0 to 1: keys and buttons are 0 or 1, sticks and triggers in between).
#[derive(Resource, Default)]
pub struct Actions {
    now: HashMap<String, f32>,
    was: HashMap<String, f32>,
    /// The controls being read; play sets them from the project when it starts.
    pub controls: Controls,
}

impl Actions {
    pub fn value(&self, action: &str) -> f32 { self.now.get(action).copied().unwrap_or(0.0) }
    pub fn pressed(&self, action: &str) -> bool { self.value(action) > 0.5 }
    pub fn just_pressed(&self, action: &str) -> bool { self.pressed(action) && self.was.get(action).copied().unwrap_or(0.0) <= 0.5 }
    /// Two pairs of actions as one direction: x from `left` to `right`, y from `back` to `forward`, at most 1 long.
    pub fn stick(&self, left: &str, right: &str, back: &str, forward: &str) -> Vec2 {
        let v = Vec2::new(self.value(right) - self.value(left), self.value(forward) - self.value(back));
        if v.length() > 1.0 { v.normalize() } else { v }
    }
}

/// Reads every action from the keyboard, the mouse and the gamepads, once a frame before anything uses them.
pub fn read(mut actions: ResMut<Actions>, keys: Res<ButtonInput<KeyCode>>, mouse: Res<ButtonInput<MouseButton>>, pads: Query<&Gamepad>) {
    let actions = &mut *actions;
    actions.was = std::mem::take(&mut actions.now);
    for a in &actions.controls.0 {
        let v = a.inputs.iter().filter_map(|n| parse(n)).map(|i| match i {
            Input::Key(k) => keys.pressed(k) as u8 as f32,
            Input::Mouse(b) => mouse.pressed(b) as u8 as f32,
            Input::Pad(b) => pads.iter().map(|p| p.get(b).unwrap_or(p.pressed(b) as u8 as f32)).fold(0.0, f32::max),
            Input::Stick(axis, s) => pads.iter().map(|p| ((p.get(axis).unwrap_or(0.0) * s - DEAD) / (1.0 - DEAD)).clamp(0.0, 1.0)).fold(0.0, f32::max),
        }).fold(0.0, f32::max);
        if v > 0.0 { actions.now.insert(a.name.clone(), v); }
    }
}

/// The first input pressed this frame, by name: for choosing an action's input by pressing it.
pub fn first_pressed(keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>, pads: &Query<&Gamepad>) -> Option<String> {
    keys.get_just_pressed().find_map(|k| name(Input::Key(*k)))
        .or_else(|| mouse.get_just_pressed().find_map(|b| name(Input::Mouse(*b))))
        .or_else(|| pads.iter().find_map(|p| p.get_just_pressed().find_map(|b| name(Input::Pad(*b)))))
        .or_else(|| pads.iter().find_map(|p| STICKS.iter().find(|(_, a, s)| p.get(*a).unwrap_or(0.0) * s > 0.7).map(|(n, _, _)| format!("Pad {n}"))))
}

pub struct InputMapPlugin;

impl Plugin for InputMapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Actions>().add_systems(PreUpdate, read.after(InputSystems));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_input_name_reads_back_as_itself() {
        for (n, _) in keys() { assert_eq!(parse(&n).and_then(name), Some(n)); }
        for n in MOUSE.iter().map(|m| m.0.to_string()).chain(PAD.iter().map(|p| format!("Pad {}", p.0))).chain(STICKS.iter().map(|s| format!("Pad {}", s.0))) {
            assert_eq!(parse(&n).and_then(name), Some(n));
        }
        assert_eq!(parse("w"), Some(Input::Key(KeyCode::KeyW)));
        assert_eq!(parse("Pad Nothing"), None);
        for d in DEFAULTS { for i in d.2 { assert!(parse(i).is_some(), "{i}"); } }
    }

    #[test]
    fn actions_read_keys_buttons_and_sticks_and_know_when_they_were_just_pressed() {
        let mut app = App::new();
        app.add_plugins(InputMapPlugin).init_resource::<ButtonInput<KeyCode>>().init_resource::<ButtonInput<MouseButton>>();
        app.world_mut().resource_mut::<Actions>().controls.get_mut("Jump").unwrap().inputs.push("MouseRight".into());
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyW);
        app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Right);
        app.update();
        let a = app.world().resource::<Actions>();
        assert!(a.pressed("Forward") && a.just_pressed("Forward") && a.pressed("Jump") && !a.pressed("Back"));
        assert_eq!(a.stick("Left", "Right", "Back", "Forward"), Vec2::Y);
        app.update();
        let a = app.world().resource::<Actions>();
        assert!(a.pressed("Forward") && !a.just_pressed("Forward"));
        // a gamepad's stick, half over: walks at half speed
        let mut pad = Gamepad::default();
        pad.analog_mut().set(GamepadAxis::LeftStickX, 0.575);
        app.world_mut().spawn(pad);
        app.update();
        assert!((app.world().resource::<Actions>().value("Right") - 0.5).abs() < 0.01);
    }

    #[test]
    fn files_from_before_an_action_get_it_with_its_defaults() {
        let mut c = Controls::default();
        c.0.retain(|a| a.name != "Use");
        c.get_mut("Jump").unwrap().inputs = vec!["K".into()];
        let c = c.with_defaults();
        assert_eq!(c.get("Use").unwrap().inputs, vec!["E", "Pad West"]);
        assert_eq!(c.get("Jump").unwrap().inputs, vec!["K"]);
    }
}
