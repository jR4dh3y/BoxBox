//! `desktop/OpenWithDialog.svelte`: search, Recommended and Other applications,
//! "Use as default", and Open.

use gpui::{div, img, prelude::*, px, AnyElement, Context, MouseButton, ObjectFit, SharedString, Window};

use super::{icon, TextScale};
use crate::components::{Button, Input, Modal};
use crate::open_with::{self, App};
use crate::theme::*;
use crate::workspace::{Load, Workspace};

impl Workspace {
    /// `appRow`: a radio, the app's icon, its name, and "Default".
    fn open_with_row(&self, app: &App, chosen: Option<&str>, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = chosen == Some(app.id.as_str());
        let id = app.id.clone();
        let accent = c(self.accent_hex());
        // h-8 gap-3 px-3 text-[13px]; checked rows take bg-selection
        div()
            .id(SharedString::from(format!("open-with-{}", app.id)))
            .flex()
            .h(px(32.))
            .flex_none()
            .items_center()
            .gap(px(12.))
            .px(px(12.))
            .text_13()
            .cursor_pointer()
            .map(|row| if selected { row.bg(self.selection_color(false)) } else { row.hover(|style| style.bg(c(SURFACE_SECONDARY))) })
            .child(
                div()
                    .size(px(14.))
                    .flex_none()
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .map(|radio| match selected {
                        true => radio.bg(accent).child(div().size(px(6.)).rounded_full().bg(c(WHITE))),
                        false => radio.bg(c(WHITE)).border_1().border_color(c(0x767676)),
                    }),
            )
            .child(
                div().size(px(24.)).flex_none().flex().items_center().justify_center().map(|slot| match &app.icon {
                    Some(path) => slot.child(img(path.clone()).size(px(24.)).object_fit(ObjectFit::Contain)),
                    None => slot.child(icon("app-window", 22., c(TEXT_MUTED))),
                }),
            )
            .child(div().flex_1().min_w_0().truncate().text_color(c(TEXT_PRIMARY)).child(app.name.clone()))
            .when(app.is_default, |row| row.child(div().flex_none().text_color(c(TEXT_MUTED)).child("Default")))
            .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                if let Some(dialog) = this.open_with.as_mut() {
                    dialog.selected = Some(id.clone());
                }
                if event.click_count() >= 2 {
                    this.confirm_open_with(cx);
                }
                cx.notify();
            }))
    }

    pub(crate) fn render_open_with(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(dialog) = &self.open_with else { return div().into_any_element() };
        let chosen = self.open_with_choice(cx);
        let search = self.open_with_search.clone();
        let focused = search.read(cx).is_focused(window);
        let body: AnyElement = match &dialog.apps {
            Load::Loading => div()
                .py(px(8.))
                .text_13()
                .text_color(c(TEXT_SECONDARY))
                .child("Loading applications…")
                .into_any_element(),
            Load::Failed(error) => div().text_13().text_color(c(DANGER)).child(error.clone()).into_any_element(),
            Load::Loaded(apps) if apps.recommended.is_empty() && apps.other.is_empty() => div()
                .text_13()
                .text_color(c(TEXT_SECONDARY))
                .child("No installed application can open this file.")
                .into_any_element(),
            Load::Loaded(apps) => {
                let query = search.read(cx).text().to_string();
                let (recommended, other) = open_with::filter(apps, &query);
                let mime = if apps.mime_type.is_empty() { "this file type".to_string() } else { apps.mime_type.clone() };
                let heading = |text: &'static str, rule: bool| {
                    div()
                        .px(px(12.))
                        .pt(px(8.))
                        .pb(px(4.))
                        .text_xs_tw()
                        .text_color(c(TEXT_MUTED))
                        .when(rule, |heading| heading.border_t_1().border_color(c(BORDER_SECONDARY)))
                        .child(text)
                };
                let empty_note = if query.trim().is_empty() {
                    format!("No application is registered for {mime}.")
                } else {
                    "No recommended application matches.".to_string()
                };
                let make_default = dialog.make_default;
                div()
                    .flex()
                    .flex_col()
                    .child(Input::new("").focused(focused).editor(search))
                    .child(
                        // mt-2 max-h-72 overflow-y-auto rounded border
                        div()
                            .id("open-with-list")
                            .mt(px(8.))
                            .max_h(px(288.))
                            .overflow_y_scroll()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(c(BORDER_SECONDARY))
                            .flex()
                            .flex_col()
                            .child(heading("Recommended", false))
                            .children(recommended.iter().map(|app| self.open_with_row(app, chosen.as_deref(), cx).into_any_element()))
                            .when(recommended.is_empty(), |list| {
                                list.child(div().px(px(12.)).pb(px(8.)).text_13().text_color(c(TEXT_SECONDARY)).child(empty_note))
                            })
                            .when(!other.is_empty(), |list| {
                                list.child(heading("Other applications", true))
                                    .children(other.iter().map(|app| self.open_with_row(app, chosen.as_deref(), cx).into_any_element()))
                            }),
                    )
                    .child(
                        // mt-3 flex items-center gap-2 text-[13px] text-text-secondary
                        div()
                            .id("open-with-default")
                            .mt(px(12.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .text_13()
                            .text_color(c(TEXT_SECONDARY))
                            .cursor_pointer()
                            .child(
                                div()
                                    .size(px(14.))
                                    .rounded(px(2.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .map(|box_| match make_default {
                                        true => box_.bg(c(self.accent_hex())).child(icon("check", 11., c(WHITE))),
                                        false => box_.bg(c(WHITE)).border_1().border_color(c(0x767676)),
                                    }),
                            )
                            .child(format!("Use as default for {mime}"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(dialog) = this.open_with.as_mut() {
                                    dialog.make_default = !dialog.make_default;
                                }
                                cx.notify();
                            })),
                    )
                    .into_any_element()
            }
        };
        let launching = dialog.launching;
        div()
            .id("open-with-overlay")
            .absolute()
            .inset_0()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Modal::new()
                    .title(format!("Open “{}” with", dialog.item.name))
                    .on_close(cx.listener(|this, _, window, cx| this.close_open_with(window, cx)))
                    .body(body)
                    .footer(
                        Button::new("open-with-confirm")
                            .label(if launching { "Opening…" } else { "Open" })
                            .disabled(chosen.is_none() || launching)
                            .on_click(cx.listener(|this, _, _, cx| this.confirm_open_with(cx))),
                    ),
            )
            .into_any_element()
    }
}
