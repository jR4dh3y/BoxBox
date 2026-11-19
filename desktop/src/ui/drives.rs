//! The Overview: `DriveCard.svelte` cards or the `DriveList.svelte` table.

use gpui::{div, prelude::*, px, Context, FontWeight, Window};

use super::{icon, TextScale};
use crate::components::{skeleton, usage_bar, usage_text, DriveCard};
use crate::format::file_size;
use crate::workspace::overview::Storage;
use crate::workspace::place_menu::Place;
use crate::theme::*;
use crate::workspace::operations::PaneTarget;
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn render_drive_cards(
        &self,
        target: PaneTarget,
        drives: &[Storage],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // `@min-[49rem]:grid-cols-2 @min-[74rem]:grid-cols-3`, against the content box inside p-6.
        let width = self.pane_width(target, window) - 48.;
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

    fn render_drive_card(&self, index: usize, drive: &Storage, cx: &mut Context<Self>) -> impl IntoElement {
        let open = drive.clone();
        let card = DriveCard::new(("drive-card", index), self.drive_info(drive), c(self.accent_hex()), self.fonts.mono.clone())
            .on_click(cx.listener(move |this, _, window, cx| this.open_storage(open.clone(), window, cx)));
        self.with_drive_menu(div().flex_1().min_w_0().flex().child(card), drive, cx)
    }

    /// The drive's right-click menu: a device's own, or Rename and Reset Name.
    fn with_drive_menu<E: InteractiveElement>(&self, element: E, drive: &Storage, cx: &mut Context<Self>) -> E {
        let menu = match drive {
            Storage::Volume(volume) => Some(Place::Device(volume.clone())),
            Storage::Drive(stats) => self.drive_key(drive).map(|(key, own)| Place::Drive { key, own, folder: Some(stats.name.clone()) }),
            _ => self.drive_key(drive).map(|(key, own)| Place::Drive { key, own, folder: None }),
        };
        let Some(menu) = menu else { return element };
        element.on_mouse_down(gpui::MouseButton::Right, cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
            cx.stop_propagation();
            this.open_place_menu(menu.clone(), event.position, cx);
        }))
    }

    pub(crate) fn render_drive_list(
        &self,
        target: PaneTarget,
        drives: &[Storage],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let width = self.pane_width(target, window);
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
            .bg(self.chrome_background())
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

        let rows = drives.iter().enumerate().map(|(index, storage)| {
            let open = storage.clone();
            let drive = self.drive_info(storage);
            let usage = drive.state.usage();
            let wait = drive.state.wait();
            let mono = self.fonts.mono.clone();
            let detail = |text: Option<String>| text.filter(|text| !text.is_empty()).unwrap_or("—".into());
            let row = div()
                .id(("drive-row", index))
                .flex()
                .h(px(32.))
                .items_center()
                .border_b_1()
                .border_color(c(BORDER_SECONDARY))
                .text_13()
                .cursor_default()
                .child(
                    name_col(cell())
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(icon(drive.glyph, 16., c(if usage.is_some() { TEXT_SECONDARY } else { TEXT_MUTED })))
                        .child(div().min_w_0().truncate().when(usage.is_none(), |name| name.text_color(c(TEXT_MUTED))).child(drive.name.clone()))
                        .when(drive.read_only, |n| n.child(div().flex_none().text_xs_tw().text_color(c(WARNING)).child("Read-only"))),
                )
                .when(wide, |row| {
                    row.child(grow(cell(), 1., 140.).font_family(mono.clone()).text_size(px(12.)).text_color(c(TEXT_MUTED)).child(detail(drive.device.clone())))
                        .child(fixed(cell(), 104.).font_family(mono).text_size(px(12.)).text_color(c(TEXT_SECONDARY)).child(detail(drive.fs_type.clone())))
                })
                .child(fixed(cell(), 96.).flex().justify_end().text_color(c(TEXT_SECONDARY)).child(drive.state.size().map_or("—".into(), file_size)))
                .when(medium, |row| row.child(fixed(cell(), 96.).flex().justify_end().text_color(c(TEXT_SECONDARY)).child(usage.map_or("—".into(), |usage| file_size(usage.free)))))
                // An unknown usage keeps the column's empty track, as the cards do.
                .child(
                    used_col(cell())
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(match wait {
                            Some(wait) => skeleton::bar(("drive-row-pulse", index), 4., wait),
                            None => usage_bar(("drive-row-usage", index), usage.map_or(0., |usage| usage.pct)).into_any_element(),
                        })
                        .child(div().w(px(44.)).flex().justify_end().text_color(c(TEXT_MUTED)).child(match usage {
                            Some(usage) => usage_text(usage.pct).into_any_element(),
                            None => "—".into_any_element(),
                        })),
                )
                .on_click(cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                    if event.click_count() >= 2 {
                        this.open_storage(open.clone(), window, cx);
                    }
                }));
            crate::components::motion::interact(("drive-row-feedback", index), self.with_drive_menu(row, storage, cx)).fill(crate::components::motion::hover_fill())
        });

        div()
            .id("drive-list")
            .size_full()
            .overflow_y_scroll()
            // As the file list: see-through over a wallpaper or a transparent window.
            .bg(self.file_background())
            .child(header)
            .children(rows)
    }
}
