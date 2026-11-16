//! The toolbar's Refresh icon turns from the press until what it asked for
//! has arrived, and always finishes the turn it is on, so it never jumps back.

use std::cell::Cell;
use std::time::{Duration, Instant};

use super::operations::PaneTarget;
use super::Workspace;

/// One full turn of the icon.
const TURN: Duration = Duration::from_millis(700);

pub(crate) struct Refreshing {
    target: PaneTarget,
    started: Instant,
    /// When the work was first seen done, measured from `started`. Set while
    /// rendering, which only has `&self`.
    done_at: Cell<Option<Duration>>,
}

impl Refreshing {
    pub fn new(target: PaneTarget) -> Self {
        Self { target, started: Instant::now(), done_at: Cell::new(None) }
    }

    /// How far round the icon is, 0–1, or None once it has stopped.
    fn turn(&self, busy: bool) -> Option<f32> {
        let elapsed = self.started.elapsed();
        let end = if busy {
            self.done_at.set(None);
            None
        } else {
            let done = *self.done_at.get().get_or_insert(elapsed);
            self.done_at.set(Some(done));
            // At least one turn, then up to the end of the turn it is on.
            Some(TURN * (done.as_secs_f32() / TURN.as_secs_f32()).ceil().max(1.) as u32)
        };
        if end.is_some_and(|end| elapsed >= end) {
            return None;
        }
        Some((elapsed.as_secs_f32() / TURN.as_secs_f32()).fract())
    }
}

impl Workspace {
    /// The Refresh icon's position for this pane's toolbar, while it turns.
    pub(crate) fn refresh_turn(&self, target: PaneTarget) -> Option<f32> {
        let refreshing = self.refreshing.as_ref().filter(|refreshing| refreshing.target == target)?;
        let pane = self.pane_by_id(target.0)?;
        // The Overview waits on servers too; a folder on its own listing.
        let busy = if pane.is_at_root() { self.servers.busy() } else { pane.busy };
        refreshing.turn(busy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_at_least_once_and_stops_at_the_end_of_a_turn() {
        let refreshing = Refreshing::new(PaneTarget(0));
        assert!(refreshing.turn(false).is_some(), "a fast refresh still turns");
        let started = refreshing.started;
        // Done early in the first turn: it turns until one turn has passed.
        let shifted = Refreshing { target: PaneTarget(0), started: started - TURN / 2, done_at: Cell::new(Some(TURN / 4)) };
        assert!(shifted.turn(false).is_some());
        let shifted = Refreshing { target: PaneTarget(0), started: started - TURN, done_at: Cell::new(Some(TURN / 4)) };
        assert_eq!(shifted.turn(false), None);
        // Done in its third turn: it finishes that turn.
        let late = Refreshing { target: PaneTarget(0), started: started - TURN * 5 / 2, done_at: Cell::new(Some(TURN * 9 / 4)) };
        assert!(late.turn(false).is_some());
        let late = Refreshing { target: PaneTarget(0), started: started - TURN * 3, done_at: Cell::new(Some(TURN * 9 / 4)) };
        assert_eq!(late.turn(false), None);
        // Still busy: it keeps turning however long it takes.
        let slow = Refreshing { target: PaneTarget(0), started: started - TURN * 10, done_at: Cell::new(None) };
        assert!(slow.turn(true).is_some());
    }
}
