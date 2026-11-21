//! `BrowseBrowser.svelte` for one pane: toolbar, content and status bar.

use gpui::{div, prelude::*, px, AnyElement, Context, Window};

use super::TextScale;
use crate::pane::{Listing, ViewMode};
use crate::theme::*;
use crate::workspace::drag_drop::{drop_zone, DropTarget};
use crate::workspace::operations::PaneTarget;
use crate::workspace::{Load, Workspace};

impl Workspace {
    pub(crate) fn render_browser(
        &self,
        target: PaneTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pane = self.pane_by_id(target.0).unwrap_or_else(|| self.pane());
        let stale_error = match &pane.listing {
            Listing::Failed(error) if !pane.items.is_empty() && !pane.is_searching() => {
                Some(error.clone())
            }
            _ => None,
        };
        let content: AnyElement = if self.trash_open {
            self.render_trash(window, cx).into_any_element()
        } else {
            match (&pane.listing, pane.current_view_mode()) {
            (Listing::Root, mode) => match (&self.home.drives, self.storage()) {
                (Load::Failed(error), _) => div()
                    .p(px(24.))
                    .text_sm_tw()
                    .text_color(c(TEXT_SECONDARY))
                    .child(error.clone())
                    .into_any_element(),
                (_, None) => div()
                    .p(px(24.))
                    .text_sm_tw()
                    .text_color(c(TEXT_SECONDARY))
                    .child("Loading drives...")
                    .into_any_element(),
                (_, Some(drives)) if drives.is_empty() => div()
                    .p(px(24.))
                    .text_sm_tw()
                    .text_color(c(TEXT_SECONDARY))
                    .child("No configured storage found")
                    .into_any_element(),
                (_, Some(drives)) => match mode {
                    ViewMode::Grid => self
                        .render_drive_cards(target, &drives, window, cx)
                        .into_any_element(),
                    ViewMode::List => self
                        .render_drive_list(target, &drives, window, cx)
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
                        .hover(|style| style.bg(c(SURFACE_SECONDARY))).active(crate::components::pressed)
                        .child("Retry")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.refresh(&crate::workspace::Refresh, window, cx)
                        })),
                )
                .into_any_element(),
            (_, ViewMode::List) => self.render_file_list(target, window, cx).into_any_element(),
            (_, ViewMode::Grid) => self.render_file_grid(target, window, cx).into_any_element(),
        }

        };
        let dropping = self.drop_hover(cx) == Some(&DropTarget::Pane(target));
        drop_zone(div(), DropTarget::Pane(target), cx.entity().downgrade())
            .id(("browser-pane", target.0))
            .debug_selector(|| format!("browser-pane-{}", target.0))
            .capture_any_mouse_down(cx.listener(move |this, _, window, cx| {
                this.active_pane = target;
                // A text field clicked here takes focus back in the bubble phase.
                this.focus_handle.focus(window);
                cx.notify();
            }))
            .on_mouse_down(gpui::MouseButton::Right, cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                // Rows stop the event, so this is empty space: the menu is for the folder.
                this.show_folder_context_menu(target, event.position, window, cx);
            }))
            // A mouse's side buttons go back and forward in the pane under the pointer,
            // which the capture above made active.
            .on_mouse_down(gpui::MouseButton::Navigate(gpui::NavigationDirection::Back), cx.listener(|this, _, window, cx| {
                this.back(&crate::workspace::Back, window, cx);
            }))
            .on_mouse_down(gpui::MouseButton::Navigate(gpui::NavigationDirection::Forward), cx.listener(|this, _, window, cx| {
                this.forward(&crate::workspace::Forward, window, cx);
            }))
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .bg(self.shell_background())
            .child(self.render_toolbar(target, window, cx))
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
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .relative()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .overflow_hidden()
                            .child(content)
                            // The folder this pane shows would take the drop.
                            .when(dropping, |content| content.child(div().absolute().inset_0().border_2().border_color(c(self.accent_hex())))),
                    )
                    // Under the toolbar of the rightmost pane, as in Dolphin.
                    .when(self.is_rightmost_pane(target), |row| row.children(self.render_details(window, cx))),
            )
            .when(!self.trash_open, |browser| {
                browser.child(self.render_status_bar(target, cx))
            })
    }
}
