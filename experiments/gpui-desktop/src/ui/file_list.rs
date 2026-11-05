//! `FileList.svelte`: sticky sortable header over virtualised 32px rows.

use gpui::{div, prelude::*, px, uniform_list, ClickEvent, Context, FontWeight};

use super::{icon, TextScale};
use crate::format::{file_date, file_icon, file_size, type_description};
use crate::pane::Listing;
use crate::sort::{SortDir, SortField};
use crate::theme::*;
use crate::workspace::Workspace;

pub const ROW_HEIGHT: f32 = 32.;
const TYPE_WIDTH: f32 = 170.;
const SIZE_WIDTH: f32 = 112.;
const MODIFIED_WIDTH: f32 = 168.;

/// Fixed-width column, or the flexible Name column when `width` is None.
fn column(width: Option<f32>) -> gpui::Div {
    let cell = div().flex().items_center().px(px(12.)).min_w_0().overflow_hidden();
    match width {
        Some(width) => cell.w(px(width)).flex_none(),
        None => cell.flex_1(),
    }
}

impl Workspace {
    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let sort = self.pane().sort;
        let columns = [
            (SortField::Name, "Name", None, false),
            (SortField::Type, "Type", Some(TYPE_WIDTH), false),
            (SortField::Size, "Size", Some(SIZE_WIDTH), true),
            (SortField::Modified, "Modified", Some(MODIFIED_WIDTH), false),
        ];
        div()
            .flex()
            .flex_none()
            .h(px(32.))
            .bg(c(SURFACE_SECONDARY))
            .border_b_1()
            .border_color(c(BORDER_PRIMARY))
            .text_size(px(13.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .children(columns.into_iter().map(|(field, label, width, right)| {
                let sorted = sort.field == field;
                let color = if sorted { ACCENT } else { TEXT_SECONDARY };
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
                    .hover(|style| style.bg(c(SURFACE_TERTIARY)).text_color(c(if sorted { ACCENT } else { TEXT_PRIMARY })))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .child(label)
                            .child(div().w(px(12.)).flex().justify_center().when(sorted, |slot| slot.child(icon(chevron, 12., c(color))))),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let next = this.pane().sort.toggled(field);
                        this.pane_mut().set_sort(next);
                        cx.notify();
                    }))
            }))
    }

    pub(crate) fn render_file_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.pane();
        let count = pane.items.len();
        let empty = count == 0 && matches!(pane.listing, Listing::Loaded);
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(c(SURFACE_PRIMARY))
            .child(self.render_header(cx))
            .when(empty, |list| {
                list.child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(8.))
                        .py(px(48.))
                        .child(icon("folder-open", 32., alpha(TEXT_MUTED, 0.5)))
                        .child(div().text_13().text_color(c(TEXT_MUTED)).child("This folder is empty")),
                )
            })
            .when(!empty, |list| {
                list.child(
                    uniform_list(
                        "file-list",
                        count,
                        cx.processor(|this, range: std::ops::Range<usize>, _window, cx| {
                            range.map(|index| this.render_row(index, cx)).collect::<Vec<_>>()
                        }),
                    )
                    .track_scroll(pane.list_scroll.clone())
                    .flex_1(),
                )
            })
    }

    fn render_row(&self, index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.pane();
        let item = &pane.items[index];
        let selected = pane.is_selected(&item.path);
        let active = pane.active == Some(index);
        let secondary = c(if selected { TEXT_PRIMARY } else { TEXT_SECONDARY });
        let name_color = c(if item.is_dir { FOLDER } else { TEXT_PRIMARY });
        let icon_color = c(if item.is_dir { FOLDER } else { TEXT_SECONDARY });
        let (path, is_dir) = (item.path.clone(), item.is_dir);

        div()
            .id(("row", index))
            .relative()
            .flex()
            .h(px(ROW_HEIGHT))
            .border_b_1()
            .border_color(c(BORDER_SECONDARY))
            .text_size(px(13.))
            .line_height(px(20.))
            .cursor_default()
            .map(|row| match (selected, active) {
                (true, _) => row.bg(c(SELECTION)).hover(|style| style.bg(c(SELECTION_HOVER))),
                (false, true) => row.bg(c(SURFACE_SECONDARY)),
                (false, false) => row.hover(|style| style.bg(c(SURFACE_SECONDARY))),
            })
            // shadow-[inset_2px_0_0_var(--color-accent)]
            .when(active, |row| row.child(div().absolute().left_0().top_0().bottom_0().w(px(2.)).bg(c(ACCENT))))
            .child(
                column(None).gap(px(8.)).child(
                    div().size(px(20.)).flex_none().flex().items_center().justify_center().child(icon(
                        file_icon(&item.name, item.is_dir),
                        16.,
                        icon_color,
                    )),
                )
                .child(div().flex_1().min_w_0().truncate().text_color(name_color).child(item.name.clone())),
            )
            .child(
                column(Some(TYPE_WIDTH))
                    .text_color(secondary)
                    .child(div().truncate().child(if item.is_dir { "Folder".into() } else { type_description(&item.name) })),
            )
            .child(
                column(Some(SIZE_WIDTH))
                    .justify_end()
                    .text_color(secondary)
                    .child(div().truncate().child(if item.is_dir { "—".into() } else { file_size(item.size) })),
            )
            .child(column(Some(MODIFIED_WIDTH)).text_color(secondary).child(div().truncate().child(file_date(&item.mod_time))))
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                this.activate_item(index, &path, is_dir, event, cx)
            }))
    }

    /// Shared by list rows and grid tiles: select on click, open folders on double-click.
    pub(crate) fn activate_item(&mut self, index: usize, path: &str, is_dir: bool, event: &ClickEvent, cx: &mut Context<Self>) {
        if event.click_count() >= 2 && is_dir {
            self.navigate(path.to_string(), cx);
            return;
        }
        self.pane_mut().click(index, event.modifiers());
        cx.notify();
    }
}
