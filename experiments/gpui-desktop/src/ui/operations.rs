//! Small keyboard-driven dialogs for file mutations.

use gpui::{div, prelude::*, px, Context, MouseButton};

use super::{icon, TextScale};
use crate::theme::*;
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn render_operation_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(dialog) = &self.operation_dialog else {
            return div().into_any_element();
        };
        let title = dialog.title();
        let placeholder = dialog.placeholder();
        let (before, selected, after) = dialog.input().display_parts();
        let before = before.to_string();
        let selected = selected.map(str::to_owned);
        let after = after.to_string();
        let has_value = !dialog.input().is_empty();
        let input = div()
            .flex()
            .items_center()
            .when(!has_value, |element| {
                element.text_color(c(TEXT_MUTED)).child(placeholder)
            })
            .when(has_value, |element| {
                element
                    .text_color(c(TEXT_PRIMARY))
                    .child(before)
                    .when_some(selected, |element, selected| {
                        element
                            .bg(c(self.accent_hex()))
                            .text_color(c(SURFACE_PRIMARY))
                            .child(selected)
                    })
                    .child(after)
            });
        div()
            .id("operation-overlay")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(alpha(0x000000, 0.58))
            .child(
                div()
                    .id("operation-dialog")
                    .w(px(420.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(c(BORDER_PRIMARY))
                    .bg(c(SURFACE_PRIMARY))
                    .shadow_lg()
                    .p(px(18.))
                    .child(
                        div()
                            .text_sm_tw()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .id("operation-input")
                            .key_context("OperationInput")
                            .track_focus(&self.operation_focus)
                            .on_key_down(cx.listener(|this, event, _, cx| this.operation_key_down(event, cx)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    window.focus(&this.operation_focus);
                                    cx.stop_propagation();
                                }),
                            )
                            .mt(px(14.))
                            .w_full()
                            .h(px(32.))
                            .flex()
                            .items_center()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(c(self.accent_hex()))
                            .bg(c(SURFACE_SECONDARY))
                            .px(px(10.))
                            .text_sm_tw()
                            .child(input),
                    )
                    .child(
                        div()
                            .mt(px(10.))
                            .text_xs_tw()
                            .text_color(c(TEXT_MUTED))
                            .child("Enter to save · Escape to cancel"),
                    )
                    .child(
                        div().mt(px(14.)).flex().justify_end().child(
                            div()
                                .id("operation-cancel")
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .rounded(px(4.))
                                .px(px(10.))
                                .py(px(6.))
                                .cursor_pointer()
                                .hover(|style| style.bg(c(SURFACE_ELEVATED)))
                                .child(icon("x", 14., c(TEXT_SECONDARY)))
                                .child("Cancel")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.operation_dialog = None;
                                    window.focus(&this.focus_handle);
                                    cx.notify();
                                })),
                        ),
                    ),
            )
            .into_any_element()
    }
}
