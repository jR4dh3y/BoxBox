//! `desktop/TransferJobs.svelte`: running and finished transfers, under the notices.

use gpui::{div, prelude::*, px, AnyElement, Context, FontWeight, SharedString};

use super::{icon, TextScale};
use crate::components::{ProgressBar, ProgressSize, ProgressVariant};
use crate::format::file_size;
use crate::theme::*;
use crate::workspace::jobs::{Job, JobState};
use crate::workspace::Workspace;

/// `jobProgressLabel` and the bar's value; `None` hides the bar for ended jobs.
fn job_status(job: &Job) -> (String, Option<f32>, u32) {
    match &job.state {
        JobState::Running => {
            let (done, total) = (job.progress.done(), job.progress.total());
            if total == 0 {
                (if job.moving { "Moving…" } else { "Copying…" }.into(), Some(0.), TEXT_MUTED)
            } else {
                let percent = (done as f64 / total as f64 * 100.).min(100.);
                (format!("{}% of {}", percent.floor(), file_size(total)), Some(percent as f32), TEXT_MUTED)
            }
        }
        JobState::Done => ("Done".into(), Some(100.), SUCCESS),
        JobState::Failed(message) => (message.clone(), None, DANGER),
        JobState::Cancelled => ("Cancelled".into(), None, TEXT_MUTED),
    }
}

/// `transferSummary`.
fn summary(jobs: &[Job]) -> String {
    let running: Vec<&Job> = jobs.iter().filter(|job| matches!(job.state, JobState::Running)).collect();
    let plural = |count: usize| if count == 1 { "" } else { "s" };
    if !running.is_empty() {
        let (done, total) = running.iter().fold((0, 0), |(done, total), job| (done + job.progress.done(), total + job.progress.total()));
        let noun = format!("{} transfer{}", running.len(), plural(running.len()));
        return if total == 0 { format!("{noun} in progress") } else { format!("{noun} · {}%", (done * 100 / total).min(100)) };
    }
    let failed = jobs.iter().filter(|job| matches!(job.state, JobState::Failed(_))).count();
    if failed > 0 {
        return format!("{failed} transfer{} failed", plural(failed));
    }
    format!("{} transfer{} finished", jobs.len(), plural(jobs.len()))
}

impl Workspace {
    pub(crate) fn render_jobs(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.jobs.is_empty() {
            return div().into_any_element();
        }
        let finished = self.jobs.iter().any(|job| !matches!(job.state, JobState::Running));
        let rows = self
            .jobs
            .iter()
            .map(|job| {
                let (status, bar, status_color) = job_status(job);
                let running = matches!(job.state, JobState::Running);
                let id = job.id;
                let group = SharedString::from(format!("job-{id}"));
                let glyph = if job.moving { "folder-input" } else { "copy" };
                // flex gap-3 border-b px-3 py-2; the type icon doubles as cancel or dismiss
                div()
                    .flex()
                    .gap(px(12.))
                    .border_b_1()
                    .border_color(c(BORDER_SECONDARY))
                    .px(px(12.))
                    .py(px(8.))
                    .child(
                        div()
                            .id(SharedString::from(format!("job-button-{id}")))
                            .group(group.clone())
                            .w(px(36.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .bg(c(SURFACE_ELEVATED))
                            .cursor_pointer()
                            .hover(|style| style.bg(alpha(DANGER, 0.2)))
                            .child(div().group_hover(group.clone(), |style| style.invisible()).child(icon(glyph, 16., c(TEXT_SECONDARY))))
                            .child(div().absolute().invisible().group_hover(group, |style| style.visible()).child(icon("x", 16., c(DANGER))))
                            .on_click(cx.listener(move |this, _, _, cx| this.cancel_or_dismiss_job(id, cx))),
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
                                    .min_w_0()
                                    .gap(px(6.))
                                    .child(div().flex_1().min_w_0().truncate().text_13().text_color(c(TEXT_PRIMARY)).child(job.title.clone()))
                                    .child(div().max_w(px(150.)).truncate().text_xs_tw().text_color(c(TEXT_MUTED)).child(format!("→ {}", job.folder))),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(12.))
                                    .when_some(bar, |line, value| {
                                        line.child(div().flex_1().child(ProgressBar::new(value).size(ProgressSize::Sm).variant(if running {
                                            ProgressVariant::Default
                                        } else {
                                            ProgressVariant::Success
                                        })))
                                    })
                                    .child(
                                        div()
                                            .when(bar.is_none(), |label| label.flex_1().min_w_0())
                                            .when(bar.is_some(), |label| label.flex_none())
                                            .truncate()
                                            .text_size(px(11.))
                                            .text_color(c(status_color))
                                            .child(status),
                                    ),
                            ),
                    )
            })
            .collect::<Vec<_>>();
        // rounded-lg border bg-surface-secondary text-xs shadow-2xl
        div()
            .id("transfer-jobs")
            .rounded(px(8.))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_SECONDARY))
            .overflow_hidden()
            .shadow_2xl()
            .child(
                // h-9 justify-between border-b bg-surface-primary pr-1.5 pl-3
                div()
                    .flex()
                    .h(px(36.))
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .border_b_1()
                    .border_color(c(BORDER_SECONDARY))
                    .bg(c(SURFACE_PRIMARY))
                    .pr(px(6.))
                    .pl(px(12.))
                    .child(div().truncate().text_13().font_weight(FontWeight::MEDIUM).text_color(c(TEXT_PRIMARY)).child(summary(&self.jobs)))
                    .when(finished, |header| {
                        header.child(
                            div()
                                .id("jobs-clear")
                                .flex_none()
                                .rounded(px(4.))
                                .px(px(8.))
                                .py(px(4.))
                                .text_xs_tw()
                                .text_color(c(TEXT_SECONDARY))
                                .cursor_pointer()
                                .hover(|style| style.bg(c(SURFACE_ELEVATED)).text_color(c(TEXT_PRIMARY)))
                                .child("Clear finished")
                                .on_click(cx.listener(|this, _, _, cx| this.clear_finished_jobs(cx))),
                        )
                    }),
            )
            .child(div().id("jobs-list").max_h(px(256.)).overflow_y_scroll().children(rows))
            .into_any_element()
    }
}
