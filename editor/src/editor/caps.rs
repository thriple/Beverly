//! Caps Lock is the movement-mode switch, and nothing else: it must not leave text in capitals. On Windows, each
//! time the key turns capitals on, the editor sends one more Caps Lock press to turn them back off, and ignores
//! that echo. (Other platforms: the mode switch works; capitals still toggle.)
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct CapsLock { ignore: u8 }

impl CapsLock {
    /// True on a real Caps Lock press.
    pub fn pressed(&mut self, keys: &ButtonInput<KeyCode>) -> bool {
        if !keys.just_pressed(KeyCode::CapsLock) { return false; }
        if self.ignore > 0 {
            self.ignore -= 1;
            return false;
        }
        self.turn_off();
        true
    }

    /// If capitals are on, turn them off (also used at start-up).
    pub fn turn_off(&mut self) {
        #[cfg(windows)]
        if win::caps_on() {
            win::toggle();
            self.ignore += 1;
        }
    }
}

pub fn startup(mut caps: ResMut<CapsLock>) { caps.turn_off(); }

#[cfg(windows)]
mod win {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_CAPITAL,
    };

    pub fn caps_on() -> bool { unsafe { GetKeyState(VK_CAPITAL as i32) & 1 != 0 } }

    pub fn toggle() {
        let key = |flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_CAPITAL, wScan: 0x3a, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
        };
        let inputs = [key(0), key(KEYEVENTF_KEYUP)];
        unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    }
}
