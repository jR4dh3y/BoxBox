//! Root view: tabs of panes plus the drive data shared by the sidebar and This PC.
//! Mirrors `lib/components/desktop/BrowseWorkspace.svelte`.

use std::sync::Arc;

use gpui::{
    actions, div, prelude::*, px, relative, App, Context, FocusHandle, KeyBinding, SharedString,
    Window,
};

use crate::api::{Api, DriveStats, MountPoint};
use crate::pane::Pane;
use crate::theme::{c, SURFACE_PRIMARY, TEXT_PRIMARY};

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
        NewFolder,
        NewFile,
        Rename,
        MoveToTrash,
        DeletePermanently,
        Properties,
        Undo,
        Copy,
        Cut,
        Paste,
        FocusSearch,
        ToggleSplit,
        NextTab,
        PreviousTab,
        GoUp,
        ToggleHidden,
        ClosePreview,
        PreviewPrevious,
        PreviewNext,
        PreviewSelection,
        OpenSelection,
        ShowShortcuts,
        EditLocation
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
        KeyBinding::new("ctrl-shift-n", NewFolder, context),
        KeyBinding::new("f2", Rename, context),
        KeyBinding::new("delete", MoveToTrash, context),
        KeyBinding::new("shift-delete", DeletePermanently, context),
        KeyBinding::new("alt-enter", Properties, context),
        KeyBinding::new("ctrl-z", Undo, context),
        KeyBinding::new("ctrl-c", Copy, context),
        KeyBinding::new("ctrl-x", Cut, context),
        KeyBinding::new("ctrl-v", Paste, context),
        KeyBinding::new("ctrl-f", FocusSearch, context),
        KeyBinding::new("f3", ToggleSplit, context),
        KeyBinding::new("ctrl-tab", NextTab, context),
        KeyBinding::new("ctrl-shift-tab", PreviousTab, context),
        KeyBinding::new("alt-up", GoUp, context),
        KeyBinding::new("ctrl-h", ToggleHidden, context),
        KeyBinding::new("space", PreviewSelection, context),
        KeyBinding::new("f1", ShowShortcuts, context),
        KeyBinding::new("ctrl-l", EditLocation, context),
        KeyBinding::new("enter", OpenSelection, context),
        KeyBinding::new("escape", ClosePreview, Some("Preview")),
        KeyBinding::new("left", PreviewPrevious, Some("Preview")),
        KeyBinding::new("right", PreviewNext, Some("Preview")),
    ]);
}

/// A tab shows one pane, or two side by side after F3 (`browse/tabs.svelte.ts`).
pub struct Tab {
    pub panes: Vec<Pane>,
    pub focused: usize,
}

