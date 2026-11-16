//! `lib/components/ui/Select.svelte`: the closed control. Its option list
//! opens as a `ContextMenu`.

use gpui::{div, prelude::*, px, rgb, App, ElementId, MouseButton, MouseDownEvent, SharedString, Window};

use super::{Type, ROUNDED};
use crate::theme::*;
use crate::ui::icon;

type PressHandler = Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Select {
    id: ElementId,
    label: SharedString,
    swatch: Option<u32>,
    disabled: bool,
    on_press: Option<PressHandler>,
}

impl Select {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self { id: id.into(), label: label.into(), swatch: None, disabled: false, on_press: None }
    }

    /// A colour dot before the label, as `0xRRGGBB`.
    pub fn swatch(mut self, swatch: Option<u32>) -> Self {
        self.swatch = swatch;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Runs when the control is pressed, as a native combo box opens on press.
    pub fn on_press(mut self, handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_press = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Select {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        // h-8 w-full rounded border bg-surface-primary pr-8 pl-3 (pl-8 with a swatch)
        // text-[13px] leading-none; chevron 14px at right-2; swatch h-3 w-3 at left-3
        div()
            .id(self.id)
            .relative()
            .h(px(32.))
            .w_full()
            .flex()
            .items_center()
            .rounded(px(ROUNDED))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_PRIMARY))
            .pl(px(if self.swatch.is_some() { 32. } else { 12. }))
            .pr(px(32.))
            .type_13()
            .line_height(px(13.))
            .text_color(c(TEXT_PRIMARY))
            .whitespace_nowrap()
            .when(self.disabled, |el| el.opacity(0.5))
            .when(!self.disabled, |el| el.cursor_pointer())
            .when_some(self.swatch, |el, swatch| {
                el.child(div().absolute().left(px(11.)).size(px(12.)).rounded_full().bg(rgb(swatch)))
            })
            .child(self.label)
            .child(div().absolute().right(px(7.)).flex().items_center().h_full().child(icon("chevron-down", 14., c(TEXT_MUTED))))
            .when_some(self.on_press.filter(|_| !self.disabled), |el, handler| el.on_mouse_down(MouseButton::Left, handler))
    }
}
