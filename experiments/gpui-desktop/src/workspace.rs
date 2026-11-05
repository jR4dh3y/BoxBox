//! Root view: tabs of panes plus the drive data shared by the sidebar and This PC.
//! Mirrors `lib/components/desktop/BrowseWorkspace.svelte`.

use std::{fs, path::PathBuf, sync::Arc};

use gpui::{
    actions, div, prelude::*, px, relative, App, Context, FocusHandle, KeyBinding, SharedString,
    Window,
};

use crate::api::{Api, DriveStats, MountPoint};
use crate::pane::{Pane, ViewMode};
use crate::sort::{Sort, SortDir, SortField};
use crate::theme::{c, SURFACE_PRIMARY, TEXT_PRIMARY};

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
            default_sort: "Name".into(),
            default_sort_direction: "Ascending".into(),
            default_view: "List".into(),
            hardware_acceleration: true,
            compact_rows: false,
            wallpaper_tint: false,
            frosted_glass: false,
        }
    }
}

impl Preferences {
    fn path() -> PathBuf {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("boxbox-gpui")
            .join("settings.json")
    }

    pub(crate) fn load() -> Self {
        fs::read(Self::path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub(crate) fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(self) {
            let _ = fs::write(path, bytes);
        }
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
        SelectAll,
        ClearSelection,
        OpenSettings,
        CloseSettings
    ]
);

pub fn bind_keys(cx: &mut App) {
    let context = Some("BrowseWorkspace");
    cx.bind_keys([
        KeyBinding::new("alt-left", Back, context),
        KeyBinding::new("backspace", Back, context),
        KeyBinding::new("alt-right", Forward, context),
        KeyBinding::new("f5", Refresh, context),
        KeyBinding::new("ctrl-r", Refresh, context),
        KeyBinding::new("ctrl-t", NewTab, context),
        KeyBinding::new("ctrl-w", CloseTab, context),
        KeyBinding::new("ctrl-a", SelectAll, context),
        KeyBinding::new("escape", ClearSelection, context),
        KeyBinding::new("ctrl-,", OpenSettings, context),
    ]);
}

pub enum Load<T> {
    Loading,
    Loaded(T),
    Failed(String),
}

pub struct Fonts {
    pub sans: SharedString,
    pub mono: SharedString,
}

pub struct Workspace {
    pub(crate) api: Arc<Api>,
    pub(crate) tabs: Vec<Pane>,
    pub(crate) active_tab: usize,
    next_pane_id: usize,
    pub(crate) roots: Vec<MountPoint>,
    pub(crate) drives: Load<Vec<DriveStats>>,
    pub(crate) fonts: Fonts,
    focus_handle: FocusHandle,
    pub(crate) settings_open: bool,
    pub(crate) settings_section: SettingsSection,
    pub(crate) preferences: Preferences,
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
        let mut workspace = Self {
            api: Arc::new(api),
            tabs: vec![Pane::new(
                0,
                initial_path,
                preferences.show_hidden_files,
                preference_sort(&preferences),
                preference_view(&preferences),
            )],
            active_tab: 0,
            next_pane_id: 1,
            roots: Vec::new(),
            drives: Load::Loading,
            fonts,
            focus_handle,
            settings_open: false,
            settings_section: SettingsSection::General,
            preferences,
        };
        workspace.load_drives(cx);
        workspace.load_active(cx);
        workspace
    }

    pub(crate) fn pane(&self) -> &Pane {
        &self.tabs[self.active_tab]
    }

    pub(crate) fn pane_mut(&mut self) -> &mut Pane {
        &mut self.tabs[self.active_tab]
    }

