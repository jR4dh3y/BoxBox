//! `Notifications.svelte`: notices stacked bottom right, above the status bar.

use gpui::{div, prelude::*, px, Context};

use crate::components::Toast;
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn render_notices(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // fixed right-4 bottom-11 flex w-[380px] flex-col gap-2
        div()
            .absolute()
            .right(px(16.))
            .bottom(px(44.))
            .w(px(380.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .children(self.notices.visible().iter().map(|notice| {
                let id = notice.id;
                Toast::new(("notice", id as usize), notice.kind, notice.message.clone())
                    .on_dismiss(cx.listener(move |this, _, _, cx| this.dismiss_notice(id, cx)))
            }))
            // Notifications.svelte renders the transfer panel after its toasts.
            .child(self.render_jobs(cx))
    }
}
