//! The right-click menu of a place: a breadcrumb, a sidebar entry or Trash, and
//! the crumbs a long path hides behind "…".

use gpui::{Context, Pixels, Point};

use super::operations::PaneTarget;
use super::Workspace;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Place {
    /// A folder on a pane's provider.
    Folder { pane: PaneTarget, path: String },
    Trash,
    /// The leading crumbs a long path does not have room for: label and path.
    Hidden { pane: PaneTarget, crumbs: Vec<(String, String)> },
}

pub(crate) struct PlaceMenu {
    pub place: Place,
    pub position: Point<Pixels>,
}

impl Workspace {
    pub(crate) fn open_place_menu(&mut self, place: Place, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.place_menu = Some(PlaceMenu { place, position });
        cx.notify();
    }

    /// Opens a folder of `pane`'s provider in a new tab.
    pub(crate) fn open_in_new_tab(&mut self, pane: PaneTarget, path: String, cx: &mut Context<Self>) {
        if let Some(pane) = self.pane_by_id(pane.0) {
            let api = pane.api.clone();
            self.open_tab_with(api, path, cx);
        }
    }

    /// Properties of a folder known only by its path.
    pub(crate) fn folder_properties(&mut self, pane: PaneTarget, path: String, cx: &mut Context<Self>) {
        let Some(api) = self.pane_by_id(pane.0).map(|pane| pane.api.clone()) else { return };
        cx.spawn(async move |this, cx| {
            let item = cx.background_executor().spawn(async move { api.stat(&path) }).await;
            this.update(cx, |this, cx| match item {
                Ok(item) => this.show_properties(pane, vec![item], cx),
                Err(error) => this.report(Err(format!("{error:#}")), cx),
            })
            .ok();
        })
        .detach();
    }
}
