//! `FileContextMenu.svelte` (items from `utils/fileContextMenu.ts`).

use gpui::{anchored, div, prelude::*, AnyElement, Context, MouseButton, Window};

use crate::components::{ContextMenu, MenuItem};
use crate::workspace::Workspace;

type Action = fn(&mut Workspace, &mut Window, &mut Context<Workspace>);

impl Workspace {
    fn menu_item(&self, id: &'static str, label: &'static str, icon: &'static str, action: Action, cx: &mut Context<Self>) -> MenuItem {
        MenuItem::action(id, label).icon(icon).on_select(cx.listener(move |this, _, window, cx| {
            // Actions read the menu's selection, so it closes afterwards.
            action(this, window, cx);
            this.context_menu = None;
            cx.notify();
        }))
    }

    /// Keeps which submenu the pointer opened, for the next frame.
    pub(crate) fn submenu_listener(&self, cx: &mut Context<Self>) -> impl Fn(Option<gpui::SharedString>, &mut Window, &mut gpui::App) + 'static {
        let workspace = cx.entity().downgrade();
        move |id, _, cx| {
            workspace
                .update(cx, |this, cx| {
                    if this.open_submenu != id {
                        this.open_submenu = id;
                        cx.notify();
                    }
                })
                .ok();
        }
    }

    pub(crate) fn render_context_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(menu) = &self.context_menu else { return div().into_any_element() };
        let Some(pane) = self.pane_by_id(menu.target_id) else { return div().into_any_element() };
        let local = pane.api.is_local();
        let read_only = self.read_only(crate::workspace::operations::PaneTarget(pane.id)) || self.operation_status.is_busy();
        let can_create = !pane.path.is_empty();
        let can_paste = self.file_clipboard.is_some();
        let items = &menu.items;
        let items_empty = items.is_empty();
        let single = items.len() == 1;
        let single_folder = single && items[0].is_dir;
        let single_file = single && !items[0].is_dir;
        #[cfg(target_os = "linux")]
        if menu.extras.pending() {
            return div().into_any_element();
        }
        #[cfg(windows)]
        if menu.windows.pending() {
            return div().into_any_element();
        }

        // Opening comes first, as in Thunar and Nautilus.
        let mut entries = Vec::new();
        if !items_empty {
            if single_folder {
                entries.push(self.menu_item("context-open", "Open", "folder-open", |this, _, cx| {
                    if let Some((target, mut items)) = this.context_selection() {
                        this.activate_item(target, items.remove(0), cx);
                    }
                }, cx));
            }
            #[cfg(target_os = "linux")]
            if single_file && local {
                entries.extend(self.open_with_entries(&menu.extras, cx));
            }
            // The Shell's own `openas` row is left out of its commands for this one.
            #[cfg(windows)]
            if single_file && local {
                entries.push(self.menu_item("context-open-with", "Open With…", "app-window", |this, window, cx| this.context_open_with(window, cx), cx));
            }
            let folders = items.iter().filter(|item| item.is_dir).count();
            if folders > 0 {
                entries.push(self.menu_item(
                    "context-new-tab",
                    if folders == 1 { "Open in New Tab" } else { "Open in New Tabs" },
                    "plus",
                    |this, _, cx| {
                        if let Some((target, items)) = this.context_selection() {
                            this.open_folders_in_tabs(target, items, cx);
                        }
                    },
                    cx,
                ));
            }
        }
        // Here, or in the open folder for empty space when it is on this computer.
        let terminal = local && (single_folder || (items_empty && pane.real_path.is_some()));
        if terminal {
            entries.push(self.menu_item("context-terminal", "Open in Terminal", "square-terminal", |this, _, cx| this.context_open_terminal(cx), cx));
        }
        if !entries.is_empty() {
            entries.push(MenuItem::Separator);
        }
        entries.extend([
            self.menu_item("context-file", "New File", "file-plus", |this, window, cx| this.context_create_file(window, cx), cx)
                .disabled(read_only || !can_create),
            self.menu_item("context-folder", "New Folder", "folder-plus", |this, window, cx| this.context_create_folder(window, cx), cx)
                .disabled(read_only || !can_create),
        ]);
        if items_empty {
            entries.extend(self.template_menu(crate::workspace::operations::PaneTarget(pane.id), |this| this.context_menu = None, cx));
        }
        entries.push(MenuItem::Separator);
        let paste = self
            .menu_item("context-paste", "Paste", "clipboard-paste", |this, window, cx| this.context_paste(window, cx), cx)
            .shortcut("Ctrl+V")
            .disabled(read_only || !can_paste);
        #[cfg(target_os = "linux")]
        let actions = self.custom_action_entries(&menu.extras, terminal, cx);
        #[cfg(not(target_os = "linux"))]
        let actions: Vec<MenuItem> = Vec::new();
        if items.is_empty() {
            entries.push(paste);
            if !actions.is_empty() {
                entries.push(MenuItem::Separator);
                entries.extend(actions);
            }
        } else {
            entries.push(self.menu_item("context-copy", "Copy", "copy", |this, _, cx| this.context_copy(cx), cx).shortcut("Ctrl+C"));
            entries.push(
                self.menu_item("context-cut", "Cut", "scissors", |this, _, cx| this.context_cut(cx), cx).shortcut("Ctrl+X").disabled(read_only),
            );
            entries.push(paste);
            if single_folder {
                let pinned = self.context_favorite_index().is_some();
                entries.push(self.menu_item(
                    "context-favorite",
                    if pinned { "Unpin from Favorites" } else { "Pin to Favorites" },
                    if pinned { "pin-off" } else { "pin" },
                    |this, _, cx| this.context_toggle_favorite(cx),
                    cx,
                ));
            }
            if local {
                entries.push(MenuItem::Separator);
                entries.push(
                    self.menu_item("context-compress", "Compress", "archive", |this, _, cx| {
                        if let Some((target, items)) = this.context_selection() {
                            this.compress(target, items, cx);
                        }
                    }, cx)
                    .disabled(read_only),
                );
                entries.extend(self.send_to_menu(crate::workspace::operations::PaneTarget(pane.id), items, cx));
                entries.extend(actions);
            }
            entries.push(MenuItem::Separator);
            entries.push(
                self.menu_item("context-rename", "Rename", "pencil", |this, window, cx| this.context_rename(window, cx), cx)
                    .shortcut("F2")
                    .disabled(read_only || !single),
            );
            entries.push(
                self.menu_item(
                    "context-delete",
                    if local { "Move to Trash" } else { "Delete" },
                    "trash-2",
                    |this, window, cx| this.context_delete(window, cx),
                    cx,
                )
                .shortcut("Del")
                .disabled(read_only),
            );
            entries.push(MenuItem::Separator);
            if !local {
                let has_folder = items.iter().any(|item| item.is_dir);
                entries.push(
                    self.menu_item(
                        "context-download",
                        if single_folder { "Download as ZIP" } else { "Download" },
                        "download",
                        |this, _, cx| this.context_download(cx),
                        cx,
                    )
                    .disabled(has_folder && !single_folder),
                );
                entries.push(
                    self.menu_item(
                        "context-share",
                        if single_file { "Share file…" } else { "Share folder…" },
                        "share-2",
                        |this, window, cx| this.context_share(window, cx),
                        cx,
                    )
                    .disabled(!single),
                );
            }
            entries.push(self.menu_item("context-properties", "Properties", "info", |this, _, cx| this.context_properties(cx), cx).shortcut("Alt+Enter"));
        }
        // The Shell's commands come first; a Shell submenu replaces BoxBox's rows while open.
        #[cfg(windows)]
        let entries = {
            let mut windows = self.windows_context_entries(&menu.windows, cx);
            if windows.is_empty() {
                entries
            } else {
                if menu.windows.depth() == 0 {
                    windows.push(MenuItem::Separator);
                    windows.extend(entries);
                }
                windows
            }
        };
        let panel = ContextMenu::new(entries).on_submenu(self.submenu_listener(cx));
        div()
            .id("context-dismiss")
            .occlude()
            .absolute()
            .inset_0()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.context_menu = None;
                cx.notify();
            }))
            .on_mouse_down(MouseButton::Right, cx.listener(|this, _, _, cx| {
                this.context_menu = None;
                cx.notify();
            }))
            .child(
                anchored().position(menu.position).snap_to_window_with_margin(crate::components::MENU_MARGIN).child(
                    div()
                        .id("file-context-menu")
                        .debug_selector(|| "file-context-menu".into())
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(panel),
                ),
            )
            .into_any_element()
    }
}

