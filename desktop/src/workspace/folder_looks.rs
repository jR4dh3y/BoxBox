//! How the user customised a folder (its menu > Customize Folder): a colour for
//! the folder, an emblem from every Lucide icon (`emblems`) and the emblem's own
//! colour. Drawn on BoxBox's folder; the desktop's icon theme keeps its own
//! folders, so with it selected these are not shown. Kept in the config dir by
//! the folder's real path on this computer, so every path that reaches it shows
//! the same look, and by server and path on a server. The real path follows
//! links, so it is resolved off the UI thread when a folder is drawn; until
//! then the path as written stands in, which is the same one without a link.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui::{Context, Task, Window};

use super::{IconStyle, Workspace};
use crate::api::{Api, FileInfo};
use crate::components::Emblem;

/// The colours offered for a folder and its emblem.
pub(crate) const COLORS: [(u32, &str); 12] = [
    (0xef4444, "Red"),
    (0xf97316, "Orange"),
    (0xeab308, "Yellow"),
    (0x22c55e, "Green"),
    (0x14b8a6, "Teal"),
    (0x06b6d4, "Cyan"),
    (0x3b82f6, "Blue"),
    (0x6366f1, "Indigo"),
    (0xa855f7, "Purple"),
    (0xec4899, "Pink"),
    (0x9ca3af, "Grey"),
    (0xf5f5f4, "White"),
];

/// Which emblem a folder carries.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EmblemChoice {
    /// A special folder's own, such as Downloads'; none for any other.
    #[default]
    Own,
    /// None, even on a special folder.
    Without,
    Icon(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct FolderLook {
    pub emblem: EmblemChoice,
    /// `None` keeps the icon style's folder colour.
    pub color: Option<u32>,
    /// `None` draws the emblem in the folder's colour.
    pub emblem_color: Option<u32>,
}

/// Past this many resolved folders the record starts again.
const RESOLVED: usize = 4096;

#[derive(Default)]
pub(crate) struct FolderLooks {
    looks: HashMap<String, FolderLook>,
    /// Each local folder's path as written, and its real path once resolved.
    real: HashMap<String, String>,
    resolving: HashSet<String>,
    resolution_generation: u64,
}

impl FolderLooks {
    pub(super) fn invalidate_paths(&mut self) {
        self.real.clear();
        self.resolving.clear();
        self.resolution_generation += 1;
    }

    pub fn load() -> Self {
        let looks = std::fs::read(crate::persist::FOLDER_LOOKS.path()).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
        Self { looks, ..Self::default() }
    }

    fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(&self.looks).unwrap_or_default()
    }

    fn get(&self, api: &Api, path: &str) -> Option<&FolderLook> {
        if self.looks.is_empty() {
            return None;
        }
        self.looks.get(&self.key(api, path)?)
    }

    /// A default look is stored as nothing, so the file holds only what differs.
    fn set(&mut self, key: String, look: FolderLook) {
        if look == FolderLook::default() {
            self.looks.remove(&key);
        } else {
            self.looks.insert(key, look);
        }
    }

    /// The folder's real path when it is known, else its path as written.
    fn key(&self, api: &Api, path: &str) -> Option<String> {
        match Written::of(api, path)? {
            Written::Server(key) => Some(key),
            Written::Local(written) => Some(self.real.get(&written).cloned().unwrap_or(written)),
        }
    }
}

/// A folder as its path names it.
enum Written {
    /// On a server, which has no links to follow.
    Server(String),
    Local(String),
}

impl Written {
    fn of(api: &Api, path: &str) -> Option<Self> {
        match api.server_url() {
            Some(url) => Some(Self::Server(format!("{url} {path}"))),
            None => api.lexical_path(path).map(|written| Self::Local(written.display().to_string())),
        }
    }
}

/// Runs off the UI thread.
fn resolve(api: &Api, path: &str) -> Option<String> {
    api.local_path(path).ok().flatten().map(|real| real.display().to_string())
}

/// The Customize Folder dialog: the look being chosen, saved on Done.
pub(crate) struct CustomizeFolder {
    pub api: Arc<Api>,
    pub folder: FileInfo,
    pub look: FolderLook,
    /// The special folder's own emblem, which `EmblemChoice::Own` shows.
    pub own: Option<&'static str>,
    /// Which colour the swatches set.
    pub paint: Paint,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Paint {
    Folder,
    Icon,
}

impl Workspace {
    /// Looks are drawn on BoxBox's folder, so the icon theme's folders take none.
    pub(crate) fn folder_looks_shown(&self) -> bool {
        self.preferences.file_icons != IconStyle::System
    }

