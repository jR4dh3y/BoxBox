//! `FileGrid.svelte`: 112px tiles in 164px rows, virtualised by row.

use gpui::{div, prelude::*, px, uniform_list, ClickEvent, Context, Window};

use super::{thin_icon, TextScale, SIDEBAR_WIDTH};
use crate::format::{file_icon, file_size};
use crate::pane::Listing;
use crate::theme::*;
use crate::workspace::Workspace;

const TILE_WIDTH: f32 = 112.;
const ROW_HEIGHT: f32 = 164.;
const GAP: f32 = 12.;
const PADDING: f32 = 12.;

/// `columnsFor` in `virtualWindow.ts`.
fn columns_for_with(width: f32, tile_width: f32, gap: f32) -> usize {
    (((width + gap) / (tile_width + gap)).floor() as usize).max(1)
}

fn display_name(name: &str, is_dir: bool, show_extensions: bool) -> String {
    if is_dir || show_extensions {
        return name.to_string();
    }
    name.rsplit_once('.')
        .filter(|(stem, extension)| !stem.is_empty() && !extension.is_empty())
        .map(|(stem, _)| stem.to_string())
        .unwrap_or_else(|| name.to_string())
}

impl Workspace {
    pub(crate) fn render_file_grid(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pane = self.pane();
        let compact = self.preferences.compact_rows;
        let tile_width = if compact { 96. } else { TILE_WIDTH };
        let row_height = if compact { 136. } else { ROW_HEIGHT };
        let gap = if compact { 8. } else { GAP };
        let padding = if compact { 8. } else { PADDING };
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
        let width = f32::from(window.viewport_size().width) - SIDEBAR_WIDTH - padding * 2.;
        let columns = columns_for_with(width, tile_width, gap);
        let rows = pane.items.len().div_ceil(columns);
        div()
            .size_full()
            .bg(c(SURFACE_PRIMARY))
            .child(
                uniform_list(
                    "file-grid",
                    rows,
                    cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                        range
                            .map(|row| {
                                let start = row * columns;
                                let end = (start + columns).min(this.pane().items.len());
                                // uniform_list needs equal rows, so the container's top padding
                                // is folded into every row instead of only the first.
                                div()
                                    .h(px(row_height))
                                    .px(px(padding))
                                    .pt(px(padding))
                                    .flex()
                                    .items_start()
                                    .gap(px(gap))
                                    .children((start..end).map(|index| this.render_tile(index, compact, tile_width, cx)))
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
        index: usize,
        compact: bool,
        tile_width: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pane = self.pane();
        let item = &pane.items[index];
        let selected = pane.is_selected(&item.path);
        let active = pane.active == Some(index);
        let (path, is_dir) = (item.path.clone(), item.is_dir);
        let name = display_name(&item.name, is_dir, self.preferences.show_file_extensions);
        crate::drag_out::draggable(
            div().id(("tile", index)),
            index,
            cx.entity().downgrade(),
            self.api.clone(),
        )
        .w(px(tile_width))
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
                .h(px(if compact { 56. } else { 80. }))
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
                        .text_size(px(if compact { 12. } else { 13. }))
                        .line_height(px(if compact { 14. } else { 16. }))
                        .line_clamp(2)
                        .text_color(c(if item.is_dir { FOLDER } else { TEXT_PRIMARY }))
                        .child(name),
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
    }
}
