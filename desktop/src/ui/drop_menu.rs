//! The menu an Alt-drop shows at the drop point: Move Here, Copy Here, Link
//! Here, or Cancel, as GTK file managers ask.

use gpui::{anchored, div, prelude::*, AnyElement, Context, MouseButton};

use crate::components::{ContextMenu, MenuItem};
use crate::workspace::operations::ClipboardMode;
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn render_drop_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(menu) = &self.drop_menu else { return div().into_any_element() };
        let item = |id: &'static str, label: &'static str, icon: &'static str, choice: Option<ClipboardMode>, link: bool, cx: &mut Context<Self>| {
            MenuItem::action(id, label).icon(icon).on_select(cx.listener(move |this, _, _, cx| this.choose_drop(choice, link, cx)))
        };
        let entries = vec![
            item("drop-move", "Move Here", "folder-input", Some(ClipboardMode::Cut), false, cx).disabled(!menu.can_move),
            item("drop-copy", "Copy Here", "copy", Some(ClipboardMode::Copy), false, cx),
            item("drop-link", "Link Here", "link", None, true, cx).disabled(!menu.can_link),
            MenuItem::Separator,
            item("drop-cancel", "Cancel", "x", None, false, cx),
        ];
        let cancel = |this: &mut Self, cx: &mut Context<Self>| this.choose_drop(None, false, cx);
        div()
            .absolute()
            .inset_0()
            .id("drop-menu-dismiss")
            .occlude()
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| cancel(this, cx)))
            .on_mouse_down(MouseButton::Right, cx.listener(move |this, _, _, cx| cancel(this, cx)))
            .child(
                anchored().position(menu.position).snap_to_window().child(
                    div()
                        .id("drop-menu")
                        .debug_selector(|| "drop-menu".into())
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(ContextMenu::new(entries)),
                ),
            )
            .into_any_element()
    }
}
