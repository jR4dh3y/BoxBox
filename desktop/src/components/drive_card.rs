//! `lib/components/DriveCard.svelte`, and the usage bar and label that the
//! Overview table shares with it.

use gpui::{div, prelude::*, px, AlignItems, App, ClickEvent, ElementId, FontWeight, Rgba, SharedString, Window};

use super::skeleton::{self, Wait};
use super::{ProgressBar, ProgressSize, ProgressVariant, Type};
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

/// A mounted filesystem's size, in bytes.
#[derive(Clone, Copy)]
pub struct Usage {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub pct: f64,
}

/// What a drive shows under its name: how full it is, or why that is unknown,
/// with its size when the partition table gives it.
#[derive(Clone)]
pub enum DriveState {
    Usage(Usage),
    Status { status: SharedString, size: Option<u64> },
    /// A drive remembered from the last run whose usage is on its way, or will
    /// not come until the user acts; its usage shows as a placeholder.
    Pending { status: SharedString, wait: Wait },
}

impl DriveState {
    pub fn usage(&self) -> Option<Usage> {
        match self {
            DriveState::Usage(usage) => Some(*usage),
            DriveState::Status { .. } | DriveState::Pending { .. } => None,
        }
    }

    pub fn size(&self) -> Option<u64> {
        match self {
            DriveState::Usage(usage) => Some(usage.total),
            DriveState::Status { size, .. } => *size,
            DriveState::Pending { .. } => None,
        }
    }

    pub fn status(&self) -> Option<&SharedString> {
        match self {
            DriveState::Usage(_) => None,
            DriveState::Status { status, .. } | DriveState::Pending { status, .. } => Some(status),
        }
    }

    pub fn wait(&self) -> Option<Wait> {
        match self {
            DriveState::Pending { wait, .. } => Some(*wait),
            DriveState::Usage(_) | DriveState::Status { .. } => None,
        }
    }
}

/// What the Overview knows about one drive or device.
#[derive(Clone)]
pub struct DriveInfo {
    pub name: SharedString,
    pub glyph: &'static str,
    pub state: DriveState,
    pub read_only: bool,
    /// The server a remote drive is on.
    pub origin: Option<SharedString>,
    pub device: Option<String>,
    pub fs_type: Option<String>,
}

#[derive(IntoElement)]
pub struct DriveCard {
    id: ElementId,
    drive: DriveInfo,
    accent: Rgba,
    mono: SharedString,
    on_click: Option<ClickHandler>,
}

impl DriveCard {
    pub fn new(id: impl Into<ElementId>, drive: DriveInfo, accent: Rgba, mono: SharedString) -> Self {
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
        let drive = self.drive;
        let usage = drive.state.usage();
        let wait = drive.state.wait();
        let pulse_id = ElementId::Name(format!("{:?}-pulse", self.id).into());
        let device = drive.device.clone().filter(|device| !device.is_empty());
        let fs_type = drive.fs_type.clone().filter(|fs| !fs.is_empty());
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
            .when(self.on_click.is_some(), |card| card.cursor_pointer().hover(move |style| style.border_color(accent).bg(c(SURFACE_TERTIARY))).active(crate::components::pressed))
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
                    .child(icon(drive.glyph, 22., c(if usage.is_some() { TEXT_SECONDARY } else { TEXT_MUTED }))),
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
                                    .when(wait.is_some(), |name| name.text_color(c(TEXT_SECONDARY)))
                                    .child(drive.name.clone()),
                            )
                            .when_some(drive.state.size(), |row, size| {
                                row.child(
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
                                        .child(file_size(size)),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .text_xs_tw()
                            .text_color(c(TEXT_MUTED))
                            .child(match (&drive.state, drive.state.status()) {
                                (DriveState::Usage(usage), _) => format!("{} used · {} free", file_size(usage.used), file_size(usage.free)).into(),
                                (_, status) => status.cloned().unwrap_or_default(),
                            })
                            .when(drive.read_only, |line| line.child(div().ml(px(4.)).text_color(c(WARNING)).child("· Read-only"))),
                    )
                    .when(drive.origin.is_some() || device.is_some() || fs_type.is_some(), |info| {
                        info.child(
                            div()
                                .flex()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_11()
                                .text_color(c(TEXT_MUTED))
                                .when_some(drive.origin.clone(), |line, origin| {
                                    let origin = if device.is_some() { format!("{origin}\u{a0}·\u{a0}") } else { origin.to_string() };
                                    line.child(div().text_color(c(TEXT_SECONDARY)).child(origin))
                                })
                                .when_some(device.clone(), |line, device| line.child(div().font_family(self.mono.clone()).child(device)))
                                .when_some(fs_type, |line, fs| {
                                    let fs = if device.is_some() { format!("\u{a0}· {fs}") } else { fs };
                                    line.child(div().text_color(c(TEXT_SECONDARY)).child(fs))
                                }),
                        )
                    })
                    // An empty track when the usage is unknown, so every card has the same rows.
                    // The row is as tall as the percentage's line either way, so the bars line up.
                    .child({
                        let row = div().mt(px(2.)).h(px(16.5)).flex().items_center().gap(px(12.));
                        match (usage, wait) {
                            (Some(usage), _) => row.child(usage_bar(usage.pct)).child(usage_text(usage.pct)),
                            (None, Some(wait)) => row.child(skeleton::bar(pulse_id, 4., wait)),
                            (None, None) => row.child(usage_bar(0.)),
                        }
                    }),
            )
            .when_some(self.on_click, |card, handler| card.on_click(handler))
    }
}
