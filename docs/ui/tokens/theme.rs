//! Beverly UI colour tokens — generated from the design canvas. Keep in sync with tokens.json.
use egui::Color32;

#[derive(Clone, Copy, Debug)]
pub struct TabTheme {
    pub name: &'static str,
    pub frame: Color32,
    pub frame_text: Color32,
    pub frame_alpha: f32,
    pub highlight: Color32,
    pub highlight_text: Color32,
    pub accent: Color32,
    pub icon: &'static str,
}

pub const INK: Color32 = Color32::from_rgb(0x0E, 0x0E, 0x10);
pub const PANEL_GLASS: Color32 = Color32::from_rgba_unmultiplied(18, 18, 21, 214);
pub const POPOVER: Color32 = Color32::from_rgb(0x1A, 0x1A, 0x1E);
pub const NODE: Color32 = Color32::from_rgb(0x17, 0x17, 0x1A);
pub const FIELD: Color32 = Color32::from_rgb(0x1F, 0x1F, 0x23);
pub const FIELD_HOVER: Color32 = Color32::from_rgb(0x26, 0x26, 0x2B);
pub const LINE: Color32 = Color32::from_rgb(0x2E, 0x2E, 0x33);
pub const DIVIDER: Color32 = Color32::from_rgba_unmultiplied(255, 255, 255, 18);
pub const TRACK: Color32 = Color32::from_rgb(0x34, 0x34, 0x3A);
pub const TOGGLE_OFF: Color32 = Color32::from_rgb(0x3A, 0x3A, 0x40);
pub const TICK: Color32 = Color32::from_rgb(0x4A, 0x4A, 0x52);
pub const TEXT: Color32 = Color32::from_rgb(0xF3, 0xF3, 0xF1);
pub const TEXT_2: Color32 = Color32::from_rgb(0xB0, 0xB0, 0xB6);
pub const TEXT_3: Color32 = Color32::from_rgb(0x8E, 0x8E, 0x95);
pub const TEXT_ON_LIGHT: Color32 = Color32::from_rgb(0x0E, 0x0E, 0x10);

pub const CAT_SHAPES: Color32 = Color32::from_rgb(0xD6, 0xD9, 0xE0);
pub const CAT_TRIGGERS: Color32 = Color32::from_rgb(0xFF, 0x7A, 0x1A);
pub const CAT_ACTIONS: Color32 = Color32::from_rgb(0x3D, 0x7B, 0xFF);
pub const CAT_FLOW: Color32 = Color32::from_rgb(0x2B, 0xF0, 0x6A);
pub const CAT_LOGIC: Color32 = Color32::from_rgb(0xB0, 0x66, 0xFF);
pub const CAT_GAME: Color32 = Color32::from_rgb(0xFF, 0x3D, 0x9A);
pub const CAT_LIGHTS: Color32 = Color32::from_rgb(0xFF, 0xE1, 0x1A);
pub const CAT_CAMERAS: Color32 = Color32::from_rgb(0x22, 0xE5, 0xFF);
pub const AXIS_X: Color32 = Color32::from_rgb(0xFF, 0x3B, 0x4E);
pub const AXIS_Y: Color32 = Color32::from_rgb(0x39, 0xF2, 0x6A);
pub const AXIS_Z: Color32 = Color32::from_rgb(0x3D, 0x7B, 0xFF);
pub const DANGER: Color32 = Color32::from_rgb(0xFF, 0x2D, 0x46);
pub const DANGER_TEXT: Color32 = Color32::from_rgb(0xFF, 0x8A, 0x98);

