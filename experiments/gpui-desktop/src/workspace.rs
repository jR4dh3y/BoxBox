//! Root view: tabs of panes plus the drive data shared by the sidebar and This PC.
//! Mirrors `lib/components/desktop/BrowseWorkspace.svelte`.

use std::{collections::HashMap, fs, sync::Arc, time::Duration};

use gpui::{
    actions, div, prelude::*, px, relative, App, Context, Entity, FocusHandle, KeyBinding,
    SharedString, Subscription, Window,
};

use crate::api::{Api, TrashItem};
use crate::pane::{Pane, ViewMode};
pub use crate::pane::Load;
use crate::components::{InputEvent, TextField};
use crate::search;
use crate::sort::{Sort, SortDir, SortField};
use crate::theme::{c, SURFACE_PRIMARY, TEXT_PRIMARY};
use crate::settings_state::{DesktopSettings, SettingsPanelState};

pub(crate) mod favorites;
mod watching;
pub(crate) mod operations;
pub(crate) mod preview;
pub(crate) mod appearance;
mod palette;
use appearance::WallpaperState;

use favorites::{Favorite, FavoriteProvider, Favorites};
use operations::{
    ContextMenuState, FileClipboard, LocationEditor, OperationDialog, OperationStatus, PaneTarget,
    PropertiesState,
};
use preview::{has_inline_thumbnail, is_text_preview, PreviewContent, PreviewState, ThumbnailCache};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsSection {
    General,
    Appearance,
    Links,
    Connections,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct Preferences {
    pub show_hidden_files: bool,
    pub show_file_extensions: bool,
    pub confirm_delete: bool,
    pub preview_on_single_click: bool,
    pub selection_checkboxes: bool,
    pub default_sort: String,
    pub default_sort_direction: String,
    pub default_view: String,
    pub hardware_acceleration: bool,
    pub compact_rows: bool,
    pub wallpaper_tint: bool,
    pub frosted_glass: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            show_hidden_files: false,
            show_file_extensions: true,
            confirm_delete: true,
            preview_on_single_click: false,
            selection_checkboxes: true,
            // `DEFAULT_SORT_BY` and `DEFAULT_SORT_DIR` in sortFiles.ts.
            default_sort: "Date modified".into(),
            default_sort_direction: "Descending".into(),
            default_view: "List".into(),
            hardware_acceleration: true,
            compact_rows: false,
            wallpaper_tint: false,
            frosted_glass: false,
        }
    }
}

