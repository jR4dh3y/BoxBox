//! Single-slot undo for rename and move-to-Trash (`browse/undo.svelte.ts`).

use gpui::{Context, Window};

use crate::api::TrashItem;
use crate::notifications::ToastKind;
use crate::workspace::{Undo as UndoAction, Workspace};

pub enum Undo {
    Rename { from: String, to: String },
    Trash(Vec<TrashItem>),
}

impl Workspace {
    pub(crate) fn set_undo(&mut self, undo: Undo) {
        self.undo = Some(undo);
    }

    pub(crate) fn undo(&mut self, _: &UndoAction, _: &mut Window, cx: &mut Context<Self>) {
        let Some(undo) = self.undo.take() else {
            return;
        };
        let message = match &undo {
            Undo::Rename { .. } => "Rename undone",
            Undo::Trash(_) => "Restored from Trash",
        };
        self.background(
            move |api| match undo {
                Undo::Rename { from, to } => api.rename(&to, &from),
                Undo::Trash(items) => items.iter().try_for_each(|item| api.restore(&item.id)),
            },
            move |this, result, cx| {
                match result {
                    Ok(()) => this.toast(ToastKind::Success, message, cx),
                    Err(error) => this.error(&error, cx),
                }
                this.reload(cx);
            },
            cx,
        );
    }
}
