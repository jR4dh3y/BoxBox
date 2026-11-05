//! File operations behind the dialogs and shortcuts (`browse/actions.svelte.ts`).

use anyhow::Result;
use gpui::Context;

use crate::api::{Api, FileInfo};
use crate::notifications::ToastKind;
use crate::workspace::Workspace;

impl Workspace {
    /// Runs `work` off the main thread, then `done` on the workspace.
    pub(crate) fn background<T: Send + 'static>(
        &mut self,
        work: impl FnOnce(&Api) -> Result<T> + Send + 'static,
        done: impl FnOnce(&mut Self, Result<T>, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let api = self.api.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { work(&api) })
                .await;
            this.update(cx, |this, cx| {
                done(this, result, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Writable folder: not This PC and not on a read-only mount.
    pub(crate) fn can_write_here(&self) -> bool {
        let path = &self.pane().path;
        let mount = path.split('/').next().unwrap_or_default();
        !path.is_empty()
            && !self
                .roots
                .iter()
                .any(|root| root.name == mount && root.read_only)
    }

    pub(crate) fn create_item(
        &mut self,
        dir: String,
        name: String,
        directory: bool,
        cx: &mut Context<Self>,
    ) {
        let created = format!("{dir}/{name}");
        self.background(
            move |api| api.create(&dir, &name, directory),
            move |this, result, cx| match result {
                Ok(()) => this.reload_and_select(created, cx),
                Err(error) => this.error(&error, cx),
            },
            cx,
        );
    }

    pub(crate) fn rename_item(&mut self, item: FileInfo, name: String, cx: &mut Context<Self>) {
        let parent = item
            .path
            .rsplit_once('/')
            .map_or(String::new(), |(parent, _)| parent.to_string());
        let new_path = format!("{parent}/{name}");
        let (from, to) = (item.path.clone(), new_path.clone());
        self.background(
            move |api| api.rename(&from, &to),
            move |this, result, cx| match result {
                Ok(()) => {
                    this.set_undo(crate::undo::Undo::Rename {
                        from: item.path,
                        to: new_path.clone(),
                    });
                    this.reload_and_select(new_path, cx);
                }
                Err(error) => this.error(&error, cx),
            },
            cx,
        );
    }

    /// Native items go to Trash; otherwise they are deleted. Stops at the first failure.
    pub(crate) fn delete_items(
        &mut self,
        items: Vec<FileInfo>,
        to_trash: bool,
        cx: &mut Context<Self>,
    ) {
        let count = items.len();
        self.background(
            move |api| {
                let mut trashed = Vec::new();
                for item in &items {
                    if to_trash {
                        trashed.push(api.trash(&item.path)?);
                    } else {
                        api.delete(&item.path)?;
                    }
                }
                Ok(trashed)
            },
            move |this, result, cx| {
                match result {
                    Ok(trashed) => {
                        let noun = if count == 1 {
                            "item".to_string()
                        } else {
                            format!("{count} items")
                        };
                        let message = if to_trash {
                            format!("Moved {noun} to Trash")
                        } else {
                            format!("Deleted {noun}")
                        };
                        if !trashed.is_empty() {
                            this.set_undo(crate::undo::Undo::Trash(trashed));
                        }
                        this.toast(ToastKind::Success, message, cx);
                    }
                    Err(error) => this.error(&error, cx),
                }
                this.reload(cx);
            },
            cx,
        );
    }
}
