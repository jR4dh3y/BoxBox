//! What a running transfer asks: a name already taken, an item that failed, or
//! too little space. Rows put the label left and the value right; the choices
//! are the footer's buttons, and the ✕ cancels the transfer.

use gpui::{div, prelude::*, px, AnyElement, Context, MouseButton};

use crate::components::{Button, ButtonVariant, Modal, Toggle, Type};
use crate::format::{file_size, full_date};
use crate::theme::*;
use crate::transfer::{Answer, Question, Side};
use crate::workspace::Workspace;

fn row(label: &'static str, value: impl IntoElement) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.))
        .min_h(px(30.))
        .child(div().flex_none().text_color(c(TEXT_SECONDARY)).child(label))
        .child(div().min_w_0().flex().justify_end().text_color(c(TEXT_PRIMARY)).child(value))
}

fn describe(side: &Side) -> String {
    match side.is_dir {
        true => format!("Folder · {}", full_date(side.mod_time)),
        false => format!("{} · {}", file_size(side.size), full_date(side.mod_time)),
    }
}

impl Workspace {
    fn answer_button(&self, id: &'static str, label: &'static str, answer: Answer, primary: bool, cx: &mut Context<Self>) -> Button {
        Button::new(id)
            .label(label)
            .variant(if primary { ButtonVariant::Primary } else { ButtonVariant::Secondary })
            .on_click(cx.listener(move |this, _, _, cx| this.answer_transfer(answer, cx)))
    }

    pub(crate) fn render_transfer_question(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(pending) = &self.transfer_question else { return div().into_any_element() };
        let (title, mut rows, buttons): (String, Vec<gpui::Div>, Vec<Button>) = match &pending.question {
            Question::Conflict { name, existing, incoming, rename_to, .. } => {
                let folders = existing.is_dir && incoming.is_dir;
                let mut buttons = vec![
                    self.answer_button("transfer-skip", "Skip", Answer::Skip, false, cx),
                    self.answer_button("transfer-rename", "Rename", Answer::Rename, false, cx),
                    self.answer_button("transfer-replace", "Replace", Answer::Replace, !folders, cx),
                ];
                if folders {
                    buttons.push(self.answer_button("transfer-merge", "Merge", Answer::Merge, true, cx));
                }
                (
                    format!("“{name}” is already in {}", pending.folder),
                    vec![
                        row("Already there", describe(existing)),
                        row("Coming in", describe(incoming)),
                        row("Rename makes", div().min_w_0().truncate().child(rename_to.clone())),
                    ],
                    buttons,
                )
            }
            Question::Failed { name, error } => (
                format!("Could not transfer “{name}”"),
                vec![div().pb(px(6.)).text_color(c(TEXT_SECONDARY)).child(error.clone())],
                vec![
                    self.answer_button("transfer-skip", "Skip", Answer::Skip, false, cx),
                    self.answer_button("transfer-retry", "Retry", Answer::Retry, true, cx),
                ],
            ),
            Question::NoSpace { needed, free } => (
                "Not enough space".to_string(),
                vec![row("Needed", file_size(*needed)), row("Free", file_size(*free))],
                vec![self.answer_button("transfer-continue", "Transfer anyway", Answer::Continue, true, cx)],
            ),
        };
        if pending.more {
            let workspace = cx.entity().downgrade();
            rows.push(row(
                "Do this for the rest",
                div().debug_selector(|| "transfer-apply-all".into()).child(Toggle::new("transfer-apply-all", pending.apply_all).on_change(move |on, _, _, cx| {
                    workspace
                        .update(cx, |this, cx| {
                            if let Some(pending) = this.transfer_question.as_mut() {
                                pending.apply_all = on;
                            }
                            cx.notify();
                        })
                        .ok();
                })),
            ));
        }
        div()
            .id("transfer-question")
            .debug_selector(|| "transfer-question".into())
            .absolute()
            .inset_0()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Modal::new()
                    .title(title)
                    .on_close(cx.listener(|this, _, _, cx| this.answer_transfer(Answer::Cancel, cx)))
                    .body(div().flex().flex_col().type_13().children(rows))
                    .footer(div().flex().gap(px(8.)).children(buttons)),
            )
            .into_any_element()
    }
}