impl Tab {
    fn new(pane: Pane) -> Self {
        Self {
            panes: vec![pane],
            focused: 0,
        }
    }
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
    pub(crate) dialog: Option<crate::dialogs::Dialog>,
    pub(crate) toasts: crate::notifications::Toasts,
    pub(crate) undo: Option<crate::undo::Undo>,
    pub(crate) preferences: crate::preferences::Preferences,
    pub(crate) clipboard: crate::clipboard::Clipboard,
    pub(crate) jobs: crate::jobs::Jobs,
    pub(crate) context_menu: Option<crate::context_menu::ContextMenu>,
    pub(crate) app_icons: crate::desktop::AppIcons,
    pub(crate) search_fields: std::collections::HashMap<usize, crate::search::SearchField>,
    pub(crate) watches: std::collections::HashMap<usize, crate::watch::Watch>,
    pub(crate) trash: crate::trash::TrashState,
    pub(crate) thumbs: crate::media::Thumbnails,
    pub(crate) preview: Option<crate::media::Preview>,
    pub(crate) volumes: Vec<crate::devices::Volume>,
    pub(crate) location: Option<crate::location::LocationEdit>,
    pub(crate) tabs: Vec<Tab>,
    pub(crate) active_tab: usize,
    next_pane_id: usize,
    pub(crate) roots: Vec<MountPoint>,
    pub(crate) drives: Load<Vec<DriveStats>>,
    pub(crate) fonts: Fonts,
    pub(crate) focus_handle: FocusHandle,
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
        let mut workspace = Self {
            api: Arc::new(api),
            dialog: None,
            toasts: Default::default(),
            undo: None,
            preferences: Default::default(),
            clipboard: Default::default(),
            jobs: Default::default(),
            context_menu: None,
            app_icons: Default::default(),
            search_fields: Default::default(),
            watches: Default::default(),
            trash: Default::default(),
            thumbs: Default::default(),
            preview: None,
            volumes: Vec::new(),
            location: None,
            tabs: vec![Tab::new(Pane::new(0, initial_path))],
            active_tab: 0,
            next_pane_id: 1,
            roots: Vec::new(),
            drives: Load::Loading,
            fonts,
            focus_handle,
        };
        let field = workspace.search_field(0, window, cx);
        workspace.search_fields.insert(0, field);
        workspace.load_drives(cx);
        workspace.watch_jobs(cx);
        workspace.load_preferences(cx);
        workspace.watch_volumes(cx);
        workspace.load_active(cx);
        workspace
    }

    /// The focused pane of the active tab: the target of actions and shortcuts.
    pub(crate) fn pane(&self) -> &Pane {
        let tab = &self.tabs[self.active_tab];
        &tab.panes[tab.focused]
    }

    pub(crate) fn pane_mut(&mut self) -> &mut Pane {
        let tab = &mut self.tabs[self.active_tab];
        &mut tab.panes[tab.focused]
    }

    /// Width of one pane: the window minus the sidebar, shared by split panes.
    pub(crate) fn pane_width(&self, window: &Window) -> f32 {
        let panes = self.tabs[self.active_tab].panes.len().max(1) as f32;
        (f32::from(window.viewport_size().width) - crate::ui::SIDEBAR_WIDTH) / panes
    }

    /// The pane a render pass draws; falls back to the focused one.
    pub(crate) fn view(&self, id: usize) -> &Pane {
        self.pane_by_id(id).unwrap_or_else(|| self.pane())
    }

    pub(crate) fn focus_pane(&mut self, id: usize, cx: &mut Context<Self>) {
        let tab = &mut self.tabs[self.active_tab];
        if let Some(index) = tab.panes.iter().position(|pane| pane.id == id) {
            if tab.focused != index {
                tab.focused = index;
                cx.notify();
            }
        }
    }

    /// F3: split off a right pane at the same folder, or close it and keep the left one.
    fn toggle_split(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tab = &mut self.tabs[self.active_tab];
        if tab.panes.len() > 1 {
            let closed: Vec<Pane> = tab.panes.drain(1..).collect();
            tab.focused = 0;
            for pane in closed {
                self.search_fields.remove(&pane.id);
                self.watches.remove(&pane.id);
            }
        } else {
            let path = tab.panes[0].path.clone();
            let pane = self.new_pane(path, window, cx);
            let tab = &mut self.tabs[self.active_tab];
            tab.panes.push(pane);
            tab.focused = 1;
            self.reload(cx);
        }
        cx.notify();
    }

    fn cycle_tab(&mut self, step: isize, cx: &mut Context<Self>) {
        let count = self.tabs.len() as isize;
        self.active_tab = (self.active_tab as isize + step).rem_euclid(count) as usize;
        cx.notify();
    }

    fn go_up(&mut self, cx: &mut Context<Self>) {
        let path = self.pane().path.clone();
        if !path.is_empty() && !path.starts_with('@') {
            let parent = path
                .rsplit_once('/')
                .map_or(String::new(), |(parent, _)| parent.to_string());
            self.navigate(parent, cx);
        }
    }

    /// Roots feed the sidebar and breadcrumbs; stats feed the This PC cards.
    pub(crate) fn load_drives(&mut self, cx: &mut Context<Self>) {
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

    /// Launches forwarded by later processes open as new tabs and raise the window.
    pub(crate) fn accept_launches(
        &mut self,
        mut launches: futures::channel::mpsc::UnboundedReceiver<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            while let Some(path) = futures::StreamExt::next(&mut launches).await {
                let opened = this.update_in(cx, |this, window, cx| {
                    this.open_tab(path, window, cx);
                    window.activate_window();
                });
                if opened.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    pub(crate) fn load_active(&mut self, cx: &mut Context<Self>) {
        let id = self.pane().id;
        self.load_pane(id, cx);
    }

    pub(crate) fn pane_by_id(&self, id: usize) -> Option<&Pane> {
        self.tabs
            .iter()
            .flat_map(|tab| &tab.panes)
            .find(|pane| pane.id == id)
    }

    pub(crate) fn pane_by_id_mut(&mut self, id: usize) -> Option<&mut Pane> {
        self.tabs
            .iter_mut()
            .flat_map(|tab| &mut tab.panes)
            .find(|pane| pane.id == id)
    }

    /// Lists the pane's folder, or runs its search; stale responses are dropped by generation.
    pub(crate) fn load_pane(&mut self, pane_id: usize, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_by_id_mut(pane_id) else {
            return;
        };
        // Bump first so a folder request still in flight cannot land on This PC.
        let generation = pane.begin_load();
        let (path, query) = (pane.path.clone(), pane.query.clone());
        if path.is_empty() || path == crate::trash::TRASH_PATH {
            self.watches.remove(&pane_id);
            if path == crate::trash::TRASH_PATH {
                self.trash.confirming = false;
                self.load_trash(cx);
            }
            return;
        }
        self.ensure_watch(pane_id, cx);
        let api = self.api.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if query.is_empty() {
                        api.list(&path)
                    } else {
                        api.search(&path, &query)
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                if let Some(pane) = this
                    .pane_by_id_mut(pane_id)
                    .filter(|pane| pane.generation == generation)
                {
                    pane.finish_load(result);
                    this.request_thumbnails(pane_id, cx);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn apply_preferences(
        &mut self,
        preferences: crate::preferences::Preferences,
        cx: &mut Context<Self>,
    ) {
        let hidden_changed = preferences.show_hidden != self.preferences.show_hidden;
        let defaults_changed = preferences.default_sort() != self.preferences.default_sort()
            || preferences.default_view() != self.preferences.default_view();
        self.preferences = preferences;
        let (sort, view) = (
            self.preferences.default_sort(),
            self.preferences.default_view(),
        );
        for pane in self.tabs.iter_mut().flat_map(|tab| &mut tab.panes) {
            pane.show_hidden = self.preferences.show_hidden;
            if defaults_changed {
                pane.set_sort(sort);
                pane.view_mode = view;
            }
        }
        if hidden_changed {
            self.reload(cx);
        }
        cx.notify();
    }

    pub(crate) fn new_pane(
        &mut self,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Pane {
        let mut pane = Pane::new(self.next_pane_id, path);
        self.next_pane_id += 1;
        pane.show_hidden = self.preferences.show_hidden;
        pane.set_sort(self.preferences.default_sort());
        pane.view_mode = self.preferences.default_view();
        let field = self.search_field(pane.id, window, cx);
        self.search_fields.insert(pane.id, field);
        pane
    }

    pub(crate) fn open_tab(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let pane = self.new_pane(path, window, cx);
        self.tabs.push(Tab::new(pane));
        self.active_tab = self.tabs.len() - 1;
        self.load_active(cx);
        cx.notify();
    }

    pub(crate) fn reload(&mut self, cx: &mut Context<Self>) {
        if self.pane().is_at_root() {
            self.load_drives(cx);
        } else {
            self.load_active(cx);
        }
        cx.notify();
    }

    pub(crate) fn reload_and_select(&mut self, path: String, cx: &mut Context<Self>) {
        self.pane_mut().pending_select = Some(path);
        self.reload(cx);
    }

    pub(crate) fn navigate(&mut self, path: String, cx: &mut Context<Self>) {
        if self.pane_mut().navigate(path) {
            let id = self.pane().id;
            self.clear_search_text(id, cx);
            self.load_active(cx);
        }
        cx.notify();
    }

    pub(crate) fn back(&mut self, _: &Back, _: &mut Window, cx: &mut Context<Self>) {
        if self.pane_mut().go_back() {
            let id = self.pane().id;
            self.clear_search_text(id, cx);
            self.load_active(cx);
            cx.notify();
        }
    }

    pub(crate) fn forward(&mut self, _: &Forward, _: &mut Window, cx: &mut Context<Self>) {
        if self.pane_mut().go_forward() {
            let id = self.pane().id;
            self.clear_search_text(id, cx);
            self.load_active(cx);
            cx.notify();
        }
    }

    pub(crate) fn refresh(&mut self, _: &Refresh, _: &mut Window, cx: &mut Context<Self>) {
        self.reload(cx);
    }

    pub(crate) fn new_tab(&mut self, _: &NewTab, window: &mut Window, cx: &mut Context<Self>) {
        self.open_tab(String::new(), window, cx);
    }

    pub(crate) fn close_tab_at(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            return;
        }
        let removed = self.tabs.remove(index);
        for pane in removed.panes {
            self.search_fields.remove(&pane.id);
            self.watches.remove(&pane.id);
        }
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
}

impl Workspace {
    fn render_panes(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = &self.tabs[self.active_tab];
        let split = tab.panes.len() > 1;
        let ids: Vec<(usize, bool)> = tab
            .panes
            .iter()
            .enumerate()
            .map(|(index, pane)| (pane.id, index == tab.focused))
            .collect();
        div()
            .flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .children(ids.into_iter().enumerate().map(|(index, (id, focused))| {
                // A distinct id keeps each pane's rows and lists apart in GPUI's element state.
                div()
                    .id(("pane", id))
                    .relative()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .when(index > 0, |pane| {
                        pane.border_l_1()
                            .border_color(c(crate::theme::BORDER_SECONDARY))
                    })
                    // Focus the pane before any row or button handler runs.
                    .capture_any_mouse_down(
                        cx.listener(move |this, _, _, cx| this.focus_pane(id, cx)),
                    )
                    .on_drop(cx.listener(
                        move |this, dragged: &crate::drag_out::DraggedItem, window, cx| {
                            let dest = this.view(id).path.clone();
                            this.drop_dragged(dragged, dest, window, cx);
                        },
                    ))
                    .on_drop(
                        cx.listener(move |this, paths: &gpui::ExternalPaths, _, cx| {
                            let dest = this.view(id).path.clone();
                            this.drop_external(paths, dest, cx);
                        }),
                    )
                    .child(self.render_browser(id, window, cx))
                    .when(split && focused, |pane| {
                        pane.child(
                            div()
                                .absolute()
                                .inset_0()
                                .border_1()
                                .border_color(crate::theme::alpha(crate::theme::ACCENT, 0.4)),
                        )
                    })
            }))
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
            .on_action(
                cx.listener(|this, _: &NewFolder, window, cx| this.open_create(true, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &NewFile, window, cx| this.open_create(false, window, cx)),
            )
            .on_action(cx.listener(|this, _: &Rename, window, cx| this.open_rename(window, cx)))
            .on_action(cx.listener(|this, _: &MoveToTrash, _, cx| this.open_delete(true, cx)))
            .on_action(
                cx.listener(|this, _: &DeletePermanently, _, cx| this.open_delete(false, cx)),
            )
            .on_action(cx.listener(|this, _: &Properties, _, cx| this.open_properties(cx)))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                this.copy_selection(crate::clipboard::ClipOp::Copy, cx)
            }))
            .on_action(cx.listener(|this, _: &Cut, _, cx| {
                this.copy_selection(crate::clipboard::ClipOp::Cut, cx)
            }))
            .on_action(cx.listener(|this, _: &Paste, _, cx| this.paste(cx)))
            .on_action(
                cx.listener(|this, _: &FocusSearch, window, cx| this.focus_search(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ToggleSplit, window, cx| this.toggle_split(window, cx)),
            )
            .on_action(cx.listener(|this, _: &NextTab, _, cx| this.cycle_tab(1, cx)))
            .on_action(cx.listener(|this, _: &PreviousTab, _, cx| this.cycle_tab(-1, cx)))
            .on_action(cx.listener(|this, _: &GoUp, _, cx| this.go_up(cx)))
            .on_action(cx.listener(|this, _: &ShowShortcuts, _, cx| {
                this.dialog = Some(crate::dialogs::Dialog::Shortcuts);
                cx.notify();
            }))
            .on_action(
                cx.listener(|this, _: &EditLocation, window, cx| this.edit_location(window, cx)),
            )
            .on_action(cx.listener(|this, _: &PreviewSelection, window, cx| {
                if let Some(item) = this
                    .pane()
                    .selected_items()
                    .into_iter()
                    .find(|item| !item.is_dir)
                {
                    this.open_preview(item, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &OpenSelection, _, cx| {
                if let [item] = this.pane().selected_items().as_slice() {
                    this.open_item(item.clone(), cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleHidden, _, cx| {
                this.update_preferences(|prefs| prefs.show_hidden = !prefs.show_hidden, cx)
            }))
            .relative()
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
                    .child(self.render_panes(window, cx)),
            )
            .child(
                div()
                    .absolute()
                    .right(px(16.))
                    .bottom(px(40.))
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(px(8.))
                    .children(self.render_jobs(cx))
                    .child(self.render_toasts(cx)),
            )
            .children(self.render_preview(cx))
            .children(self.render_context_menu(window, cx))
            .children(self.render_dialog(cx))
    }
}
