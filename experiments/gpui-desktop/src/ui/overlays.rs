//! `FileContextMenu.svelte` (items from `utils/fileContextMenu.ts`), the
//! properties dialog in `BrowseDialogs.svelte`, and `FilePreview.svelte`.

use gpui::{anchored, div, img, prelude::*, px, AnyElement, Context, MouseButton, ObjectFit, Window};

use super::TextScale;
use crate::components::{ContextMenu, MenuItem, Modal, ModalSize, Type};
use crate::format::{file_date, file_size, type_description};
use crate::workspace::properties::permission_string;
use crate::workspace::Load;
use crate::theme::*;
use crate::workspace::preview::PreviewContent;
use crate::workspace::{OperationEscape, Workspace};

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

    pub(crate) fn render_context_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(menu) = &self.context_menu else { return div().into_any_element() };
        let Some(pane) = self.pane_by_id(menu.target_id) else { return div().into_any_element() };
        let local = pane.api.is_local();
        let read_only = self.read_only(crate::workspace::operations::PaneTarget(pane.id)) || self.operation_status.is_busy();
        let can_create = !pane.path.is_empty();
        let can_paste = self.file_clipboard.is_some();
        let items = &menu.items;
        let single = items.len() == 1;
        let single_folder = single && items[0].is_dir;
        let single_file = single && !items[0].is_dir;

        let mut entries = vec![
            self.menu_item("context-file", "New File", "file-plus", |this, window, cx| this.context_create_file(window, cx), cx)
                .disabled(read_only || !can_create),
            self.menu_item("context-folder", "New Folder", "folder-plus", |this, window, cx| this.context_create_folder(window, cx), cx)
                .disabled(read_only || !can_create),
            MenuItem::Separator,
        ];
        let paste = self
            .menu_item("context-paste", "Paste", "clipboard-paste", |this, window, cx| this.context_paste(window, cx), cx)
            .shortcut("Ctrl+V")
            .disabled(read_only || !can_paste);
        if items.is_empty() {
            entries.push(paste);
        } else {
            if single_folder && local {
                entries.push(self.menu_item("context-terminal", "Open in Terminal", "info", |this, _, cx| this.context_open_terminal(cx), cx));
            }
            if single_file && local {
                entries.push(self.menu_item("context-open-with", "Open with…", "info", |this, window, cx| this.context_open_with(window, cx), cx));
            }
            if single_file {
                entries.push(self.menu_item("context-preview", "Preview", "info", |this, window, cx| this.context_preview(window, cx), cx));
            }
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
        div()
            .id("context-dismiss")
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
                anchored().position(menu.position).snap_to_window().child(
                    div()
                        .id("file-context-menu")
                        .debug_selector(|| "file-context-menu".into())
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(ContextMenu::new(entries)),
                ),
            )
            .into_any_element()
    }

    pub(crate) fn render_properties(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(properties) = &self.properties else { return div().into_any_element() };
        let parent = |path: &str| path.rsplit_once('/').map(|(parent, _)| parent.to_string()).unwrap_or_else(|| "/".into());
        let size = match &properties.totals {
            Load::Loading => "Calculating…".to_string(),
            Load::Failed(error) => error.clone(),
            Load::Loaded(totals) => file_size(totals.bytes),
        };
        let mut rows: Vec<(&str, String)> = match properties.items.as_slice() {
            [item] => {
                let mut rows = vec![
                    ("Name", item.name.clone()),
                    ("Type", if item.is_dir { "Folder".into() } else { type_description(&item.name).to_string() }),
                    ("Location", parent(&item.path)),
                    ("Size", size),
                ];
                if let (true, Load::Loaded(totals)) = (item.is_dir, &properties.totals) {
                    // The walk counts the folder itself.
                    rows.push(("Contents", counted(totals.files, "file", totals.folders.saturating_sub(1), "folder")));
                }
                rows.push(("Modified", file_date(&item.mod_time).to_string()));
                match &properties.ownership {
                    Some(Load::Loaded(ownership)) => {
                        rows.push(("Permissions", format!("{} ({:o})", permission_string(ownership.mode), ownership.mode & 0o7777)));
                        rows.push(("Owner", ownership.owner.clone()));
                        rows.push(("Group", ownership.group.clone()));
                    }
                    Some(Load::Failed(error)) => rows.push(("Permissions", error.clone())),
                    Some(Load::Loading) | None => {}
                }
                rows
            }
            items => {
                let folders = items.iter().filter(|item| item.is_dir).count();
                let mut rows = vec![("Selected", format!("{} items", items.len())), ("Folders", folders.to_string()), ("Size", size)];
                let location = parent(&items[0].path);
                if items.iter().all(|item| parent(&item.path) == location) {
                    rows.push(("Location", location));
                }
                rows
            }
        };
        rows.push(("Provider", properties.provider_label.clone()));
        // grid-cols-[5.5rem_minmax(0,1fr)] gap-x-4 gap-y-2.5 text-[13px]
        let list = div().flex().flex_col().gap(px(10.)).type_13().children(rows.into_iter().map(|(label, value)| {
            div()
                .flex()
                .gap(px(16.))
                .child(div().w(px(88.)).flex_none().text_color(c(TEXT_SECONDARY)).child(label))
                .child(div().flex_1().min_w_0().text_color(c(TEXT_PRIMARY)).child(value))
        }));
        div()
            .id("properties-overlay")
            .debug_selector(|| "properties-overlay".into())
            .absolute()
            .inset_0()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(Modal::new().title("Properties").on_close(cx.listener(|this, _, _, cx| {
                this.properties = None;
                cx.notify();
            })).body(list))
            .into_any_element()
    }

    pub(crate) fn render_preview(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(preview) = &self.preview else { return div().into_any_element() };
        let content = match &preview.content {
            PreviewContent::Loading => div().text_13().text_color(c(TEXT_MUTED)).child("Loading preview…").into_any_element(),
            PreviewContent::Failed(error) => div().text_13().text_color(c(DANGER)).child(error.clone()).into_any_element(),
            PreviewContent::Image(image) => img(image.clone()).size_full().object_fit(ObjectFit::Contain).into_any_element(),
            PreviewContent::Text(text) => div()
                .id("preview-text")
                .size_full()
                .overflow_y_scroll()
                .font_family(self.fonts.mono.clone())
                .text_13()
                .whitespace_normal()
                .child(text.clone())
                .into_any_element(),
        };
        div()
            .id("preview-overlay")
            .absolute()
            .inset_0()
            .key_context("PreviewInput")
            .track_focus(&self.preview_focus)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Modal::new()
                    .title(preview.item.name.clone())
                    .size(ModalSize::Lg)
                    .on_close(cx.listener(|this, _, window, cx| this.operation_escape(&OperationEscape, window, cx)))
                    .body(div().debug_selector(|| "preview-panel".into()).h(px(480.)).min_h_0().child(content)),
            )
            .into_any_element()
    }
}

/// "3 files, 1 folder".
fn counted(first: u64, first_noun: &str, second: u64, second_noun: &str) -> String {
    let plural = |count: u64, noun: &str| format!("{count} {noun}{}", if count == 1 { "" } else { "s" });
    format!("{}, {}", plural(first, first_noun), plural(second, second_noun))
}
