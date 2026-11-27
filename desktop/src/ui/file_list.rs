//! `FileList.svelte`: the view. It wires `FileListHeader` and `FileRow` to the
//! pane; the components own the look.

use std::rc::Rc;

use chrono::{Datelike as _, Local, NaiveDate};
use gpui::{div, prelude::*, px, uniform_list, ClickEvent, Context, SharedString};

use super::{icon, TextScale};
use crate::components::{sideways, table_frame, Column, ColumnLayout, ColumnSpec, EntryIcon, FileListHeader, FileRow, Selection, Table};
use crate::api::FileInfo;
use crate::format::{display_name, file_date_at, file_size, type_description};
use crate::theme::*;
use crate::workspace::drag_drop::{drop_zone, DropTarget};
use crate::sort::{Sort, SortDir, SortField};
use crate::workspace::operations::{ClipboardMode, PaneTarget};
use crate::workspace::place_menu::Place;
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn render_file_list(&self, target: PaneTarget, window: &gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.pane_by_id(target.0).unwrap_or_else(|| self.pane());
        let count = pane.items.len();
        let placeholder = pane.placeholder();
        let empty = placeholder.is_some();
        let layout = self.preferences.columns(Table::Files);
        let specs: Rc<[ColumnSpec]> = Rc::from(layout.specs(Table::Files));
        let sorted = pane.display_sort().map(|sort| (sort.field.column(), sort.dir));
        let table = table_frame(&layout, Table::Files).child(
            self.table_header(Table::Files, layout, target, cx).sorted(sorted).on_sort({
                let workspace = cx.entity().downgrade();
                move |column, _, cx| {
                    let Some(field) = SortField::of_column(column) else { return };
                    let _ = workspace.update(cx, |this, cx| {
                        let Some(pane) = this.pane_by_id(target.0) else { return };
                        let next = pane.display_sort().map_or(Sort { field, dir: SortDir::Asc }, |sort| sort.toggled(field));
                        this.active_pane = target;
                        this.choose_sort(target, next, cx);
                    });
                }
            }),
        );
        let table = table
            .when_some(placeholder, |list, (glyph, message)| {
                list.child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(8.))
                        .py(px(48.))
                        .child(icon(glyph, 32., alpha(TEXT_MUTED, 0.5)))
                        .child(div().text_13().text_color(c(TEXT_MUTED)).child(message)),
                )
            })
            .when(!empty, |list| {
                list.child(
                    uniform_list(
                        "file-list",
                        count,
                        cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                            let specs = specs.clone();
                            let _span = crate::profile::span("list rows").with(|| format!("{} rows", range.len()));
                            let size = crate::components::ListMetrics::new(this.preferences.list_zoom).icon + 4.;
                            let now = Local::now();
                            let date = (now.date_naive(), now.year());
                            range
                                .filter_map(|index| {
                                    let image = this.request_and_item_icon(target, index, size, cx);
                                    this.render_row(target, index, specs.clone(), image, date, cx)
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .track_scroll(pane.list_scroll.clone())
                    .flex_1(),
                )
            });
        div()
            .relative()
            .size_full()
            .bg(self.file_background())
            .child(sideways(("file-table", target.0), table))
            .child(self.pointer_layer(target, window, cx))
    }

    /// A table's header with what every table does: border drags and double
    /// clicks size its columns, and a right click offers them.
    pub(crate) fn table_header(&self, table: Table, layout: ColumnLayout, target: PaneTarget, cx: &mut Context<Self>) -> FileListHeader {
        let workspace = cx.entity().downgrade();
        let menu = workspace.clone();
        FileListHeader::new(table, layout, target.0)
            .background(self.chrome_background())
            .on_resize(move |layout, _, cx| {
                let _ = workspace.update(cx, |this, cx| this.resize_columns(table, layout, cx));
            })
            .on_menu(move |position, _, cx| {
                let _ = menu.update(cx, |this, cx| this.open_place_menu(Place::Columns(table), position, cx));
            })
    }

    /// Sets a table's columns in every pane, and saves them once a drag has
    /// rested, not at every step.
    pub(crate) fn resize_columns(&mut self, table: Table, layout: ColumnLayout, cx: &mut Context<Self>) {
        if self.preferences.columns(table) == layout {
            return;
        }
        self.preferences.set_columns(table, layout.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(400)).await;
            this.update(cx, |this, cx| {
                if this.preferences.columns(table) == layout {
                    this.save_preferences(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn render_row(
        &self,
        target: PaneTarget,
        index: usize,
        specs: Rc<[ColumnSpec]>,
        image: EntryIcon,
        (today, year): (NaiveDate, i32),
        cx: &mut Context<Self>,
    ) -> Option<FileRow> {
        let pane = self.pane_by_id(target.0)?;
        let item = pane.items.get(index)?;
        let (path, is_dir) = (item.path.clone(), item.is_dir);
        let folder = DropTarget::Folder { pane: target, path: path.clone() };
        let style = self.glyph_style(&pane.api, item);
        let kind: SharedString = if is_dir { SharedString::from("Folder") } else { SharedString::from(type_description(&item.name)) };
        let row = FileRow::new(("row", index), display_name(&item.name, is_dir, self.preferences.show_file_extensions), is_dir, self.selection_style())
            .icon(style.glyph)
            .tint(style.tint, style.emblem)
            .image(image)
            .location(if pane.shows_locations() { crate::search::location(&pane.path, &item.path).to_string() } else { String::new() })
            .value(Column::Type, kind)
            .value(Column::Size, if is_dir { SharedString::from("—") } else { SharedString::from(file_size(item.size)) })
            .value(Column::Modified, file_date_at(item.mod_time, today, year))
            .selected(pane.is_selected(&item.path))
            .active(pane.active == Some(index))
            .cut(self.is_cut(pane, item))
            .drop_target(self.drop_hover(cx) == Some(&folder))
            .zoom(self.preferences.list_zoom)
            .columns(specs);
        let row = match self.renaming(target, &item.path) {
            true => row.editor(crate::components::rename_box(self.rename_field.clone())),
            false => row,
        };
        let row = match self.preferences.selection_checkboxes {
            true => row.checkbox(self.toggle_selection(target, index, cx)),
            false => row,
        };
        let (workspace, api) = (cx.entity().downgrade(), pane.api.clone());
        let card = self.hover_card(target, item, cx);
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
        Some(row.decorate(move |row| {
            let row = crate::drag_out::draggable(row.debug_selector(move || format!("row-{}-{index}", target.0)), target, index, workspace.clone(), api);
            let row = if is_dir { drop_zone(row, folder, workspace).on_mouse_down(gpui::MouseButton::Middle, on_middle) } else { row };
            let row = row.on_mouse_down(gpui::MouseButton::Right, on_menu).on_click(on_click);
            match card {
                Some(card) => row.tooltip(card),
                None => row,
            }
        }))
    }

    /// How selected rows and tiles are drawn, from the accent.
    pub(crate) fn selection_style(&self) -> Selection {
        Selection { background: self.selection_color(), accent: c(self.accent_hex()) }
    }

    /// Whether the item is on the clipboard to be moved.
    pub(crate) fn is_cut(&self, pane: &crate::pane::Pane, item: &FileInfo) -> bool {
        self.file_clipboard.as_ref().is_some_and(|clipboard| {
            clipboard.mode == ClipboardMode::Cut
                && clipboard.entries.iter().any(|entry| entry.item.path == item.path && std::sync::Arc::ptr_eq(&entry.api, &pane.api))
        })
    }

    /// The selection checkbox: toggles one item, like Ctrl+click.
    pub(crate) fn toggle_selection(
        &self,
        target: PaneTarget,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Box<dyn Fn(&ClickEvent, &mut gpui::Window, &mut gpui::App)> {
        Box::new(cx.listener(move |this, _, _, cx| {
            this.active_pane = target;
            if let Some(pane) = this.pane_mut_by_id(target.0) {
                pane.click(index, gpui::Modifiers { control: true, ..Default::default() });
            }
            cx.notify();
        }))
    }

    /// Shared by list rows and grid tiles: select on click, open folders on double-click.
    pub(crate) fn activate_list_item(
        &mut self,
        index: usize,
        path: &str,
        is_dir: bool,
        event: &ClickEvent,
        cx: &mut Context<Self>,
    ) {
        if event.click_count() >= 2 {
            if is_dir { self.navigate(path.to_string(), cx); }
            else if let Some(item) = self.pane().items.get(index).cloned() {
                self.activate_item(self.active_pane, item, cx);
            }
            return;
        }
        self.pane_mut().click(index, event.modifiers());
        cx.notify();
    }
}

