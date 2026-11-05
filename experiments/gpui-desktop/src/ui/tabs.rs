//! The tab strip in `BrowseWorkspace.svelte`.

use gpui::{div, prelude::*, px, Context};

use super::{icon, TextScale};
use crate::theme::*;
use crate::workspace::{NewTab, Workspace};

impl Workspace {
    pub(crate) fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let closable = self.tabs.len() > 1;
        let tabs = self.tabs.iter().enumerate().map(|(index, pane)| {
            let title = match pane.path.rsplit('/').next() {
                Some(name) if !name.is_empty() => name.to_string(),
                _ => "This PC".to_string(),
            };
            let active = index == self.active_tab;
            div()
                .flex()
                .min_w_0()
                .items_center()
                .rounded(px(4.))
                .when(active, |tab| tab.bg(c(SURFACE_ELEVATED)))
                .child(
                    div()
                        .id(("tab", index))
                        .max_w(px(176.))
                        .px(px(8.))
                        .py(px(4.))
                        .text_xs_tw()
                        .truncate()
                        .cursor_pointer()
                        .child(title)
                        .on_click(cx.listener(move |this, _, _, cx| this.activate_tab(index, cx))),
                )
                .when(closable, |tab| {
                    tab.child(
                        div()
                            .id(("close-tab", index))
                            .p(px(4.))
                            .cursor_pointer()
                            .child(icon("x", 12., c(TEXT_PRIMARY)))
                            .on_click(cx.listener(move |this, _, _, cx| this.close_tab_at(index, cx))),
                    )
                })
        });

        div()
            .id("tabs")
            .h(px(32.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.))
            .overflow_x_scroll()
            .border_b_1()
            .border_color(c(BORDER_SECONDARY))
            .px(px(8.))
            .children(tabs)
            .child(
                div()
                    .id("new-tab")
                    .p(px(4.))
                    .cursor_pointer()
                    .child(icon("plus", 14., c(TEXT_SECONDARY)))
                    .on_click(cx.listener(|this, _, window, cx| this.new_tab(&NewTab, window, cx))),
            )
            .child(div().ml_auto().p(px(4.)).child(icon("columns-2", 14., c(TEXT_SECONDARY))))
    }
}
