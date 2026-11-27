//! Chromium's checkbox and radio with `accent-color` in its dark scheme, as the
//! web app draws them: a dark mark with a light border, or the accent with a
//! white tick or dot. Callers make them clickable and position them.

use gpui::{div, prelude::*, px, Div, Rgba};

use crate::theme::*;
use crate::ui::icon;

const OFF_FILL: u32 = 0x3b3b3b;
const OFF_BORDER: u32 = 0x858585;

/// A square mark, `size` across.
pub fn checkbox(checked: bool, accent: Rgba, size: f32) -> Div {
    div().size(px(size)).flex_none().flex().items_center().justify_center().rounded(px(2.)).map(|mark| match checked {
        true => mark.bg(accent).child(icon("check", size * 0.75, c(WHITE))),
        false => mark.bg(c(OFF_FILL)).border_1().border_color(c(OFF_BORDER)),
    })
}

/// A round mark for one choice among several.
pub fn radio(checked: bool, accent: Rgba) -> Div {
    div().size(px(14.)).flex_none().flex().items_center().justify_center().rounded_full().map(|mark| match checked {
        true => mark.bg(accent).child(div().size(px(6.)).rounded_full().bg(c(WHITE))),
        false => mark.bg(c(OFF_FILL)).border_1().border_color(c(OFF_BORDER)),
    })
}
