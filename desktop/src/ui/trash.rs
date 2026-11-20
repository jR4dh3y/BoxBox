//! The Trash page: the folder lists' table, with what was deleted, from where
//! and when. Rows select as a folder's do; Restore and Delete Permanently act
//! on the selection from the toolbar, the row menu and the Delete key, as in
//! Thunar.

use gpui::{div, prelude::*, px, uniform_list, AnyElement, ClickEvent, Context, SharedString};

use super::{icon, TextScale};
use crate::api::{FileInfo, TrashItem};
use crate::components::{sideways, table_frame, Button, ButtonSize, ButtonVariant, Column, ColumnSpec, FileRow, ListMetrics, Table};
use crate::format::{deletion_millis, file_date, type_description, Kind};
use crate::theme::*;
use crate::workspace::trash_table::location;
use crate::workspace::{Load, Workspace};

/// What the icon cache needs of a trashed item: its name and kind. It has no
/// path a listing could read.
fn icon_entry(item: &TrashItem) -> FileInfo {
    FileInfo { name: item.name.clone(), path: String::new(), size: 0, is_dir: item.is_dir, hidden: false, mod_time: None }
}

impl Workspace {
    pub(crate) fn empty_trash_button(&self, cx: &mut Context<Self>) -> Button {
        Button::new("empty-trash")
            .label("Empty Trash")
            .variant(ButtonVariant::Danger)
            .size(ButtonSize::Sm)
            .disabled(self.trash_items().is_empty() || self.operation_status.is_busy())
            .on_click(cx.listener(|this, _, _, cx| this.empty_trash(cx)))
    }

    /// Restores the selected items, when any of them can go back.
    pub(crate) fn restore_trash_button(&self, cx: &mut Context<Self>) -> Button {
        let restorable = self.selected_trash().iter().any(|item| item.can_restore);
        Button::new("restore-trash")
            .label("Restore")
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::Sm)
            .disabled(!restorable || self.operation_status.is_busy())
            .on_click(cx.listener(|this, _, _, cx| this.restore_trash(this.selected_trash(), cx)))
    }

    pub(crate) fn render_trash(&self, window: &gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let message = |text: SharedString, glyph: Option<&str>| {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .items_center()
                .justify_center()
                .gap(px(12.))
                .text_sm_tw()
                .text_color(c(TEXT_SECONDARY))
                .when_some(glyph, |el, glyph| el.child(icon(glyph, 32., alpha(TEXT_MUTED, 0.5))))
                .child(text)
                .into_any_element()
        };
        let body: AnyElement = match &self.trash {
            Load::Loading => message("Loading Trash…".into(), None),
            Load::Failed(error) => message(error.clone().into(), Some("alert-triangle")),
            Load::Loaded(items) if items.is_empty() => message("Trash is empty".into(), Some("trash-2")),
            Load::Loaded(items) => {
                let layout = self.table_columns(Table::Trash, self.active_pane, window);
                let specs = layout.specs(Table::Trash);
                let header = self
                    .table_header(Table::Trash, layout.clone(), self.active_pane, cx)
                    .sorted(Some(self.trash_table.sort))
                    .on_sort({
                        let workspace = cx.entity().downgrade();
                        move |column, _, cx| {
                            let _ = workspace.update(cx, |this, cx| this.sort_trash_by(column, cx));
                        }
                    });
                let rows = uniform_list(
                    "trash-list",
                    items.len(),
                    cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                        range.filter_map(|index| this.render_trash_row(index, specs.clone(), cx)).collect::<Vec<_>>()
                    }),
                )
                .track_scroll(self.trash_table.scroll.clone())
                .flex_1();
                sideways("trash-table", table_frame(&layout, Table::Trash).child(header).child(rows)).into_any_element()
            }
        };
        div().id("trash-page").size_full().flex().flex_col().bg(self.file_background()).child(body)
    }

    fn render_trash_row(&mut self, index: usize, specs: Vec<ColumnSpec>, cx: &mut Context<Self>) -> Option<FileRow> {
        let item = self.trash_items().get(index)?.clone();
        let api = self.trash_provider.clone()?;
        let size = ListMetrics::new(self.preferences.list_zoom).icon + 4.;
        let entry = icon_entry(&item);
        self.request_item_icon(&api, &entry, size, cx);
        let kind = Kind::of(&item.name, item.is_dir);
        let deleted = deletion_millis(&item.deleted_at);
        let selected = self.trash_table.selected.contains(&item.id);
        let row = FileRow::new(("trash-row", index), item.name.clone(), item.is_dir, self.selection_style())
            .icon(kind.glyph())
            .tint(self.kind_tint(kind), None)
            .image(self.item_icon(&api, &entry, size))
            .columns(specs)
            .value(Column::Location, location(&item.original_path))
            .value(Column::Deleted, file_date(deleted))
            .value(Column::Type, if item.is_dir { "Folder".into() } else { type_description(&item.name) })
            .selected(selected)
            .active(self.trash_table.anchor == Some(index) && self.trash_table.selected.len() > 1)
            .zoom(self.preferences.list_zoom);
        let row = match self.preferences.selection_checkboxes {
            true => row.checkbox(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.click_trash(index, gpui::Modifiers { control: true, ..Default::default() }, cx);
            })),
            false => row,
        };
        let on_click = cx.listener(move |this, event: &ClickEvent, _, cx| this.click_trash(index, event.modifiers(), cx));
        let on_menu = cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
            cx.stop_propagation();
            this.trash_menu(index, event.position, cx);
        });
        let name = item.name.clone();
        Some(row.decorate(move |row| {
            row.debug_selector(move || format!("trash-row-{name}")).on_click(on_click).on_mouse_down(gpui::MouseButton::Right, on_menu)
        }))
    }
}
