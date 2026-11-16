//! Placeholders for what is on its way, drawn at the size of what replaces
//! them, so nothing moves when it arrives.

use std::sync::LazyLock;
use std::time::{Duration, Instant};

use gpui::{div, prelude::*, px, Animation, AnimationExt, AnyElement, Div, ElementId};

use crate::theme::*;

const PERIOD: Duration = Duration::from_millis(1400);
/// A wait this long has stopped being news: the pulse settles, so a server
/// that never answers does not keep the window drawing.
const PULSE_FOR: Duration = Duration::from_secs(10);

/// Every pulse keeps one phase, so the sidebar and the Overview breathe
/// together whenever each was drawn first.
static PHASE: LazyLock<Instant> = LazyLock::new(Instant::now);

const DIM: f32 = 0.45;

fn opacity() -> f32 {
    let turn = PHASE.elapsed().as_secs_f32() / PERIOD.as_secs_f32();
    DIM + 0.35 * (0.5 - 0.5 * (turn * std::f32::consts::TAU).cos())
}

/// How long something has been awaited; its placeholders pulse at first.
#[derive(Clone, Copy, Debug)]
pub enum Wait {
    Since(Instant),
    /// It will not arrive without the user, as for a server that needs pairing.
    Stopped,
}

impl Wait {
    fn pulsing(self) -> bool {
        matches!(self, Wait::Since(since) if since.elapsed() < PULSE_FOR)
    }
}

/// `element`, pulsing while the wait is young, then held dim.
pub fn pulse(id: impl Into<ElementId>, element: Div, wait: Wait) -> AnyElement {
    if !wait.pulsing() {
        return element.opacity(DIM).into_any_element();
    }
    // The phase comes from `PHASE`, not the animation's own clock, which starts
    // whenever an element is first drawn.
    element.with_animation(id, Animation::new(PERIOD).repeat(), |element, _| element.opacity(opacity())).into_any_element()
}

/// A rounded bar standing in for a value, such as a usage track.
pub fn bar(id: impl Into<ElementId>, height: f32, wait: Wait) -> AnyElement {
    pulse(id, div().flex_1().h(px(height)).rounded_full().bg(alpha(TEXT_MUTED, 0.5)), wait)
}
