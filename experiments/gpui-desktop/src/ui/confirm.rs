//! The confirm dialog on `Modal`: a message, the affected names, and one action.

use gpui::{div, prelude::*, px, AnyElement, Context, MouseButton};

use super::TextScale;
use crate::components::{Button, ButtonVariant, Modal};
use crate::theme::*;
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn render_confirm(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(confirm) = &self.confirm else { return div().into_any_element() };
        // flex flex-col gap-3 text-[13px] text-text-secondary; names in a max-h-40 list
        let body = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .text_13()
            .text_color(c(TEXT_SECONDARY))
            .child(confirm.message.clone())
            .when(!confirm.items.is_empty(), |body| {
                body.child(
                    div()
                        .id("confirm-items")
                        .max_h(px(160.))
                        .overflow_y_scroll()
                        .children(confirm.items.iter().map(|name| div().py(px(2.)).truncate().text_color(c(TEXT_PRIMARY)).child(name.clone()))),
                )
            });
        div()
            .id("confirm-overlay")
            .absolute()
            .inset_0()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Modal::new()
                    .title(confirm.title.clone())
                    .close_selector("confirm-cancel")
                    .on_close(cx.listener(|this, _, _, cx| this.answer(false, cx)))
                    .body(body)
                    .footer(
                        Button::new("confirm-accept")
                            .label(confirm.confirm_label.clone())
                            .variant(if confirm.danger { ButtonVariant::Danger } else { ButtonVariant::Primary })
                            .on_click(cx.listener(|this, _, _, cx| this.answer(true, cx))),
                    ),
            )
            .into_any_element()
    }
}
