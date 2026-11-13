//! This PC: `DriveCard.svelte` cards or the `DriveList.svelte` table.

use gpui::{div, prelude::*, px, Context, FontWeight, Window};

use super::{icon, TextScale};
use crate::components::{usage_bar, usage_text, DriveCard};
use crate::api::DriveStats;
use crate::format::file_size;
use crate::theme::*;
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn render_drive_cards(
        &self,
        drives: &[DriveStats],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // `@min-[49rem]:grid-cols-2 @min-[74rem]:grid-cols-3`, against the content box inside p-6.
        let width = self.browser_width(window) - 48.;
        let columns = if width >= 1184. {
            3
        } else if width >= 784. {
            2
        } else {
            1
        };
        let mut rows = Vec::new();
        for (row_index, chunk) in drives.chunks(columns).enumerate() {
            let mut row = div().flex().gap(px(16.));
            for (offset, drive) in chunk.iter().enumerate() {
                // Taffy floors a flex item's base size at its padding and border, so a padded
                // card would outgrow an empty column; a bare cell keeps columns equal, as grid does.
                row = row.child(div().flex_1().flex_basis(px(0.)).min_w_0().flex().child(self.render_drive_card(row_index * columns + offset, drive, cx)));
            }
            for _ in chunk.len()..columns {
                row = row.child(div().flex_1().flex_basis(px(0.)));
            }
            rows.push(row);
        }
        div()
            .id("drive-cards")
            .size_full()
            .overflow_y_scroll()
            .p(px(24.))
            .child(div().flex().flex_col().gap(px(16.)).children(rows))
    }

    fn render_drive_card(&self, index: usize, drive: &DriveStats, cx: &mut Context<Self>) -> impl IntoElement {
        let name = drive.name.clone();
        DriveCard::new(("drive-card", index), drive.clone(), c(self.accent_hex()), self.fonts.mono.clone())
            .on_click(cx.listener(move |this, _, _, cx| this.navigate(name.clone(), cx)))
    }

    pub(crate) fn render_drive_list(
        &self,
        drives: &[DriveStats],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let width = self.browser_width(window);
        // Container breakpoints @sm (24rem) and @3xl (48rem) choose the columns.
        let wide = width >= 768.;
        let medium = width >= 384.;
        let grow = |cell: gpui::Div, factor: f32, min: f32| {
            let mut cell = cell.flex_basis(px(0.)).min_w(px(min));
            cell.style().flex_grow = Some(factor);
            cell
        };
        let fixed = |cell: gpui::Div, width: f32| cell.w(px(width)).flex_none();
        let cell = || {
            div()
                .px(px(12.))
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
        };
        let name_col = move |c: gpui::Div| {
            if wide {
                grow(c, 1., 140.)
            } else if medium {
                grow(c, 1., 80.)
            } else {
                grow(c, 1., 0.)
            }
        };
        let used_col = move |c: gpui::Div| {
            if wide {
                grow(c, 1.4, 180.)
            } else if medium {
                grow(c, 1., 90.)
            } else {
                grow(c, 0.8, 80.)
            }
        };

        let header = div()
            .flex()
            .flex_none()
            .h(px(32.))
            .items_center()
            .border_b_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_SECONDARY))
            .text_13()
            .font_weight(FontWeight::MEDIUM)
            .text_color(c(TEXT_SECONDARY))
            .child(name_col(cell()).child("Name"))
            .when(wide, |h| {
                h.child(grow(cell(), 1., 140.).child("Device"))
                    .child(fixed(cell(), 104.).child("Filesystem"))
            })
            .child(fixed(cell(), 96.).flex().justify_end().child("Size"))
            .when(medium, |h| {
                h.child(fixed(cell(), 96.).flex().justify_end().child("Free"))
            })
            .child(used_col(cell()).child("Used"));

        let rows = drives.iter().enumerate().map(|(index, drive)| {
            let name = drive.name.clone();
            div()
                .id(("drive-row", index))
                .flex()
                .h(px(32.))
                .items_center()
                .border_b_1()
                .border_color(c(BORDER_SECONDARY))
                .text_13()
                .cursor_default()
                .hover(|style| style.bg(c(SURFACE_SECONDARY)))
                .child(
                    name_col(cell())
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(icon("hard-drive", 16., c(TEXT_SECONDARY)))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .child(drive.display_name().to_string()),
                        )
                        .when(drive.read_only, |n| {
                            n.child(
                                div()
                                    .flex_none()
                                    .text_xs_tw()
                                    .text_color(c(WARNING))
                                    .child("Read-only"),
                            )
                        }),
                )
                .when(wide, |row| {
                    let mono = self.fonts.mono.clone();
                    row.child(
                        grow(cell(), 1., 140.)
                            .font_family(mono.clone())
                            .text_size(px(12.))
                            .text_color(c(TEXT_MUTED))
                            .child(
                                drive
                                    .device
                                    .clone()
                                    .filter(|d| !d.is_empty())
                                    .unwrap_or("—".into()),
                            ),
                    )
                    .child(
                        fixed(cell(), 104.)
                            .font_family(mono)
                            .text_size(px(12.))
                            .text_color(c(TEXT_SECONDARY))
                            .child(
                                drive
                                    .fs_type
                                    .clone()
                                    .filter(|f| !f.is_empty())
                                    .unwrap_or("—".into()),
                            ),
                    )
                })
                .child(
                    fixed(cell(), 96.)
                        .flex()
                        .justify_end()
                        .text_color(c(TEXT_SECONDARY))
                        .child(file_size(drive.total_bytes)),
                )
                .when(medium, |row| {
                    row.child(
                        fixed(cell(), 96.)
                            .flex()
                            .justify_end()
                            .text_color(c(TEXT_SECONDARY))
                            .child(file_size(drive.free_bytes)),
                    )
                })
                .child(
                    used_col(cell())
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(usage_bar(drive.used_pct))
                        .child(
                            div()
                                .w(px(44.))
                                .flex()
                                .justify_end()
                                .child(usage_text(drive.used_pct)),
                        ),
                )
                .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                    if event.click_count() >= 2 {
                        this.navigate(name.clone(), cx);
                    }
                }))
        });

        div()
            .id("drive-list")
            .size_full()
            .overflow_y_scroll()
            .bg(c(SURFACE_PRIMARY))
            .child(header)
            .children(rows)
    }
}
