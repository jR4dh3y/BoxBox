//! Removable volumes in the sidebar (`api/desktopVolumes.ts`), polled every 5 s, and the F1
//! keyboard shortcuts dialog (`KeyboardShortcutsDialog.svelte`).

use std::time::Duration;

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Window};
use serde::Deserialize;

use crate::theme::*;
use crate::ui::modal::modal;
use crate::ui::{icon, TextScale};
use crate::workspace::Workspace;

const POLL: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub browse_path: Option<String>,
    #[serde(default)]
    pub mounted: bool,
    #[serde(default)]
    pub can_mount: bool,
    #[serde(default)]
    pub can_unmount: bool,
    #[serde(default)]
    pub can_eject: bool,
}

#[derive(Deserialize)]
pub struct VolumeList {
    #[serde(default)]
    pub volumes: Vec<Volume>,
}

pub const SHORTCUTS: &[(&str, &[(&str, &str)])] = &[
    (
        "Files and selection",
        &[
            (
                "Click · Shift + Click · Ctrl + Click",
                "Select · Extend · Toggle",
            ),
            ("Ctrl + A", "Select all loaded items"),
            ("Escape", "Clear selection or close a dialog"),
            ("Enter", "Open selected item"),
            ("Space", "Preview selected file"),
            ("Alt + Enter", "Show selection details"),
            ("Ctrl + C · X · V", "Copy · Cut · Paste"),
            ("Ctrl + Shift + N", "New folder"),
            ("F2", "Rename selected item"),
            ("Delete", "Move to Trash"),
            ("Shift + Delete", "Permanently delete with confirmation"),
        ],
    ),
    (
        "Browse",
        &[
            ("Ctrl + L", "Edit location"),
            ("Ctrl + F", "Search this folder"),
            ("Alt + Left · Right · Up", "Back · Forward · Parent folder"),
            ("F5", "Refresh"),
            ("Ctrl + H", "Show or hide hidden files"),
            ("Ctrl + T · W", "New tab · Close tab"),
            ("Ctrl + Tab", "Next tab"),
            ("Ctrl + Shift + Tab", "Previous tab"),
            ("F3", "Toggle split pane"),
            ("F1", "Show keyboard shortcuts"),
        ],
    ),
    (
        "Desktop",
        &[
            ("Ctrl + Z", "Undo the last rename or move to Trash"),
            ("Drag · Ctrl + Drag", "Move · Copy to a folder or pane"),
            ("Drag out of the window", "Copy to another app (X11)"),
        ],
    ),
];

impl Workspace {
    pub(crate) fn watch_volumes(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            let api = match this.update(cx, |this, _| this.api.clone()) {
                Ok(api) => api,
                Err(_) => break,
            };
            let result = cx
                .background_executor()
                .spawn(async move { api.volumes() })
                .await;
            if this
                .update(cx, |this, cx| {
                    let volumes = result.map(|list| list.volumes).unwrap_or_default();
                    if volumes != this.volumes {
                        this.volumes = volumes;
                        cx.notify();
                    }
                })
                .is_err()
            {
                break;
            }
            cx.background_executor().timer(POLL).await;
        })
        .detach();
    }

    fn volume_action(&mut self, volume: Volume, action: &'static str, cx: &mut Context<Self>) {
        let id = volume.id.clone();
        self.background(
            move |api| {
                api.volume_action(&id, action)?;
                Ok(api.volumes()?.volumes)
            },
            move |this, result, cx| match result {
                Ok(volumes) => {
                    let mounted = volumes
                        .iter()
                        .find(|v| v.id == volume.id)
                        .and_then(|v| v.browse_path.clone());
                    this.volumes = volumes;
                    this.load_drives(cx);
                    if action == "mount" {
                        if let Some(path) = mounted {
                            this.navigate(path, cx);
                        }
                    }
                }
                Err(error) => this.error(&error, cx),
            },
            cx,
        );
    }

    pub(crate) fn render_devices(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.volumes.is_empty() {
            return None;
        }
        Some(
            div()
                .py(px(8.))
                .border_b_1()
                .border_color(c(BORDER_SECONDARY))
                .child(
                    div()
                        .px(px(12.))
                        .pb(px(4.))
                        .text_11()
                        .text_color(c(TEXT_SECONDARY))
                        .child("DEVICES"),
                )
                .children(self.volumes.iter().enumerate().map(|(index, volume)| {
                    let (open, eject) = (volume.clone(), volume.clone());
                    let ejectable = volume.can_eject || volume.can_unmount;
                    div()
                        .flex()
                        .items_center()
                        .pr(px(8.))
                        .child(
                            div()
                                .id(("volume", index))
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .py(px(6.))
                                .pl(px(20.))
                                .text_13()
                                .cursor_pointer()
                                .hover(|style| style.bg(c(SURFACE_SECONDARY)))
                                .child(icon("hard-drive", 16., alpha(TEXT_PRIMARY, 0.8)))
                                .child(div().truncate().child(volume.name.clone()))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    match (&open.browse_path, open.mounted) {
                                        (Some(path), true) => this.navigate(path.clone(), cx),
                                        _ if open.can_mount => {
                                            this.volume_action(open.clone(), "mount", cx)
                                        }
                                        _ => {}
                                    }
                                })),
                        )
                        .when(ejectable, |row| {
                            row.child(
                                div()
                                    .id(("eject", index))
                                    .cursor_pointer()
                                    .child(icon("arrow-up-from-line", 14., c(TEXT_SECONDARY)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let action =
                                            if eject.can_eject { "eject" } else { "unmount" };
                                        this.volume_action(eject.clone(), action, cx)
                                    })),
                            )
                        })
                        .into_any_element()
                }))
                .into_any_element(),
        )
    }

    pub(crate) fn render_shortcuts(
        &self,
        on_close: impl Fn(&mut Window, &mut gpui::App) + 'static,
    ) -> AnyElement {
        let body = div()
            .id("shortcuts")
            .flex()
            .flex_col()
            .gap(px(16.))
            .max_h(px(480.))
            .overflow_y_scroll()
            .children(SHORTCUTS.iter().map(|(title, rows)| {
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .pb(px(6.))
                            .text_11()
                            .text_color(c(TEXT_MUTED))
                            .child(SharedString::from(title.to_uppercase())),
                    )
                    .children(rows.iter().map(|(keys, description)| {
                        div()
                            .flex()
                            .justify_between()
                            .gap(px(16.))
                            .py(px(5.))
                            .border_b_1()
                            .border_color(c(BORDER_SECONDARY))
                            .text_13()
                            .child(
                                div()
                                    .text_color(c(TEXT_PRIMARY))
                                    .child(SharedString::from(*description)),
                            )
                            .child(
                                div()
                                    .font_family(self.fonts.mono.clone())
                                    .text_xs_tw()
                                    .text_color(c(TEXT_SECONDARY))
                                    .child(SharedString::from(*keys)),
                            )
                    }))
            }));
        modal(
            "Keyboard shortcuts",
            576.,
            body.into_any_element(),
            Vec::new(),
            on_close,
        )
        .into_any_element()
    }
}
