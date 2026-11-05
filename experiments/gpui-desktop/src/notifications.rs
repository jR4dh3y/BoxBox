//! `Notifications.svelte`: toasts in the bottom-right corner that dismiss themselves.

use std::time::Duration;

use gpui::{div, prelude::*, px, Context, SharedString};

use crate::theme::*;
use crate::ui::{icon, TextScale};
use crate::workspace::Workspace;

const DISMISS_AFTER: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Error,
    Info,
}

pub struct Toast {
    id: u64,
    kind: ToastKind,
    message: SharedString,
}

#[derive(Default)]
pub struct Toasts {
    items: Vec<Toast>,
    next_id: u64,
}

impl Workspace {
    pub(crate) fn toast(
        &mut self,
        kind: ToastKind,
        message: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let id = self.toasts.next_id;
        self.toasts.next_id += 1;
        self.toasts.items.push(Toast {
            id,
            kind,
            message: message.into(),
        });
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DISMISS_AFTER).await;
            this.update(cx, |this, cx| this.dismiss_toast(id, cx)).ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn error(&mut self, error: &anyhow::Error, cx: &mut Context<Self>) {
        self.toast(ToastKind::Error, format!("{error:#}"), cx);
    }

    fn dismiss_toast(&mut self, id: u64, cx: &mut Context<Self>) {
        self.toasts.items.retain(|toast| toast.id != id);
        cx.notify();
    }

    pub(crate) fn render_toasts(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .w(px(360.))
            .children(self.toasts.items.iter().map(|toast| {
                let (name, color) = match toast.kind {
                    ToastKind::Success => ("circle-check", SUCCESS),
                    ToastKind::Error => ("circle-alert", DANGER),
                    ToastKind::Info => ("info", INFO),
                };
                let id = toast.id;
                div()
                    .id(("toast", id as usize))
                    .flex()
                    .items_start()
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(10.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(BORDER_PRIMARY))
                    .bg(c(SURFACE_ELEVATED))
                    .shadow_lg()
                    .text_13()
                    .child(div().pt(px(2.)).child(icon(name, 16., c(color))))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_color(c(TEXT_PRIMARY))
                            .child(toast.message.clone()),
                    )
                    .child(
                        div()
                            .id(("toast-close", id as usize))
                            .cursor_pointer()
                            .child(icon("x", 14., c(TEXT_SECONDARY)))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.dismiss_toast(id, cx)),
                            ),
                    )
            }))
    }
}
