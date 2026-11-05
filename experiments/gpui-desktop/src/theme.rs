//! Design tokens from `frontend/src/routes/layout.css` (default dark theme, no wallpaper).

use gpui::{rgb, rgba, Rgba};

pub const SURFACE_PRIMARY: u32 = 0x1e1e1e;
pub const SURFACE_SECONDARY: u32 = 0x252525;
pub const SURFACE_TERTIARY: u32 = 0x2a2a2a;
pub const SURFACE_ELEVATED: u32 = 0x333333;
pub const BORDER_PRIMARY: u32 = 0x333333;
pub const BORDER_SECONDARY: u32 = 0x2a2a2a;
pub const TEXT_PRIMARY: u32 = 0xe0e0e0;
pub const TEXT_SECONDARY: u32 = 0xa6a6a6;
pub const TEXT_MUTED: u32 = 0x8c8c8c;
pub const TEXT_DISABLED: u32 = 0x444444;
pub const ACCENT: u32 = 0x4a9eff;
pub const INFO: u32 = 0x4a9eff;
pub const DANGER: u32 = 0xdc3545;
pub const WARNING: u32 = 0xf39c12;
pub const FOLDER: u32 = 0xe8c36a;
pub const SELECTION: u32 = 0x2d4a6f;
pub const SELECTION_HOVER: u32 = 0x345580;
pub const WHITE: u32 = 0xffffff;

pub fn c(hex: u32) -> Rgba {
    rgb(hex)
}

/// `hex` at `alpha` (0–1), like Tailwind's `color/NN` modifier.
pub fn alpha(hex: u32, alpha: f32) -> Rgba {
    rgba((hex << 8) | (alpha.clamp(0.0, 1.0) * 255.0).round() as u32)
}
