//! `FileList.svelte`: the view. It wires `FileListHeader` and `FileRow` to the
//! pane; the components own the look.

use gpui::{div, prelude::*, px, uniform_list, ClickEvent, Context};

use super::{icon, TextScale};
use crate::components::{FileListHeader, FileRow, Selection};
use crate::api::FileInfo;
use crate::format::{display_name, file_date, file_icon, file_size, type_description};
use crate::theme::*;
use crate::workspace::drag_drop::{drop_zone, DropTarget};
use crate::workspace::operations::{ClipboardMode, PaneTarget};
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn render_file_list(&self, target: PaneTarget, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.pane_by_id(target.0).unwrap_or_else(|| self.pane());
        let count = pane.items.len();
        let placeholder = pane.placeholder();
        let empty = placeholder.is_some();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(self.file_background())
            .child(FileListHeader::new(pane.sort).unsorted(pane.shows_ranked()).on_sort({
                let workspace = cx.entity().downgrade();
                move |field, _, _, cx| {
                    let _ = workspace.update(cx, |this, cx| {
                        this.active_pane = target;
                        let next = this.pane().sort.toggled(field);
                        this.set_pane_sort(target, next, cx);
                    });
                }
            }))
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
                            range.filter_map(|index| this.render_row(target, index, cx)).collect::<Vec<_>>()
                        }),
                    )
                    .track_scroll(pane.list_scroll.clone())
                    .flex_1(),
                )
            })
    }

    fn render_row(&self, target: PaneTarget, index: usize, cx: &mut Context<Self>) -> Option<FileRow> {
        let pane = self.pane_by_id(target.0)?;
        let item = pane.items.get(index)?;
        let (path, is_dir) = (item.path.clone(), item.is_dir);
        let folder = DropTarget::Folder { pane: target, path: path.clone() };
        let row = FileRow::new(("row", index), display_name(&item.name, is_dir, self.preferences.show_file_extensions), is_dir, self.selection_style())
            .icon(file_icon(&item.name, is_dir))
            .location(if pane.shows_locations() { crate::search::location(&pane.path, &item.path).to_string() } else { String::new() })
            .details(
                if is_dir { "Folder".into() } else { type_description(&item.name) },
                if is_dir { "—".into() } else { file_size(item.size) },
                file_date(&item.mod_time),
            )
            .selected(pane.is_selected(&item.path))
            .active(pane.active == Some(index))
            .cut(self.is_cut(pane, item))
            .drop_target(self.drop_hover(cx) == Some(&folder))
            .compact(self.preferences.compact_rows);
        let row = match self.renaming(target, &item.path) {
            true => row.editor(crate::components::rename_box(self.rename_field.clone())),
            false => row,
        };
        let row = match self.preferences.selection_checkboxes {
            true => row.checkbox(self.toggle_selection(target, index, cx)),
            false => row,
        };
        let (workspace, api) = (cx.entity().downgrade(), pane.api.clone());
        let on_menu = cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
            this.show_item_context_menu(target, index, event.position, cx);
            cx.stop_propagation();
        });
        let on_click = cx.listener(move |this, event: &ClickEvent, window, cx| {
            this.active_pane = target;
            this.activate_list_item(index, &path, is_dir, event, window, cx)
        });
        Some(row.decorate(move |row| {
            let row = crate::drag_out::draggable(row.debug_selector(move || format!("row-{}-{index}", target.0)), target, index, workspace.clone(), api);
            let row = if is_dir { drop_zone(row, folder, workspace) } else { row };
            row.on_mouse_down(gpui::MouseButton::Right, on_menu)
                .on_click(on_click)
        }))
    }

    /// How selected rows and tiles are drawn, from the accent.
    pub(crate) fn selection_style(&self) -> Selection {
        Selection { background: self.selection_color(false), hover: self.selection_color(true), accent: c(self.accent_hex()) }
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
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if event.click_count() >= 2 {
            if is_dir { self.navigate(path.to_string(), cx); }
            else if let Some(item) = self.pane().items.get(index).cloned() {
                self.activate_item(self.active_pane, item, window, cx);
            }
            return;
        }
        self.pane_mut().click(index, event.modifiers());
        if !is_dir && self.preferences.preview_on_single_click && !event.modifiers().control && !event.modifiers().shift {
            if let Some(item) = self.pane().items.get(index).cloned() {
                self.preview_item(self.active_pane, item, window, cx);
            }
        }
        cx.notify();
    }
}
