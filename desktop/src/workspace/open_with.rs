//! The Open With dialog's state: the file, the apps for it, the choice.

use std::path::PathBuf;

use gpui::{Context, Window};

use super::{Load, Workspace};
use crate::api::FileInfo;
use crate::components::InputEvent;
use crate::open_with::{self, Apps};

pub(crate) struct OpenWith {
    pub item: FileInfo,
    pub path: PathBuf,
    pub apps: Load<Apps>,
    pub selected: Option<String>,
    pub make_default: bool,
    pub launching: bool,
}

impl Workspace {
    /// Opens the dialog for a local file and lists its apps in the background.
    pub(crate) fn open_with_dialog(&mut self, item: FileInfo, window: &mut Window, cx: &mut Context<Self>) {
        let api = self.pane().api.clone();
        let Ok(Some(path)) = api.local_path(&item.path) else {
            self.report(Err("Open With works for files on this computer.".into()), cx);
            return;
        };
        self.open_with = Some(OpenWith { item, path: path.clone(), apps: Load::Loading, selected: None, make_default: false, launching: false });
        self.open_with_search.update(cx, |field, cx| field.set_text("", 0..0, cx));
        self.open_with_search.read(cx).focus(window);
        cx.spawn(async move |this, cx| {
            let request = path.clone();
            let apps = cx.background_executor().spawn(async move { open_with::list(&request) }).await;
            this.update(cx, |this, cx| {
                if let Some(dialog) = this.open_with.as_mut().filter(|dialog| dialog.path == path) {
                    dialog.selected = apps.as_ref().ok().and_then(open_with::initial_choice);
                    dialog.apps = match apps {
                        Ok(apps) => Load::Loaded(apps),
                        Err(error) => Load::Failed(format!("{error:#}")),
                    };
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Enter in the search opens the chosen app; Escape closes.
    pub(crate) fn on_open_with_search(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Submit => self.confirm_open_with(cx),
            InputEvent::Cancel => self.close_open_with(window, cx),
            InputEvent::Changed => cx.notify(),
        }
    }

    pub(crate) fn close_open_with(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_with = None;
        self.focus_handle.focus(window);
        cx.notify();
    }

    /// The selected app, if the current search still shows it.
    pub(crate) fn open_with_choice(&self, cx: &gpui::App) -> Option<String> {
        let dialog = self.open_with.as_ref()?;
        let Load::Loaded(apps) = &dialog.apps else { return None };
        let selected = dialog.selected.as_ref()?;
        let (recommended, other) = open_with::filter(apps, self.open_with_search.read(cx).text());
        recommended.iter().chain(&other).any(|app| &app.id == selected).then(|| selected.clone())
    }

    pub(crate) fn confirm_open_with(&mut self, cx: &mut Context<Self>) {
        let Some(app_id) = self.open_with_choice(cx) else { return };
        let Some(dialog) = self.open_with.as_mut().filter(|dialog| !dialog.launching) else { return };
        dialog.launching = true;
        let (path, make_default, name) = (dialog.path.clone(), dialog.make_default, dialog.item.name.clone());
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move { open_with::launch(&path, &app_id, make_default) }).await;
            this.update(cx, |this, cx| match result {
                Ok(()) => {
                    this.open_with = None;
                    cx.notify();
                }
                Err(error) => {
                    if let Some(dialog) = this.open_with.as_mut() {
                        dialog.launching = false;
                    }
                    this.report(Err(format!("Could not open {name}: {error:#}")), cx);
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}
