//! Components, one per Svelte component they reproduce.

mod browser;
mod conflicts;
mod confirm;
mod drives;
mod file_grid;
mod file_list;
mod notifications;
mod open_with;
mod operations;
mod overlays;
mod select_menu;
mod settings;
pub(crate) use settings::SettingsField;
mod sidebar;
mod status_bar;
mod tabs;
mod toolbar;

use gpui::{px, svg, Rgba, Styled, Svg};

/// Sidebar width (`w-[220px]`), which panes subtract for their layout breakpoints.
pub const SIDEBAR_WIDTH: f32 = 220.;

/// A Lucide icon at `size` px, tinted like `currentColor`.
pub fn icon(name: &str, size: f32, color: Rgba) -> Svg {
    svg()
        .path(format!("icons/{name}.svg"))
        .size(px(size))
        .flex_none()
        .text_color(color)
}

/// The same icon drawn with `strokeWidth={1.7}`.
pub fn thin_icon(name: &str, size: f32, color: Rgba) -> Svg {
    svg()
        .path(format!("icons/thin/{name}.svg"))
        .size(px(size))
        .flex_none()
        .text_color(color)
}

/// Tailwind type scale: font size with its paired line height.
pub trait TextScale: Styled + Sized {
    /// `text-xs`: 12px / 16px.
    fn text_xs_tw(self) -> Self {
        self.text_size(px(12.)).line_height(px(16.))
    }
    /// `text-sm`: 14px / 20px.
    fn text_sm_tw(self) -> Self {
        self.text_size(px(14.)).line_height(px(20.))
    }
    /// `text-[13px]` inheriting the body's 1.5 line height.
    fn text_13(self) -> Self {
        self.text_size(px(13.)).line_height(px(19.5))
    }
    /// `text-[11px]` inheriting the body's 1.5 line height.
    fn text_11(self) -> Self {
        self.text_size(px(11.)).line_height(px(16.5))
    }
}

impl<T: Styled> TextScale for T {}