impl Preferences {
    pub(crate) fn load() -> Self {
        fs::read(crate::persist::PREFERENCES.path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn encode(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(self).map_err(|error| format!("cannot serialize settings: {error}"))
    }
}

actions!(
    boxbox,
    [
        Back,
        Forward,
        Refresh,
        NewTab,
        CloseTab,
        NextTab,
        PreviousTab,
        ParentFolder,
        ToggleHidden,
        OpenConnections,
        PropertiesSelected,
        SelectAll,
        ClearSelection,
        OpenSettings,
        CloseSettings,
        FocusSearch,
        CreateFolder,
        CreateFile,
        RenameSelected,
        DeleteSelected,
        DownloadSelected,
        CopySelected,
        CutSelected,
        PasteInto,
        SelectUp,
        SelectDown,
        ExtendUp,
        ExtendDown,
        SelectLeft,
        SelectRight,
        OpenActive,
        PreviewActive,
        FocusLocation,
        ToggleSplit,
        AddFavorite,
        OperationEscape,
        SettingsSubmit,
        SettingsCancel,
        SettingsBackspace,
        SettingsPaste,
        SettingsSelectAll
    ]
);

pub fn bind_keys(cx: &mut App) {
    let context = Some("BrowseWorkspace");
    cx.bind_keys([
        KeyBinding::new("alt-left", Back, context),
        KeyBinding::new("alt-up", ParentFolder, context),
        KeyBinding::new("ctrl-tab", NextTab, context),
        KeyBinding::new("ctrl-shift-tab", PreviousTab, context),
        KeyBinding::new("ctrl-h", ToggleHidden, context),
        KeyBinding::new("ctrl-shift-o", OpenConnections, context),
        KeyBinding::new("alt-enter", PropertiesSelected, context),
        KeyBinding::new("f3", ToggleSplit, context),
        KeyBinding::new("backspace", Back, context),
        KeyBinding::new("alt-right", Forward, context),
        KeyBinding::new("f5", Refresh, context),
        KeyBinding::new("ctrl-r", Refresh, context),
        KeyBinding::new("ctrl-t", NewTab, context),
        KeyBinding::new("ctrl-w", CloseTab, context),
        KeyBinding::new("ctrl-a", SelectAll, context),
        KeyBinding::new("escape", ClearSelection, context),
        KeyBinding::new("ctrl-,", OpenSettings, context),
        KeyBinding::new("ctrl-f", FocusSearch, context),
        KeyBinding::new("ctrl-l", FocusLocation, context),
        KeyBinding::new("ctrl-shift-n", CreateFolder, context),
        KeyBinding::new("ctrl-shift-e", CreateFile, context),
        KeyBinding::new("f2", RenameSelected, context),
        KeyBinding::new("delete", DeleteSelected, context),
        KeyBinding::new("ctrl-d", DownloadSelected, context),
        KeyBinding::new("ctrl-c", CopySelected, context),
        KeyBinding::new("ctrl-x", CutSelected, context),
        KeyBinding::new("ctrl-v", PasteInto, context),
        KeyBinding::new("up", SelectUp, context),
        KeyBinding::new("down", SelectDown, context),
        KeyBinding::new("shift-up", ExtendUp, context),
        KeyBinding::new("shift-down", ExtendDown, context),
        KeyBinding::new("left", SelectLeft, context),
        KeyBinding::new("right", SelectRight, context),
        KeyBinding::new("enter", OpenActive, context),
        KeyBinding::new("space", PreviewActive, context),
        KeyBinding::new("ctrl-shift-5", ToggleSplit, context),
        KeyBinding::new("ctrl-b", AddFavorite, context),
        KeyBinding::new("escape", OperationEscape, Some("PreviewInput")),
        KeyBinding::new("enter", SettingsSubmit, Some("SettingsInput")),
        KeyBinding::new("escape", SettingsCancel, Some("SettingsInput")),
        KeyBinding::new("escape", CloseSettings, Some("SettingsPanel")),
        KeyBinding::new("backspace", SettingsBackspace, Some("SettingsInput")),
        KeyBinding::new("ctrl-v", SettingsPaste, Some("SettingsInput")),
        KeyBinding::new("ctrl-a", SettingsSelectAll, Some("SettingsInput")),
    ]);
}


pub struct Fonts {
    pub sans: SharedString,
    pub mono: SharedString,
}

pub struct Workspace {
    directory_watches: watching::DirectoryWatches,
    pub(crate) tabs: Vec<Pane>,
    pub(crate) active_tab: usize,
    pub(crate) split_panes: HashMap<usize, Pane>,
    pub(crate) active_pane: PaneTarget,
    next_pane_id: usize,
    pub(crate) trash: Load<Vec<TrashItem>>,
    pub(crate) trash_open: bool,
    /// Sidebar sections the user folded, by title.
    pub(crate) collapsed_sections: std::collections::HashSet<&'static str>,
    pub(crate) trash_error: Option<String>,
    trash_provider: Option<Arc<Api>>,
    pub(crate) fonts: Fonts,
    pub(crate) focus_handle: FocusHandle,
    /// One search field per pane, as each pane's toolbar has one.
    pub(crate) search_fields: HashMap<usize, (Entity<TextField>, Subscription)>,
    pub(crate) settings_open: bool,
    pub(crate) settings_section: SettingsSection,
    pub(crate) preferences: Preferences,
    pub(crate) desktop_settings: DesktopSettings,
    pub(crate) settings_panel: SettingsPanelState,
    pub(crate) operation_status: OperationStatus,
    pub(crate) operation_dialog: Option<OperationDialog>,
    pub(crate) settings_focus: FocusHandle,
    pub(crate) dialog_field: Entity<TextField>,
    pub(crate) location_editor: Option<LocationEditor>,
    pub(crate) location_field: Entity<TextField>,
    _subscriptions: Vec<Subscription>,
    pub(crate) file_clipboard: Option<FileClipboard>,
    pub(crate) context_menu: Option<ContextMenuState>,
    /// The pane whose toolbar "+" menu is open.
    pub(crate) add_menu: Option<PaneTarget>,
    pub(crate) preview: Option<PreviewState>,
    pub(crate) preview_focus: FocusHandle,
    pub(crate) properties: Option<PropertiesState>,
    pub(crate) thumbnail_cache: ThumbnailCache,
    pub(crate) favorites: Favorites,
    pub(crate) favorites_error: Option<String>,
    pub(crate) wallpaper: WallpaperState,
    pub(crate) wallpaper_generation: u64,
    mutation_generation: u64,
}

impl Workspace {
    pub fn new(
        api: Api,
        fonts: Fonts,
        initial_path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);
        let preferences = Preferences::load();
        let api = Arc::new(api);
        let initial_pane = Pane::with_api(
            0,
            initial_path,
            api.clone(),
            preferences.show_hidden_files,
            preference_sort(&preferences),
            preference_view(&preferences),
        );
        let dialog_field = cx.new(|cx| TextField::new("", cx));
        let location_field = cx.new(|cx| TextField::new("Type a location", cx));
        let mut workspace = Self {
            directory_watches: watching::DirectoryWatches::default(),
            tabs: vec![initial_pane],
            active_tab: 0,
            split_panes: HashMap::new(),
            active_pane: PaneTarget(0),
            next_pane_id: 1,
            trash: Load::Loading,
            trash_open: false,
            collapsed_sections: Default::default(),
            trash_error: None,
            trash_provider: None,
            fonts,
            focus_handle,
            search_fields: HashMap::new(),
            settings_open: false,
            settings_section: SettingsSection::General,
            preferences,
            desktop_settings: DesktopSettings::load(),
            settings_panel: SettingsPanelState::default(),
            operation_status: OperationStatus::Idle,
            operation_dialog: None,
            settings_focus: cx.focus_handle(),
            dialog_field: dialog_field.clone(),
            location_editor: None,
            location_field: location_field.clone(),
            _subscriptions: vec![
                cx.subscribe_in(&dialog_field, window, |this, _, event, window, cx| this.on_dialog_field(event, window, cx)),
                cx.subscribe_in(&location_field, window, |this, _, event, window, cx| this.on_location_field(event, window, cx)),
            ],
            file_clipboard: None,
            context_menu: None,
            add_menu: None,
            preview: None,
            preview_focus: cx.focus_handle(),
            properties: None,
            thumbnail_cache: ThumbnailCache::default(),
            favorites: Favorites::load(),
            favorites_error: None,
            wallpaper: WallpaperState::Empty,
            wallpaper_generation: 0,
            mutation_generation: 0,
        };
        workspace.reload_wallpaper(cx);
        workspace.load_drives(PaneTarget(0), cx);
        workspace.load_pane(PaneTarget(0), cx);
        workspace.start_directory_watches(cx);
        workspace
    }

    pub(crate) fn browser_width(&self, window: &Window) -> f32 {
        let width = f32::from(window.viewport_size().width) - crate::ui::SIDEBAR_WIDTH;
        if self.split_panes.contains_key(&self.tabs[self.active_tab].id) { (width - 1.) / 2. } else { width }
    }

    pub(crate) fn pane(&self) -> &Pane {
        self.pane_by_id(self.active_pane.0)
            .unwrap_or(&self.tabs[self.active_tab])
    }

    pub(crate) fn pane_mut(&mut self) -> &mut Pane {
        let target = self.active_pane.0;
        if let Some(index) = self.tabs.iter().position(|pane| pane.id == target) {
            return &mut self.tabs[index];
        }
        if let Some(pane) = self.split_panes.get_mut(&target) {
            return pane;
        }
        &mut self.tabs[self.active_tab]
    }

    pub(crate) fn pane_by_id(&self, id: usize) -> Option<&Pane> {
        self.tabs
            .iter()
            .chain(self.split_panes.values())
            .find(|pane| pane.id == id)
    }

    pub(crate) fn pane_mut_by_id(&mut self, id: usize) -> Option<&mut Pane> {
        self.tabs
            .iter_mut()
            .chain(self.split_panes.values_mut())
            .find(|pane| pane.id == id)
    }

    pub(crate) fn active_api(&self) -> Arc<Api> {
        self.pane().api.clone()
    }

    /// Activates a provider in a new tab, preserving every existing local and remote pane.
    pub(crate) fn open_provider(&mut self, api: Api, cx: &mut Context<Self>) {
        let api = Arc::new(api);
        let id = self.next_pane_id;
        self.next_pane_id += 1;
        self.tabs.push(Pane::with_api(
            id,
            String::new(),
            api,
            self.preferences.show_hidden_files,
            preference_sort(&self.preferences),
            preference_view(&self.preferences),
        ));
        self.active_tab = self.tabs.len() - 1;
        self.active_pane = PaneTarget(id);
        self.trash_open = false;
        self.trash_error = None;
        self.thumbnail_cache.invalidate();
        self.load_drives(PaneTarget(id), cx);
        self.load_pane(PaneTarget(id), cx);
        cx.notify();
    }

    fn load_drives(&mut self, target: PaneTarget, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_by_id(target.0) else {
            return;
        };
        let api = pane.api.clone();
        cx.spawn(async move |this, cx| {
            let (roots, drives) = cx
                .background_executor()
                .spawn(async move { (api.roots(), api.drives()) })
                .await;
            this.update(cx, |this, cx| {
                if let Some(pane) = this.pane_mut_by_id(target.0) {
                    pane.roots = match roots {
                        Ok(roots) => Load::Loaded(roots),
                        Err(error) => Load::Failed(format!("{error:#}")),
                    };
                    pane.drives = match drives {
                        Ok(drives) => Load::Loaded(drives),
                        Err(error) => Load::Failed(format!("{error:#}")),
                    };
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn load_trash(&mut self, api: Arc<Api>, cx: &mut Context<Self>) {
        if !api.is_local() {
            return;
        }
        self.trash_provider = Some(api.clone());
        self.trash = Load::Loading;
        self.trash_error = None;
        let request_api = api.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { request_api.trash() })
                .await;
            this.update(cx, |this, cx| {
                if this
                    .trash_provider
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, &api))
                {
                    this.trash = match result {
                        Ok(items) => Load::Loaded(items),
                        Err(error) => {
                            this.trash_error = Some(format!("{error:#}"));
                            Load::Failed(format!("{error:#}"))
                        }
                    };
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn load_pane(&mut self, target: PaneTarget, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_mut_by_id(target.0) else {
            return;
        };
        let generation = pane.begin_load();
        if pane.is_at_root() {
            return;
        }
        let path = pane.path.clone();
        let api = pane.api.clone();
        let view = pane.view();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let items = api.list(&path).map(|items| crate::pane::arrange(items, view));
                    (items, api.local_path(&path).ok().flatten())
                })
                .await;
            this.update(cx, |this, cx| {
                if let Some(pane) = this.pane_mut_by_id(target.0) {
                    if pane.generation == generation {
                        let stale = pane.finish_load(result.0, view);
                        if let Some(path) = result.1 { this.directory_watches.watch(target.0, path); }
                        if stale { this.resort(target, cx); }
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// Writes a settings file on a background thread. `done` runs on the UI thread
    /// unless a newer write to the same file replaced this one.
    pub(crate) fn persist(
        &self,
        store: &'static crate::persist::Store,
        bytes: Vec<u8>,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Result<(), String>, &mut Context<Self>) + 'static,
    ) {
        let write = store.write(bytes);
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { write() }).await;
            let result = match result {
                Ok(crate::persist::Written::Superseded) => return,
                Ok(crate::persist::Written::Saved) => Ok(()),
                Err(error) => Err(format!("cannot write {}: {error}", store.path().display())),
            };
            this.update(cx, |this, cx| {
                done(this, result, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Saves favorites; `success` is the status to show once they are on disk.
    pub(crate) fn save_favorites(&mut self, success: Option<&'static str>, cx: &mut Context<Self>) {
        let bytes = match self.favorites.encode() {
            Ok(bytes) => bytes,
            Err(error) => {
                self.favorites_error = Some(error.to_string());
                return;
            }
        };
        self.persist(&crate::persist::FAVORITES, bytes, cx, move |this, result, _| match result {
            Ok(()) => {
                this.favorites_error = None;
                if let Some(message) = success {
                    this.operation_status = OperationStatus::Success(message.into());
                }
            }
            Err(error) => {
                if success.is_some() {
                    this.operation_status = OperationStatus::Error(error.clone());
                }
                this.favorites_error = Some(error);
            }
        });
    }

    /// Sorts a pane's entries in the background after its sort changed.
    pub(crate) fn resort(&mut self, target: PaneTarget, cx: &mut Context<Self>) {
        let Some(job) = self.pane_by_id(target.0).map(Pane::sort_job) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let (job, (items, folder_items)) = cx
                .background_executor()
                .spawn(async move {
                    let sorted = job.run();
                    (job, sorted)
                })
                .await;
            this.update(cx, |this, cx| {
                if this.pane_mut_by_id(target.0).is_some_and(|pane| pane.apply_sort(&job, items, folder_items)) {
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Changes one pane's sort from its column headers.
    pub(crate) fn set_pane_sort(&mut self, target: PaneTarget, sort: crate::sort::Sort, cx: &mut Context<Self>) {
        if let Some(pane) = self.pane_mut_by_id(target.0) {
            pane.set_sort(sort);
            self.resort(target, cx);
            cx.notify();
        }
    }

    fn all_pane_ids(&self) -> Vec<usize> {
        self.tabs
            .iter()
            .chain(self.split_panes.values())
            .map(|pane| pane.id)
            .collect()
    }

    fn refresh_provider(&mut self, api: Arc<Api>, cx: &mut Context<Self>) {
        self.thumbnail_cache.invalidate();
        let targets: Vec<_> = self
            .all_pane_ids()
            .into_iter()
            .filter(|id| {
                self.pane_by_id(*id)
                    .is_some_and(|pane| Arc::ptr_eq(&pane.api, &api))
            })
            .map(PaneTarget)
            .collect();
        for target in targets {
            if self.pane_by_id(target.0).is_some_and(Pane::is_at_root) {
                self.load_drives(target, cx);
            } else {
                self.load_pane(target, cx);
                self.schedule_search_for(target.0, Duration::ZERO, cx);
            }
        }
    }

    fn apply_preferences(&mut self, cx: &mut Context<Self>) {
        let sort = preference_sort(&self.preferences);
        let view = preference_view(&self.preferences);
        let show_hidden_files = self.preferences.show_hidden_files;
        let ids = self.all_pane_ids();
        let mut hidden_changed = false;
        let mut resorted = Vec::new();
        for id in &ids {
            if let Some(pane) = self.pane_mut_by_id(*id) {
                if pane.sort != sort {
                    resorted.push(PaneTarget(*id));
                }
                hidden_changed |= pane.apply_preferences(show_hidden_files, sort, view);
            }
        }
        match self.preferences.encode() {
            Ok(bytes) => self.persist(&crate::persist::PREFERENCES, bytes, cx, |this, result, _| {
                if let Err(error) = result {
                    this.operation_status = OperationStatus::Error(format!("Could not save preferences: {error}"));
                }
            }),
            Err(error) => self.operation_status = OperationStatus::Error(format!("Could not save preferences: {error}")),
        }
        for target in resorted {
            self.resort(target, cx);
        }
        if hidden_changed {
            for id in ids {
                self.load_pane(PaneTarget(id), cx);
                self.schedule_search_for(id, Duration::ZERO, cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn navigate(&mut self, path: String, cx: &mut Context<Self>) {
        self.navigate_in(self.active_pane, path, cx);
    }

    pub(crate) fn navigate_in(
        &mut self,
        target: PaneTarget,
        path: String,
        cx: &mut Context<Self>,
    ) {
        self.trash_open = false;
        self.trash_error = None;
        if self
            .pane_mut_by_id(target.0)
            .is_some_and(|pane| pane.navigate(path))
        {
            self.load_pane(target, cx);
        }
        cx.notify();
    }

    pub(crate) fn back(&mut self, _: &Back, _: &mut Window, cx: &mut Context<Self>) {
        if self.trash_open {
            self.trash_open = false;
            self.trash_error = None;
            cx.notify();
            return;
        }
        if self.pane_mut().go_back() {
            let target = self.active_pane;
            self.load_pane(target, cx);
            cx.notify();
        }
    }

    pub(crate) fn forward(&mut self, _: &Forward, _: &mut Window, cx: &mut Context<Self>) {
        if self.pane_mut().go_forward() {
            let target = self.active_pane;
            self.load_pane(target, cx);
            cx.notify();
        }
    }

    pub(crate) fn refresh(&mut self, _: &Refresh, _: &mut Window, cx: &mut Context<Self>) {
        if self.trash_open {
            if let Some(api) = self.trash_provider.clone() {
                self.load_trash(api, cx);
            }
        } else {
            let target = self.active_pane;
            self.thumbnail_cache.invalidate();
            if self.pane().is_at_root() {
                self.load_drives(target, cx);
            } else {
                self.load_pane(target, cx);
                self.schedule_search_for(target.0, Duration::ZERO, cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn open_trash(&mut self, cx: &mut Context<Self>) {
        let api = self.active_api();
        if !api.is_local() {
            self.operation_status =
                OperationStatus::Error("Trash is available only for local storage".into());
            cx.notify();
            return;
        }
        self.trash_open = true;
        self.trash_error = None;
        self.load_trash(api, cx);
        cx.notify();
    }

    pub(crate) fn restore_trash(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(api) = self.trash_provider.clone().filter(|api| api.is_local()) else {
            return;
        };
        if self.operation_status.is_busy() {
            return;
        }
        let token = self.begin_operation("Restoring from Trash");
        let refresh_api = api.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { api.restore_trash(&id) })
                .await;
            this.update(cx, |this, cx| {
                if this.mutation_generation == token {
                    this.operation_status = match &result {
                        Ok(()) => OperationStatus::Success("Restored from Trash".into()),
                        Err(error) => OperationStatus::Error(format!(
                            "Could not restore item: {error:#}"
                        )),
                    };
                    this.trash_error = result
                        .as_ref()
                        .err()
                        .map(|error| format!("Could not restore item: {error:#}"));
                    this.load_trash(refresh_api.clone(), cx);
                    this.refresh_provider(refresh_api.clone(), cx);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn delete_trash_item(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(api) = self.trash_provider.clone().filter(|api| api.is_local()) else {
            return;
        };
        if self.operation_status.is_busy() {
            return;
        }
        let token = self.begin_operation("Deleting permanently");
        let refresh_api = api.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { api.delete_trash(&id) })
                .await;
            this.update(cx, |this, cx| {
                if this.mutation_generation == token {
                    this.operation_status = match &result {
                        Ok(()) => OperationStatus::Success("Deleted permanently".into()),
                        Err(error) => OperationStatus::Error(format!(
                            "Could not permanently delete item: {error:#}"
                        )),
                    };
                    this.trash_error = result.as_ref().err().map(|error| {
                        format!("Could not permanently delete item: {error:#}")
                    });
                    this.load_trash(refresh_api.clone(), cx);
                    this.refresh_provider(refresh_api.clone(), cx);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }



    pub(crate) fn new_tab(&mut self, _: &NewTab, _: &mut Window, cx: &mut Context<Self>) {
        let api = self.active_api();
        let id = self.next_pane_id;
        self.next_pane_id += 1;
        self.tabs.push(Pane::with_api(
            id,
            String::new(),
            api,
            self.preferences.show_hidden_files,
            preference_sort(&self.preferences),
            preference_view(&self.preferences),
        ));
        self.active_tab = self.tabs.len() - 1;
        self.active_pane = PaneTarget(id);
        self.trash_open = false;
        self.load_drives(PaneTarget(id), cx);
        self.load_pane(PaneTarget(id), cx);
        cx.notify();
    }

    pub(crate) fn close_tab_at(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            return;
        }
        let removed = self.tabs.remove(index);
        self.split_panes.remove(&removed.id);
        if self.active_tab > index || self.active_tab >= self.tabs.len() {
            self.active_tab = self.active_tab.saturating_sub(1);
        }
        self.active_pane = PaneTarget(self.tabs[self.active_tab].id);
        self.trash_open = false;
        cx.notify();
    }

    fn close_tab(&mut self, _: &CloseTab, _: &mut Window, cx: &mut Context<Self>) {
        self.close_tab_at(self.active_tab, cx);
    }

    pub(crate) fn activate_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(pane) = self.tabs.get(index) else {
            return;
        };
        self.active_tab = index;
        self.active_pane = PaneTarget(pane.id);
        self.trash_open = false;
        cx.notify();
    }

    /// Sends one pane's search after `delay`, unless the input or folder changes first.
    fn schedule_search_for(
        &mut self,
        pane_id: usize,
        delay: Duration,
        cx: &mut Context<Self>,
    ) {
        let Some(pane) = self.pane_by_id(pane_id) else {
            return;
        };
        if search::active_query(&pane.search_input).is_none() {
            return;
        }
        let revision = pane.search_generation;
        cx.spawn(async move |this, cx| {
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            this.update(cx, |this, cx| {
                if this
                    .pane_by_id(pane_id)
                    .is_some_and(|pane| pane.search_generation == revision)
                {
                    this.run_search(pane_id, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn run_search(&mut self, pane_id: usize, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_mut_by_id(pane_id) else {
            return;
        };
        let Some((generation, query)) = pane.begin_search() else {
            return;
        };
        let path = pane.path.clone();
        let api = pane.api.clone();
        let view = pane.view();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { api.search(&path, &query).map(|items| crate::pane::arrange(items, view)) })
                .await;
            this.update(cx, |this, cx| {
                if let Some(pane) = this.pane_mut_by_id(pane_id) {
                    if pane.search_generation == generation {
                        if pane.finish_search(result, view) {
                            this.resort(PaneTarget(pane_id), cx);
                        }
                        cx.notify();
                    }
                }
            })
                .ok();
        })
        .detach();
        cx.notify();
    }

    /// Creates a search field for each new pane and drops those of closed panes.
    pub(crate) fn sync_search_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ids = self.all_pane_ids();
        self.search_fields.retain(|id, _| ids.contains(id));
        for id in ids {
            if self.search_fields.contains_key(&id) {
                continue;
            }
            let field = cx.new(|cx| TextField::new("Search files and folders...", cx));
            let subscription =
                cx.subscribe_in(&field, window, move |this, _, event, window, cx| this.on_search_field(id, event, window, cx));
            self.search_fields.insert(id, (field, subscription));
        }
        // Navigation clears a pane's search, so the field follows the pane.
        for (id, (field, _)) in &self.search_fields {
            let Some(pane) = self.pane_by_id(*id) else { continue };
            if field.read(cx).text() != pane.search_input {
                let text = pane.search_input.clone();
                let end = text.len();
                field.update(cx, |field, cx| field.set_text(text, end..end, cx));
            }
        }
    }

    fn on_search_field(&mut self, pane_id: usize, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some((field, _)) = self.search_fields.get(&pane_id) else { return };
        match event {
            InputEvent::Changed => {
                let text = field.read(cx).text().to_string();
                if self.pane_mut_by_id(pane_id).is_some_and(|pane| pane.set_search_input(text)) {
                    self.schedule_search_for(pane_id, search::DEBOUNCE, cx);
                }
            }
            // Enter searches now instead of waiting for the debounce.
            InputEvent::Submit => self.schedule_search_for(pane_id, Duration::ZERO, cx),
            InputEvent::Cancel => {
                if let Some(pane) = self.pane_mut_by_id(pane_id) {
                    pane.clear_search();
                }
                window.focus(&self.focus_handle);
            }
        }
        cx.notify();
    }

    pub(crate) fn search_field(&self, pane_id: usize) -> Option<&Entity<TextField>> {
        self.search_fields.get(&pane_id).map(|(field, _)| field)
    }

    fn focus_search(&mut self, _: &FocusSearch, window: &mut Window, cx: &mut Context<Self>) {
        if !self.pane().is_at_root() {
            if let Some(field) = self.search_field(self.active_pane.0) {
                field.read(cx).focus(window);
            }
            cx.notify();
        }
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.pane_mut().select_all();
        cx.notify();
    }

    fn clear_selection(&mut self, _: &ClearSelection, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open { self.settings_open = false; self.settings_panel.dialog = None; }
        else if self.properties.take().is_some() || self.context_menu.take().is_some() {}
        else if self.preview.is_some() { self.operation_escape(&OperationEscape, window, cx); return; }
        else { self.pane_mut().clear_selection(); }
        self.focus_handle.focus(window);
        cx.notify();
    }
    pub(crate) fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.pane_mut().move_selection(-1, false);
        cx.notify();
    }

    pub(crate) fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.pane_mut().move_selection(1, false);
        cx.notify();
    }

    pub(crate) fn extend_up(&mut self, _: &ExtendUp, _: &mut Window, cx: &mut Context<Self>) {
        self.pane_mut().move_selection(-1, true);
        cx.notify();
    }

    pub(crate) fn extend_down(&mut self, _: &ExtendDown, _: &mut Window, cx: &mut Context<Self>) {
        self.pane_mut().move_selection(1, true);
        cx.notify();
    }

    pub(crate) fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.pane_mut().move_selection(-1, false);
        cx.notify();
    }

    pub(crate) fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.pane_mut().move_selection(1, false);
        cx.notify();
    }

    pub(crate) fn toggle_split(
        &mut self,
        _: &ToggleSplit,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let primary_id = self.tabs[self.active_tab].id;
        if self.split_panes.remove(&primary_id).is_some() {
            self.active_pane = PaneTarget(primary_id);
        } else {
            let (path, api, show_hidden_files, sort, view_mode) = {
                let source = self
                    .pane_by_id(self.active_pane.0)
                    .unwrap_or(&self.tabs[self.active_tab]);
                (
                    source.path.clone(),
                    source.api.clone(),
                    source.show_hidden_files,
                    source.sort,
                    source.view_mode,
                )
            };
            let id = self.next_pane_id;
            self.next_pane_id += 1;
            let secondary = Pane::with_api(id, path, api, show_hidden_files, sort, view_mode);
            self.split_panes.insert(primary_id, secondary);
            self.active_pane = PaneTarget(id);
            self.load_drives(PaneTarget(id), cx);
            self.load_pane(PaneTarget(id), cx);
        }
        self.trash_open = false;
        cx.notify();
    }


    pub(crate) fn open_settings(
        &mut self,
        _: &OpenSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_open = true;
        self.settings_section = SettingsSection::General;
        self.settings_focus.focus(window);
        cx.notify();
    }

    pub(crate) fn close_settings(
        &mut self,
        _: &CloseSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_open = false;
        self.settings_panel.dialog = None;
        self.focus_handle.focus(window);
        cx.notify();
    }

    pub(crate) fn commit_preferences(&mut self, cx: &mut Context<Self>) {
        self.apply_preferences(cx);
    }

    pub(crate) fn select_settings_section(
        &mut self,
        section: SettingsSection,
        cx: &mut Context<Self>,
    ) {
        self.settings_section = section;
        cx.notify();
    }
}

fn preference_sort(preferences: &Preferences) -> Sort {
    let field = match preferences.default_sort.as_str() {
        "Size" => SortField::Size,
        "Date modified" => SortField::Modified,
        "Type" => SortField::Type,
        _ => SortField::Name,
    };
    let dir = if preferences.default_sort_direction == "Descending" {
        SortDir::Desc
    } else {
        SortDir::Asc
    };
    Sort { field, dir }
}

fn preference_view(preferences: &Preferences) -> ViewMode {
    if preferences.default_view == "Grid" {
        ViewMode::Grid
    } else {
        ViewMode::List
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_search_fields(window, cx);
        let primary = PaneTarget(self.tabs[self.active_tab].id);
        let mut browsers = div().flex().flex_1().min_h_0().min_w_0()
            .child(self.render_browser(primary, window, cx));
        if let Some(secondary) = self.split_panes.get(&primary.0) {
            browsers = browsers.child(div().w(px(1.)).h_full().bg(c(crate::theme::BORDER_PRIMARY)))
                .child(self.render_browser(PaneTarget(secondary.id), window, cx));
        }
        div()
            .key_context("BrowseWorkspace")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::back))
            .on_action(cx.listener(Self::forward))
            .on_action(cx.listener(Self::refresh))
            .on_action(cx.listener(Self::new_tab))
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(|this, _: &NextTab, _, cx| this.activate_tab((this.active_tab + 1) % this.tabs.len(), cx)))
            .on_action(cx.listener(|this, _: &PreviousTab, _, cx| this.activate_tab((this.active_tab + this.tabs.len() - 1) % this.tabs.len(), cx)))
            .on_action(cx.listener(|this, _: &ParentFolder, _, cx| {
                let parent = this.pane().path.rsplit_once('/').map(|(parent, _)| parent).unwrap_or("").to_string();
                this.navigate(parent, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleHidden, _, cx| {
                this.preferences.show_hidden_files = !this.preferences.show_hidden_files;
                this.commit_preferences(cx);
            }))
            .on_action(cx.listener(|this, _: &OpenConnections, window, cx| {
                this.open_settings(&OpenSettings, window, cx);
                this.settings_section = SettingsSection::Connections;
            }))
            .on_action(cx.listener(|this, _: &PropertiesSelected, _, cx| {
                let pane = this.pane();
                let items = pane.selected_items();
                if let [item] = items.as_slice() {
                    this.properties = Some(PropertiesState { item: item.clone(), provider_label: pane.api.server_url().unwrap_or("Local storage").to_string() });
                    cx.notify();
                }
            }))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::clear_selection))
            .on_action(cx.listener(Self::open_settings))
            .on_action(cx.listener(Self::close_settings))
            .on_action(cx.listener(Self::focus_search))
            .on_action(cx.listener(Self::create_folder))
            .on_action(cx.listener(Self::create_file))
            .on_action(cx.listener(Self::rename_selected))
            .on_action(cx.listener(Self::delete_selected))
            .on_action(cx.listener(Self::download_selected))
            .on_action(cx.listener(Self::copy_selected))
            .on_action(cx.listener(Self::cut_selected))
            .on_action(cx.listener(Self::paste_clipboard))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::extend_up))
            .on_action(cx.listener(Self::extend_down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::open_active))
            .on_action(cx.listener(Self::preview_active))
            .on_action(cx.listener(Self::focus_location))
            .on_action(cx.listener(Self::toggle_split))
            .on_action(cx.listener(Self::add_favorite))
            .on_action(cx.listener(Self::operation_escape))
            .relative()
            .size_full()
            .flex()
            .bg(c(SURFACE_PRIMARY))
            .child(self.render_wallpaper(window))
            .text_color(c(TEXT_PRIMARY))
            .font_family(self.fonts.sans.clone())
            // The page's body line-height; Tailwind sizes like text-xs override it.
            .line_height(relative(1.5))
            .text_size(px(16.))
            .child(div().relative().h_full().child(self.render_sidebar(cx)))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(self.render_tabs(cx))
                    .child(browsers),
            )
            .when(self.settings_open, |root| {
                root.child(self.render_settings(cx))
            })
            .when(self.preview.is_some(), |root| root.child(self.render_preview(cx)))
            .when(self.context_menu.is_some(), |root| root.child(self.render_context_menu(cx)))
            .when(self.properties.is_some(), |root| root.child(self.render_properties(cx)))
            .when(self.operation_dialog.is_some(), |root| {
                root.child(self.render_operation_dialog(window, cx))
            })
    }
}

#[cfg(test)]
mod parity_test;
