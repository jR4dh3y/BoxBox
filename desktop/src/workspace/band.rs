//! Rubber-band selection: a drag that starts on empty space in the list or grid
//! selects every item the rectangle touches, with Ctrl adding to the selection,
//! as in Nautilus and Thunar. A click on empty space clears the selection.
//! Rows and tiles are uniform, so the items under the rectangle come from
//! arithmetic, not from element bounds.

use std::collections::HashSet;
use std::time::Duration;

use gpui::{point, px, Bounds, Context, Modifiers, Pixels, Point, Window};

use super::Workspace;
use crate::components::GridMetrics;
use crate::pane::ViewMode;
use crate::workspace::operations::PaneTarget;

/// How far the pointer moves before a press becomes a drag.
const THRESHOLD: f32 = 4.;
/// The most a held drag scrolls per frame past the pane's edge.
const MAX_SCROLL_STEP: f32 = 24.;

pub(crate) struct Band {
    pub target: PaneTarget,
    /// Where the press was, in content coordinates (the scrolled-away part included).
    origin: Point<f32>,
    /// The pointer in window coordinates.
    pointer: Point<Pixels>,
    /// The selection before the drag, kept when Ctrl adds to it.
    base: HashSet<String>,
    additive: bool,
    /// Past the threshold: the rectangle is drawn and selects.
    pub dragging: bool,
}

/// Where items are, in content coordinates.
#[derive(Clone, Copy)]
pub(crate) enum Layout {
    List { row: f32 },
    Grid { metrics: GridMetrics, columns: usize },
}

impl Layout {
    /// The item under a point, if any.
    pub fn item_at(&self, at: Point<f32>, count: usize) -> Option<usize> {
        self.items_in(at, at, count).into_iter().next()
    }

    /// Items a rectangle between two corners touches, in order.
    pub fn items_in(&self, a: Point<f32>, b: Point<f32>, count: usize) -> Vec<usize> {
        let (left, right, top, bottom) = (a.x.min(b.x), a.x.max(b.x), a.y.min(b.y), a.y.max(b.y));
        if bottom < 0. || count == 0 {
            return Vec::new();
        }
        let rows = |height: f32, total: usize| {
            let first = (top.max(0.) / height).floor() as usize;
            let last = ((bottom / height).floor() as usize).min(total.saturating_sub(1));
            first..=last
        };
        match *self {
            Layout::List { row } => rows(row, count).filter(|&index| index < count).collect(),
            Layout::Grid { metrics, columns } => {
                let mut found = Vec::new();
                for row in rows(metrics.row_height, count.div_ceil(columns)) {
                    // The row's top padding is folded into every row.
                    let tile_top = row as f32 * metrics.row_height + metrics.padding;
                    if bottom < tile_top || top > tile_top + metrics.tile_height() {
                        continue;
                    }
                    for column in 0..columns {
                        let tile_left = metrics.padding + column as f32 * (metrics.tile_width + metrics.gap);
                        let index = row * columns + column;
                        if index < count && right >= tile_left && left <= tile_left + metrics.tile_width {
                            found.push(index);
                        }
                    }
                }
                found
            }
        }
    }
}

impl Workspace {
    /// The pane's item layout, its viewport and its scroll offset, while it lists a folder.
    pub(crate) fn pane_layout(&self, target: PaneTarget, window: &Window) -> Option<(Layout, Bounds<Pixels>, Point<Pixels>)> {
        let pane = self.pane_by_id(target.0)?;
        if pane.is_at_root() || pane.placeholder().is_some() {
            return None;
        }
        let (layout, handle) = match pane.current_view_mode() {
            ViewMode::List => (Layout::List { row: self.list_metrics().row_height }, pane.list_scroll.0.borrow().base_handle.clone()),
            ViewMode::Grid => {
                let metrics = self.grid_metrics();
                (Layout::Grid { metrics, columns: metrics.columns(self.pane_width(target, window)) }, pane.grid_scroll.0.borrow().base_handle.clone())
            }
        };
        Some((layout, handle.bounds(), handle.offset()))
    }

    /// A left press in the pane: starts a band when it is on empty space. Returns
    /// whether it did.
    pub(crate) fn band_press(&mut self, target: PaneTarget, position: Point<Pixels>, modifiers: Modifiers, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some((layout, viewport, offset)) = self.pane_layout(target, window) else { return false };
        if !viewport.contains(&position) {
            return false;
        }
        let origin = content(position, viewport, offset);
        let Some(pane) = self.pane_by_id(target.0) else { return false };
        if layout.item_at(origin, pane.items.len()).is_some() {
            return false;
        }
        let additive = modifiers.control || modifiers.shift;
        self.band = Some(Band { target, origin, pointer: position, base: pane.selected.clone(), additive, dragging: false });
        self.active_pane = target;
        self.scroll_band_edges(window, cx);
        true
    }

    pub(crate) fn band_move(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(band) = self.band.as_mut() else { return };
        band.pointer = position;
        let (target, dragging) = (band.target, band.dragging);
        if !dragging {
            let Some((_, viewport, offset)) = self.pane_layout(target, window) else { return };
            let Some(band) = self.band.as_mut() else { return };
            let moved = content(position, viewport, offset) - band.origin;
            if moved.x.abs().max(moved.y.abs()) < THRESHOLD {
                return;
            }
            band.dragging = true;
        }
        self.band_select(window);
        cx.notify();
    }

