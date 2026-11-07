//! Copy, cut and paste (`browse/paste.svelte.ts`, `browse/transfers.ts`).

use std::collections::HashSet;

use anyhow::{bail, Result};
use gpui::{div, prelude::*, px, AnyElement, Context, Window};
use serde::Deserialize;

use crate::dialogs::Dialog;
use crate::jobs::{Job, JobState};
use crate::notifications::ToastKind;
use crate::theme::*;
use crate::ui::modal::{button, modal, ButtonKind};
use crate::ui::TextScale;
use crate::workspace::Workspace;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipOp {
    Copy,
    Cut,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipItem {
    pub path: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClipContents {
    pub operation: ClipOp,
    pub items: Vec<ClipItem>,
}

/// The in-app clipboard. `published` is false when the OS clipboard could not be updated,
/// in which case this copy is newer and wins.
#[derive(Default)]
pub struct Clipboard {
    contents: Option<ClipContents>,
    published: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Planned {
    pub source: String,
    pub name: String,
    /// `Some` when the name is taken: the "Keep both" name.
    pub keep_both: Option<String>,
}

/// `stem (copy N).ext`: N from 1, the extension is the last dot not at the start or end.
pub fn keep_both_name(name: &str, taken: &HashSet<String>) -> String {
    let (stem, ext) = match name.rfind('.') {
        Some(dot) if dot > 0 && dot < name.len() - 1 => (&name[..dot], &name[dot..]),
        _ => (name, ""),
    };
    (1..)
        .map(|n| format!("{stem} (copy {n}){ext}"))
        .find(|candidate| !taken.contains(candidate))
        .unwrap_or_default()
}

/// `planTransfers`: one entry per item; conflicts get a unique keep-both name.
pub fn plan(items: &[ClipItem], existing: &[String]) -> Result<Vec<Planned>> {
    let mut taken: HashSet<String> = existing.iter().cloned().collect();
    let mut planned = Vec::new();
    for item in items {
        if matches!(item.name.as_str(), "" | "." | "..") || item.name.contains(['/', '\\']) {
            bail!("Cannot paste {:?}", item.name);
        }
        let keep_both = taken
            .contains(&item.name)
            .then(|| keep_both_name(&item.name, &taken));
        taken.insert(keep_both.clone().unwrap_or_else(|| item.name.clone()));
        planned.push(Planned {
            source: item.path.clone(),
            name: item.name.clone(),
            keep_both,
        });
    }
    Ok(planned)
}

impl Workspace {
    pub(crate) fn copy_selection(&mut self, op: ClipOp, cx: &mut Context<Self>) {
        let items: Vec<ClipItem> = self
            .pane()
            .selected_items()
            .into_iter()
            .map(|item| ClipItem {
                path: item.path,
                name: item.name,
            })
            .collect();
        if items.is_empty() || (op == ClipOp::Cut && !self.can_write_here()) {
            return;
        }
        let paths: Vec<String> = items.iter().map(|item| item.path.clone()).collect();
        self.clipboard = Clipboard {
            contents: Some(ClipContents {
                operation: op,
                items,
            }),
            published: false,
        };
        let operation = if op == ClipOp::Cut { "cut" } else { "copy" };
        self.background(
            move |api| api.set_clipboard(operation, &paths),
            |this, result, _| this.clipboard.published = result.is_ok(),
            cx,
        );
        cx.notify();
    }

    pub(crate) fn paste(&mut self, cx: &mut Context<Self>) {
        if !self.can_write_here() {
            return;
        }
        let dest = self.pane().path.clone();
        let local = (!self.clipboard.published)
            .then(|| self.clipboard.contents.clone())
            .flatten();
        let fallback = self.clipboard.contents.clone();
        self.background(
            move |api| {
                // Prefer the OS clipboard, so files copied in other apps paste here too.
                match local {
                    Some(contents) => Ok(contents),
                    None => api.clipboard().or_else(|error| fallback.ok_or(error)),
                }
            },
            move |this, result, cx| match result {
                Ok(contents) if contents.items.is_empty() => {
                    this.toast(ToastKind::Info, "The clipboard has no files to paste", cx)
                }
                Ok(contents) => this.transfer(contents.operation, contents.items, dest, cx),
                Err(error) => this.error(&error, cx),
            },
            cx,
        );
    }

    /// Plans against the destination's names; conflicts ask Skip or Keep both first.
    pub(crate) fn transfer(
        &mut self,
        op: ClipOp,
        items: Vec<ClipItem>,
        dest: String,
        cx: &mut Context<Self>,
    ) {
        let list_dest = dest.clone();
        self.background(
            move |api| {
                let existing: Vec<String> = api
                    .list(&list_dest)?
                    .into_iter()
                    .map(|item| item.name)
                    .collect();
                plan(&items, &existing)
            },
            move |this, result, cx| match result {
                Ok(planned) if planned.iter().any(|item| item.keep_both.is_some()) => {
                    let choices = vec![false; planned.len()];
                    this.dialog = Some(Dialog::Conflict {
                        op,
                        dest,
                        planned,
                        keep_both: choices,
                    });
                    cx.notify();
                }
                Ok(planned) => this.submit_paste(op, dest, planned, cx),
                Err(error) => this.error(&error, cx),
            },
            cx,
        );
    }

    /// Submits sequentially and stops at the first error, like `submitTransfers`.
    pub(crate) fn submit_paste(
        &mut self,
        op: ClipOp,
        dest: String,
        planned: Vec<Planned>,
        cx: &mut Context<Self>,
    ) {
        let kind = if op == ClipOp::Cut { "move" } else { "copy" };
        let sources: Vec<String> = planned.iter().map(|item| item.source.clone()).collect();
        self.background(
            move |api| {
                let mut jobs = Vec::new();
                for item in &planned {
                    let name = item.keep_both.as_deref().unwrap_or(&item.name);
                    jobs.push(api.create_job(kind, &item.source, &format!("{dest}/{name}"))?);
                }
                Ok(jobs)
            },
            move |this, result, cx| match result {
                Ok(jobs) => {
                    if op == ClipOp::Cut && jobs.len() == sources.len() {
                        let ids = jobs.iter().map(|job| job.id.clone()).collect();
                        this.jobs.pending_cuts.push((ids, sources));
                    }
                    let failed: Vec<Job> = jobs
                        .iter()
                        .filter(|job| job.state == JobState::Failed)
                        .cloned()
                        .collect();
                    this.upsert_jobs(jobs, false, cx);
                    if failed.is_empty() {
                        this.reload(cx);
                    }
                }
                Err(error) => this.error(&error, cx),
            },
            cx,
        );
    }

    pub(crate) fn clear_cut(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        let still_cut = self.clipboard.contents.as_ref().is_some_and(|contents| {
            contents.operation == ClipOp::Cut
                && contents
                    .items
                    .iter()
                    .map(|item| &item.path)
                    .eq(paths.iter())
        });
        if still_cut {
            self.clipboard = Clipboard::default();
        }
        self.background(
            move |api| api.set_clipboard("clear-cut", &paths),
            |_, _, _| {},
            cx,
        );
    }

    pub(crate) fn render_conflict(
        &self,
        op: ClipOp,
        dest: &str,
        planned: &[Planned],
        keep_both: &[bool],
        cx: &mut Context<Self>,
        on_close: impl Fn(&mut Window, &mut gpui::App) + 'static,
    ) -> AnyElement {
        let _ = op;
        let folder = dest.rsplit('/').next().unwrap_or(dest).to_string();
        let kept = planned
            .iter()
            .zip(keep_both)
            .filter(|(item, keep)| item.keep_both.is_none() || **keep)
            .count();
        let rows = planned
            .iter()
            .enumerate()
            .filter(|(_, item)| item.keep_both.is_some())
            .map(|(index, item)| {
                let keep = keep_both[index];
                let toggle = |label: &'static str, active: bool, value: bool| {
                    div()
                        .id((label, index))
                        .px(px(10.))
                        .py(px(3.))
                        .rounded(px(4.))
                        .cursor_pointer()
                        .text_xs_tw()
                        .when(active, |chip| {
                            chip.bg(c(SURFACE_ELEVATED)).text_color(c(TEXT_PRIMARY))
                        })
                        .when(!active, |chip| chip.text_color(c(TEXT_SECONDARY)))
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(Dialog::Conflict { keep_both, .. }) = &mut this.dialog {
                                keep_both[index] = value;
                                cx.notify();
                            }
                        }))
                };
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .py(px(6.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_13()
                            .child(item.name.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(2.))
                            .p(px(2.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(c(BORDER_PRIMARY))
                            .child(toggle("Skip", !keep, false))
                            .child(toggle("Keep both", keep, true)),
                    )
            });
        let label = if kept == 0 {
            "Skip all".to_string()
        } else {
            format!(
                "Continue with {kept} item{}",
                if kept == 1 { "" } else { "s" }
            )
        };
        let this = cx.entity().downgrade();
        modal(
            format!("Already in {folder}"),
            480.,
            div()
                .flex()
                .flex_col()
                .max_h(px(320.))
                .children(rows)
                .into_any_element(),
            vec![button(
                "conflict-continue",
                label,
                ButtonKind::Primary,
                move |_, window, cx| {
                    this.update(cx, |this, cx| this.confirm_dialog(window, cx))
                        .ok();
                },
            )
            .into_any_element()],
            on_close,
        )
        .into_any_element()
    }
}

/// The items a conflict dialog submits: non-conflicting ones plus conflicts set to keep both.
pub fn resolve(planned: Vec<Planned>, keep_both: &[bool]) -> Vec<Planned> {
    planned
        .into_iter()
        .zip(keep_both)
        .filter(|(item, keep)| item.keep_both.is_none() || **keep)
        .map(|(item, _)| item)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(name: &str) -> ClipItem {
        ClipItem {
            path: format!("home/src/{name}"),
            name: name.into(),
        }
    }

    #[test]
    fn keep_both_names_are_unique() {
        let taken: HashSet<String> = ["a.txt", "a (copy 1).txt"].map(String::from).into();
        assert_eq!(keep_both_name("a.txt", &taken), "a (copy 2).txt");
        assert_eq!(keep_both_name(".env", &HashSet::new()), ".env (copy 1)");
        assert_eq!(
            keep_both_name("archive.", &HashSet::new()),
            "archive. (copy 1)"
        );
    }

    #[test]
    fn plan_flags_conflicts_and_resolve_skips_them() {
        let planned = plan(
            &[clip("a.txt"), clip("b.txt"), clip("a.txt")],
            &["a.txt".into()],
        )
        .unwrap();
        assert_eq!(planned[0].keep_both.as_deref(), Some("a (copy 1).txt"));
        assert_eq!(planned[1].keep_both, None);
        assert_eq!(planned[2].keep_both.as_deref(), Some("a (copy 2).txt"));
        let names: Vec<_> = resolve(planned, &[false, false, true])
            .into_iter()
            .map(|item| item.keep_both.unwrap_or(item.name))
            .collect();
        assert_eq!(names, ["b.txt", "a (copy 2).txt"]);
        assert!(plan(&[clip("..")], &[]).is_err());
    }
}
