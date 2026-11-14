//! Back and Forward, and their menus. A place that no longer exists is left out
//! of the menus and skipped, as Thunar does; existence is checked off the UI thread.

use std::collections::HashSet;
use std::sync::Arc;

use gpui::{Context, Pixels, Point};

use super::operations::PaneTarget;
use super::Workspace;
use crate::api::Api;

/// A Back or Forward menu: the places it lists, nearest first.
pub(crate) struct HistoryMenu {
    pub target: PaneTarget,
    pub back: bool,
    pub places: Vec<String>,
    pub position: Point<Pixels>,
}

/// Whether a folder in history can still be opened. This PC always can.
fn exists(api: &Api, path: &str) -> bool {
    if path.is_empty() {
        return true;
    }
    match api.local_path(path) {
        Ok(Some(real)) => real.is_dir(),
        Ok(None) => api.list(path).is_ok(),
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
        let (api, from) = (pane.api.clone(), pane.path.clone());
        cx.spawn(async move |this, cx| {
            let missing = cx
                .background_executor()
                .spawn(async move {
                    let mut missing = HashSet::new();
                    let mut found = 0;
                    for place in places {
                        if found == steps {
                            break;
                        }
                        if exists(&api, &place) {
                            found += 1;
                        } else {
                            missing.insert(place);
                        }
                    }
                    missing
                })
                .await;
            this.update(cx, |this, cx| {
                let Some(pane) = this.pane_mut_by_id(target.0).filter(|pane| pane.path == from) else { return };
                if pane.travel(back, steps, &missing) {
                    this.load_pane(target, cx);
                }
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
        let api: Arc<Api> = pane.api.clone();
        cx.spawn(async move |this, cx| {
            let places = cx.background_executor().spawn(async move { places.into_iter().filter(|place| exists(&api, place)).collect::<Vec<_>>() }).await;
            this.update(cx, |this, cx| {
                this.history_menu = (!places.is_empty()).then_some(HistoryMenu { target, back, places, position });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// A place's name in the history menu: its folder name, or the drive's.
    pub(crate) fn place_label(&self, target: PaneTarget, path: &str) -> String {
        if path.is_empty() {
            return "This PC".into();
        }
        if !path.contains('/') {
            if let Some(crate::pane::Load::Loaded(roots)) = self.pane_by_id(target.0).map(|pane| &pane.roots) {
                if let Some(root) = roots.iter().find(|root| root.name == path) {
                    return root.display_name().to_string();
                }
            }
        }
        path.rsplit('/').next().unwrap_or(path).to_string()
    }
}
