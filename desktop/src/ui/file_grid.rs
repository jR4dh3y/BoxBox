//! `FileGrid.svelte`: the view. It virtualises `FileTile`s by row; the
//! component owns the look.

use gpui::{div, prelude::*, px, uniform_list, ClickEvent, Context, Window};

use super::{icon, TextScale};
use crate::components::{FileTile, PeekCard};
use crate::format::{display_name, file_size};
use crate::theme::*;
use crate::workspace::drag_drop::{drop_zone, DropTarget};
use crate::workspace::operations::PaneTarget;
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn render_file_grid(&self, target: PaneTarget, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.pane_by_id(target.0).unwrap_or_else(|| self.pane());
        let metrics = self.grid_metrics();
        if let Some((glyph, message)) = pane.placeholder() {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .child(icon(glyph, 32., alpha(TEXT_MUTED, 0.5)))
                .child(div().text_13().text_color(c(TEXT_MUTED)).child(message))
                .into_any_element();
        }
        let columns = metrics.columns(self.pane_width(target, window));
        let rows = pane.items.len().div_ceil(columns);
        div()
            .relative()
            .size_full()
            .bg(self.file_background())
            .child(
                uniform_list(
                    "file-grid",
                    rows,
                    cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                        let _span = crate::profile::span("grid rows").with(|| format!("{} rows", range.len()));
                        let count = this.pane_by_id(target.0).map_or(0, |pane| pane.items.len());
                        range
                            .map(|row| {
                                let start = row * columns;
                                let end = (start + columns).min(count);
                                // uniform_list needs equal rows, so the container's top padding
                                // is folded into every row instead of only the first.
                                div()
                                    .h(px(metrics.row_height))
                                    .px(px(metrics.padding))
                                    .pt(px(metrics.padding))
                                    .flex()
                                    .items_start()
                                    .gap(px(metrics.gap))
                                    .children((start..end).filter_map(|index| {
                                        let item = this.pane_by_id(target.0)?.items.get(index)?.clone();
                                        this.request_folder_peek(target, &item, cx);
                                        this.request_thumbnail(target, item, cx);
                                        this.request_entry_icon(target, index, metrics.theme_icon(), cx);
                                        this.render_tile(target, index, cx)
                                    }))
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(pane.grid_scroll.clone())
                .size_full(),
            )
            .child(self.pointer_layer(target, window, cx))
            .into_any_element()
    }

    /// The items drawn inside a folder's tile, front first. A folder with an emblem keeps it instead.
    fn peek_cards(&self, api: &std::sync::Arc<crate::api::Api>, item: &crate::api::FileInfo, emblem: Option<&'static str>) -> Vec<PeekCard> {
        if !item.is_dir || emblem.is_some() || !self.folder_previews_shown() {
            return Vec::new();
        }
        let Some(items) = self.folder_peek(api, item) else { return Vec::new() };
        items
            .iter()
            .map(|inside| match self.thumbnail(api, inside) {
                Some(image) => PeekCard::Image(image),
                None => PeekCard::Sheet,
            })
            .collect()
    }

    fn render_tile(&self, target: PaneTarget, index: usize, cx: &mut Context<Self>) -> Option<FileTile> {
        let pane = self.pane_by_id(target.0)?;
        let item = pane.items.get(index)?.clone();
        let (path, is_dir) = (item.path.clone(), item.is_dir);
        let thumbnail = self.thumbnail(&pane.api, &item);
        let folder = DropTarget::Folder { pane: target, path: path.clone() };
        let style = self.glyph_style(&pane.api, &item);
        let tile = FileTile::new(
            ("tile", index),
            display_name(&item.name, is_dir, self.preferences.show_file_extensions),
            is_dir,
            self.selection_style(),
        )
        .icon(style.glyph)
        .tint(style.tint, style.emblem)
        .image(self.item_icon(&pane.api, &item, crate::components::GridMetrics::new(self.preferences.grid_zoom).theme_icon()))
        .detail(match (pane.shows_locations(), is_dir) {
            // A search result says where it is; a file in its folder, how big.
            (true, _) => crate::search::location(&pane.path, &item.path).to_string().into(),
            (false, true) => gpui::SharedString::default(),
            (false, false) => file_size(item.size).into(),
        })
        .badge(thumbnail.as_ref().and_then(|_| self.app_badge(&pane.api, &item, crate::components::GridMetrics::new(self.preferences.grid_zoom).theme_icon())))
        .peek(self.peek_cards(&pane.api, &item, style.emblem))
        .thumbnail(thumbnail)
        .selected(pane.is_selected(&item.path))
        .active(pane.active == Some(index))
        .cut(self.is_cut(pane, &item))
        .drop_target(self.drop_hover(cx) == Some(&folder))
        .zoom(self.preferences.grid_zoom);
        let tile = match self.renaming(target, &item.path) {
            true => tile.editor(crate::components::rename_box(self.rename_field.clone())),
            false => tile,
        };
        let tile = match self.preferences.selection_checkboxes {
            true => tile.checkbox(self.toggle_selection(target, index, cx)),
            false => tile,
        };
        let (workspace, api) = (cx.entity().downgrade(), pane.api.clone());
        let card = self.hover_card(target, &item, cx);
        let on_menu = cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
            this.show_item_context_menu(target, index, event.position, window, cx);
            cx.stop_propagation();
        });
        let tab = path.clone();
        let on_middle = cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| this.middle_click_folder(target, tab.clone(), event, cx));
        let on_click = cx.listener(move |this, event: &ClickEvent, _, cx| {
            this.active_pane = target;
            this.activate_list_item(index, &path, is_dir, event, cx)
        });
        Some(tile.decorate(move |tile| {
            let tile = crate::drag_out::draggable(tile, target, index, workspace.clone(), api);
            let tile = if is_dir { drop_zone(tile, folder, workspace).on_mouse_down(gpui::MouseButton::Middle, on_middle) } else { tile };
            let tile = tile.on_mouse_down(gpui::MouseButton::Right, on_menu).on_click(on_click);
            match card {
                Some(card) => tile.tooltip(card),
                None => tile,
            }
        }))
    }
}
