//! `desktop/TrashPage.svelte`: restore or permanently delete trashed items. Deleting asks for a
//! second click ("Confirm delete") because it cannot be undone.

use chrono::{DateTime, Local};
use gpui::{div, prelude::*, px, AnyElement, Context, FontWeight, Window};

use crate::api::TrashItem;
use crate::theme::*;
use crate::ui::{icon, TextScale};
use crate::workspace::{Load, Workspace};

/// The Trash page's pane path; `@` cannot start a mount name.
pub const TRASH_PATH: &str = "@trash";

pub struct TrashState {
    pub items: Load<Vec<TrashItem>>,
    pub selected: Option<String>,
    pub confirming: bool,
    pub busy: bool,
    pub error: Option<String>,
}

impl Default for TrashState {
    fn default() -> Self {
        Self {
            items: Load::Loading,
            selected: None,
            confirming: false,
            busy: false,
            error: None,
        }
    }
}

/// `fullFileDate`: medium date, short time.
pub fn full_date(raw: &str) -> String {
    DateTime::parse_from_rfc3339(raw)
        .map(|date| {
            date.with_timezone(&Local)
                .format("%b %-d, %Y, %-I:%M %p")
                .to_string()
        })
        .unwrap_or_else(|_| "Unknown".into())
}

pub fn parent_folder(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

#[derive(Clone, Copy)]
enum TrashAction {
    Restore,
    Delete,
}

fn action_button(
    id: &'static str,
    label: &'static str,
    glyph: &'static str,
    enabled: bool,
    bg: u32,
    action: TrashAction,
    cx: &mut Context<Workspace>,
) -> impl IntoElement {
    let text = if bg == SURFACE_SECONDARY {
        TEXT_SECONDARY
    } else {
        WHITE
    };
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(6.))
        .h(px(28.))
        .px(px(10.))
        .rounded(px(4.))
        .text_xs_tw()
        .bg(c(bg))
        .text_color(c(text))
        .when(bg == SURFACE_SECONDARY, |button| {
            button.border_1().border_color(c(BORDER_PRIMARY))
        })
        .when(!enabled, |button| button.opacity(0.5))
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| this.run_trash_action(action, cx)))
        })
        .child(icon(glyph, 14., c(text)))
        .child(label)
}

impl Workspace {
    pub(crate) fn load_trash(&mut self, cx: &mut Context<Self>) {
        self.background(
            |api| api.trash_items(),
            |this, result, _| {
                this.trash.items = match result {
                    Ok(items) => {
                        if this
                            .trash
                            .selected
                            .as_ref()
                            .is_some_and(|id| !items.iter().any(|item| &item.id == id))
                        {
                            this.trash.selected = None;
                        }
                        Load::Loaded(items)
                    }
                    Err(error) => Load::Failed(format!("{error:#}")),
                };
            },
            cx,
        );
    }

    fn run_trash_action(&mut self, action: TrashAction, cx: &mut Context<Self>) {
        let Some(id) = self.trash.selected.clone() else {
            return;
        };
        if matches!(action, TrashAction::Delete) && !self.trash.confirming {
            self.trash.confirming = true;
            cx.notify();
            return;
        }
        self.trash.confirming = false;
        self.trash.busy = true;
        self.trash.error = None;
        self.background(
            move |api| match action {
                TrashAction::Restore => api.restore(&id),
                TrashAction::Delete => api.delete_from_trash(&id),
            },
            |this, result, cx| {
                this.trash.busy = false;
                if let Err(error) = result {
                    this.trash.error = Some(format!("{error:#}"));
                }
                this.trash.selected = None;
                this.load_trash(cx);
            },
            cx,
        );
    }

