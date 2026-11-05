//! `Toolbar.svelte` with `EditablePathBar.svelte` and the compact `SearchBar.svelte`.

use std::time::Duration;

use gpui::{
    div, percentage, prelude::*, px, Animation, AnimationExt as _, AnyElement, Context,
    MouseButton, Transformation, Window,
};

use super::{icon, TextScale, SIDEBAR_WIDTH};
use crate::pane::Search;
use crate::search;
use crate::theme::*;
use crate::workspace::{Back, Forward, OpenSettings, Refresh, Workspace};

struct Crumb {
    label: String,
    path: String,
}

impl Workspace {
    /// Root label, then the path below it, like `EditablePathBar`'s breadcrumbs.
    fn breadcrumbs(&self) -> Vec<Crumb> {
        let path = &self.pane().path;
        let root = self
            .roots
            .iter()
            .find(|root| path == &root.name || path.starts_with(&format!("{}/", root.name)));
        let (mut crumbs, rest, mut prefix) = match root {
            Some(root) => (
                vec![Crumb {
                    label: root.display_name().to_string(),
                    path: root.name.clone(),
                }],
                &path[root.name.len()..],
                root.name.clone(),
            ),
            None => (Vec::new(), path.as_str(), String::new()),
        };
        for segment in rest.split('/').filter(|segment| !segment.is_empty()) {
            prefix = if prefix.is_empty() {
                segment.to_string()
            } else {
                format!("{prefix}/{segment}")
            };
            crumbs.push(Crumb {
                label: segment.to_string(),
                path: prefix.clone(),
            });
        }
        crumbs
    }

    fn nav_button(
        &self,
        id: &'static str,
        glyph: &'static str,
        size: f32,
        enabled: bool,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .size(px(28.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .when(enabled, |button| {
                button
                    .cursor_pointer()
                    .hover(|style| style.bg(c(SURFACE_ELEVATED)))
            })
            .child(icon(
                glyph,
                size,
                c(if enabled {
                    TEXT_SECONDARY
                } else {
                    TEXT_DISABLED
                }),
            ))
    }

    fn render_path_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let crumbs = self.breadcrumbs();
        let last = crumbs.len().saturating_sub(1);
        let content: AnyElement = if crumbs.is_empty() {
            div()
                .text_13()
                .whitespace_nowrap()
                .child("This PC")
                .into_any_element()
        } else {
            let mut row = div()
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap(px(4.))
                .overflow_hidden()
                .whitespace_nowrap();
            for (index, crumb) in crumbs.into_iter().enumerate() {
                if index > 0 {
                    row = row.child(div().text_xs_tw().text_color(c(TEXT_MUTED)).child("/"));
                }
                if index == last {
                    row = row.child(div().text_13().whitespace_nowrap().child(crumb.label));
                } else {
                    let path = crumb.path;
                    row = row.child(
                        div()
                            .id(("crumb", index))
                            .text_13()
                            .whitespace_nowrap()
                            .text_color(c(TEXT_SECONDARY))
                            .cursor_pointer()
                            .hover(|style| style.text_color(c(WHITE)).underline())
                            .child(crumb.label)
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.navigate(path.clone(), cx)),
                            ),
                    );
                }
            }
            row.into_any_element()
        };
        div()
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap(px(6.))
            .rounded(px(4.))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_SECONDARY))
            .px(px(8.))
            .py(px(4.))
            .child(
                div()
                    .id("home")
                    .size(px(18.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .child(icon("home", 14., c(TEXT_SECONDARY)))
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(String::new(), cx))),
            )
            .child(content)
    }

    /// `SearchBar.svelte` (compact): a focusable field fed by key-down events, so text goes in
    /// at the end only and there is no IME or caret movement.
    fn render_search(
        &self,
        toolbar_width: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // `w-28 @lg:w-48 @3xl:w-64 @5xl:w-96` against the toolbar's container width.
        let width = match toolbar_width {
            w if w >= 1024. => 384.,
            w if w >= 768. => 256.,
            w if w >= 512. => 192.,
            _ => 112.,
        };
        let pane = self.pane();
        let focused = self.search_focus.is_focused(window);
        let input = &pane.search_input;
        // Icon, clear button and padding take about 70px; 14px text averages 7px a character.
        let max_chars = ((width - 70.) / 7.0_f32).max(4.) as usize;
        let shown = search::visible_tail(input, max_chars);
        let loading = matches!(pane.search, Search::Loading);
        let glyph: AnyElement = if loading {
            icon("refresh-cw", 16., c(ACCENT))
                .with_animation(
                    "search-spinner",
                    Animation::new(Duration::from_secs(1)).repeat(),
                    |svg, delta| svg.with_transformation(Transformation::rotate(percentage(delta))),
                )
                .into_any_element()
        } else {
            icon("search", 16., c(TEXT_MUTED)).into_any_element()
        };
        div()
            .id("search")
            .key_context("SearchInput")
            .track_focus(&self.search_focus)
            .on_key_down(cx.listener(Self::search_key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    window.focus(&this.search_focus);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .w(px(width))
            .flex_shrink()
            .min_w(px(112.))
            .h(px(28.))
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .rounded(px(4.))
            .border_1()
            .border_color(c(if focused { ACCENT } else { BORDER_PRIMARY }))
            .bg(c(SURFACE_SECONDARY))
            .text_sm_tw()
            .cursor_text()
            .overflow_hidden()
            .child(glyph)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .when(input.is_empty(), |field| {
                        field
                            .text_color(c(TEXT_MUTED))
                            .child(div().truncate().child("Search files and folders..."))
                    })
                    .when(!input.is_empty(), |field| {
                        field.text_color(c(TEXT_PRIMARY)).child(shown)
                    })
                    .when(focused, |field| {
                        field.child(div().w(px(1.)).h(px(14.)).flex_none().bg(c(TEXT_PRIMARY)))
                    }),
            )
            .when(!input.is_empty(), |field| {
                field.child(
                    div()
                        .id("search-clear")
                        .flex_none()
                        .cursor_pointer()
                        .child(icon("x", 14., c(TEXT_MUTED)))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.clear_search_input(cx);
                            window.focus(&this.search_focus);
                        })),
                )
            })
    }

    pub(crate) fn render_toolbar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pane = self.pane();
        let toolbar_width = f32::from(window.viewport_size().width) - SIDEBAR_WIDTH;
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .border_b_1()
            .border_color(c(BORDER_SECONDARY))
            .bg(c(SURFACE_PRIMARY))
            .px(px(12.))
            .py(px(6.))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(px(2.))
                    .child(
                        self.nav_button("back", "chevron-left", 18., pane.can_go_back())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.back(&Back, window, cx)),
                            ),
                    )
                    .child(
                        self.nav_button("forward", "chevron-right", 18., pane.can_go_forward())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.forward(&Forward, window, cx)
                            })),
                    ),
            )
            .child(self.render_path_bar(cx))
            .children((!pane.is_at_root()).then(|| self.render_search(toolbar_width, window, cx)))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(px(4.))
                    // At This PC nothing can be created, so the add button is disabled.
                    .child(self.nav_button("add", "plus", 16., !pane.is_at_root()))
                    .child(
                        self.nav_button("refresh", "refresh-cw", 16., true)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.refresh(&Refresh, window, cx)
                            })),
                    )
                    .child(
                        self.nav_button("settings", "settings", 16., true)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_settings(&OpenSettings, window, cx)
                            })),
                    ),
            )
    }
}
