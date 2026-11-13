//! `lib/components/StatusBar.svelte`.

use gpui::{div, prelude::*, px, Context};

use super::{icon, TextScale};
use crate::format::file_size;
use crate::pane::ViewMode;
use crate::theme::*;
use crate::workspace::operations::PaneTarget;
use crate::workspace::{Load, Workspace};

impl Workspace {
    pub(crate) fn render_status_bar(&self, target: PaneTarget, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.pane_by_id(target.0).unwrap_or_else(|| self.pane());
        let item_count = if pane.is_at_root() {
            match &pane.drives {
                Load::Loaded(drives) => drives.len(),
                _ => 0,
            }
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
        let toggle =
            |id: &'static str, glyph: &'static str, target: ViewMode, cx: &mut Context<Self>| {
                let active = mode == target;
                div()
                    .id(id)
                    .size(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(2.))
                    .cursor_pointer()
                    .when(active, |button| button.bg(c(SURFACE_ELEVATED)))
                    .child(icon(
                        glyph,
                        14.,
                        c(if active { TEXT_PRIMARY } else { TEXT_MUTED }),
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.pane_mut().set_view_mode(target);
                        cx.notify();
                    }))
            };

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
            .child(
                div()
                    .flex()
                    .gap(px(2.))
                    .rounded(px(4.))
                    .bg(c(SURFACE_SECONDARY))
                    .p(px(2.))
                    .child(toggle("view-list", "list", ViewMode::List, cx))
                    .child(toggle("view-grid", "layout-grid", ViewMode::Grid, cx)),
            )
    }
}
