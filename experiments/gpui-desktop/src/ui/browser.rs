//! `BrowseBrowser.svelte` for one pane: toolbar, content and status bar.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Window};

use crate::ui::icon;
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
            self.render_trash(cx).into_any_element()
        } else {
            match (&pane.listing, pane.current_view_mode()) {
            (Listing::Root, mode) => match &pane.drives {
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
            (_, ViewMode::List) => self.render_file_list(target, cx).into_any_element(),
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
            .on_mouse_down(gpui::MouseButton::Right, cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                this.context_menu = Some(crate::workspace::operations::ContextMenuState {
                    target_id: target.0, items: Vec::new(), position: event.position,
                });
                cx.notify();
            }))
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .bg(self.shell_background())
            .when(!self.trash_open, |browser| {
                browser.child(self.render_toolbar(target, window, cx))
            })
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
                    .child(content)
                    // The folder this pane shows would take the drop.
                    .when(dropping, |content| content.child(div().absolute().inset_0().border_2().border_color(c(self.accent_hex())))),
            )
            .when(!self.trash_open, |browser| {
                browser.child(self.render_status_bar(target, cx))
            })
    }

    fn render_trash(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let body: AnyElement = match &self.trash {
            Load::Loading => div()
                .p(px(24.))
                .text_sm_tw()
                .text_color(c(TEXT_SECONDARY))
                .child("Loading Trash...")
                .into_any_element(),
            Load::Failed(error) => div()
                .p(px(24.))
                .text_sm_tw()
                .text_color(c(TEXT_SECONDARY))
                .child(error.clone())
                .into_any_element(),
            Load::Loaded(items) if items.is_empty() => div()
                .flex()
                .items_center()
                .justify_center()
                .flex_1()
                .text_sm_tw()
                .text_color(c(TEXT_SECONDARY))
                .child("Trash is empty")
                .into_any_element(),
            Load::Loaded(items) => div()
                .flex()
                .flex_col()
                .gap(px(1.))
                .p(px(12.))
                .children(items.iter().map(|item| {
                    let restore_id = item.id.clone();
                    let delete_id = item.id.clone();
                    let can_restore = item.can_restore;
                    div()
                        .id(SharedString::from(format!("trash-{}", item.id)))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .px(px(12.))
                        .py(px(9.))
                        .rounded(px(4.))
                        .hover(|style| style.bg(c(SURFACE_SECONDARY)))
                        .child(icon(
                            if item.is_dir { "folder" } else { "file" },
                            18.,
                            c(TEXT_SECONDARY),
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_13().child(item.name.clone()))
                                .child(
                                    div()
                                        .child(format!(
                                            "{} • deleted {}",
                                            item.original_path, item.deleted_at
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("restore-{}", item.id)))
                                .debug_selector(|| format!("restore-{}", item.name))
                                .px(px(8.))
                                .py(px(4.))
                                .rounded(px(4.))
                                .cursor_pointer()
                                .text_xs_tw()
                                .hover(|style| style.bg(c(SURFACE_TERTIARY)))
                                .child(if can_restore { "Restore" } else { "Unavailable" })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if can_restore {
                                        this.restore_trash(restore_id.clone(), cx)
                                    }
                                })),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("delete-trash-{}", item.id)))
                                .px(px(8.))
                                .py(px(4.))
                                .rounded(px(4.))
                                .cursor_pointer()
                                .text_xs_tw()
                                .text_color(c(DANGER))
                                .hover(|style| style.bg(c(SURFACE_TERTIARY)))
                                .child("Delete")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let id = delete_id.clone();
                                    let confirm = crate::workspace::confirm::Confirm::new(
                                        "Delete permanently",
                                        "This item will be removed permanently.",
                                        "Delete",
                                        move |this, cx| this.delete_trash_item(id, cx),
                                    );
                                    this.ask(confirm, cx);
                                })),
                        )
                }))
                .into_any_element(),
        };
        let header = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.))
            .border_b_1()
            .border_color(c(BORDER_PRIMARY))
            .px(px(12.))
            .py(px(8.))
            .child(
                div()
                    .id("trash-back")
                    .cursor_pointer()
                    .rounded(px(4.))
                    .px(px(8.))
                    .py(px(4.))
                    .hover(|style| style.bg(c(SURFACE_ELEVATED)))
                    .child("Back")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.back(&crate::workspace::Back, window, cx)
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Trash"),
            )
            .child(
                div()
                    .id("trash-refresh")
                    .cursor_pointer()
                    .rounded(px(4.))
                    .px(px(8.))
                    .py(px(4.))
                    .hover(|style| style.bg(c(SURFACE_ELEVATED)))
                    .child(icon("refresh-cw", 15., c(TEXT_SECONDARY)))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.refresh(&crate::workspace::Refresh, window, cx)
                    })),
            );
        div()
            .id("trash-page")
            .size_full()
            .flex()
            .flex_col()
            .child(header)
            .child(
                div()
                    .id("trash-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(body),
            )
}
}
