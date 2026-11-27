//! The Trash page and the sidebar's Trash: listing, restoring, deleting and
//! emptying, whether it holds anything, and items dropped onto it.

use std::sync::Arc;

use gpui::Context;

use super::confirm::Confirm;
use super::operations::{same_provider, PaneTarget};
use super::{Load, Workspace};
use crate::api::{Api, FileInfo, TrashItem};
use crate::components::ToastKind;

/// The directory watch that stands for Trash rather than a pane.
pub(crate) const TRASH_WATCH: usize = usize::MAX;

impl Workspace {
    /// The local provider whose Trash the sidebar shows.
    pub(crate) fn local_api(&self) -> Option<Arc<Api>> {
        self.tabs.iter().chain(self.split_panes.values()).map(|pane| pane.api.clone()).find(|api| api.is_local())
    }

    /// Watches the home Trash so its icon follows changes made by other apps too.
    pub(crate) fn watch_trash(&mut self, cx: &mut Context<Self>) {
        if let Some(folder) = self.local_api().and_then(|api| api.trash_files()) {
            self.directory_watches.watch(TRASH_WATCH, folder);
        }
        // Windows lists the shared Recycle Bin, which Explorer and other apps change too.
        #[cfg(windows)]
        {
            let (changed, changes) = async_channel::unbounded();
            crate::windows::recycle_bin::watch(changed);
            cx.spawn(async move |this, cx| {
                while changes.recv().await.is_ok() {
                    if this.update(cx, |this, cx| this.refresh_trash_state(cx)).is_err() {
                        break;
                    }
                }
            })
            .detach();
        }
        self.refresh_trash_state(cx);
    }

