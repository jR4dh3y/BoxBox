//! Zoom, as in Nautilus and Dolphin: Ctrl+plus, Ctrl+minus and Ctrl+wheel step
//! the size of the view the pane shows, Ctrl+0 resets it. The list and the grid
//! keep their own step, for every folder, in the preferences.

use gpui::{point, px, Context, ScrollDelta, Window};

use super::{Workspace, ZoomIn, ZoomOut, ZoomReset};
use crate::components::{GridMetrics, ListMetrics};
use crate::pane::ViewMode;
use crate::workspace::operations::PaneTarget;

/// The step Ctrl+0 returns to: the web app's default size.
const DEFAULT: u8 = 1;
/// Touchpad pixels per zoom step.
const PIXELS_PER_STEP: f32 = 40.;

#[derive(Clone, Copy)]
pub(crate) enum Zoom {
    In,
    Out,
    Reset,
}

impl Workspace {
    pub(crate) fn list_metrics(&self) -> ListMetrics {
        ListMetrics::new(self.preferences.list_zoom)
    }

    pub(crate) fn grid_metrics(&self) -> GridMetrics {
        GridMetrics::new(self.preferences.grid_zoom)
    }

    pub(crate) fn zoom_in(&mut self, _: &ZoomIn, window: &mut Window, cx: &mut Context<Self>) {
        self.zoom(self.active_pane, Zoom::In, window, cx);
    }

    pub(crate) fn zoom_out(&mut self, _: &ZoomOut, window: &mut Window, cx: &mut Context<Self>) {
        self.zoom(self.active_pane, Zoom::Out, window, cx);
    }

    pub(crate) fn zoom_reset(&mut self, _: &ZoomReset, window: &mut Window, cx: &mut Context<Self>) {
        self.zoom(self.active_pane, Zoom::Reset, window, cx);
    }

    /// Ctrl+wheel over a pane: a notch is a step; a touchpad steps every few pixels.
    pub(crate) fn zoom_wheel(&mut self, target: PaneTarget, delta: ScrollDelta, window: &mut Window, cx: &mut Context<Self>) {
        let steps = match delta {
            ScrollDelta::Lines(lines) => {
                self.zoom_scroll = 0.;
                lines.y.signum() as i32
            }
            ScrollDelta::Pixels(pixels) => {
                self.zoom_scroll += f32::from(pixels.y);
                let steps = (self.zoom_scroll / PIXELS_PER_STEP).trunc();
                self.zoom_scroll -= steps * PIXELS_PER_STEP;
                steps as i32
            }
        };
        for _ in 0..steps.unsigned_abs() {
            self.zoom(target, if steps > 0 { Zoom::In } else { Zoom::Out }, window, cx);
        }
    }

    /// Steps the view `target` shows, keeping the item at its top in place.
    pub(crate) fn zoom(&mut self, target: PaneTarget, zoom: Zoom, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_by_id(target.0) else { return };
        if pane.is_at_root() {
            return;
        }
        let mode = pane.current_view_mode();
        let (current, steps) = match mode {
            ViewMode::List => (self.preferences.list_zoom, ListMetrics::STEPS.len() as u8),
            ViewMode::Grid => (self.preferences.grid_zoom, GridMetrics::STEPS.len() as u8),
        };
        let next = match zoom {
            Zoom::In => (current + 1).min(steps - 1),
            Zoom::Out => current.saturating_sub(1),
            Zoom::Reset => DEFAULT,
        };
        if next == current {
            return;
        }
        let first = self.first_visible(target, window);
        match mode {
            ViewMode::List => self.preferences.list_zoom = next,
            ViewMode::Grid => self.preferences.grid_zoom = next,
        }
        self.scroll_to_top(target, first, window);
        self.save_preferences(cx);
        cx.notify();
    }

    /// The item drawn first in the pane's view.
    fn first_visible(&self, target: PaneTarget, window: &Window) -> usize {
        let Some(pane) = self.pane_by_id(target.0) else { return 0 };
        match pane.current_view_mode() {
            ViewMode::List => {
                let offset = pane.list_scroll.0.borrow().base_handle.offset();
                (-f32::from(offset.y) / self.list_metrics().row_height).floor().max(0.) as usize
            }
            ViewMode::Grid => {
                let metrics = self.grid_metrics();
                let offset = pane.grid_scroll.0.borrow().base_handle.offset();
                let row = (-f32::from(offset.y) / metrics.row_height).floor().max(0.) as usize;
                row * metrics.columns(self.browser_width(window))
            }
        }
    }

    /// Scrolls so the row holding `index` is at the top, at the current size.
    fn scroll_to_top(&self, target: PaneTarget, index: usize, window: &Window) {
        let Some(pane) = self.pane_by_id(target.0) else { return };
        let (handle, y) = match pane.current_view_mode() {
            ViewMode::List => (pane.list_scroll.0.borrow().base_handle.clone(), index as f32 * self.list_metrics().row_height),
            ViewMode::Grid => {
                let metrics = self.grid_metrics();
                let row = index / metrics.columns(self.browser_width(window));
                (pane.grid_scroll.0.borrow().base_handle.clone(), row as f32 * metrics.row_height)
            }
        };
        handle.set_offset(point(handle.offset().x, px(-y)));
    }
}
