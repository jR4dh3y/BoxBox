//! Rename in place: the name in a row or tile becomes a field. Enter or a click
//! elsewhere renames; Escape keeps the old name, as in Thunar and Nautilus.

use gpui::{Context, Window};

use super::names::{basename_end, name_problem};
use super::operations::PaneTarget;
use super::undo::Undo;
use super::Workspace;
use crate::api::FileInfo;
use crate::components::InputEvent;

pub(crate) struct InlineRename {
    pub target: PaneTarget,
    pub path: String,
    pub name: String,
}

impl Workspace {
    pub(crate) fn start_rename(&mut self, target: PaneTarget, item: FileInfo, window: &mut Window, cx: &mut Context<Self>) {
        let end = basename_end(&item.name, item.is_dir);
        let name = item.name.clone();
        self.rename_field.update(cx, |field, cx| field.set_text(name, 0..end, cx));
        self.rename_field.read(cx).focus(window);
        if let Some(index) = self.pane_by_id(target.0).and_then(|pane| pane.items.iter().position(|entry| entry.path == item.path)) {
            self.reveal(target, index, window);
        }
        self.context_menu = None;
        self.inline_rename = Some(InlineRename { target, path: item.path, name: item.name });
        cx.notify();
    }

    /// Whether the row or tile for `path` in `target` shows the rename field.
    pub(crate) fn renaming(&self, target: PaneTarget, path: &str) -> bool {
        self.inline_rename.as_ref().is_some_and(|rename| rename.target == target && rename.path == path)
    }

    pub(crate) fn on_rename_field(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Submit => self.finish_rename(true, window, cx),
            InputEvent::Cancel => self.finish_rename(false, window, cx),
            InputEvent::Changed => {}
        }
    }

    /// The field lost focus: keep what was typed, if it is a name.
    pub(crate) fn on_rename_blur(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_rename(true, window, cx);
    }

    fn finish_rename(&mut self, save: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rename) = self.inline_rename.take() else { return };
        let focused = self.rename_field.read(cx).is_focused(window);
        let input = self.rename_field.read(cx).text().trim().to_string();
        if save && input != rename.name {
            let problem = self.pane_by_id(rename.target.0).and_then(|pane| {
                name_problem(&input, pane.items.iter().filter(|item| item.path != rename.path).map(|item| item.name.as_str()))
            });
            match problem {
                // Enter on a bad name keeps the field open to fix it.
                Some(problem) if focused => {
                    self.inline_rename = Some(rename);
                    self.set_operation_error(problem, cx);
                    return;
                }
                Some(problem) => self.set_operation_error(problem, cx),
                None => self.run_rename(rename.target, rename.path, input, cx),
            }
        }
        if focused {
            self.focus_handle.focus(window);
        }
        cx.notify();
    }

    fn run_rename(&mut self, target: PaneTarget, path: String, name: String, cx: &mut Context<Self>) {
        let Some(api) = self.pane_by_id(target.0).map(|pane| pane.api.clone()) else { return };
        let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
        let renamed = if parent.is_empty() { name.clone() } else { format!("{parent}/{name}") };
        let generation = self.begin_operation(format!("Renaming to {name}…"));
        let (request_api, from, to) = (api.clone(), path.clone(), renamed.clone());
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { request_api.rename(&from, &to) }).await;
            this.update(cx, |this, cx| {
                let result = result.map(|_| {
                    this.last_undo = Some(Undo::Rename { api: api.clone(), before: path, after: renamed.clone() });
                    if let Some(pane) = this.pane_mut_by_id(target.0) {
                        pane.selected.clear();
                        pane.selected.insert(renamed);
                    }
                    format!("Renamed to “{name}”")
                });
                this.refresh_provider(api, cx);
                this.finish_operation(generation, result, cx);
            })
            .ok();
        })
        .detach();
    }
}
