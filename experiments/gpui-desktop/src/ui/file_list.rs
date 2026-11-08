//! `FileList.svelte`: sticky sortable header over virtualised 32px rows.

use gpui::{div, prelude::*, px, uniform_list, ClickEvent, Context, FontWeight};

use super::{icon, TextScale};
use crate::format::{file_date, file_icon, file_size, type_description};
use crate::sort::{SortDir, SortField};
use crate::theme::*;
use crate::workspace::operations::PaneTarget;
use crate::workspace::Workspace;

pub const ROW_HEIGHT: f32 = 32.;
const TYPE_WIDTH: f32 = 170.;
const SIZE_WIDTH: f32 = 112.;
const MODIFIED_WIDTH: f32 = 168.;
const MIN_NAME_WIDTH: f32 = 200.;

/// Secondary columns that fit beside a readable Name column; Type goes first, then Modified.
#[derive(Clone, Copy)]
pub(crate) struct Columns {
    kind: bool,
    modified: bool,
}

impl Columns {
    pub(crate) fn for_width(width: f32) -> Self {
        let spare = width - MIN_NAME_WIDTH - SIZE_WIDTH;
        Self {
            modified: spare >= MODIFIED_WIDTH,
            kind: spare >= MODIFIED_WIDTH + TYPE_WIDTH,
        }
    }


}

/// Fixed-width column, or the flexible Name column when `width` is None.
fn column(width: Option<f32>) -> gpui::Div {
    let cell = div()
        .flex()
        .items_center()
        .px(px(12.))
        .min_w_0()
        .overflow_hidden();
    match width {
        Some(width) => cell.w(px(width)).flex_none(),
        None => cell.flex_1(),
    }
}

impl Workspace {
    fn render_header(&self, target: PaneTarget, shown: Columns, cx: &mut Context<Self>) -> impl IntoElement {
        let sort = self.pane_by_id(target.0).unwrap_or_else(|| self.pane()).sort;
        let columns = [
            (SortField::Name, "Name", None, false, true),
            (SortField::Type, "Type", Some(TYPE_WIDTH), false, shown.kind),
            (SortField::Size, "Size", Some(SIZE_WIDTH), true, true),
            (
                SortField::Modified,
                "Modified",
                Some(MODIFIED_WIDTH),
                false,
                shown.modified,
            ),
        ]
        .into_iter()
        .filter(|column| column.4)
        .map(|(field, label, width, right, _)| (field, label, width, right));
        div()
            .flex()
            .flex_none()
            .h(px(if self.preferences.compact_rows { 28. } else { 32. }))
            .bg(c(SURFACE_SECONDARY))
            .border_b_1()
            .border_color(c(BORDER_PRIMARY))
            .text_size(px(13.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .children(columns.map(|(field, label, width, right)| {
                let sorted = sort.field == field;
                // `thSortedClass` loses to `thClass`'s text colour in the built CSS, so the web
                // draws every header in text-secondary.
                let color = TEXT_SECONDARY;
                let chevron = match sort.dir {
                    SortDir::Asc => "chevron-up",
                    SortDir::Desc => "chevron-down",
                };
                column(width)
                    .id(label)
                    .h_full()
                    .when(right, |cell| cell.justify_end())
                    .text_color(c(color))
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .hover(|style| style.bg(c(SURFACE_TERTIARY)).text_color(c(TEXT_PRIMARY)))
                    .child(
                        div().flex().items_center().gap(px(4.)).child(label).child(
                            div()
                                .w(px(12.))
                                .flex()
                                .justify_center()
                                .when(sorted, |slot| slot.child(icon(chevron, 12., c(color)))),
                        ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let next = this.pane().sort.toggled(field);
                        this.pane_mut().set_sort(next);
                        cx.notify();
                    }))
            }))
    }

    pub(crate) fn render_file_list(
        &self,
        target: PaneTarget,
        window: &gpui::Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let shown = Columns::for_width(self.browser_width(window));
        let pane = self.pane_by_id(target.0).unwrap_or_else(|| self.pane());
        let count = pane.items.len();
        let compact = self.preferences.compact_rows;
        let row_height = if compact { 28. } else { ROW_HEIGHT };
        let placeholder = pane.placeholder();
        let empty = placeholder.is_some();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(self.file_background())
            .child(self.render_header(target, shown, cx))
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
                            range
                                .filter_map(|index| {
                                    let item = this.pane_by_id(target.0)?.items.get(index)?.clone();
                                    this.request_thumbnail(target, item, cx);
                                    Some(this.render_row(target, index, shown, compact, row_height, cx))
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .track_scroll(pane.list_scroll.clone())
                    .flex_1(),
                )
            })
    }

