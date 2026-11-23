//! `lib/components/StatusBar.svelte`.

use gpui::{div, prelude::*, px, Context};

use super::TextScale;
use crate::components::ViewSwitch;
use crate::format::file_size;
use crate::theme::*;
use crate::workspace::operations::PaneTarget;
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn render_status_bar(&self, target: PaneTarget, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.pane_by_id(target.0).unwrap_or_else(|| self.pane());
        let item_count = if pane.is_at_root() {
            self.storage().map_or(0, |drives| drives.len())
        } else {
            pane.items.len()
        };
        let selected = pane.selected.len();
        let text = if selected > 0 {
            let bytes = pane.selected_bytes();
            let size = if bytes > 0 {
                format!(" · {}", file_size(bytes))
            } else {
                String::new()
            };
            format!("{selected} of {item_count} selected{size}")
        } else {
            format!(
                "{item_count} item{}",
                if item_count == 1 { "" } else { "s" }
            )
        };
        // A running operation's label takes the place of the counts, as file managers do.
        let text = self.operation_status.message().map(str::to_owned).unwrap_or(text);
        let mode = pane.current_view_mode();
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .border_t_1()
            .border_color(c(BORDER_SECONDARY))
            .bg(self.chrome_background())
            .px(px(12.))
            .py(px(4.))
            .text_xs_tw()
            .text_color(c(TEXT_SECONDARY))
            .child(text)
            .child(ViewSwitch::new("view", mode, {
                let view = cx.entity().downgrade();
                move |target, _, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.choose_view_mode(this.active_pane, target, cx);
                        cx.notify();
                    });
                }
            }))
    }
}