    /// Reads whether Trash holds anything, and reloads the Trash page if it is open.
    pub(crate) fn refresh_trash_state(&mut self, cx: &mut Context<Self>) {
        let Some(api) = self.local_api() else { return };
        if self.trash_open {
            self.load_trash(api.clone(), cx);
        }
        cx.spawn(async move |this, cx| {
            let full = cx.background_executor().spawn(async move { api.trash_has_items() }).await;
            this.update(cx, |this, cx| {
                if this.trash_full != full {
                    this.trash_full = full;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn load_trash(&mut self, api: Arc<Api>, cx: &mut Context<Self>) {
        if !api.is_local() {
            return;
        }
        self.trash_provider = Some(api.clone());
        if !matches!(self.trash, Load::Loaded(_)) {
            self.trash = Load::Loading;
        }
        self.trash_error = None;
        let request_api = api.clone();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { request_api.trash() }).await;
            this.update(cx, |this, cx| {
                if this.trash_provider.as_ref().is_some_and(|current| Arc::ptr_eq(current, &api)) {
                    match result {
                        Ok(items) => {
                            this.trash_full = !items.is_empty();
                            this.show_trash_items(items);
                        }
                        Err(error) => {
                            this.trash_error = Some(format!("{error:#}"));
                            this.trash = Load::Failed(format!("{error:#}"));
                        }
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn open_trash(&mut self, cx: &mut Context<Self>) {
        let api = self.active_api();
        if !api.is_local() {
            self.report(Err("Trash is available only for local storage".into()), cx);
            return;
        }
        self.trash_open = true;
        self.trash_error = None;
        self.load_trash(api, cx);
        cx.notify();
    }

    /// Puts items back where they were. The notice offers to show one item there.
    pub(crate) fn restore_trash(&mut self, items: Vec<TrashItem>, cx: &mut Context<Self>) {
        let Some(api) = self.trash_provider.clone().filter(|api| api.is_local()) else { return };
        let items: Vec<TrashItem> = items.into_iter().filter(|item| item.can_restore).collect();
        if items.is_empty() || self.operation_status.is_busy() {
            return;
        }
        let token = self.begin_operation("Restoring from Trash");
        let request_api = api.clone();
        let ids: Vec<String> = items.iter().map(|item| item.id.clone()).collect();
        cx.spawn(async move |this, cx| {
            // Each item on its own, so one that cannot go back does not stop the rest.
            let failures = cx
                .background_executor()
                .spawn(async move { ids.iter().filter_map(|id| request_api.restore_trash(id).err().map(|error| format!("{error:#}"))).collect::<Vec<_>>() })
                .await;
            this.update(cx, |this, cx| {
                if this.mutation_generation != token {
                    return;
                }
                this.operation_status = super::operations::OperationStatus::Idle;
                match (failures.first(), items.as_slice()) {
                    (Some(error), [item]) => this.report(Err(format!("Could not restore “{}”: {error}", item.name)), cx),
                    (Some(error), _) => this.report(Err(format!("Could not restore {} of {} items: {error}", failures.len(), items.len())), cx),
                    (None, [item]) => {
                        let message = format!("Restored “{}”", item.name);
                        match api.virtual_path(std::path::Path::new(&item.original_path)) {
                            Some(path) => this.notify_with_action(ToastKind::Success, message, "Show", move |this, cx| this.show_item(path.clone(), cx), cx),
                            None => this.notify(ToastKind::Success, message, cx),
                        }
                    }
                    (None, _) => this.notify(ToastKind::Success, format!("Restored {} items", items.len()), cx),
                }
                this.trash_error = failures.first().map(|error| format!("Could not restore item: {error}"));
                this.refresh_trash_state(cx);
                this.load_trash(api.clone(), cx);
                this.refresh_provider(api.clone(), cx);
            })
            .ok();
        })
        .detach();
    }

    /// Opens the folder that holds `path` in the active pane, with the item selected.
    pub(crate) fn show_item(&mut self, path: String, cx: &mut Context<Self>) {
        let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent).to_string();
        let target = self.active_pane;
        self.navigate_in(target, parent, cx);
        if let Some(pane) = self.pane_mut_by_id(target.0) {
            pane.selected.clear();
            pane.selected.insert(path);
        }
        cx.notify();
    }

    /// Asks, then deletes items from Trash for good.
    pub(crate) fn delete_trash_items(&mut self, items: Vec<TrashItem>, cx: &mut Context<Self>) {
        let Some(api) = self.trash_provider.clone().filter(|api| api.is_local()) else { return };
        if items.is_empty() {
            return;
        }
        let question = super::operations::delete_question(items.iter().map(|item| item.name.as_str()));
        let confirm = Confirm::question(question, move |this, cx| {
            if this.operation_status.is_busy() {
                return;
            }
            let token = this.begin_operation("Deleting permanently");
            let request_api = api.clone();
            let ids: Vec<String> = items.iter().map(|item| item.id.clone()).collect();
            cx.spawn(async move |this, cx| {
                let result = cx.background_executor().spawn(async move { ids.iter().try_for_each(|id| request_api.delete_trash(id).map(|_| ())) }).await;
                this.update(cx, |this, cx| {
                    if this.mutation_generation == token {
                        this.report(result.as_ref().map(|_| ()).map_err(|error| format!("Could not permanently delete item: {error:#}")), cx);
                        this.refresh_trash_state(cx);
                        this.load_trash(api.clone(), cx);
                    }
                })
                .ok();
            })
            .detach();
        });
        self.ask(confirm, cx);
    }

    /// Asks, then deletes everything in Trash.
    pub(crate) fn empty_trash(&mut self, cx: &mut Context<Self>) {
        let Some(api) = self.local_api() else { return };
        let count = match &self.trash {
            Load::Loaded(items) if self.trash_open => Some(items.len()),
            _ => None,
        };
        let question = match count {
            Some(1) => "Delete the item in Trash?".to_string(),
            Some(count) => format!("Delete all {count} items in Trash?"),
            None => "Empty Trash?".to_string(),
        };
        let confirm = Confirm::question(question, move |this, cx| {
            if this.operation_status.is_busy() {
                return;
            }
            let token = this.begin_operation("Emptying Trash…");
            let request_api = api.clone();
            cx.spawn(async move |this, cx| {
                let result = cx.background_executor().spawn(async move { request_api.empty_trash() }).await;
                this.update(cx, |this, cx| {
                    if this.mutation_generation == token {
                        this.finish_operation(token, result.map(|_| ()), cx);
                    }
                    this.trash_provider = Some(api.clone());
                    this.refresh_trash_state(cx);
                    this.load_trash(api, cx);
                })
                .ok();
            })
            .detach();
        });
        self.ask(confirm, cx);
    }

    /// Files from another app dropped onto Trash, when they are on this computer.
    pub(crate) fn trash_external(&mut self, paths: Vec<std::path::PathBuf>, cx: &mut Context<Self>) {
        let Some(api) = self.local_api() else { return };
        let request_api = api.clone();
        cx.spawn(async move |this, cx| {
            let items = cx
                .background_executor()
                .spawn(async move { paths.iter().filter_map(|path| request_api.virtual_path(path)).map(|path| request_api.stat(&path)).collect::<anyhow::Result<Vec<_>>>() })
                .await;
            this.update(cx, |this, cx| match items {
                Ok(items) if !items.is_empty() => this.trash_dropped(api, items, cx),
                Ok(_) => {}
                Err(error) => this.report(Err(format!("Could not move to Trash: {error:#}")), cx),
            })
            .ok();
        })
        .detach();
    }

    /// Items dropped onto Trash go there without a question, as in other file managers.
    pub(crate) fn trash_dropped(&mut self, source: Arc<Api>, items: Vec<FileInfo>, cx: &mut Context<Self>) {
        let Some(local) = self.local_api().filter(|local| same_provider(local, &source)) else {
            self.report(Err("Only local files can go to Trash".into()), cx);
            return;
        };
        let target = self
            .tabs
            .iter()
            .chain(self.split_panes.values())
            .find(|pane| Arc::ptr_eq(&pane.api, &local))
            .map_or(self.active_pane, |pane| PaneTarget(pane.id));
        self.run_delete(target, local, items, false, cx);
    }
}
