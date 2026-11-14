//! The place menu: for a breadcrumb or sidebar folder, Trash, or hidden crumbs.

use gpui::{anchored, div, prelude::*, AnyElement, Context, MouseButton, Window};

use crate::components::{ContextMenu, MenuItem};
use crate::workspace::operations::PaneTarget;
use crate::workspace::place_menu::Place;
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn render_place_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(menu) = &self.place_menu else { return div().into_any_element() };
        type Run = Box<dyn Fn(&mut Workspace, &mut Window, &mut Context<Workspace>)>;
        let item = |id: &'static str, label: &'static str, icon: &'static str, run: Run, cx: &mut Context<Self>| {
            MenuItem::action(id, label).icon(icon).on_select(cx.listener(move |this, _, window, cx| {
                this.place_menu = None;
                run(this, window, cx);
                cx.notify();
            }))
        };
        // Open in New Tab, Paste Into Folder and Properties, for a folder of `pane`.
        let folder_items = |pane: PaneTarget, path: String, cx: &mut Context<Self>| -> Vec<MenuItem> {
            let listing = path.is_empty() || path == crate::local::RECENT || path == crate::local::NETWORK;
            let local = self.pane_by_id(pane.0).is_some_and(|pane| pane.api.is_local());
            let (tab, paste, properties) = (path.clone(), path.clone(), path);
            vec![
                item("place-new-tab", "Open in New Tab", "plus", Box::new(move |this, _, cx| this.open_in_new_tab(pane, tab.clone(), cx)), cx),
                item("place-paste", "Paste Into Folder", "clipboard-paste", Box::new(move |this, _, cx| this.paste_into_folder(pane, paste.clone(), cx)), cx).disabled(listing),
                MenuItem::Separator,
                item("place-properties", "Properties", "info", Box::new(move |this, _, cx| this.folder_properties(pane, properties.clone(), cx)), cx).disabled(listing || !local),
            ]
        };
        let items = match &menu.place {
            Place::Folder { pane, path } => folder_items(*pane, path.clone(), cx),
            Place::Favorite(index) => {
                let index = *index;
                let path = self.favorites.entries.get(index).map(|favorite| favorite.path.clone()).unwrap_or_default();
                // A bookmark of a network address opens by mounting it, from the row.
                let mut items = match path.contains("://") {
                    true => Vec::new(),
                    false => {
                        let mut items = folder_items(self.active_pane, path, cx);
                        items.push(MenuItem::Separator);
                        items
                    }
                };
                items.push(item("place-unpin", "Remove from Favorites", "pin-off", Box::new(move |this, _, cx| this.remove_favorite(index, cx)), cx));
                items
            }
            Place::Device(volume) => {
                let root = volume.mount_point().and_then(|point| self.pane().api.virtual_path(point));
                let mut items = match root {
                    Some(root) => folder_items(self.active_pane, root, cx),
                    None => {
                        let volume = volume.clone();
                        vec![item("place-mount", "Mount", "hard-drive", Box::new(move |this, window, cx| this.open_volume(volume.clone(), window, cx)), cx)]
                    }
                };
                if volume.removable() {
                    let eject = volume.clone();
                    items.push(MenuItem::Separator);
                    items.push(item(
                        "place-eject",
                        if volume.ejectable { "Eject" } else { "Unmount" },
                        "arrow-up-from-line",
                        Box::new(move |this, _, cx| this.eject_volume(eject.clone(), cx)),
                        cx,
                    ));
                }
                items
            }
            Place::Trash => vec![
                item("place-open", "Open", "folder-open", Box::new(|this, _, cx| this.open_trash(cx)), cx),
                MenuItem::Separator,
                item("place-empty-trash", "Empty Trash", "trash-2", Box::new(|this, _, cx| this.empty_trash(cx)), cx).disabled(!self.trash_full),
            ],
            Place::Hidden { pane, crumbs } => crumbs
                .iter()
                .enumerate()
                .map(|(index, (label, path))| {
                    let (pane, path) = (*pane, path.clone());
                    MenuItem::action(("hidden-crumb", index), label.clone()).icon("folder").on_select(cx.listener(move |this, _, _, cx| {
                        this.place_menu = None;
                        this.navigate_in(pane, path.clone(), cx);
                    }))
                })
                .collect(),
        };
        let close = |this: &mut Self, cx: &mut Context<Self>| {
            this.place_menu = None;
            cx.notify();
        };
        div()
            .absolute()
            .inset_0()
            .id("place-menu-dismiss")
            .occlude()
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| close(this, cx)))
            .on_mouse_down(MouseButton::Right, cx.listener(move |this, _, _, cx| close(this, cx)))
            .child(
                anchored().position(menu.position).snap_to_window().child(
                    div()
                        .id("place-menu")
                        .debug_selector(|| "place-menu".into())
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(ContextMenu::new(items)),
                ),
            )
            .into_any_element()
    }
}