    /// Roots feed the sidebar and breadcrumbs; stats feed the This PC cards.
    fn load_drives(&mut self, cx: &mut Context<Self>) {
        let api = self.api.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { (api.roots(), api.drives()) })
                .await;
            this.update(cx, |this, cx| {
                let (roots, drives) = result;
                this.roots = roots.unwrap_or_default();
                this.drives = match drives {
                    Ok(drives) => Load::Loaded(drives),
                    Err(error) => Load::Failed(format!("{error:#}")),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn load_active(&mut self, cx: &mut Context<Self>) {
        let pane = self.pane_mut();
        // Bump first so a folder request still in flight cannot land on This PC.
        let generation = pane.begin_load();
        if pane.is_at_root() {
            return;
        }
        let (pane_id, path) = (pane.id, pane.path.clone());
        let api = self.api.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { api.list(&path) })
                .await;
            this.update(cx, |this, cx| {
                let current = this
                    .tabs
                    .iter_mut()
                    .find(|pane| pane.id == pane_id && pane.generation == generation);
                if let Some(pane) = current {
                    pane.finish_load(result);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn apply_preferences(&mut self, cx: &mut Context<Self>) {
        let sort = preference_sort(&self.preferences);
        let view = preference_view(&self.preferences);
        let show_hidden_files = self.preferences.show_hidden_files;
        let hidden_changed = self
            .pane_mut()
            .apply_preferences(show_hidden_files, sort, view);
        self.preferences.save();
        if hidden_changed {
            self.load_active(cx);
        }
        cx.notify();
    }

    pub(crate) fn navigate(&mut self, path: String, cx: &mut Context<Self>) {
        if self.pane_mut().navigate(path) {
            self.load_active(cx);
        }
        cx.notify();
    }

    pub(crate) fn back(&mut self, _: &Back, _: &mut Window, cx: &mut Context<Self>) {
        if self.pane_mut().go_back() {
            self.load_active(cx);
            cx.notify();
        }
    }

    pub(crate) fn forward(&mut self, _: &Forward, _: &mut Window, cx: &mut Context<Self>) {
        if self.pane_mut().go_forward() {
            self.load_active(cx);
            cx.notify();
        }
    }

    pub(crate) fn refresh(&mut self, _: &Refresh, _: &mut Window, cx: &mut Context<Self>) {
        if self.pane().is_at_root() {
            self.load_drives(cx);
        } else {
            self.load_active(cx);
        }
        cx.notify();
    }

    pub(crate) fn new_tab(&mut self, _: &NewTab, _: &mut Window, cx: &mut Context<Self>) {
        self.tabs.push(Pane::new(
            self.next_pane_id,
            String::new(),
            self.preferences.show_hidden_files,
            preference_sort(&self.preferences),
            preference_view(&self.preferences),
        ));
        self.next_pane_id += 1;
        self.active_tab = self.tabs.len() - 1;
        cx.notify();
    }

    pub(crate) fn close_tab_at(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.remove(index);
        if self.active_tab > index || self.active_tab >= self.tabs.len() {
            self.active_tab = self.active_tab.saturating_sub(1);
        }
        cx.notify();
    }

    fn close_tab(&mut self, _: &CloseTab, _: &mut Window, cx: &mut Context<Self>) {
        self.close_tab_at(self.active_tab, cx);
    }

    pub(crate) fn activate_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        self.active_tab = index;
        cx.notify();
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.pane_mut().select_all();
        cx.notify();
    }

    fn clear_selection(&mut self, _: &ClearSelection, _: &mut Window, cx: &mut Context<Self>) {
        self.pane_mut().clear_selection();
        cx.notify();
    }

    pub(crate) fn open_settings(&mut self, _: &OpenSettings, _: &mut Window, cx: &mut Context<Self>) {
        self.settings_open = true;
        self.settings_section = SettingsSection::General;
        cx.notify();
    }

    pub(crate) fn close_settings(&mut self, _: &CloseSettings, _: &mut Window, cx: &mut Context<Self>) {
        self.settings_open = false;
        cx.notify();
    }

    pub(crate) fn commit_preferences(&mut self, cx: &mut Context<Self>) {
        self.apply_preferences(cx);
    }

    pub(crate) fn select_settings_section(&mut self, section: SettingsSection, cx: &mut Context<Self>) {
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
        div()
            .key_context("BrowseWorkspace")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::back))
            .on_action(cx.listener(Self::forward))
            .on_action(cx.listener(Self::refresh))
            .on_action(cx.listener(Self::new_tab))
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::clear_selection))
            .on_action(cx.listener(Self::open_settings))
            .on_action(cx.listener(Self::close_settings))
            .size_full()
            .flex()
            .bg(c(SURFACE_PRIMARY))
            .text_color(c(TEXT_PRIMARY))
            .font_family(self.fonts.sans.clone())
            // The page's body line-height; Tailwind sizes like text-xs override it.
            .line_height(relative(1.5))
            .text_size(px(16.))
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(self.render_tabs(cx))
                    .child(self.render_browser(window, cx)),
            )
            .when(self.settings_open, |root| root.child(self.render_settings(cx)))
    }
}
