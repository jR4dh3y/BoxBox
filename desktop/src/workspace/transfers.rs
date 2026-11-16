//! Paste and drop: a `transfer::run` on its own thread, which asks this window
//! when it meets a taken name, a failure or too little space, and waits for
//! the answer while the window stays responsive.

use std::sync::{mpsc, Arc};

use gpui::Context;

use super::operations::{same_provider, ClipboardMode, FileClipboard, PaneTarget};
use super::Workspace;
use crate::api::Api;
use crate::transfer::{self, Answer, Question};

/// A question from a running transfer, on screen until answered.
pub(crate) struct PendingQuestion {
    pub job: u64,
    pub question: Question,
    /// Whether "Apply to all" is offered: more of the same may follow.
    pub more: bool,
    pub apply_all: bool,
    /// The destination folder as the user sees it.
    pub folder: String,
    reply: mpsc::Sender<(Answer, bool)>,
}

impl Workspace {
    /// Copies or moves `clipboard` into `parent`. Items already where they would
    /// go are left alone; a folder cannot go inside itself.
    pub(crate) fn run_paste(&mut self, target: PaneTarget, clipboard: FileClipboard, destination: Arc<Api>, parent: String, cx: &mut Context<Self>) {
        if self.operation_status.is_busy() {
            return;
        }
        if parent.is_empty() {
            self.set_operation_error("Choose a destination folder before pasting.", cx);
            return;
        }
        let mode = match clipboard.mode {
            ClipboardMode::Copy => transfer::Mode::Copy,
            ClipboardMode::Cut => transfer::Mode::Move,
        };
        let mut items = Vec::new();
        for entry in &clipboard.entries {
            let same = same_provider(&entry.api, &destination);
            let path = &entry.item.path;
            if entry.item.is_dir && same && (parent == *path || parent.starts_with(&format!("{path}/"))) {
                self.set_operation_error("A folder cannot be copied or moved into itself.", cx);
                return;
            }
            if mode == transfer::Mode::Move && same && path.rsplit_once('/').map_or("", |(folder, _)| folder) == parent {
                continue;
            }
            items.push((entry.api.clone(), entry.item.clone()));
        }
        if items.is_empty() {
            self.set_operation_error("All selected items are already in this folder.", cx);
            return;
        }
        let generation = self.begin_operation(format!("Transferring {} item(s)…", items.len()));
        let title = match items.as_slice() {
            [(_, item)] => item.name.clone(),
            items => format!("{} items", items.len()),
        };
        let folder = self.folder_label(&destination, &parent);
        let (job, progress) = self.start_job(title, folder, mode == transfer::Mode::Move, cx);
        let mut affected = Vec::<Arc<Api>>::new();
        for api in items.iter().map(|(api, _)| api).chain(std::iter::once(&destination)) {
            if !affected.iter().any(|known| same_provider(known, api)) {
                affected.push(api.clone());
            }
        }
        let job_destination = destination.clone();
        let job_spec = transfer::Job { mode, items, destination, parent };
        let (questions, asked) = async_channel::unbounded::<(Question, bool, mpsc::Sender<(Answer, bool)>)>();
        let (done, finished) = async_channel::bounded(1);
        let measured = progress.clone();
        let spawned = std::thread::Builder::new().name("transfer".into()).spawn(move || {
            let ask = |question: Question, more: bool| {
                let (reply, answer) = mpsc::channel();
                if questions.send_blocking((question, more, reply)).is_err() {
                    return (Answer::Cancel, false);
                }
                answer.recv().unwrap_or((Answer::Cancel, false))
            };
            let result = measured.scope(|| {
                for (api, item) in &job_spec.items {
                    if let Ok(Some(path)) = api.local_path(&item.path) {
                        crate::progress::measure(&path);
                    }
                }
                transfer::run(&job_spec, &ask)
            });
            let _ = done.send_blocking(result);
        });
        if let Err(error) = spawned {
            self.finish_transfer(job, generation, Err(error.into()), cx);
            return;
        }
        // Questions arrive while the thread runs; the channel closes when it ends.
        let label_api = job_destination;
        cx.spawn(async move |this, cx| {
            while let Ok((question, more, reply)) = asked.recv().await {
                let shown = this.update(cx, |this, cx| {
                    let folder = match &question {
                        Question::Conflict { parent, .. } => this.folder_label(&label_api, parent),
                        _ => String::new(),
                    };
                    this.transfer_question = Some(PendingQuestion { job, question, more, apply_all: false, folder, reply });
                    cx.notify();
                });
                if shown.is_err() {
                    return;
                }
            }
        })
        .detach();
        let moved = clipboard;
        cx.spawn(async move |this, cx| {
            let result = finished.recv().await.unwrap_or_else(|_| Err(anyhow::anyhow!("The transfer stopped")));
            this.update(cx, move |this, cx| {
                this.transfer_question = None;
                for api in affected {
                    this.refresh_provider(api, cx);
                }
                // A cut is spent once pasted; a drag leaves the clipboard alone.
                if result.is_ok() && mode == transfer::Mode::Move && this.file_clipboard.as_ref().is_some_and(|held| held.same_items(&moved)) {
                    this.file_clipboard = None;
                }
                this.finish_transfer(job, generation, result.map(|_| ()), cx);
                if this.pane_by_id(target.0).is_none() {
                    this.report(Err("The destination pane was closed during the transfer.".into()), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// The dialog's answer goes back to the waiting transfer. Cancel also
    /// stops it, so its job shows as cancelled.
    pub(crate) fn answer_transfer(&mut self, answer: Answer, cx: &mut Context<Self>) {
        let Some(pending) = self.transfer_question.take() else { return };
        if answer == Answer::Cancel {
            if let Some(job) = self.jobs.iter().find(|job| job.id == pending.job) {
                job.progress.cancel();
            }
        }
        let _ = pending.reply.send((answer, pending.apply_all && pending.more));
        cx.notify();
    }
}

impl Workspace {
    /// A folder's name as the sidebar and path bar show it: a place's label
    /// (Documents, not `documents`), or the last part of the path.
    pub(crate) fn folder_label(&self, api: &Api, path: &str) -> String {
        if let Some((_, last)) = path.rsplit_once('/') {
            return last.to_string();
        }
        let roots = [&self.home.roots].into_iter().chain(self.tabs.iter().map(|pane| &pane.roots));
        roots
            .filter_map(|roots| match roots {
                crate::pane::Load::Loaded(roots) => roots.iter().find(|root| root.name == path),
                _ => None,
            })
            .next()
            .map_or_else(|| path.to_string(), |root| self.root_name(api, root).to_string())
    }
}
