//! `BrowseBrowser.svelte` for one pane: toolbar, content and status bar.

use gpui::{div, prelude::*, px, AnyElement, Context, Window};

use super::TextScale;
use crate::pane::{Listing, ViewMode};
use crate::theme::*;
use crate::workspace::{Load, Workspace};

impl Workspace {
    pub(crate) fn render_browser(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pane = self.pane();
        let stale_error = match &pane.listing {
            Listing::Failed(error) if !pane.items.is_empty() && !pane.is_searching() => {
                Some(error.clone())
            }
            _ => None,
        };
        let content: AnyElement = match (&pane.listing, pane.current_view_mode()) {
            (Listing::Root, mode) => match &self.drives {
                Load::Loading => div()
                    .p(px(24.))
                    .text_sm_tw()
                    .text_color(c(TEXT_SECONDARY))
                    .child("Loading drives...")
                    .into_any_element(),
                Load::Failed(error) => div()
                    .p(px(24.))
                    .text_sm_tw()
                    .text_color(c(TEXT_SECONDARY))
                    .child(error.clone())
                    .into_any_element(),
                Load::Loaded(drives) if drives.is_empty() => div()
                    .p(px(24.))
                    .text_sm_tw()
                    .text_color(c(TEXT_SECONDARY))
                    .child("No configured storage found")
                    .into_any_element(),
                Load::Loaded(drives) => match mode {
                    ViewMode::Grid => self
                        .render_drive_cards(drives, window, cx)
                        .into_any_element(),
                    ViewMode::List => self
                        .render_drive_list(drives, window, cx)
                        .into_any_element(),
                },
            },
            (Listing::Failed(error), _) if pane.items.is_empty() && !pane.is_searching() => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(12.))
                .p(px(24.))
                .text_sm_tw()
                .child(error.clone())
                .child(
                    div()
                        .id("retry")
                        .rounded(px(4.))
                        .border_1()
                        .border_color(c(BORDER_PRIMARY))
                        .px(px(12.))
                        .py(px(4.))
                        .cursor_pointer()
                        .hover(|style| style.bg(c(SURFACE_SECONDARY)))
                        .child("Retry")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.refresh(&crate::workspace::Refresh, window, cx)
                        })),
                )
                .into_any_element(),
            (_, ViewMode::List) => self.render_file_list(window, cx).into_any_element(),
            (_, ViewMode::Grid) => self.render_file_grid(window, cx).into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .bg(c(SURFACE_PRIMARY))
            .child(self.render_toolbar(window, cx))
            .when_some(stale_error, |browser, error| {
                browser.child(
                    div()
                        .flex_none()
                        .px(px(12.))
                        .py(px(6.))
                        .border_b_1()
                        .border_color(c(BORDER_PRIMARY))
                        .bg(c(SURFACE_SECONDARY))
                        .text_13()
                        .text_color(c(TEXT_SECONDARY))
                        .child(format!("Could not refresh: {error}")),
                )
            })
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(content),
            )
            .child(self.render_status_bar(cx))
    }
}
