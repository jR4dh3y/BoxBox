//! `FileGrid.svelte`: 112px tiles in 164px rows, virtualised by row.

use gpui::{div, prelude::*, px, uniform_list, ClickEvent, Context, Window};

use super::{thin_icon, TextScale};
use crate::format::{file_icon, file_size};
use crate::pane::Listing;
use crate::theme::*;
use crate::workspace::Workspace;

const TILE_WIDTH: f32 = 112.;
const ROW_HEIGHT: f32 = 164.;
const GAP: f32 = 12.;
const PADDING: f32 = 12.;

/// `columnsFor` in `virtualWindow.ts`.
fn columns_for(width: f32) -> usize {
    (((width + GAP) / (TILE_WIDTH + GAP)).floor() as usize).max(1)
}

impl Workspace {
    pub(crate) fn render_file_grid(
        &self,
        pane_id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pane = self.view(pane_id);
        if pane.items.is_empty() && matches!(pane.listing, Listing::Loaded) {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .child(super::icon("folder-open", 32., alpha(TEXT_MUTED, 0.5)))
                .child(
                    div()
                        .text_13()
                        .text_color(c(TEXT_MUTED))
                        .child("This folder is empty"),
                )
                .into_any_element();
        }
        let width = self.pane_width(window) - PADDING * 2.;
        let columns = columns_for(width);
        let rows = pane.items.len().div_ceil(columns);
        div()
            .size_full()
            .bg(c(SURFACE_PRIMARY))
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    this.open_background_menu(event.position, cx)
                }),
            )
            .child(
                uniform_list(
                    "file-grid",
                    rows,
                    cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                        range
                            .map(|row| {
                                let start = row * columns;
                                let end = (start + columns).min(this.view(pane_id).items.len());
                                // uniform_list needs equal rows, so the container's top padding
                                // is folded into every row instead of only the first.
                                div()
                                    .h(px(ROW_HEIGHT))
                                    .px(px(PADDING))
                                    .pt(px(PADDING))
                                    .flex()
                                    .items_start()
                                    .gap(px(GAP))
                                    .children(
                                        (start..end)
                                            .map(|index| this.render_tile(pane_id, index, cx)),
                                    )
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(pane.grid_scroll.clone())
                .size_full(),
            )
            .into_any_element()
    }

    fn render_tile(
        &self,
        pane_id: usize,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pane = self.view(pane_id);
        let item = &pane.items[index];
        let selected = pane.is_selected(&item.path);
        let active = pane.active == Some(index);
        let (path, is_dir) = (item.path.clone(), item.is_dir);
        let drop_dest = item.is_dir.then(|| item.path.clone());
        crate::drag_out::draggable(
            div().id(("tile", index)),
            pane_id,
            index,
            cx.entity().downgrade(),
            self.api.clone(),
        )
        .w(px(TILE_WIDTH))
        .flex_none()
        .flex()
        .flex_col()
        .min_w_0()
        .rounded(px(4.))
        .border_1()
        .p(px(8.))
        .cursor_default()
        .map(|tile| match (selected, active) {
            (true, _) => tile
                .border_color(alpha(ACCENT, 0.7))
                .bg(c(SELECTION))
                .hover(|style| style.bg(c(SELECTION_HOVER))),
            (false, true) => tile
                .border_color(alpha(ACCENT, 0.))
                .bg(c(SURFACE_SECONDARY)),
            (false, false) => tile
                .border_color(alpha(ACCENT, 0.))
                .hover(|style| style.bg(c(SURFACE_SECONDARY))),
        })
        .child(
            div()
                .h(px(80.))
                .w_full()
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .rounded(px(4.))
                .border_1()
                .border_color(c(BORDER_PRIMARY))
                .bg(c(SURFACE_PRIMARY))
                .child(thin_icon(
                    file_icon(&item.name, item.is_dir),
                    30.,
                    c(if item.is_dir { FOLDER } else { TEXT_SECONDARY }),
                )),
        )
        .child(
            div()
                .w_full()
                .mt(px(6.))
                .min_w_0()
                .overflow_hidden()
                .child(
                    div()
                        .w_full()
                        .text_center()
                        .text_size(px(13.))
                        .line_height(px(16.))
                        .line_clamp(2)
                        .text_color(c(if item.is_dir { FOLDER } else { TEXT_PRIMARY }))
                        .child(item.name.clone()),
                )
                .child(
                    div()
                        .w_full()
                        .mt(px(2.))
                        // GPUI's ellipsis collapses centred text to "…", so clip instead.
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_center()
                        .text_size(px(11.))
                        .line_height(px(14.))
                        .text_color(c(if selected { TEXT_PRIMARY } else { TEXT_MUTED }))
                        .child(if item.is_dir {
                            "—".into()
                        } else {
                            file_size(item.size)
                        }),
                ),
        )
        .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
            this.activate_item(index, &path, is_dir, event, cx)
        }))
        .on_mouse_down(
            gpui::MouseButton::Right,
            cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                cx.stop_propagation();
                this.open_item_menu(index, event.position, cx);
            }),
        )
        .when_some(drop_dest, |target, dest| {
            let external = dest.clone();
            target
                .drag_over::<crate::drag_out::DraggedItem>(|style, _, _, _| {
                    style.bg(c(SELECTION_HOVER))
                })
                .on_drop(cx.listener(
                    move |this, dragged: &crate::drag_out::DraggedItem, window, cx| {
                        cx.stop_propagation();
                        this.drop_dragged(dragged, dest.clone(), window, cx);
                    },
                ))
                .on_drop(
                    cx.listener(move |this, paths: &gpui::ExternalPaths, _, cx| {
                        cx.stop_propagation();
                        this.drop_external(paths, external.clone(), cx);
                    }),
                )
        })
    }
}
