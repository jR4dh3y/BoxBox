//! `desktop/TransferConflictDialog.svelte`: each name that is already taken,
//! skipped or kept as a copy, then Continue.

use gpui::{div, prelude::*, px, AnyElement, Context, MouseButton, SharedString};

use super::TextScale;
use crate::components::{Button, Modal, ModalSize};
use crate::theme::*;
use crate::workspace::Workspace;

impl Workspace {
    /// `choiceClass`: the chosen option filled with the accent.
    fn conflict_choice(&self, id: SharedString, label: &'static str, chosen: bool, index: usize, keep: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = self.accent_hex();
        div()
            .id(id)
            .rounded(px(4.))
            .border_1()
            .px(px(10.))
            .py(px(4.))
            .text_xs_tw()
            .cursor_pointer()
            .map(|button| match chosen {
                true => button.border_color(c(accent)).bg(c(accent)).text_color(c(WHITE)),
                false => button
                    .border_color(c(BORDER_PRIMARY))
                    .bg(c(SURFACE_SECONDARY))
                    .text_color(c(TEXT_SECONDARY))
                    .hover(|style| style.bg(c(SURFACE_TERTIARY)).text_color(c(TEXT_PRIMARY))),
            })
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                if let Some(plan) = this.transfer_conflicts.as_mut().and_then(|conflicts| conflicts.planned.get_mut(index)) {
                    plan.keep = keep;
                }
                cx.notify();
            }))
    }

    pub(crate) fn render_conflicts(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(conflicts) = &self.transfer_conflicts else { return div().into_any_element() };
        let folder = conflicts.parent.rsplit('/').next().filter(|name| !name.is_empty()).unwrap_or("/");
        let count = conflicts.planned.iter().filter(|plan| plan.keep_both.is_none() || plan.keep).count();
        let rows = conflicts.planned.iter().enumerate().filter_map(|(index, plan)| {
            plan.keep_both.as_ref()?;
            // flex items-center gap-3 border-b px-3 py-1.5 text-[13px]
            Some(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .border_b_1()
                    .border_color(c(BORDER_SECONDARY))
                    .px(px(12.))
                    .py(px(6.))
                    .text_13()
                    .child(div().flex_1().min_w_0().truncate().text_color(c(TEXT_PRIMARY)).child(plan.entry.item.name.clone()))
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .gap(px(4.))
                            .child(self.conflict_choice(SharedString::from(format!("conflict-skip-{index}")), "Skip", !plan.keep, index, false, cx))
                            .child(self.conflict_choice(SharedString::from(format!("conflict-keep-{index}")), "Keep both", plan.keep, index, true, cx)),
                    ),
            )
        }).collect::<Vec<_>>();
        div()
            .id("conflicts-overlay")
            .absolute()
            .inset_0()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Modal::new()
                    .title(format!("Already in {folder}"))
                    .size(ModalSize::Lg)
                    .on_close(cx.listener(|this, _, _, cx| {
                        this.transfer_conflicts = None;
                        cx.notify();
                    }))
                    .body(
                        div()
                            .id("conflict-list")
                            .max_h(px(400.))
                            .overflow_y_scroll()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(c(BORDER_SECONDARY))
                            .children(rows),
                    )
                    .footer(
                        Button::new("conflicts-continue")
                            .label(match count {
                                0 => "Skip all".to_string(),
                                1 => "Continue with 1 item".to_string(),
                                count => format!("Continue with {count} items"),
                            })
                            .on_click(cx.listener(|this, _, _, cx| this.resolve_conflicts(cx))),
                    ),
            )
            .into_any_element()
    }
}
