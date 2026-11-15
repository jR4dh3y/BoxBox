//! Back and Forward, and their menus. A place that no longer exists is left out
//! of the menus and skipped, as Thunar does; existence is checked off the UI thread.

use gpui::{Context, Pixels, Point};

use super::operations::PaneTarget;
use super::Workspace;
use crate::pane::{Moved, Place};

/// A Back or Forward menu: the places it lists, nearest first.
pub(crate) struct HistoryMenu {
    pub target: PaneTarget,
    pub back: bool,
    pub places: Vec<Place>,
    pub position: Point<Pixels>,
}

/// Whether a folder in history can still be opened. The Overview always can.
fn exists(place: &Place) -> bool {
    if place.path.is_empty() {
        return true;
    }
    match place.api.local_path(&place.path) {
        Ok(Some(real)) => real.is_dir(),
        Ok(None) => place.api.list(&place.path).is_ok(),
        Err(_) => false,
    }
}

impl Workspace {
    /// Moves `steps` existing places back or forward in a pane's history, dropping
    /// the places on the way that no longer exist.
    pub(crate) fn travel(&mut self, target: PaneTarget, back: bool, steps: usize, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_by_id(target.0) else { return };
        let (backward, forward) = pane.history();
        let places = if back { backward } else { forward };
        let from = pane.place();
        cx.spawn(async move |this, cx| {
            let missing = cx
                .background_executor()
                .spawn(async move {
                    let mut missing = Vec::new();
                    let mut found = 0;
                    for place in places {
                        if found == steps {
                            break;
                        }
                        if exists(&place) {
                            found += 1;
                        } else {
                            missing.push(place);
                        }
                    }
                    missing
                })
                .await;
            this.update(cx, |this, cx| {
                let Some(pane) = this.pane_mut_by_id(target.0).filter(|pane| pane.place() == from) else { return };
                let moved = pane.travel(back, steps, &missing);
                this.after_move(target, moved, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Opens the Back or Forward menu with the places that still exist.
    pub(crate) fn open_history_menu(&mut self, target: PaneTarget, back: bool, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_by_id(target.0) else { return };
        let (backward, forward) = pane.history();
        let places = if back { backward } else { forward };
        if places.is_empty() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let places = cx.background_executor().spawn(async move { places.into_iter().filter(exists).collect::<Vec<_>>() }).await;
            this.update(cx, |this, cx| {
                this.history_menu = (!places.is_empty()).then_some(HistoryMenu { target, back, places, position });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// A place's name in the history menu: its folder name, or the drive's.
    pub(crate) fn place_label(&self, target: PaneTarget, place: &Place) -> String {
        let path = place.path.as_str();
        if path.is_empty() {
            return "Overview".into();
        }
        // Drive labels are known for the pane's own provider.
        let same_provider = self.pane_by_id(target.0).is_some_and(|pane| std::sync::Arc::ptr_eq(&pane.api, &place.api));
        if !path.contains('/') && same_provider {
            if let Some(crate::pane::Load::Loaded(roots)) = self.pane_by_id(target.0).map(|pane| &pane.roots) {
                if let Some(root) = roots.iter().find(|root| root.name == path) {
                    return root.display_name().to_string();
                }
            }
        }
        path.rsplit('/').next().unwrap_or(path).to_string()
    }

    /// Loads what a pane needs after it moved: the folder, and the drives of a
    /// provider it moved to.
    pub(crate) fn after_move(&mut self, target: PaneTarget, moved: Moved, cx: &mut Context<Self>) {
        if moved == Moved::Across {
            self.load_drives(target, cx);
        }
        if moved != Moved::No {
            self.load_pane(target, cx);
        }
    }
}
