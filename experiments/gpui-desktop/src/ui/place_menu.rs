//! The place menu: for a breadcrumb or sidebar folder, Trash, or hidden crumbs.

use gpui::{anchored, div, prelude::*, AnyElement, Context, MouseButton};

use crate::components::{ContextMenu, MenuItem};
use crate::workspace::place_menu::Place;
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn render_place_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(menu) = &self.place_menu else { return div().into_any_element() };
        let item = |id: &'static str, label: &'static str, icon: &'static str, run: Box<dyn Fn(&mut Workspace, &mut Context<Workspace>)>, cx: &mut Context<Self>| {
            MenuItem::action(id, label).icon(icon).on_select(cx.listener(move |this, _, _, cx| {
                this.place_menu = None;
                run(this, cx);
                cx.notify();
            }))
        };
        let items = match &menu.place {
            Place::Folder { pane, path } => {
                let (pane, path) = (*pane, path.clone());
                let (tab_path, paste_path, properties_path) = (path.clone(), path.clone(), path.clone());
                let local = self.pane_by_id(pane.0).is_some_and(|pane| pane.api.is_local());
                vec![
                    item("place-new-tab", "Open in New Tab", "plus", Box::new(move |this, cx| this.open_in_new_tab(pane, tab_path.clone(), cx)), cx),
                    item("place-paste", "Paste Into Folder", "clipboard-paste", Box::new(move |this, cx| this.paste_into_folder(pane, paste_path.clone(), cx)), cx)
                        .disabled(path.is_empty()),
                    MenuItem::Separator,
                    item("place-properties", "Properties", "info", Box::new(move |this, cx| this.folder_properties(pane, properties_path.clone(), cx)), cx)
                        .disabled(path.is_empty() || !local),
                ]
            }
            Place::Trash => vec![
                item("place-open", "Open", "folder-open", Box::new(|this, cx| this.open_trash(cx)), cx),
                MenuItem::Separator,
                item("place-empty-trash", "Empty Trash", "trash-2", Box::new(|this, cx| this.empty_trash(cx)), cx).disabled(!self.trash_full),
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