    fn render_row(
        &self,
        target: PaneTarget,
        index: usize,
        shown: Columns,
        compact: bool,
        row_height: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pane = self.pane_by_id(target.0).unwrap_or_else(|| self.pane());
        let target = crate::workspace::operations::PaneTarget(pane.id);
        let item = &pane.items[index];
        let selected = pane.is_selected(&item.path);
        let active = pane.active == Some(index);
        let secondary = c(if selected {
            TEXT_PRIMARY
        } else {
            TEXT_SECONDARY
        });
        let name_color = c(if item.is_dir { FOLDER } else { TEXT_PRIMARY });
        let icon_color = c(if item.is_dir { FOLDER } else { TEXT_SECONDARY });
        let (path, is_dir) = (item.path.clone(), item.is_dir);
        let name = display_name(&item.name, is_dir, self.preferences.show_file_extensions);

        crate::drag_out::draggable(
            div().id(("row", index)).debug_selector(|| format!("row-{}-{index}", target.0)),
            index,
            cx.entity().downgrade(),
            pane.api.clone(),
        )
        .relative()
        .w_full()
        .flex()
        .h(px(row_height))
        .border_b_1()
        .border_color(c(BORDER_SECONDARY))
        .text_size(px(if compact { 12. } else { 13. }))
        .line_height(px(if compact { 18. } else { 20. }))
        .cursor_default()
        .map(|row| match (selected, active) {
            (true, _) => row
                .bg(self.selection_color(false))
                .hover({ let color = self.selection_color(true); move |style| style.bg(color) }),
            (false, true) => row.bg(c(SURFACE_SECONDARY)),
            (false, false) => row.hover(|style| style.bg(c(SURFACE_SECONDARY))),
        })
        // shadow-[inset_2px_0_0_var(--color-accent)]
        .when(active, |row| {
            row.child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(2.))
                    .bg(c(self.accent_hex())),
            )
        })
        .when(self.preferences.selection_checkboxes, |element| element.child(
            div().id(("selection-checkbox", index)).size(px(18.)).flex_none().rounded(px(3.)).border_1()
                .border_color(c(BORDER_PRIMARY)).text_xs_tw().child(if selected { "✓" } else { "" })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.active_pane = target;
                    this.pane_mut().click(index, gpui::Modifiers { control: true, ..Default::default() });
                    cx.stop_propagation(); cx.notify();
                }))
        ))
        .child(
            column(None)
                .gap(px(8.))
                .child(
                    div()
                        .size(px(20.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .map(|slot| match self.thumbnail(&pane.api, item) {
                            Some(image) => slot.child(gpui::img(image).size_full().object_fit(gpui::ObjectFit::Contain)),
                            None => slot.child(icon(file_icon(&item.name, item.is_dir), 16., icon_color)),
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(name_color)
                        .child(name),
                ),
        )
        .when(shown.kind, |row| {
            row.child(
                column(Some(TYPE_WIDTH))
                    .text_color(secondary)
                    .child(div().truncate().child(if item.is_dir {
                        "Folder".into()
                    } else {
                        type_description(&item.name)
                    })),
            )
        })
        .child(
            column(Some(SIZE_WIDTH))
                .justify_end()
                .text_color(secondary)
                .child(div().truncate().child(if item.is_dir {
                    "—".into()
                } else {
                    file_size(item.size)
                })),
        )
        .when(shown.modified, |row| {
            row.child(
                column(Some(MODIFIED_WIDTH))
                    .text_color(secondary)
                    .child(div().truncate().child(file_date(&item.mod_time))),
            )
        })
        .on_mouse_down(gpui::MouseButton::Right, cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
            this.show_item_context_menu(target, index, event.position, cx);
            cx.stop_propagation();
        }))
        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
            this.active_pane = target;
            this.activate_list_item(index, &path, is_dir, event, window, cx)
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

fn display_name(name: &str, is_dir: bool, show_extensions: bool) -> String {
    if is_dir || show_extensions {
        return name.to_string();
    }
    name.rsplit_once('.')
        .filter(|(stem, extension)| !stem.is_empty() && !extension.is_empty())
        .map(|(stem, _)| stem.to_string())
        .unwrap_or_else(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::Columns;

    #[test]
    fn narrow_widths_drop_type_then_modified() {
        let at = |width| {
            let columns = Columns::for_width(width);
            (columns.kind, columns.modified)
        };
        assert_eq!(at(1060.), (true, true));
        assert_eq!(at(420.), (false, false));
        assert_eq!(at(500.), (false, true));
    }
}
