//! `BrowseDialogs.svelte`: the name dialog for new folders, new files and rename.

use gpui::{div, prelude::*, px, Context};

use crate::components::{Button, Input, Modal};
use crate::theme::*;
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn render_operation_dialog(&self, window: &gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(dialog) = &self.operation_dialog else {
            return div().into_any_element();
        };
        let busy = self.operation_status.is_busy();
        let error = self.dialog_error.clone();
        let field = self.dialog_field.clone();
        let focused = field.read(cx).is_focused(window);
        div()
            .id("operation-overlay")
            .absolute()
            .inset_0()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Modal::new()
                    .title(dialog.title())
                    .on_close(cx.listener(|this, _, window, cx| this.close_dialog(window, cx)))
                    .body(Input::new("").placeholder(dialog.placeholder()).focused(focused).editor(field))
                    .when_some(error, |modal, error| {
                        modal.body(div().mt(px(8.)).text_size(px(12.)).text_color(c(DANGER)).child(error))
                    })
                    .footer(
                        Button::new("operation-confirm")
                            .label(dialog.confirm_label())
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.on_dialog_field(&crate::components::InputEvent::Submit, window, cx)
                            })),
                    ),
            )
            .into_any_element()
    }
}
