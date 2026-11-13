//! `KeyboardShortcutsDialog.svelte`, listing the keys this app binds in
//! `workspace::bind_keys`.

use gpui::{div, prelude::*, px, AnyElement, Context, FontWeight, MouseButton};

use super::TextScale;
use crate::components::{Modal, ModalSize};
use crate::theme::*;
use crate::workspace::Workspace;

const GROUPS: [(&str, &[(&str, &str)]); 3] = [
    (
        "Files and selection",
        &[
            ("Arrow keys", "Move and select"),
            ("Shift + Up · Down", "Extend selection"),
            ("Ctrl + A", "Select all"),
            ("Escape", "Clear selection or close a dialog"),
            ("Enter", "Open selected item"),
            ("Space", "Preview"),
            ("Alt + Enter", "Show selection details"),
            ("Ctrl + C · X · V", "Copy · Cut · Paste"),
            ("Ctrl + Shift + N · E", "New folder · New file"),
            ("F2", "Rename selected item"),
            ("Delete", "Trash locally; delete on a server"),
            ("Ctrl + B", "Pin to Favorites"),
        ],
    ),
    (
        "Browse",
        &[
            ("Ctrl + L", "Edit location"),
            ("Ctrl + F", "Search this folder"),
            ("Alt + Left · Right · Up", "Back · Forward · Parent folder"),
            ("F5", "Refresh"),
            ("Ctrl + H", "Show or hide hidden files"),
            ("Ctrl + T · W", "New tab · Close tab"),
            ("Ctrl + Tab", "Next tab"),
            ("Ctrl + Shift + Tab", "Previous tab"),
            ("F3", "Toggle split pane"),
            ("Ctrl + ,", "Settings"),
            ("F1", "Show keyboard shortcuts"),
        ],
    ),
    ("Desktop", &[("Ctrl + Z", "Undo the last rename or move to Trash"), ("Ctrl + Shift + O", "Servers")]),
];

impl Workspace {
    pub(crate) fn render_shortcuts(&self, cx: &mut Context<Self>) -> AnyElement {
        // grid gap-6 text-xs sm:grid-cols-2; each group a captioned table
        let group = |(title, rows): (&'static str, &'static [(&'static str, &'static str)])| {
            div()
                .flex()
                .flex_col()
                .child(div().pb(px(6.)).text_13().font_weight(FontWeight::MEDIUM).text_color(c(TEXT_PRIMARY)).child(title))
                .children(rows.iter().map(|(keys, description)| {
                    div()
                        .flex()
                        .border_t_1()
                        .border_color(c(BORDER_SECONDARY))
                        .py(px(6.))
                        .text_xs_tw()
                        .child(div().w(px(150.)).flex_none().pr(px(12.)).text_color(c(TEXT_PRIMARY)).child(*keys))
                        .child(div().flex_1().text_color(c(TEXT_SECONDARY)).child(*description))
                }))
        };
        let [files, browse, desktop] = GROUPS;
        div()
            .id("shortcuts-overlay")
            .absolute()
            .inset_0()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Modal::new()
                    .title("Keyboard shortcuts")
                    .size(ModalSize::Lg)
                    .on_close(cx.listener(|this, _, _, cx| {
                        this.shortcuts_open = false;
                        cx.notify();
                    }))
                    .body(
                        div()
                            .flex()
                            .gap(px(24.))
                            .child(div().flex_1().child(group(files)))
                            .child(div().flex_1().flex().flex_col().gap(px(24.)).child(group(browse)).child(group(desktop))),
                    ),
            )
            .into_any_element()
    }
}