    pub(crate) fn render_trash(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let trash = &self.trash;
        let selected = trash.selected.as_ref().and_then(|id| match &trash.items {
            Load::Loaded(items) => items.iter().find(|item| &item.id == id),
            _ => None,
        });
        let delete_label = if trash.confirming {
            "Confirm delete"
        } else {
            "Delete permanently"
        };
        let can_restore = selected.is_some_and(|item| item.can_restore) && !trash.busy;
        let toolbar = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(6.))
            .border_b_1()
            .border_color(c(BORDER_SECONDARY))
            .child(self.render_history_buttons(cx))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(30.))
                    .px(px(12.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(c(BORDER_PRIMARY))
                    .bg(c(SURFACE_SECONDARY))
                    .text_sm_tw()
                    .child(icon("trash-2", 14., c(TEXT_SECONDARY)))
                    .child("Trash"),
            )
            .child(action_button(
                "trash-delete",
                delete_label,
                "trash-2",
                selected.is_some() && !trash.busy,
                if trash.confirming {
                    DANGER
                } else {
                    SURFACE_SECONDARY
                },
                TrashAction::Delete,
                cx,
            ))
            .child(action_button(
                "trash-restore",
                "Restore",
                "undo-2",
                can_restore,
                ACCENT_SOLID,
                TrashAction::Restore,
                cx,
            ))
            .child(
                self.nav_button("trash-refresh", "refresh-cw", 16., true)
                    .on_click(cx.listener(|this, _, _, cx| this.load_trash(cx))),
            );
        let banner = trash.error.clone().map(|error| (error, false)).or_else(|| {
            selected.filter(|_| trash.confirming).map(|item| {
                (
                    format!(
                        "“{}” will be deleted permanently. This cannot be undone.",
                        item.name
                    ),
                    true,
                )
            })
        });
        let content: AnyElement = match &trash.items {
            Load::Loading => div()
                .py(px(40.))
                .flex()
                .justify_center()
                .text_color(c(TEXT_SECONDARY))
                .child("Loading Trash…")
                .into_any_element(),
            Load::Failed(error) => div()
                .py(px(40.))
                .flex()
                .justify_center()
                .gap(px(12.))
                .text_color(c(DANGER))
                .child(error.clone())
                .child(
                    div()
                        .id("trash-retry")
                        .cursor_pointer()
                        .text_color(c(TEXT_PRIMARY))
                        .child("Try again")
                        .on_click(cx.listener(|this, _, _, cx| this.load_trash(cx))),
                )
                .into_any_element(),
            Load::Loaded(items) if items.is_empty() => div()
                .py(px(48.))
                .flex()
                .justify_center()
                .text_color(c(TEXT_MUTED))
                .child("Trash is empty")
                .into_any_element(),
            Load::Loaded(items) => {
                let header = |label: &'static str, width: Option<f32>| {
                    let cell = div()
                        .h(px(32.))
                        .flex()
                        .items_center()
                        .px(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(TEXT_SECONDARY))
                        .child(label);
                    match width {
                        Some(width) => cell.w(px(width)).flex_none(),
                        None => cell.flex_1().min_w_0(),
                    }
                };
                let width = f32::from(window.viewport_size().width) - crate::ui::SIDEBAR_WIDTH;
                div()
                    .id("trash-items")
                    .size_full()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .flex()
                            .bg(c(SURFACE_SECONDARY))
                            .border_b_1()
                            .border_color(c(BORDER_PRIMARY))
                            .child(header("Name", Some(width * 0.4)))
                            .child(header("Original location", None))
                            .child(header("Deleted", Some(176.))),
                    )
                    .children(items.iter().map(|item| {
                        let is_selected = trash.selected.as_deref() == Some(item.id.as_str());
                        let id = item.id.clone();
                        let from = parent_folder(&item.original_path);
                        div()
                            .id(gpui::SharedString::from(format!("trash-{}", item.id)))
                            .flex()
                            .h(px(32.))
                            .items_center()
                            .border_b_1()
                            .border_color(c(BORDER_SECONDARY))
                            .cursor_default()
                            .when(is_selected, |row| row.bg(c(SELECTION)))
                            .when(!is_selected, |row| {
                                row.hover(|style| style.bg(c(SURFACE_SECONDARY)))
                            })
                            .child(
                                div()
                                    .w(px(width * 0.4))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .px(px(12.))
                                    .min_w_0()
                                    .child(if item.is_dir {
                                        icon("folder", 16., c(FOLDER))
                                    } else {
                                        icon("file", 16., c(TEXT_SECONDARY))
                                    })
                                    .child(div().truncate().child(item.name.clone())),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .px(px(12.))
                                    .truncate()
                                    .text_color(c(TEXT_SECONDARY))
                                    .child(if from.is_empty() {
                                        "—".to_string()
                                    } else {
                                        from.to_string()
                                    }),
                            )
                            .child(
                                div()
                                    .w(px(176.))
                                    .flex_none()
                                    .px(px(12.))
                                    .truncate()
                                    .text_color(c(TEXT_SECONDARY))
                                    .child(full_date(&item.deleted_at)),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if this.trash.selected.as_ref() != Some(&id) {
                                    this.trash.confirming = false;
                                }
                                this.trash.selected = Some(id.clone());
                                cx.notify();
                            }))
                    }))
                    .into_any_element()
            }
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .text_13()
            .bg(c(SURFACE_PRIMARY))
            .child(toolbar)
            .children(banner.map(|(text, _)| {
                div()
                    .flex_none()
                    .px(px(12.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(c(BORDER_SECONDARY))
                    .text_color(c(DANGER))
                    .child(text)
            }))
            .child(div().flex_1().min_h_0().child(content))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_folder_and_dates() {
        assert_eq!(parent_folder("home/docs/a.txt"), "home/docs");
        assert_eq!(parent_folder("a.txt"), "");
        assert_eq!(full_date("nope"), "Unknown");
        assert!(full_date("2026-01-02T10:00:00Z").contains("2026"));
    }
}
