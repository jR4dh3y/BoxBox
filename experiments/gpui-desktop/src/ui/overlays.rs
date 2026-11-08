use gpui::{anchored, div, img, prelude::*, px, AnyElement, Context, MouseButton, ObjectFit, Window};

use super::TextScale;
use crate::{format::{file_date, file_size}, theme::*, workspace::{Workspace, OperationEscape, SavePreview}};
use crate::workspace::preview::PreviewContent;

impl Workspace {
    fn overlay(&self, id: &'static str) -> gpui::Stateful<gpui::Div> {
        div().id(id).absolute().inset_0().flex().items_center().justify_center().bg(alpha(0, 0.58))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
    }

    fn menu_action(&self, id: &'static str, label: &'static str, cx: &mut Context<Self>,
        action: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
    ) -> impl IntoElement {
        self.menu_action_enabled(id, label, true, cx, action)
    }

    fn menu_action_enabled(&self, id: &'static str, label: &'static str, enabled: bool, cx: &mut Context<Self>,
        action: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
    ) -> impl IntoElement {
        div().id(id).debug_selector(|| id.to_string()).px(px(12.)).py(px(6.)).text_13().cursor_pointer()
            .when(enabled, |item| item.hover(|style| style.bg(c(SURFACE_ELEVATED))))
            .when(!enabled, |item| item.text_color(c(TEXT_MUTED)).cursor_default())
            .child(label).on_click(cx.listener(move |this, _, window, cx| { if enabled { action(this, window, cx); } }))
    }