    /// The look the user gave the folder at `path`, while BoxBox's icons are shown.
    pub(crate) fn folder_look(&self, api: &Api, path: &str) -> Option<&FolderLook> {
        self.folder_looks_shown().then(|| self.folder_looks.get(api, path)).flatten()
    }

    /// The emblem `look` gives a folder whose own is `own`.
    pub(crate) fn look_emblem(look: Option<&FolderLook>, own: Option<&'static str>) -> Option<Emblem> {
        let tint = look.and_then(|look| look.emblem_color);
        let name = match look.map(|look| &look.emblem) {
            None | Some(EmblemChoice::Own) => own?,
            Some(EmblemChoice::Without) => return None,
            Some(EmblemChoice::Icon(name)) => crate::emblems::find(name)?.name,
        };
        Some(Emblem { name, tint })
    }

    pub(crate) fn open_customize_folder(&mut self, api: Arc<Api>, folder: FileInfo, window: &mut Window, cx: &mut Context<Self>) {
        let look = self.folder_looks.get(&api, &folder.path).cloned().unwrap_or_default();
        let own = self.special_emblem(&api, &folder);
        self.customize = Some(CustomizeFolder { api, folder, look, own, paint: Paint::Folder });
        self.customize_search.update(cx, |field, cx| field.set_text(String::new(), 0..0, cx));
        self.customize_search.read(cx).focus(window);
        cx.notify();
    }

    /// Customize Folder… in a folder's menu.
    pub(crate) fn context_customize_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((target, items)) = self.context_selection() else { return };
        let (Some(api), [folder]) = (self.pane_by_id(target.0).map(|pane| pane.api.clone()), items.as_slice()) else { return };
        self.open_customize_folder(api, folder.clone(), window, cx);
    }

    pub(crate) fn close_customize_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.customize = None;
        self.focus_handle.focus(window);
        cx.notify();
    }

    /// Changes the look being chosen.
    pub(crate) fn change_look(&mut self, change: impl FnOnce(&mut FolderLook), cx: &mut Context<Self>) {
        if let Some(dialog) = self.customize.as_mut() {
            change(&mut dialog.look);
            cx.notify();
        }
    }

    /// Resolves the real path of a local folder being drawn, once, while any
    /// folder has a look, so a look saved through one path shows through another.
    pub(crate) fn resolve_folder_look(&mut self, api: &Arc<Api>, folder: &FileInfo, cx: &mut Context<Self>) {
        if !self.folder_looks_shown() || !folder.is_dir || self.folder_looks.looks.is_empty() {
            return;
        }
        let looks = &mut self.folder_looks;
        let Some(Written::Local(written)) = Written::of(api, &folder.path) else { return };
        if looks.real.contains_key(&written) || !looks.resolving.insert(written.clone()) {
            return;
        }
        let generation = looks.resolution_generation;
        let (api, path) = (Arc::clone(api), folder.path.clone());
        let real = cx.background_executor().spawn(async move { resolve(&api, &path) });
        cx.spawn(async move |this, cx| {
            let real = real.await;
            this.update(cx, |this, cx| {
                let looks = &mut this.folder_looks;
                if looks.resolution_generation != generation {
                    return;
                }
                looks.resolving.remove(&written);
                let real = real.unwrap_or_else(|| written.clone());
                if looks.real.len() >= RESOLVED {
                    looks.real.clear();
                }
                if looks.looks.get(&real) != looks.looks.get(&written) {
                    cx.notify();
                }
                looks.real.insert(written, real);
            })
            .ok();
        })
        .detach();
    }

    /// Done: keeps the chosen look, by the folder's real path.
    pub(crate) fn save_customize_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.customize.take() else { return };
        self.focus_handle.focus(window);
        cx.notify();
        let Some(written) = Written::of(&dialog.api, &dialog.folder.path) else { return };
        let real = match written {
            Written::Server(key) => Task::ready(key),
            Written::Local(written) => {
                let (api, path) = (dialog.api.clone(), dialog.folder.path.clone());
                cx.background_executor().spawn(async move { resolve(&api, &path).unwrap_or(written) })
            }
        };
        let look = dialog.look;
        cx.spawn(async move |this, cx| {
            let key = real.await;
            this.update(cx, |this, cx| {
                this.folder_looks.set(key, look);
                this.persist(&crate::persist::FOLDER_LOOKS, this.folder_looks.encode(), cx, |this, result, cx| {
                    if let Err(error) = result {
                        this.report(Err(format!("Could not save the folder's look: {error:#}")), cx);
                    }
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn on_customize_search(&mut self, event: &crate::components::InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        use crate::components::InputEvent;
        match event {
            InputEvent::Submit => self.save_customize_folder(window, cx),
            InputEvent::Cancel => self.close_customize_folder(window, cx),
            InputEvent::Changed => cx.notify(),
        }
    }
}
