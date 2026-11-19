//! `lib/components/ui/ProgressBar.svelte` with a known value.

use gpui::{div, prelude::*, px, relative, AnimationExt as _, App, Window};

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
    /// The empty part's colour; the default suits a bar on the page, not on a raised card.
    track: u32,
    /// Grows from empty to its value when it appears, keyed by this id.
    grow: Option<gpui::ElementId>,
}

impl ProgressBar {
    pub fn new(percent: f32) -> Self {
        Self { percent: percent.clamp(0., 100.), variant: ProgressVariant::Default, size: ProgressSize::Md, track: SURFACE_ELEVATED, grow: None }
    }

    pub fn variant(mut self, variant: ProgressVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: ProgressSize) -> Self {
        self.size = size;
        self
    }

    pub fn track(mut self, track: u32) -> Self {
        self.track = track;
        self
    }

    /// Grows from empty to its value each time it appears, as a gauge does.
    pub fn grow(mut self, id: impl Into<gpui::ElementId>) -> Self {
        self.grow = Some(id.into());
        self
    }
}

impl RenderOnce for ProgressBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
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
            .bg(c(self.track))
            .child({
                let share = self.percent / 100.;
                let bar = div().h_full().w(relative(share)).rounded_full().bg(c(fill));
                match self.grow.filter(|_| super::motion::enabled(cx)) {
                    Some(id) => bar
                        .with_animation(id, gpui::Animation::new(super::motion::GROW).with_easing(super::motion::ease_out_quint), move |bar, t| bar.w(relative(share * t)))
                        .into_any_element(),
                    None => bar.into_any_element(),
                }
            })
    }
}
