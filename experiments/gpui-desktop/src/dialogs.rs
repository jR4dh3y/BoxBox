//! `BrowseDialogs.svelte`: create, rename, delete and properties dialogs.

use gpui::{div, prelude::*, px, AnyElement, Context, Entity, Focusable, Subscription, Window};

use crate::api::FileInfo;
use crate::format::{file_date, file_size, type_description};
use crate::theme::*;
use crate::ui::modal::{button, input_frame, modal, ButtonKind};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::TextScale;
use crate::workspace::Workspace;

pub enum Dialog {
    Create {
        directory: bool,
        dir: String,
        input: Entity<TextInput>,
        _events: Subscription,
    },
    Rename {
        item: FileInfo,
        input: Entity<TextInput>,
        _events: Subscription,
    },
    Delete {
        items: Vec<FileInfo>,
        to_trash: bool,
    },
    Properties(FileInfo),
    OpenWith(crate::desktop::OpenWith),
    Settings,
    Conflict {
        op: crate::clipboard::ClipOp,
        dest: String,
        planned: Vec<crate::clipboard::Planned>,
        keep_both: Vec<bool>,
    },
}

/// The name without its extension, which rename and new-file dialogs preselect.
pub fn basename_len(name: &str, is_dir: bool) -> usize {
    match name.rfind('.') {
        Some(dot) if !is_dir && dot > 0 => dot,
        _ => name.len(),
    }
}

/// Mirrors the server's rule: one path segment, not `.` or `..`.
pub fn validate_name(name: &str) -> Result<&str, &'static str> {
    let name = name.trim();
    match name {
        "" => Err("Enter a name"),
        "." | ".." => Err("That name is reserved"),
        _ if name.contains('/') || name.contains('\0') => Err("Names cannot contain /"),
        _ => Ok(name),
    }
}

