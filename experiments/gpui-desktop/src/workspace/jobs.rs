//! `desktop/TransferJobs.svelte`'s jobs: one per paste or import, with progress
//! read from the transfer's counters, a cancel, and the outcome.

use std::time::Duration;

use gpui::Context;

use super::Workspace;
use crate::progress::Progress;

pub(crate) enum JobState {
    Running,
    Done,
    Failed(String),
    Cancelled,
}

pub(crate) struct Job {
    pub id: u64,
    /// The item's name, or "N items".
    pub title: String,
    /// The destination folder's name.
    pub folder: String,
    pub moving: bool,
    pub progress: Progress,
    pub state: JobState,
}

impl Workspace {
    /// Registers a running transfer and keeps the panel ticking until it ends.
    pub(crate) fn start_job(&mut self, title: String, folder: String, moving: bool, cx: &mut Context<Self>) -> (u64, Progress) {
        self.next_job_id += 1;
        let progress = Progress::default();
        let id = self.next_job_id;
        self.jobs.push(Job { id, title, folder, moving, progress: progress.clone(), state: JobState::Running });
        if self.jobs.iter().filter(|job| matches!(job.state, JobState::Running)).count() == 1 {
            // Progress lives in atomics, so the panel only needs redrawing while jobs run.
            cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(Duration::from_millis(200)).await;
                let running = this
                    .update(cx, |this, cx| {
                        cx.notify();
                        this.jobs.iter().any(|job| matches!(job.state, JobState::Running))
                    })
                    .unwrap_or(false);
                if !running {
                    break;
                }
            })
            .detach();
        }
        (id, progress)
    }

    pub(crate) fn finish_job(&mut self, id: u64, result: &anyhow::Result<String>, cx: &mut Context<Self>) {
        if let Some(job) = self.jobs.iter_mut().find(|job| job.id == id) {
            job.state = match result {
                Ok(_) => JobState::Done,
                Err(_) if job.progress.is_cancelled() => JobState::Cancelled,
                Err(error) => JobState::Failed(format!("{error:#}")),
            };
        }
        cx.notify();
    }

    /// Ends a transfer's job and its operation. A cancel is shown in the panel only,
    /// not as an error notice.
    pub(crate) fn finish_transfer(&mut self, job: u64, generation: u64, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        self.finish_job(job, &result, cx);
        let cancelled = self.jobs.iter().any(|entry| entry.id == job && matches!(entry.state, JobState::Cancelled));
        if cancelled {
            self.operation_status = super::operations::OperationStatus::Idle;
            cx.notify();
        } else {
            self.finish_operation(generation, result, cx);
        }
    }

    /// The job's button: cancels a running job, dismisses a finished one.
    pub(crate) fn cancel_or_dismiss_job(&mut self, id: u64, cx: &mut Context<Self>) {
        match self.jobs.iter().find(|job| job.id == id) {
            Some(job) if matches!(job.state, JobState::Running) => job.progress.cancel(),
            Some(_) => self.jobs.retain(|job| job.id != id),
            None => {}
        }
        cx.notify();
    }

    pub(crate) fn clear_finished_jobs(&mut self, cx: &mut Context<Self>) {
        self.jobs.retain(|job| matches!(job.state, JobState::Running));
        cx.notify();
    }
}
