//! `Notifications.svelte`: notices stacked bottom right, above the status bar.

use gpui::{div, prelude::*, px, Context, Window};

use crate::components::Toast;
use crate::workspace::Workspace;

/// A notice's height, reckoned a little over, as the stack closes over one leaving.
const LEAVING_HEIGHT: f32 = 72.;

impl Workspace {
    pub(crate) fn render_notices(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        // fixed right-4 bottom-11 flex w-[380px] flex-col gap-2
        div()
            .absolute()
            .right(px(16.))
            .bottom(px(44.))
            .w(px(380.))
            .flex()
            .flex_col()
            .gap(px(8.))
            // A click on a toast or the transfers is not a click on the files under them.
            .occlude()
            .children(self.notices.drawn().map(|notice| {
                let id = notice.id;
                let toast = Toast::new(("notice", id as usize), notice.kind, notice.message.clone())
                    .on_dismiss(cx.listener(move |this, _, _, cx| this.dismiss_notice(id, cx)));
                let toast = match &notice.action {
                    Some((label, _)) => toast.action(label.clone(), cx.listener(move |this, _, _, cx| this.run_notice_action(id, cx))),
                    None => toast,
                };
                // A dismissed notice slides off to the right as the stack closes over it.
                let left = 1. - crate::components::motion::progress(notice.leaving, crate::components::motion::LEAVE, window, cx);
                match notice.leaving {
                    None => div().child(toast),
                    Some(_) => div()
                        .relative()
                        .left(px(48. * (1. - left)))
                        .opacity(left)
                        .max_h(px(LEAVING_HEIGHT * left))
                        .mb(px(-8. * (1. - left)))
                        .overflow_hidden()
                        .child(toast),
                }
            }))
            // Notifications.svelte renders the transfer panel after its toasts.
            .child(self.render_jobs(cx))
    }
}