pub const TABS: [TabTheme; 8] = [
    TabTheme { name: "Level", frame: Color32::from_rgb(0x0E, 0x0E, 0x10), frame_text: Color32::from_rgb(0xF3, 0xF3, 0xF1), frame_alpha: 0.78, highlight: Color32::from_rgb(0xFF, 0xD6, 0x0A), highlight_text: Color32::from_rgb(0x0E, 0x0E, 0x10), accent: Color32::from_rgb(0x8C, 0x7D, 0xFF), icon: "mountain" },
    TabTheme { name: "Characters", frame: Color32::from_rgb(0xFF, 0x6E, 0xB4), frame_text: Color32::from_rgb(0x0E, 0x0E, 0x10), frame_alpha: 0.86, highlight: Color32::from_rgb(0xFF, 0x6E, 0xB4), highlight_text: Color32::from_rgb(0x0E, 0x0E, 0x10), accent: Color32::from_rgb(0x2B, 0xF0, 0xC8), icon: "person" },
    TabTheme { name: "Vehicles", frame: Color32::from_rgb(0xC6, 0xFF, 0x1A), frame_text: Color32::from_rgb(0x0E, 0x0E, 0x10), frame_alpha: 0.86, highlight: Color32::from_rgb(0xC6, 0xFF, 0x1A), highlight_text: Color32::from_rgb(0x0E, 0x0E, 0x10), accent: Color32::from_rgb(0xB3, 0x4D, 0xFF), icon: "car" },
    TabTheme { name: "UI/HUD", frame: Color32::from_rgb(0x2E, 0x4B, 0xF5), frame_text: Color32::from_rgb(0xFF, 0xFF, 0xFF), frame_alpha: 0.86, highlight: Color32::from_rgb(0x5C, 0x78, 0xFF), highlight_text: Color32::from_rgb(0xFF, 0xFF, 0xFF), accent: Color32::from_rgb(0xFF, 0xB0, 0x20), icon: "layout" },
    TabTheme { name: "Audio", frame: Color32::from_rgb(0x19, 0xE0, 0xFF), frame_text: Color32::from_rgb(0x0E, 0x0E, 0x10), frame_alpha: 0.86, highlight: Color32::from_rgb(0x19, 0xE0, 0xFF), highlight_text: Color32::from_rgb(0x0E, 0x0E, 0x10), accent: Color32::from_rgb(0xFF, 0x7A, 0x59), icon: "wave" },
    TabTheme { name: "Effects", frame: Color32::from_rgb(0xFF, 0x8A, 0x1F), frame_text: Color32::from_rgb(0x0E, 0x0E, 0x10), frame_alpha: 0.86, highlight: Color32::from_rgb(0xFF, 0x8A, 0x1F), highlight_text: Color32::from_rgb(0x0E, 0x0E, 0x10), accent: Color32::from_rgb(0x1E, 0xA7, 0xFF), icon: "flame" },
    TabTheme { name: "Screens", frame: Color32::from_rgb(0xB7, 0x7B, 0xFF), frame_text: Color32::from_rgb(0x0E, 0x0E, 0x10), frame_alpha: 0.86, highlight: Color32::from_rgb(0xB7, 0x7B, 0xFF), highlight_text: Color32::from_rgb(0x0E, 0x0E, 0x10), accent: Color32::from_rgb(0xD8, 0xFF, 0x4A), icon: "monitor" },
    TabTheme { name: "Animatics", frame: Color32::from_rgb(0xE9, 0xE7, 0xE2), frame_text: Color32::from_rgb(0x0E, 0x0E, 0x10), frame_alpha: 0.86, highlight: Color32::from_rgb(0xFF, 0x2D, 0x46), highlight_text: Color32::from_rgb(0x0E, 0x0E, 0x10), accent: Color32::from_rgb(0x19, 0xD3, 0xC5), icon: "film" },
];

pub mod size {
    pub const TOP_BAR: f32 = 44.0;
    pub const RIM: f32 = 8.0;
    pub const VIEW_RADIUS: f32 = 14.0;
    pub const NOTCH_DEPTH: f32 = 34.0;
    pub const WINDOW_RADIUS: f32 = 10.0;
    pub const DOCK_WIDTH: f32 = 352.0;
    pub const DOCK_INSET: f32 = 12.0;
    pub const PANEL_RADIUS: f32 = 16.0;
    pub const CONTROL_RADIUS: f32 = 7.0;
    pub const ROW_HEIGHT: f32 = 28.0;
    pub const ROW_GAP: f32 = 8.0;
}
