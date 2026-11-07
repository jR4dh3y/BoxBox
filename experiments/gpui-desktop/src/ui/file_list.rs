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
    fn render_header(
        &self,
        pane_id: usize,
        shown: Columns,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let sort = self.view(pane_id).sort;
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
            .h(px(32.))
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
                        let next = this.view(pane_id).sort.toggled(field);
                        this.pane_mut().set_sort(next);
                        cx.notify();
                    }))
            }))
    }

    pub(crate) fn render_file_list(
        &self,
        pane_id: usize,
        window: &gpui::Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let shown = Columns::for_width(self.pane_width(window));
        let pane = self.view(pane_id);
        let count = pane.items.len();
        let empty = count == 0 && matches!(pane.listing, Listing::Loaded);
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(c(SURFACE_PRIMARY))
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    this.open_background_menu(event.position, cx)
                }),
            )
            .child(self.render_header(pane_id, shown, cx))
            .when(empty, |list| {
                list.child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(8.))
                        .py(px(48.))
                        .child(icon("folder-open", 32., alpha(TEXT_MUTED, 0.5)))
                        .child(
                            div()
                                .text_13()
                                .text_color(c(TEXT_MUTED))
                                .child("This folder is empty"),
                        ),
                )
            })
            .when(!empty, |list| {
                list.child(
                    uniform_list(
                        "file-list",
                        count,
                        cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                            range
                                .map(|index| this.render_row(pane_id, index, shown, cx))
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
        pane_id: usize,
        index: usize,
        shown: Columns,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pane = self.view(pane_id);
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

        let drop_dest = item.is_dir.then(|| item.path.clone());
        crate::drag_out::draggable(
            div().id(("row", index)),
            pane_id,
            index,
            cx.entity().downgrade(),
            self.api.clone(),
        )
        .relative()
        .w_full()
        .flex()
        .h(px(ROW_HEIGHT))
        .border_b_1()
        .border_color(c(BORDER_SECONDARY))
        .text_size(px(13.))
        .line_height(px(20.))
        .cursor_default()
        .map(|row| match (selected, active) {
            (true, _) => row
                .bg(c(SELECTION))
                .hover(|style| style.bg(c(SELECTION_HOVER))),
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
                    .bg(c(ACCENT)),
            )
        })
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
                        .child(icon(file_icon(&item.name, item.is_dir), 16., icon_color)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(name_color)
                        .child(
                            crate::settings::display_name(
                                &item.name,
                                item.is_dir,
                                self.preferences.show_extensions,
                            )
                            .to_string(),
                        ),
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
        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
            this.activate_item(index, &path, is_dir, event, window, cx)
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

    /// Shared by list rows and grid tiles: select on click, open folders on double-click.
    pub(crate) fn activate_item(
        &mut self,
        index: usize,
        path: &str,
        is_dir: bool,
        event: &ClickEvent,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if event.click_count() >= 2 {
            if let Some(item) = self
                .pane()
                .items
                .get(index)
                .filter(|item| item.path == path && item.is_dir == is_dir)
            {
                self.open_item(item.clone(), cx);
            }
            return;
        }
        self.pane_mut().click(index, event.modifiers());
        // "Preview on single click" previews files on a plain click.
        let plain =
            !(event.modifiers().shift || event.modifiers().control || event.modifiers().platform);
        if plain && !is_dir && self.preferences.preview_on_single_click {
            if let Some(item) = self.pane().items.get(index).cloned() {
                self.open_preview(item, window, cx);
            }
        }
        cx.notify();
    }
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
