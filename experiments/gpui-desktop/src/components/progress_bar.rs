//! `lib/components/ui/ProgressBar.svelte` with a known value.

use gpui::{div, prelude::*, px, relative, App, Window};

use crate::theme::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProgressVariant {
    Default,
    Info,
    Success,
    Warning,
    Danger,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProgressSize {
    Sm,
    Md,
}

#[derive(IntoElement)]
pub struct ProgressBar {
    /// 0–100.
    percent: f32,
    variant: ProgressVariant,
    size: ProgressSize,
}

impl ProgressBar {
    pub fn new(percent: f32) -> Self {
        Self { percent: percent.clamp(0., 100.), variant: ProgressVariant::Default, size: ProgressSize::Md }
    }

    pub fn variant(mut self, variant: ProgressVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: ProgressSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for ProgressBar {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let fill = match self.variant {
            ProgressVariant::Default => ACCENT,
            ProgressVariant::Info => INFO,
            ProgressVariant::Success => SUCCESS,
            ProgressVariant::Warning => WARNING,
            ProgressVariant::Danger => DANGER,
        };
        // track: w-full rounded-full bg-surface-elevated h-1 (sm) / h-2 (md); fill: h-full rounded-full
        div()
            .w_full()
            .h(px(match self.size {
                ProgressSize::Sm => 4.,
                ProgressSize::Md => 8.,
            }))
            .overflow_hidden()
            .rounded_full()
            .bg(c(SURFACE_ELEVATED))
            .child(div().h_full().w(relative(self.percent / 100.)).rounded_full().bg(c(fill)))
    }
}