    pub(crate) fn render_context_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(menu) = &self.context_menu else { return div().into_any_element(); };
        let Some(pane) = self.pane_by_id(menu.target_id) else { return div().into_any_element(); };
        let local = pane.api.is_local();
        let read_only = match &pane.drives {
            crate::pane::Load::Loaded(drives) => drives.iter().any(|drive| drive.name == pane.path.split('/').next().unwrap_or("") && drive.read_only),
            _ => false,
        };
        let writable = !read_only && !self.operation_status.is_busy();
        let can_create = writable && !pane.path.is_empty();
        let single = menu.items.len() == 1;
        let panel = div().id("file-context-menu").debug_selector(|| "file-context-menu".into()).w(px(200.)).py(px(4.))
            .rounded(px(6.)).border_1().border_color(c(BORDER_PRIMARY)).bg(c(SURFACE_PRIMARY)).shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .when(!menu.items.is_empty(), |panel| panel
                .child(self.menu_action("context-open", "Open", cx, |this, window, cx| this.context_open(window, cx)))
                .child(self.menu_action_enabled("context-preview", "Preview", single && !menu.items[0].is_dir, cx, |this, window, cx| this.context_preview(window, cx)))
                .child(self.menu_action("context-copy", "Copy", cx, |this, _, cx| this.context_copy(cx)))
                .child(self.menu_action_enabled("context-cut", "Cut", writable, cx, |this, _, cx| this.context_cut(cx)))
                .when(menu.items.len() == 1 && menu.items[0].is_dir, |panel| panel
                    .child(self.menu_action("context-favorite", if self.context_favorite_index().is_some() { "Unpin from Favorites" } else { "Pin to Favorites" }, cx,
                        |this, _, cx| this.context_toggle_favorite(cx))))
                .child(self.menu_action_enabled("context-rename", "Rename", writable && single, cx, |this, window, cx| this.context_rename(window, cx)))
                .child(self.menu_action_enabled("context-delete", if local { "Move to Trash" } else { "Delete" }, writable, cx, |this, window, cx| this.context_delete(window, cx)))
                .when(!local, |panel| panel
                    .child(self.menu_action("context-download", "Download", cx, |this, _, cx| this.context_download(cx)))
                    .child(self.menu_action_enabled("context-share", if single && !menu.items[0].is_dir { "Share file…" } else { "Share folder…" }, single, cx, |this, window, cx| this.context_share(window, cx))))
                .child(self.menu_action_enabled("context-properties", "Properties", single, cx, |this, _, cx| this.context_properties(cx)))
                .when(local && single && menu.items[0].is_dir, |panel| panel
                    .child(self.menu_action("context-terminal", "Open in Terminal", cx, |this, _, cx| this.context_open_terminal(cx)))))
            .child(self.menu_action_enabled("context-paste", "Paste", can_create && self.file_clipboard.is_some(), cx, |this, window, cx| this.context_paste(window, cx)))
            .child(self.menu_action_enabled("context-folder", "New folder", can_create, cx, |this, window, cx| this.context_create_folder(window, cx)))
            .child(self.menu_action_enabled("context-file", "New file", can_create, cx, |this, window, cx| this.context_create_file(window, cx)))
            .child(self.menu_action_enabled("context-import", "Upload files", can_create, cx, |this, _, cx| this.context_import_files(cx)));
        div().absolute().inset_0().id("context-dismiss")
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.context_menu = None; cx.notify(); }))
            .child(anchored().position(menu.position).child(panel)).into_any_element()
    }

    pub(crate) fn render_properties(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(properties) = &self.properties else { return div().into_any_element(); };
        let item = &properties.item;
        self.overlay("properties-overlay").child(div().w(px(440.)).p(px(20.)).rounded(px(8.))
            .bg(c(SURFACE_PRIMARY)).border_1().border_color(c(BORDER_PRIMARY)).flex().flex_col().gap(px(12.))
            .child(div().text_lg().child(item.name.clone()))
            .child(format!("Location: {}", item.path))
            .child(format!("Size: {}", file_size(item.size)))
            .child(format!("Modified: {}", file_date(&item.mod_time)))
            .child(format!("Provider: {}", properties.provider_label))
            .child(self.menu_action("properties-close", "Close", cx, |this, _, cx| { this.properties = None; cx.notify(); })))
            .into_any_element()
    }

    pub(crate) fn render_preview(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(preview) = &self.preview else { return div().into_any_element(); };
        let content = match &preview.content {
            PreviewContent::Loading => div().child("Loading preview…").into_any_element(),
            PreviewContent::Failed(error) => div().text_color(c(DANGER)).child(error.clone()).into_any_element(),
            PreviewContent::Image(image) => img(image.clone()).size_full().object_fit(ObjectFit::Contain).into_any_element(),
            PreviewContent::Text { buffer, .. } => {
                let (before, selected, after) = buffer.display_parts();
                let text = match selected {
                    Some(selected) => format!("{before}{selected}{after}"),
                    None => format!("{before}▏{after}"),
                };
                div().id("preview-text").size_full().overflow_y_scroll().font_family(self.fonts.mono.clone())
                    .text_13().child(text).into_any_element()
            }
        };
        let dirty = matches!(preview.content, PreviewContent::Text { dirty: true, .. });
        self.overlay("preview-overlay").child(div().id("preview-panel").debug_selector(|| "preview-panel".into()).key_context("PreviewInput")
            .track_focus(&self.preview_focus)
            .on_key_down(cx.listener(|this, event, _, cx| this.operation_key_down(event, cx)))
            .w(gpui::relative(0.85)).h(gpui::relative(0.85)).bg(c(SURFACE_PRIMARY))
            .border_1().border_color(c(BORDER_PRIMARY)).rounded(px(8.)).flex().flex_col()
            .child(div().flex().items_center().px(px(16.)).py(px(8.)).gap(px(12.))
                .child(div().flex_1().child(format!("{}{}", preview.item.name, if dirty { " *" } else { "" })))
                .when(dirty, |header| header.child(self.menu_action("preview-save", "Save", cx, |this, window, cx| this.save_preview(&SavePreview, window, cx))))
                .child(self.menu_action("preview-discard", "Close", cx, |this, window, cx| this.operation_escape(&OperationEscape, window, cx)))
                .when(dirty, |header| header.child(self.menu_action("preview-discard-changes", "Discard changes", cx, |this, window, cx| {
                    this.preview = None; this.focus_handle.focus(window); cx.notify();
                }))))
            .child(div().flex_1().min_h_0().p(px(16.)).child(content))).into_any_element()
    }
}
