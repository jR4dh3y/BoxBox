//! Each folder's view, list or grid, remembered when it is picked there, as
//! Nautilus and Dolphin do. A folder never set opens in the default view from
//! Settings. Kept in the state dir, by server and path.

use std::collections::HashMap;

use gpui::Context;

use super::Workspace;
use crate::api::Api;
use crate::pane::ViewMode;
use crate::workspace::operations::PaneTarget;

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub(crate) struct FolderViews(HashMap<String, ViewMode>);

impl FolderViews {
    /// A small file, read before the first frame so a folder opens in its view.
    pub fn load() -> Self {
        std::fs::read(crate::persist::FOLDER_VIEWS.path()).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
    }

    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// The view `path` opens in.
    pub fn get(&self, api: &Api, path: &str, default: ViewMode) -> ViewMode {
        self.0.get(&key(api, path)).copied().unwrap_or(default)
    }

    /// Remembers the view picked in `path`. The default is stored as nothing, so
    /// the file holds only the folders that differ.
    pub fn set(&mut self, api: &Api, path: &str, mode: ViewMode, default: ViewMode) {
        if mode == default {
            self.0.remove(&key(api, path));
        } else {
            self.0.insert(key(api, path), mode);
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
        self.folder_views.set(&api, &path, mode, default);
        self.persist(&crate::persist::FOLDER_VIEWS, self.folder_views.encode(), cx, |_, result, _| {
            if let Err(error) = result {
                log::debug!("{error}");
            }
        });
        cx.notify();
    }

    /// Puts the pane in its folder's view, on entering the folder.
    pub(crate) fn apply_folder_view(&mut self, target: PaneTarget) {
        let default = self.preferences.default_view;
        let Some(pane) = self.pane_by_id(target.0).filter(|pane| !pane.is_at_root()) else { return };
        let mode = self.folder_views.get(&pane.api, &pane.path, default);
        if let Some(pane) = self.pane_mut_by_id(target.0) {
            pane.view_mode = mode;
        }
    }
}

/// A path is only unique together with the server it is on.
fn key(api: &Api, path: &str) -> String {
    match api.server_url() {
        Some(url) => format!("{url} {path}"),
        None => path.to_string(),
    }
}