    /// The press ended: a click on empty space clears the selection.
    pub(crate) fn band_release(&mut self, cx: &mut Context<Self>) {
        let Some(band) = self.band.take() else { return };
        if !band.dragging && !band.additive {
            if let Some(pane) = self.pane_mut_by_id(band.target.0) {
                pane.clear_selection();
            }
        }
        cx.notify();
    }

    /// The band's corners in window coordinates, while it is dragged in `target`.
    pub(crate) fn band_rect(&self, target: PaneTarget, window: &Window) -> Option<Bounds<Pixels>> {
        let band = self.band.as_ref().filter(|band| band.target == target && band.dragging)?;
        let (_, viewport, offset) = self.pane_layout(target, window)?;
        let origin = viewport.origin + offset + point(px(band.origin.x), px(band.origin.y));
        Some(Bounds::from_corners(
            point(origin.x.min(band.pointer.x), origin.y.min(band.pointer.y)),
            point(origin.x.max(band.pointer.x), origin.y.max(band.pointer.y)),
        ))
    }

    fn band_select(&mut self, window: &Window) {
        let Some(band) = self.band.as_ref() else { return };
        let Some((layout, viewport, offset)) = self.pane_layout(band.target, window) else { return };
        let corner = content(band.pointer, viewport, offset);
        let (origin, target, additive) = (band.origin, band.target, band.additive);
        let mut selected = if additive { band.base.clone() } else { HashSet::new() };
        let Some(pane) = self.pane_mut_by_id(target.0) else { return };
        let touched = layout.items_in(origin, corner, pane.items.len());
        selected.extend(touched.iter().filter_map(|&index| pane.items.get(index)).map(|item| item.path.clone()));
        pane.selected = selected;
    }

    /// While the band is held past the top or bottom edge, scrolls toward the
    /// pointer each frame and grows the band with it.
    fn scroll_band_edges(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(16)).await;
            let alive = this
                .update_in(cx, |this, window, cx| {
                    let Some(band) = this.band.as_ref() else { return false };
                    if !band.dragging {
                        return true;
                    }
                    let (target, pointer) = (band.target, band.pointer);
                    let Some(pane) = this.pane_by_id(target.0) else { return false };
                    let handle = match pane.current_view_mode() {
                        ViewMode::List => pane.list_scroll.0.borrow().base_handle.clone(),
                        ViewMode::Grid => pane.grid_scroll.0.borrow().base_handle.clone(),
                    };
                    let viewport = handle.bounds();
                    let past = if pointer.y < viewport.top() {
                        f32::from(pointer.y - viewport.top())
                    } else if pointer.y > viewport.bottom() {
                        f32::from(pointer.y - viewport.bottom())
                    } else {
                        return true;
                    };
                    let step = past.clamp(-MAX_SCROLL_STEP, MAX_SCROLL_STEP);
                    let offset = handle.offset();
                    let lowest = -f32::from(handle.max_offset().height);
                    let y = (f32::from(offset.y) - step).clamp(lowest, 0.);
                    if y != f32::from(offset.y) {
                        handle.set_offset(point(offset.x, px(y)));
                        this.band_select(window);
                        cx.notify();
                    }
                    true
                })
                .unwrap_or(false);
            if !alive {
                break;
            }
        })
        .detach();
    }
}

/// A window point in the content of a scrolled viewport.
fn content(position: Point<Pixels>, viewport: Bounds<Pixels>, offset: Point<Pixels>) -> Point<f32> {
    let at = position - viewport.origin - offset;
    point(f32::from(at.x), f32::from(at.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, y: f32) -> Point<f32> {
        point(x, y)
    }

    #[test]
    fn a_list_band_takes_every_row_it_crosses() {
        let list = Layout::List { row: 32. };
        assert_eq!(list.items_in(at(10., 40.), at(300., 100.), 10), vec![1, 2, 3]);
        // Below the last row there is nothing to take.
        assert_eq!(list.items_in(at(10., 300.), at(10., 400.), 5), Vec::<usize>::new());
        assert_eq!(list.item_at(at(5., 33.), 10), Some(1));
        assert_eq!(list.item_at(at(5., 33.), 1), None);
    }

    #[test]
    fn a_grid_band_skips_the_gaps_between_tiles() {
        let metrics = GridMetrics::new(1);
        let grid = Layout::Grid { metrics, columns: 3 };
        // The gap after the first tile is empty space; the tile itself is not.
        let gap_x = metrics.padding + metrics.tile_width + metrics.gap / 2.;
        assert_eq!(grid.item_at(at(gap_x, 50.), 9), None);
        assert_eq!(grid.item_at(at(metrics.padding + 5., metrics.padding + 5.), 9), Some(0));
        assert_eq!(grid.item_at(at(metrics.padding + 5., 4.), 9), None);
        // From the first tile's gap across into the second row's middle column.
        let second_row = metrics.row_height + metrics.padding + 10.;
        assert_eq!(grid.items_in(at(gap_x, 20.), at(gap_x + metrics.tile_width, second_row), 9), vec![1, 4]);
        // Past the last item there is nothing to take.
        assert_eq!(grid.items_in(at(0., 0.), at(1000., 1000.), 4), vec![0, 1, 2, 3]);
    }
}
