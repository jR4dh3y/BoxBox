//! Background copy/move/delete jobs (`stores/jobs.svelte.ts`, `desktop/TransferJobs.svelte`).

use std::collections::{HashMap, HashSet};

use futures::StreamExt;
use gpui::{div, prelude::*, px, Context, FontWeight};
use serde::Deserialize;

use crate::format::file_size;
use crate::notifications::ToastKind;
use crate::sse::{self, SseMessage};
use crate::theme::*;
use crate::ui::{icon, TextScale};
use crate::workspace::Workspace;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobType {
    Copy,
    Move,
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: JobType,
    pub state: JobState,
    pub progress: f64,
    #[serde(default)]
    pub bytes_total: u64,
    pub source_path: String,
    #[serde(default)]
    pub dest_path: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

impl Job {
    pub fn is_active(&self) -> bool {
        matches!(self.state, JobState::Pending | JobState::Running)
    }

    /// `jobProgressLabel` in `utils/jobProgress.ts`.
    pub fn label(&self) -> String {
        match self.state {
            JobState::Pending => "Queued".into(),
            JobState::Running if self.progress >= 100. => "Finishing…".into(),
            JobState::Running if self.progress <= 0. && self.bytes_total == 0 => match self.kind {
                JobType::Copy => "Copying…".into(),
                JobType::Move => "Moving…".into(),
                JobType::Delete => "Deleting…".into(),
            },
            JobState::Running if self.bytes_total > 0 => {
                format!(
                    "{}% of {}",
                    self.progress.floor(),
                    file_size(self.bytes_total)
                )
            }
            JobState::Running => format!("{}%", self.progress.floor()),
            JobState::Completed => "Done".into(),
            JobState::Failed => self.error.clone().unwrap_or_else(|| "Failed".into()),
            JobState::Cancelled => "Cancelled".into(),
        }
    }

    fn name(&self) -> &str {
        self.source_path
            .rsplit('/')
            .find(|part| !part.is_empty())
            .unwrap_or(&self.source_path)
    }

    fn destination_folder(&self) -> Option<&str> {
        let dest = self.dest_path.as_deref()?;
        dest.rsplit('/').filter(|part| !part.is_empty()).nth(1)
    }
}

/// Weighted progress of active jobs, or None unless every one knows its size.
pub fn transfer_summary(jobs: &[&Job]) -> String {
    let active: Vec<_> = jobs.iter().filter(|job| job.is_active()).collect();
    if !active.is_empty() {
        let noun = format!(
            "{} transfer{}",
            active.len(),
            if active.len() == 1 { "" } else { "s" }
        );
        let total: u64 = active.iter().map(|job| job.bytes_total).sum();
        if active.iter().all(|job| job.bytes_total > 0) && total > 0 {
            let done: f64 = active
                .iter()
                .map(|job| job.bytes_total as f64 * job.progress.min(100.) / 100.)
                .sum();
            return format!("{noun} · {}%", (done / total as f64 * 100.).floor());
        }
        return format!("{noun} in progress");
    }
    let failed = jobs
        .iter()
        .filter(|job| job.state == JobState::Failed)
        .count();
    if failed > 0 {
        format!("{failed} failed")
    } else {
        "Transfers complete".into()
    }
}

#[derive(Default)]
pub struct Jobs {
    jobs: HashMap<String, Job>,
    order: Vec<String>,
    /// Finished since launch and not dismissed: shown until "Clear finished".
    dismissed: HashSet<String>,
    /// Cut-paste batches waiting for their jobs before the clipboard is cleared.
    pub(crate) pending_cuts: Vec<(HashSet<String>, Vec<String>)>,
}

impl Jobs {
    fn visible(&self) -> Vec<&Job> {
        self.order
            .iter()
            .filter(|id| !self.dismissed.contains(*id))
            .filter_map(|id| self.jobs.get(id))
            .collect()
    }
}

impl Workspace {
    pub(crate) fn watch_jobs(&mut self, cx: &mut Context<Self>) {
        let mut events = sse::subscribe(self.api.clone(), "/jobs/events".into());
        cx.spawn(async move |this, cx| {
            while let Some(message) = events.next().await {
                let SseMessage::Event { name, data } = message else {
                    continue;
                };
                let jobs: Vec<Job> = match name.as_str() {
                    "snapshot" => {
                        #[derive(Deserialize)]
                        struct Snapshot {
                            jobs: Vec<Job>,
                        }
                        serde_json::from_str::<Snapshot>(&data)
                            .map(|s| s.jobs)
                            .unwrap_or_default()
                    }
                    "job" => serde_json::from_str::<Job>(&data).into_iter().collect(),
                    _ => continue,
                };
                let snapshot = name == "snapshot";
                if this
                    .update(cx, |this, cx| this.upsert_jobs(jobs, snapshot, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    pub(crate) fn upsert_jobs(&mut self, jobs: Vec<Job>, snapshot: bool, cx: &mut Context<Self>) {
        let mut finished = false;
        for job in jobs {
            let was_active = self.jobs.jobs.get(&job.id).is_none_or(Job::is_active);
            if !self.jobs.jobs.contains_key(&job.id) {
                // A snapshot replays history; only list jobs still running.
                if snapshot && !job.is_active() {
                    continue;
                }
                self.jobs.order.push(job.id.clone());
            }
            if was_active && !job.is_active() {
                finished = true;
                if job.state == JobState::Failed {
                    let message = format!(
                        "{}: {}",
                        job.name(),
                        job.error.clone().unwrap_or_else(|| "Failed".into())
                    );
                    self.toast(ToastKind::Error, message, cx);
                }
            }
            self.jobs.jobs.insert(job.id.clone(), job);
        }
        if finished {
            self.finish_cut_pastes(cx);
            self.reload(cx);
        }
        cx.notify();
    }

    /// After a cut paste finishes, clear the cut from the OS and in-app clipboards.
    fn finish_cut_pastes(&mut self, cx: &mut Context<Self>) {
        let pending = std::mem::take(&mut self.jobs.pending_cuts);
        let state = |id: &String| self.jobs.jobs.get(id).map(|job| job.state);
        let mut clear = Vec::new();
        for (ids, paths) in pending {
            let states: Vec<_> = ids.iter().map(state).collect();
            if states.iter().any(|state| {
                state.is_none_or(|state| matches!(state, JobState::Pending | JobState::Running))
            }) {
                self.jobs.pending_cuts.push((ids, paths));
            } else if states
                .iter()
                .all(|state| *state == Some(JobState::Completed))
            {
                clear.push(paths);
            }
        }
        for paths in clear {
            self.clear_cut(paths, cx);
        }
    }

    fn cancel_job(&mut self, id: String, cx: &mut Context<Self>) {
        self.background(
            move |api| api.send("DELETE", &format!("/jobs/{id}"), None).map(|_| ()),
            |this, result, cx| {
                if let Err(error) = result {
                    this.error(&error, cx);
                }
            },
            cx,
        );
    }

    pub(crate) fn render_jobs(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let jobs = self.jobs.visible();
        if jobs.is_empty() {
            return None;
        }
        let any_finished = jobs.iter().any(|job| !job.is_active());
        Some(
            div()
                .w(px(360.))
                .overflow_hidden()
                .rounded(px(8.))
                .border_1()
                .border_color(c(BORDER_PRIMARY))
                .bg(c(SURFACE_SECONDARY))
                .shadow_2xl()
                .text_xs_tw()
                .child(
                    div()
                        .flex()
                        .h(px(36.))
                        .items_center()
                        .justify_between()
                        .gap(px(8.))
                        .pl(px(12.))
                        .pr(px(6.))
                        .border_b_1()
                        .border_color(c(BORDER_SECONDARY))
                        .bg(c(SURFACE_PRIMARY))
                        .child(
                            div()
                                .truncate()
                                .text_13()
                                .font_weight(FontWeight::MEDIUM)
                                .child(transfer_summary(&jobs)),
                        )
                        .when(any_finished, |header| {
                            header.child(
                                div()
                                    .id("clear-finished")
                                    .px(px(8.))
                                    .py(px(4.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .text_color(c(TEXT_SECONDARY))
                                    .hover(|style| {
                                        style.bg(c(SURFACE_ELEVATED)).text_color(c(TEXT_PRIMARY))
                                    })
                                    .child("Clear finished")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let finished: Vec<_> = this
                                            .jobs
                                            .jobs
                                            .values()
                                            .filter(|job| !job.is_active())
                                            .map(|job| job.id.clone())
                                            .collect();
                                        this.jobs.dismissed.extend(finished);
                                        cx.notify();
                                    })),
                            )
                        }),
                )
                .children(jobs.into_iter().map(|job| {
                    let id = job.id.clone();
                    let active = job.is_active();
                    let type_icon = match job.kind {
                        JobType::Copy => "copy",
                        JobType::Move => "folder-input",
                        JobType::Delete => "trash-2",
                    };
                    let status_color = match job.state {
                        JobState::Failed => DANGER,
                        JobState::Completed => SUCCESS,
                        _ => TEXT_MUTED,
                    };
                    let bar = (job.state == JobState::Running
                        && (job.progress > 0. || job.bytes_total > 0))
                        .then_some(job.progress.clamp(0., 100.) as f32 / 100.)
                        .or((job.state == JobState::Completed).then_some(1.));
                    div()
                        .flex()
                        .gap(px(12.))
                        .px(px(12.))
                        .py(px(8.))
                        .border_b_1()
                        .border_color(c(BORDER_SECONDARY))
                        .child(
                            div()
                                .id(gpui::SharedString::from(format!("job-{id}")))
                                .w(px(36.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(4.))
                                .bg(c(SURFACE_ELEVATED))
                                .cursor_pointer()
                                .hover(|style| style.bg(alpha(DANGER, 0.2)))
                                .child(icon(type_icon, 16., c(TEXT_SECONDARY)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if active {
                                        this.cancel_job(id.clone(), cx);
                                    } else {
                                        this.jobs.dismissed.insert(id.clone());
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(6.))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .text_13()
                                                .child(job.name().to_string()),
                                        )
                                        .children(job.destination_folder().map(|folder| {
                                            div()
                                                .max_w(px(150.))
                                                .truncate()
                                                .text_color(c(TEXT_MUTED))
                                                .child(format!("→ {folder}"))
                                        })),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(12.))
                                        .children(bar.map(|value| {
                                            div()
                                                .flex_1()
                                                .h(px(4.))
                                                .rounded(px(2.))
                                                .bg(c(SURFACE_ELEVATED))
                                                .child(
                                                    div()
                                                        .h_full()
                                                        .rounded(px(2.))
                                                        .w(gpui::relative(value))
                                                        .bg(c(
                                                            if job.state == JobState::Completed {
                                                                SUCCESS
                                                            } else {
                                                                ACCENT
                                                            },
                                                        )),
                                                )
                                        }))
                                        .child(
                                            div()
                                                .truncate()
                                                .text_11()
                                                .text_color(c(status_color))
                                                .child(job.label()),
                                        ),
                                ),
                        )
                }))
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(state: JobState, progress: f64, bytes: u64) -> Job {
        Job {
            id: "1".into(),
            kind: JobType::Copy,
            state,
            progress,
            bytes_total: bytes,
            source_path: "home/a.txt".into(),
            dest_path: Some("home/docs/a.txt".into()),
            error: None,
        }
    }

    #[test]
    fn labels_match_job_progress() {
        assert_eq!(job(JobState::Running, 0., 0).label(), "Copying…");
        assert_eq!(job(JobState::Running, 42.7, 2048).label(), "42% of 2 KB");
        assert_eq!(job(JobState::Running, 100., 2048).label(), "Finishing…");
        assert_eq!(
            job(JobState::Pending, 0., 0).destination_folder(),
            Some("docs")
        );
    }

    #[test]
    fn summary_weights_by_size() {
        let (a, b) = (
            job(JobState::Running, 50., 100),
            job(JobState::Running, 100., 300),
        );
        assert_eq!(transfer_summary(&[&a, &b]), "2 transfers · 87%");
    }
}
