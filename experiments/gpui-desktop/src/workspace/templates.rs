//! New files from the XDG templates folder: a copy of the template under a free
//! name, which then opens for renaming, as in Nautilus.

use gpui::{Context, SharedString, Window};

use super::operations::PaneTarget;
use super::Workspace;
use crate::api::FileInfo;
use crate::components::MenuItem;

/// The templates folder's place, from `LocalFiles`.
const TEMPLATES: &str = "templates";

impl Workspace {
    /// Reads the templates again, for the menu about to show them.
    pub(crate) fn refresh_templates(&mut self, cx: &mut Context<Self>) {
        let Some(api) = self.local_api() else { return };
        cx.spawn(async move |this, cx| {
            let mut found = cx
                .background_executor()
                .spawn(async move { api.list(TEMPLATES).unwrap_or_default() })
                .await;
            found.retain(|item| !item.name.starts_with('.'));
            found.sort_by_key(|item| item.name.to_lowercase());
            this.update(cx, |this, cx| {
                if this.templates.iter().map(|item| &item.path).ne(found.iter().map(|item| &item.path)) {
                    this.templates = found;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// The "New from Template" submenu for `target`, or nothing when there are no
    /// templates or the folder is not local.
    pub(crate) fn template_menu(&self, target: PaneTarget, close: fn(&mut Workspace), cx: &mut Context<Self>) -> Option<MenuItem> {
        let pane = self.pane_by_id(target.0)?;
        if self.templates.is_empty() || !pane.api.is_local() {
            return None;
        }
        let items = self
            .templates
            .iter()
            .enumerate()
            .map(|(index, template)| {
                let label = match template.name.rfind('.') {
                    Some(dot) if dot > 0 && !template.is_dir => &template.name[..dot],
                    _ => template.name.as_str(),
                };
                let template = template.clone();
                MenuItem::action(("template", index), label.to_string())
                    .icon(crate::format::file_icon(&template.name, template.is_dir))
                    .disabled(self.read_only(target))
                    .on_select(cx.listener(move |this, _, _, cx| {
                        close(this);
                        this.open_submenu = None;
                        this.create_from_template(target, template.clone(), cx);
                    }))
            })
            .collect();
        let id: SharedString = "new-from-template".into();
        let open = self.open_submenu.as_ref() == Some(&id);
        Some(MenuItem::submenu(id, "New from Template", items, open).icon("layout-template"))
    }

    pub(crate) fn create_from_template(&mut self, target: PaneTarget, template: FileInfo, cx: &mut Context<Self>) {
        if !self.ensure_writable(target, cx) || self.operation_status.is_busy() {
            return;
        }
        let Some(pane) = self.pane_by_id(target.0) else { return };
        let (api, folder) = (pane.api.clone(), pane.path.clone());
        let taken: std::collections::HashSet<String> = pane.items.iter().map(|item| item.name.clone()).collect();
        let name = super::names::unused_name(&template.name, template.is_dir, &|name| taken.contains(name));
        let destination = format!("{folder}/{name}");
        let generation = self.begin_operation(format!("Creating {name}…"));
        let (request_api, created) = (api.clone(), destination.clone());
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { request_api.copy(&template.path, &created) }).await;
            this.update(cx, |this, cx| {
                if result.is_ok() {
                    // Once the folder shows it, the new item opens for renaming.
                    this.pending_rename = Some((target, destination));
                }
                this.refresh_provider(api, cx);
                this.finish_operation(generation, result.map(|()| format!("Created “{name}”")), cx);
            })
            .ok();
        })
        .detach();
    }

    /// Starts the rename that a creation asked for, once its item is listed.
    pub(crate) fn start_pending_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((target, path)) = self.pending_rename.clone() else { return };
        let Some(item) = self.pane_by_id(target.0).and_then(|pane| pane.items.iter().find(|item| item.path == path).cloned()) else { return };
        self.pending_rename = None;
        if let Some(pane) = self.pane_mut_by_id(target.0) {
            pane.selected.clear();
            pane.selected.insert(path);
        }
        self.start_rename(target, item, window, cx);
    }
}
