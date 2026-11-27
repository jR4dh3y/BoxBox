//! The confirm dialog on `Modal`: a short No/Yes question, or a titled message
//! with a named action.

use gpui::{div, prelude::*, px, AnyElement, Context, FontWeight, MouseButton, SharedString};

use super::TextScale;
use crate::components::{Button, ButtonVariant, Modal, ModalSize};
use crate::theme::*;
use crate::workspace::confirm::Ask;
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn render_confirm(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(confirm) = &self.confirm else { return div().into_any_element() };
        let accept = |label: SharedString| {
            Button::new("confirm-accept")
                .label(label)
                .variant(if confirm.danger { ButtonVariant::Danger } else { ButtonVariant::Primary })
                .on_click(cx.listener(|this, _, _, cx| this.answer(true, cx)))
        };
        let modal = match &confirm.ask {
            // No title bar: the question and its answers.
            Ask::Question(question) => Modal::new().size(ModalSize::Sm).body(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(div().text_sm_tw().font_weight(FontWeight::MEDIUM).text_color(c(TEXT_PRIMARY)).child(question.clone()))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.))
                            .child(Button::new("confirm-cancel").label("No").variant(ButtonVariant::Secondary).on_click(cx.listener(|this, _, _, cx| this.answer(false, cx))))
                            .child(accept("Yes".into())),
                    ),
            ),
            Ask::Explained { title, message, confirm_label } => Modal::new()
                .title(title.clone())
                .close_selector("confirm-cancel")
                .on_close(cx.listener(|this, _, _, cx| this.answer(false, cx)))
                .body(div().text_13().text_color(c(TEXT_SECONDARY)).child(message.clone()))
                .when_some(confirm.secondary.as_ref().map(|(label, _)| label.clone()), |modal, label| {
                    modal.footer(
                        Button::new("confirm-secondary")
                            .label(label)
                            .variant(ButtonVariant::Secondary)
                            .on_click(cx.listener(|this, _, _, cx| this.answer_secondary(cx))),
                    )
                })
                .footer(accept(confirm_label.clone())),
        };
        div()
            .id("confirm-overlay")
            .absolute()
            .inset_0()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(modal)
            .into_any_element()
    }
}
