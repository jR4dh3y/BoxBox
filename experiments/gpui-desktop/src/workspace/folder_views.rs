//! Each folder's view, list or grid, and its sort, remembered when they are
//! picked there, as Nautilus and Dolphin do. A folder never set opens with the
//! defaults from Settings. Kept in the state dir, by server and path.

use std::collections::HashMap;

use gpui::Context;

use super::Workspace;
use crate::api::Api;
use crate::pane::ViewMode;
use crate::sort::Sort;
use crate::workspace::operations::PaneTarget;

/// What a folder changed from the defaults; `None` follows Settings.
#[derive(Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct FolderView {
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<ViewMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sort: Option<Sort>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub(crate) struct FolderViews(HashMap<String, FolderView>);

impl FolderViews {
    /// A small file, read before the first frame so a folder opens in its view.
    pub fn load() -> Self {
        std::fs::read(crate::persist::FOLDER_VIEWS.path()).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
    }

    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// The view `path` opens in.
    pub fn mode(&self, api: &Api, path: &str, default: ViewMode) -> ViewMode {
        self.0.get(&key(api, path)).and_then(|view| view.mode).unwrap_or(default)
    }

    /// The sort `path` opens with.
    pub fn sort(&self, api: &Api, path: &str, default: Sort) -> Sort {
        self.0.get(&key(api, path)).and_then(|view| view.sort).unwrap_or(default)
    }

    /// Remembers the view picked in `path`. A default is stored as nothing, so
    /// the file holds only what differs.
    pub fn set_mode(&mut self, api: &Api, path: &str, mode: ViewMode, default: ViewMode) {
        self.update(api, path, |view| view.mode = (mode != default).then_some(mode));
    }

    pub fn set_sort(&mut self, api: &Api, path: &str, sort: Sort, default: Sort) {
        self.update(api, path, |view| view.sort = (sort != default).then_some(sort));
    }

    fn update(&mut self, api: &Api, path: &str, change: impl FnOnce(&mut FolderView)) {
        let key = key(api, path);
        let mut view = self.0.remove(&key).unwrap_or_default();
        change(&mut view);
        if view != FolderView::default() {
            self.0.insert(key, view);
        }
    }
}

impl Workspace {
    /// The status bar's List or Grid: in a folder, that folder keeps it.
    pub(crate) fn choose_view_mode(&mut self, target: PaneTarget, mode: ViewMode, cx: &mut Context<Self>) {
        let default = self.preferences.default_view;
        let Some(pane) = self.pane_mut_by_id(target.0) else { return };
        pane.set_view_mode(mode);
        if pane.is_at_root() {
            return;
        }
        let (api, path) = (pane.api.clone(), pane.path.clone());
        self.folder_views.set_mode(&api, &path, mode, default);
        self.save_folder_views(cx);
        cx.notify();
    }

    /// A column header's sort: the folder keeps it, as it keeps its view.
    /// Search results are not a folder, so their sort is not kept.
    pub(crate) fn choose_sort(&mut self, target: PaneTarget, sort: Sort, cx: &mut Context<Self>) {
        let default = super::preference_sort(&self.preferences);
        let Some(pane) = self.pane_mut_by_id(target.0) else { return };
        pane.set_sort(sort);
        if !pane.is_at_root() && !pane.is_searching() {
            let (api, path) = (pane.api.clone(), pane.path.clone());
            self.folder_views.set_sort(&api, &path, sort, default);
            self.save_folder_views(cx);
        }
        self.resort(target, cx);
        cx.notify();
    }

    fn save_folder_views(&mut self, cx: &mut Context<Self>) {
        self.persist(&crate::persist::FOLDER_VIEWS, self.folder_views.encode(), cx, |_, result, _| {
            if let Err(error) = result {
                log::debug!("{error}");
            }
        });
    }

    /// Puts the pane in its folder's view and sort, on entering the folder.
    /// Returns whether the sort changed, so shown entries need sorting again.
    pub(crate) fn apply_folder_view(&mut self, target: PaneTarget) -> bool {
        let default_mode = self.preferences.default_view;
        let default_sort = super::preference_sort(&self.preferences);
        let Some(pane) = self.pane_by_id(target.0).filter(|pane| !pane.is_at_root()) else { return false };
        let mode = self.folder_views.mode(&pane.api, &pane.path, default_mode);
        let sort = self.folder_views.sort(&pane.api, &pane.path, default_sort);
        let Some(pane) = self.pane_mut_by_id(target.0) else { return false };
        pane.view_mode = mode;
        // Setting an equal sort would take search results out of their ranking.
        let changed = pane.sort != sort;
        if changed {
            pane.set_sort(sort);
        }
        changed
    }
}

/// A path is only unique together with the server it is on.
fn key(api: &Api, path: &str) -> String {
    match api.server_url() {
        Some(url) => format!("{url} {path}"),
        None => path.to_string(),
    }
}
