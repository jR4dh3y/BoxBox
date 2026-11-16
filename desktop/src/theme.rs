//! Design tokens. The colours are generated from the `@theme` block of
//! `frontend/src/routes/layout.css` by `build.rs`.

use gpui::{rgb, rgba, Rgba};

include!(concat!(env!("OUT_DIR"), "/tokens.rs"));

pub const WHITE: u32 = 0xffffff;

pub fn c(hex: u32) -> Rgba {
    rgb(hex)
}

/// `hex` at `alpha` (0–1), like Tailwind's `color/NN` modifier.
pub fn alpha(hex: u32, alpha: f32) -> Rgba {
    rgba((hex << 8) | (alpha.clamp(0.0, 1.0) * 255.0).round() as u32)
}