impl Workspace {
    fn text_dialog_input(
        &mut self,
        placeholder: &str,
        text: &str,
        selection: std::ops::Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<TextInput>, Subscription) {
        let input = cx.new(|cx| {
            let mut input = TextInput::new(placeholder.to_string(), cx);
            input.set_text(text.to_string(), selection, cx);
            input
        });
        let events = cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
            InputEvent::Submit => this.confirm_dialog(window, cx),
            InputEvent::Cancel => this.close_dialog(window, cx),
            InputEvent::Changed => {}
        });
        window.focus(&input.focus_handle(cx));
        (input, events)
    }

    pub(crate) fn open_create(
        &mut self,
        directory: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dir = self.pane().path.clone();
        if !self.can_write_here() {
            return;
        }
        let (placeholder, name) = if directory {
            ("New folder", "New folder")
        } else {
            ("untitled.txt", "untitled.txt")
        };
        let selection = 0..basename_len(name, directory);
        let (input, events) = self.text_dialog_input(placeholder, name, selection, window, cx);
        self.dialog = Some(Dialog::Create {
            directory,
            dir,
            input,
            _events: events,
        });
        cx.notify();
    }

    pub(crate) fn open_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut items = self.pane().selected_items();
        if items.len() != 1 {
            return;
        }
        let item = items.remove(0);
        let selection = 0..basename_len(&item.name, item.is_dir);
        let (input, events) = self.text_dialog_input("", &item.name.clone(), selection, window, cx);
        self.dialog = Some(Dialog::Rename {
            item,
            input,
            _events: events,
        });
        cx.notify();
    }

    pub(crate) fn open_delete(&mut self, to_trash: bool, cx: &mut Context<Self>) {
        let items = self.pane().selected_items();
        if items.is_empty() || !self.can_write_here() {
            return;
        }
        if self.preferences.confirm_delete || !to_trash {
            self.dialog = Some(Dialog::Delete { items, to_trash });
            cx.notify();
        } else {
            self.delete_items(items, true, cx);
        }
    }

    pub(crate) fn open_properties(&mut self, cx: &mut Context<Self>) {
        if let Some(item) = self.pane().selected_items().into_iter().next() {
            self.dialog = Some(Dialog::Properties(item));
            cx.notify();
        }
    }

    pub(crate) fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog = None;
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub(crate) fn confirm_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A bad name keeps the dialog open.
        if let Some(Dialog::Create { input, .. } | Dialog::Rename { input, .. }) = &self.dialog {
            if let Err(message) = validate_name(input.read(cx).text()) {
                self.toast(crate::notifications::ToastKind::Error, message, cx);
                return;
            }
        }
        match self.dialog.take() {
            Some(Dialog::Create {
                directory,
                dir,
                input,
                ..
            }) => {
                let name = validate_name(input.read(cx).text())
                    .unwrap_or_default()
                    .to_string();
                self.create_item(dir, name, directory, cx);
            }
            Some(Dialog::Rename { item, input, .. }) => {
                let name = validate_name(input.read(cx).text())
                    .unwrap_or_default()
                    .to_string();
                if name != item.name {
                    self.rename_item(item, name, cx);
                }
            }
            Some(Dialog::Delete { items, to_trash }) => self.delete_items(items, to_trash, cx),
            Some(Dialog::Conflict {
                op,
                dest,
                planned,
                keep_both,
            }) => {
                let planned = crate::clipboard::resolve(planned, &keep_both);
                if !planned.is_empty() {
                    self.submit_paste(op, dest, planned, cx);
                }
            }
            Some(dialog @ Dialog::OpenWith(_)) => {
                self.dialog = Some(dialog);
                self.confirm_open_with(cx);
                return;
            }
            Some(Dialog::Properties(_) | Dialog::Settings) | None => {}
        }
        self.close_dialog(window, cx);
    }

    pub(crate) fn render_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dialog = self.dialog.as_ref()?;
        let on_close = {
            let this = cx.entity().downgrade();
            move |window: &mut Window, cx: &mut gpui::App| {
                this.update(cx, |this, cx| this.close_dialog(window, cx))
                    .ok();
            }
        };
        let confirm = |id: &'static str, label: &'static str, kind: ButtonKind| {
            let this = cx.entity().downgrade();
            button(id, label, kind, move |_, window, cx| {
                this.update(cx, |this, cx| this.confirm_dialog(window, cx))
                    .ok();
            })
            .into_any_element()
        };
        let element =
            match dialog {
                Dialog::Create {
                    directory, input, ..
                } => modal(
                    if *directory { "New folder" } else { "New file" },
                    448.,
                    input_frame(input.clone()).into_any_element(),
                    vec![confirm("create", "Create", ButtonKind::Primary)],
                    on_close,
                )
                .into_any_element(),
                Dialog::Rename { input, .. } => modal(
                    "Rename",
                    448.,
                    input_frame(input.clone()).into_any_element(),
                    vec![confirm("rename", "Rename", ButtonKind::Primary)],
                    on_close,
                )
                .into_any_element(),
                Dialog::Delete { items, to_trash } => {
                    let verb = if *to_trash {
                        "will be moved to Trash."
                    } else {
                        "will be deleted permanently."
                    };
                    let body = match items.as_slice() {
                        [item] => div().text_13().text_color(c(TEXT_SECONDARY)).child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(px(4.))
                                .child(div().text_color(c(TEXT_PRIMARY)).child(item.name.clone()))
                                .child(verb),
                        ),
                        items => div()
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .text_13()
                            .text_color(c(TEXT_SECONDARY))
                            .child(format!("{} items {verb}", items.len()))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .max_h(px(160.))
                                    .overflow_hidden()
                                    .children(items.iter().map(|item| {
                                        div()
                                            .py(px(2.))
                                            .truncate()
                                            .text_color(c(TEXT_PRIMARY))
                                            .child(item.name.clone())
                                    })),
                            ),
                    };
                    let (title, label) = if *to_trash {
                        ("Move to Trash", "Move to Trash")
                    } else {
                        ("Delete", "Delete")
                    };
                    modal(
                        title,
                        448.,
                        body.into_any_element(),
                        vec![confirm("delete", label, ButtonKind::Danger)],
                        on_close,
                    )
                    .into_any_element()
                }
                Dialog::OpenWith(dialog) => self.render_open_with(dialog, cx, on_close),
                Dialog::Settings => self.render_settings(cx, on_close),
                Dialog::Conflict {
                    op,
                    dest,
                    planned,
                    keep_both,
                } => self.render_conflict(*op, dest, planned, keep_both, cx, on_close),
                Dialog::Properties(item) => {
                    let location = item
                        .path
                        .rsplit_once('/')
                        .map_or("/", |(parent, _)| parent)
                        .to_string();
                    let mut rows: Vec<(&str, String)> = vec![
                        ("Name", item.name.clone()),
                        (
                            "Type",
                            if item.is_dir {
                                "Folder".into()
                            } else {
                                type_description(&item.name).to_string()
                            },
                        ),
                        ("Location", location),
                    ];
                    if !item.is_dir {
                        rows.push(("Size", file_size(item.size).to_string()));
                    }
                    rows.push(("Modified", file_date(&item.mod_time).to_string()));
                    rows.push(("Permissions", item.permissions.clone()));
                    let body = div().flex().flex_col().gap(px(10.)).text_13().children(
                        rows.into_iter().map(|(label, value)| {
                            div()
                                .flex()
                                .gap(px(16.))
                                .child(
                                    div()
                                        .w(px(88.))
                                        .flex_none()
                                        .text_color(c(TEXT_SECONDARY))
                                        .child(label),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_color(c(TEXT_PRIMARY))
                                        .child(value),
                                )
                        }),
                    );
                    modal(
                        "Properties",
                        448.,
                        body.into_any_element(),
                        Vec::new(),
                        on_close,
                    )
                    .into_any_element()
                }
            };
        Some(element)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basename_skips_extension_for_files_only() {
        assert_eq!(basename_len("report.final.pdf", false), 12);
        assert_eq!(basename_len(".bashrc", false), 7);
        assert_eq!(basename_len("photos.2024", true), 11);
    }

    #[test]
    fn names_are_single_segments() {
        assert_eq!(validate_name("  a.txt "), Ok("a.txt"));
        for bad in ["", " ", ".", "..", "a/b"] {
            assert!(validate_name(bad).is_err(), "{bad:?}");
        }
    }
}
