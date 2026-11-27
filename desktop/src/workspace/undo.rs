//! `browse/undo.svelte.ts`: Ctrl+Z reverses the last move to Trash or rename.

use std::sync::Arc;

use gpui::{Context, Window};

use super::Workspace;
use crate::api::Api;

pub(crate) enum Undo {
    Trash { api: Arc<Api>, ids: Vec<String> },
    Rename { api: Arc<Api>, before: String, after: String },
}

impl Workspace {
    pub(crate) fn undo(&mut self, _: &super::Undo, _: &mut Window, cx: &mut Context<Self>) {
        if self.operation_status.is_busy() {
            return;
        }
        if let Some(undo) = self.last_undo.take() {
            self.run_undo(undo, cx);
        }
    }

    /// The Undo button on a "Moved to Trash" notice: restores those items, and
    /// Ctrl+Z no longer tries to.
    pub(crate) fn undo_trash(&mut self, api: Arc<Api>, ids: Vec<String>, cx: &mut Context<Self>) {
        if self.operation_status.is_busy() {
            return;
        }
        if matches!(&self.last_undo, Some(Undo::Trash { ids: last, .. }) if *last == ids) {
            self.last_undo = None;
        }
        self.run_undo(Undo::Trash { api, ids }, cx);
    }

    fn run_undo(&mut self, undo: Undo, cx: &mut Context<Self>) {
        let generation = self.begin_operation("Undoing…");
        let api = match &undo {
            Undo::Trash { api, .. } | Undo::Rename { api, .. } => api.clone(),
        };
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    match undo {
                        Undo::Rename { api, before, after } => api.rename(&after, &before).map(|_| ()),
                        Undo::Trash { api, ids } => ids.iter().try_for_each(|id| api.restore_trash(id)),
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                this.refresh_provider(api, cx);
                this.refresh_trash_state(cx);
                this.finish_operation(generation, result, cx);
            })
            .ok();
        })
        .detach();
    }
}
