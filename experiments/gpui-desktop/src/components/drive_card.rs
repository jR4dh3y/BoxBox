//! `lib/components/DriveCard.svelte`, and the usage bar and label that the
//! This PC list shares with it.

use gpui::{div, prelude::*, px, AlignItems, App, ClickEvent, ElementId, FontWeight, Rgba, SharedString, Window};

use super::{ProgressBar, ProgressSize, ProgressVariant, Type};
use crate::api::DriveStats;
use crate::format::file_size;
use crate::theme::*;
use crate::ui::{icon, TextScale};

/// `driveUsageLevel`: the bar's variant and the label's colour.
fn usage_level(used_pct: f64) -> (ProgressVariant, u32) {
    if used_pct >= 90. {
        (ProgressVariant::Danger, DANGER)
    } else if used_pct >= 75. {
        (ProgressVariant::Warning, WARNING)
    } else {
        (ProgressVariant::Info, TEXT_MUTED)
    }
}

/// `ProgressBar size="sm"` in the usage colour.
pub fn usage_bar(used_pct: f64) -> impl IntoElement {
    div().flex_1().child(ProgressBar::new(used_pct as f32).size(ProgressSize::Sm).variant(usage_level(used_pct).0))
}

/// The percentage beside the bar: text-[11px] font-medium.
pub fn usage_text(used_pct: f64) -> impl IntoElement {
    div().flex_none().type_11().font_weight(FontWeight::MEDIUM).text_color(c(usage_level(used_pct).1)).child(format!("{used_pct:.1}%"))
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct DriveCard {
    id: ElementId,
    drive: DriveStats,
    accent: Rgba,
    mono: SharedString,
    on_click: Option<ClickHandler>,
}

impl DriveCard {
    pub fn new(id: impl Into<ElementId>, drive: DriveStats, accent: Rgba, mono: SharedString) -> Self {
        Self { id: id.into(), drive, accent, mono, on_click: None }
    }

    pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for DriveCard {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let accent = self.accent;
        div()
            .id(self.id)
            .flex_1()
            .min_w_0()
            .flex()
            .gap(px(12.))
            .rounded(px(8.))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_SECONDARY))
            .p(px(16.))
            .cursor_pointer()
            .hover(move |style| style.border_color(accent).bg(c(SURFACE_TERTIARY)))
            .child(
                div()
                    .size(px(88.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .map(|mut icon_box| {
                        icon_box.style().align_self = Some(AlignItems::Center);
                        icon_box
                    })
                    .rounded(px(4.))
                    .bg(c(SURFACE_ELEVATED))
                    .child(icon("hard-drive", 22., c(TEXT_SECONDARY))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm_tw()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(self.drive.display_name().to_string()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .rounded(px(4.))
                                    .bg(c(SURFACE_ELEVATED))
                                    .px(px(8.))
                                    .py(px(2.))
                                    .text_xs_tw()
                                    .font_weight(FontWeight::MEDIUM)
                                    .whitespace_nowrap()
                                    .text_color(c(TEXT_SECONDARY))
                                    .child(file_size(self.drive.total_bytes)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .text_xs_tw()
                            .text_color(c(TEXT_MUTED))
                            .child(format!(
                                "{} used · {} free",
                                file_size(self.drive.used_bytes),
                                file_size(self.drive.free_bytes)
                            ))
                            .when(self.drive.read_only, |line| {
                                line.child(
                                    div().ml(px(4.)).text_color(c(WARNING)).child("· Read-only"),
                                )
                            }),
                    )
                    .when_some(
                        self.drive.device.clone().filter(|device| !device.is_empty()),
                        |info, device| {
                            info.child(
                                div()
                                    .flex()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_11()
                                    .text_color(c(TEXT_MUTED))
                                    .child(div().font_family(self.mono.clone()).child(device))
                                    .when_some(
                                        self.drive.fs_type.clone().filter(|fs| !fs.is_empty()),
                                        |line, fs| {
                                            line.child(
                                                div()
                                                    .text_color(c(TEXT_SECONDARY))
                                                    .child(format!("\u{a0}· {fs}")),
                                            )
                                        },
                                    ),
                            )
                        },
                    )
                    .child(
                        div()
                            .mt(px(2.))
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(usage_bar(self.drive.used_pct))
                            .child(usage_text(self.drive.used_pct)),
                    ),
            )
            .when_some(self.on_click, |card, handler| card.on_click(handler))
    }
}
