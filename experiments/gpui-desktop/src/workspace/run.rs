//! Activating a file: a document opens in its app; an executable script or
//! program asks to run first; a launcher starts its app when trusted and asks
//! when not, as Thunar and Nautilus do.

use std::path::PathBuf;

use gpui::Context;

use super::confirm::Confirm;
use super::operations::PaneTarget;
use super::Workspace;
use crate::api::FileInfo;
use crate::launch::Runnable;

impl Workspace {
    pub(crate) fn open_item(&mut self, target: PaneTarget, item: FileInfo, cx: &mut Context<Self>) {
        let Some(api) = self.pane_by_id(target.0).map(|pane| pane.api.clone()) else { return };
        if !api.is_local() {
            self.open_document(target, item, cx);
            return;
        }
        let path = item.path.clone();
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_executor()
                .spawn(async move { api.local_path(&path).ok().flatten().map(|real| (crate::launch::classify(&real), real)) })
                .await;
            this.update(cx, |this, cx| match found {
                Some((Runnable::Document, _)) | None => this.open_document(target, item, cx),
                Some((Runnable::Program { script }, real)) => this.ask_to_run(target, item, real, script, cx),
                Some((Runnable::Launcher { trusted: true, name }, real)) => this.start_launcher(name, real, false, cx),
                Some((Runnable::Launcher { trusted: false, name }, real)) => this.ask_to_trust(target, item, name, real, cx),
            })
            .ok();
        })
        .detach();
    }

    fn ask_to_run(&mut self, target: PaneTarget, item: FileInfo, real: PathBuf, script: bool, cx: &mut Context<Self>) {
        let name = item.name.clone();
        let message = match script {
            true => format!("“{name}” is an executable script. Run it, or open it to read?"),
            false => format!("“{name}” is a program. Run it only if you trust where it came from."),
        };
        let mut confirm = Confirm::new(format!("Run “{name}”?"), message, "Run", move |_, cx| {
            let ran = name.clone();
            cx.spawn(async move |this, cx| {
                let result = cx.background_executor().spawn(async move { crate::launch::run(&real) }).await;
                this.update(cx, |this, cx| this.report(result.map(|()| format!("Started “{ran}”")).map_err(|error| format!("Could not run “{ran}”: {error:#}")), cx)).ok();
            })
            .detach();
        })
        .safe();
        if script {
            confirm = confirm.secondary("Open", move |this, cx| this.open_document(target, item, cx));
        }
        self.ask(confirm, cx);
    }

    fn ask_to_trust(&mut self, target: PaneTarget, item: FileInfo, name: String, real: PathBuf, cx: &mut Context<Self>) {
        let confirm = Confirm::new(
            "Untrusted launcher",
            format!("“{name}” is not marked as trusted. Launch it only if you trust where it came from; it will then launch without asking."),
            "Trust and Launch",
            move |this, cx| this.start_launcher(name, real, true, cx),
        )
        .safe()
        .secondary("Open", move |this, cx| this.open_document(target, item, cx));
        self.ask(confirm, cx);
    }

    /// Starts a launcher's app, marking the launcher trusted first if asked.
    fn start_launcher(&mut self, name: String, real: PathBuf, trust: bool, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if trust {
                        crate::launch::trust(&real)?;
                    }
                    crate::launch::launch(&real)
                })
                .await;
            this.update(cx, |this, cx| this.report(result.map(|()| format!("Started {name}")).map_err(|error| format!("Could not start {name}: {error:#}")), cx)).ok();
        })
        .detach();
    }
}
