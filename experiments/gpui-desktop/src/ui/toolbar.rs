//! `Toolbar.svelte` with `EditablePathBar.svelte` and the compact `SearchBar.svelte`.

use std::time::Duration;

use gpui::{
    anchored, canvas, deferred, div, percentage, point, prelude::*, px, AnchoredPositionMode, Animation,
    AnimationExt as _, AnyElement, Context, Transformation, Window,
};

use super::{icon, TextScale};
use crate::pane::Search;
use crate::theme::*;
use crate::workspace::operations::PaneTarget;
use crate::components::{ContextMenu, MenuItem, Tooltip};
use crate::workspace::drag_drop::{drop_zone, DropTarget};
use crate::workspace::{Back, CreateFile, CreateFolder, Forward, OpenSettings, Refresh, Workspace};

pub(crate) struct Crumb {
    pub label: String,
    path: String,
}

/// How many leading crumbs to hide behind "…" so the rest fit in `available`.
/// The current folder always shows.
fn hidden_crumbs(widths: &[f32], available: f32) -> usize {
    const SEPARATOR: f32 = 4. + 7. + 4. + 4.;
    const OVERFLOW: f32 = 15. + SEPARATOR;
    let need = |from: usize| -> f32 {
        let shown: f32 = widths[from..].iter().map(|width| width + 4.).sum::<f32>() + SEPARATOR * (widths.len() - from - 1) as f32;
        shown + if from > 0 { OVERFLOW } else { 0. }
    };
    (0..widths.len().saturating_sub(1)).find(|&from| need(from) <= available).unwrap_or(widths.len().saturating_sub(1))
}

#[cfg(test)]
mod tests {
    #[test]
    fn long_paths_hide_their_first_crumbs() {
        let widths = [40., 60., 80., 100.];
        assert_eq!(super::hidden_crumbs(&widths, 1000.), 0);
        assert_eq!(super::hidden_crumbs(&widths, 250.), 2);
        // The current folder stays even when nothing else fits.
        assert_eq!(super::hidden_crumbs(&widths, 10.), 3);
    }
}

impl Workspace {
    /// Root label, then the path below it, like `EditablePathBar`'s breadcrumbs.
    pub(crate) fn breadcrumbs(&self, target: PaneTarget) -> Vec<Crumb> {
        let pane = self.pane_by_id(target.0).unwrap_or_else(|| self.pane());
        let path = &pane.path;
        let under = |root: &str| path == root || path.starts_with(&format!("{root}/"));
        // A mounted device or share starts the trail with its name, not its mount folder.
        let volume = self
            .devices
            .volumes()
            .filter_map(|volume| Some((volume.name.clone(), pane.api.virtual_path(volume.mount_point()?)?)))
            .filter(|(_, root)| under(root))
            .max_by_key(|(_, root)| root.len());
        let root = volume.or_else(|| match &pane.roots {
            crate::pane::Load::Loaded(roots) => {
                roots.iter().find(|root| under(&root.name)).map(|root| (root.display_name().to_string(), root.name.clone()))
            }
            _ => None,
        });
        let (mut crumbs, rest, mut prefix) = match root {
            Some((label, root)) => (vec![Crumb { label, path: root.clone() }], &path[root.len()..], root),
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

    fn render_path_bar(&self, target: PaneTarget, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.location_editor.as_ref().is_some_and(|editor| editor.target_id == target.0) {
            // The same box as the breadcrumbs, holding the text field.
            return div()
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .rounded(px(4.))
                .border_1()
                .border_color(c(BORDER_FOCUS))
                .bg(self.chrome_background())
                .px(px(8.))
                .py(px(4.))
                .text_13()
                .text_color(c(TEXT_PRIMARY))
                .child(self.location_field.clone())
                .into_any_element();
        }
        let crumbs = self.breadcrumbs(target);
        let content: AnyElement = if crumbs.is_empty() {
            div()
                .text_13()
                .whitespace_nowrap()
                .child("This PC")
                .into_any_element()
        } else {
            self.render_crumbs(target, crumbs, window, cx).into_any_element()
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
            .bg(self.chrome_background())
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
            .child(content).into_any_element()
    }

    /// The crumbs that fit, after a "…" that lists the ones that do not. Each opens
    /// on click, opens a tab on middle-click, has a menu, takes drops, opens when a
    /// drag rests on it, and shows its full path on hover.
    fn render_crumbs(&self, target: PaneTarget, crumbs: Vec<Crumb>, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.pane_by_id(target.0).unwrap_or_else(|| self.pane());
        let last = crumbs.len() - 1;
        let font = gpui::font(self.fonts.sans.clone());
        let widths: Vec<f32> = crumbs
            .iter()
            .map(|crumb| {
                let run = gpui::TextRun { len: crumb.label.len(), font: font.clone(), color: gpui::black(), background_color: None, underline: None, strikethrough: None };
                f32::from(window.text_system().shape_line(crumb.label.clone().into(), px(13.), &[run], None).width)
            })
            .collect();
        let hidden = self.path_bar_widths.borrow().get(&target.0).map_or(0, |&available| hidden_crumbs(&widths, available));
        let full_path = |index: usize| -> String {
            match &pane.real_path {
                Some(real) => real.ancestors().nth(last - index).unwrap_or(real).display().to_string(),
                None => format!("{}/{}", pane.api.server_url().unwrap_or(""), crumbs[index].path),
            }
        };
        let hover = self.drop_hover(cx).cloned();
        let mut row = div().relative().flex().flex_1().min_w_0().items_center().gap(px(4.)).overflow_hidden().whitespace_nowrap();
        if hidden > 0 {
            let listed: Vec<(String, String)> = crumbs[..hidden].iter().map(|crumb| (crumb.label.clone(), crumb.path.clone())).collect();
            row = row
                .child(
                    div()
                        .id("crumb-overflow")
                        .debug_selector(|| "crumb-overflow".into())
                        .flex_none()
                        .px(px(4.))
                        .rounded(px(4.))
                        .text_13()
                        .text_color(c(TEXT_SECONDARY))
                        .cursor_pointer()
                        .hover(|style| style.bg(c(SURFACE_ELEVATED)))
                        .child("…")
                        .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.open_place_menu(crate::workspace::place_menu::Place::Hidden { pane: target, crumbs: listed.clone() }, event.position, cx);
                        })),
                )
                .child(div().text_xs_tw().text_color(c(TEXT_MUTED)).child("/"));
        }
        for (index, crumb) in crumbs.iter().enumerate().skip(hidden) {
            if index > hidden {
                row = row.child(div().flex_none().text_xs_tw().text_color(c(TEXT_MUTED)).child("/"));
            }
            let path = crumb.path.clone();
            let zone = DropTarget::Crumb { pane: target, path: path.clone() };
            let dropping = hover.as_ref() == Some(&zone);
            let (open, tab, menu) = (path.clone(), path.clone(), path.clone());
            let element = div()
                .id(("crumb", index))
                .debug_selector(move || format!("crumb-{}-{index}", target.0))
                .min_w_0()
                .px(px(2.))
                .rounded(px(4.))
                .text_13()
                .whitespace_nowrap()
                .truncate()
                .when(dropping, |crumb| crumb.bg(alpha(ACCENT, 0.25)))
                .map(|crumb| match index == last {
                    true => crumb.flex_shrink(),
                    false => crumb
                        .flex_none()
                        .text_color(c(TEXT_SECONDARY))
                        .cursor_pointer()
                        .hover(|style| style.text_color(c(WHITE)).underline())
                        .on_click(cx.listener(move |this, _, _, cx| this.navigate_in(target, open.clone(), cx))),
                })
                .on_mouse_down(gpui::MouseButton::Middle, cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.open_in_new_tab(target, tab.clone(), cx);
                }))
                .on_mouse_down(gpui::MouseButton::Right, cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.open_place_menu(crate::workspace::place_menu::Place::Folder { pane: target, path: menu.clone() }, event.position, cx);
                }))
                .tooltip(Tooltip::text(full_path(index)))
                .child(crumb.label.clone());
            row = row.child(drop_zone(element, zone, cx.entity().downgrade()));
        }
        // The row's width, for the next frame's choice of crumbs.
        let widths_store = self.path_bar_widths.clone();
        row.child(
            canvas(
                move |bounds, _, _| bounds.size.width,
                move |_, width, window, _| {
                    let width = f32::from(width);
                    let previous = widths_store.borrow_mut().insert(target.0, width);
                    if previous.is_none_or(|previous| (previous - width).abs() > 0.5) {
                        window.request_animation_frame();
                    }
                },
            )
            .absolute()
            .size_full(),
        )
    }

    /// `SearchBar.svelte` (compact): the pane's text field with a search icon, or a
    /// spinner while a search runs, and a clear button.
    fn render_search(&self, target: PaneTarget, toolbar_width: f32, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        // `w-28 @lg:w-48 @3xl:w-64 @5xl:w-96` against the toolbar's container width.
        let width = match toolbar_width {
            w if w >= 1024. => 384.,
            w if w >= 768. => 256.,
            w if w >= 512. => 192.,
            _ => 112.,
        };
        let pane = self.pane_by_id(target.0).unwrap_or_else(|| self.pane());
        let field = self.search_field(target.0).cloned();
        let focused = field.as_ref().is_some_and(|field| field.read(cx).is_focused(window));
        let loading = matches!(pane.search, Search::Loading);
        let glyph: AnyElement = if loading {
            icon("refresh-cw", 16., c(self.accent_hex()))
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
            .w(px(width))
            .flex_shrink()
            .min_w(px(112.))
            .h(px(28.))
            .flex()
            .items_center()
            // The icon sits at left-3 of the outer box and the text at pl-10 inside the border.
            .gap(px(13.))
            .pl(px(11.))
            .pr(px(12.))
            .rounded(px(4.))
            .border_1()
            .border_color(c(if focused { BORDER_FOCUS } else { BORDER_PRIMARY }))
            .bg(self.chrome_background())
            .text_sm_tw()
            .text_color(c(TEXT_PRIMARY))
            .overflow_hidden()
            .child(glyph)
            .when_some(field, |row, field| row.child(field))
            .when(!pane.search_input.is_empty(), |row| {
                row.child(
                    div()
                        .id("search-clear")
                        .flex_none()
                        .cursor_pointer()
                        .child(icon("x", 14., c(TEXT_MUTED)))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Some(pane) = this.pane_mut_by_id(target.0) {
                                pane.clear_search();
                            }
                            if let Some(field) = this.search_field(target.0) {
                                field.read(cx).focus(window);
                            }
                            cx.notify();
                        })),
                )
            })
    }

    /// `AddMenu.svelte`: under the "+" button, everything that puts something new here.
    /// Local folders copy files in; a server connection uploads them.
    fn render_add_menu(&self, target: PaneTarget, cx: &mut Context<Self>) -> impl IntoElement {
        let pane = self.pane_by_id(target.0).unwrap_or_else(|| self.pane());
        let can_create = !pane.is_at_root() && !self.operation_status.is_busy();
        let local = pane.api.is_local();
        let item = |id: &'static str, label: &'static str, icon: &'static str, run: fn(&mut Workspace, PaneTarget, &mut Window, &mut Context<Workspace>), cx: &mut Context<Self>| {
            MenuItem::action(id, label).icon(icon).disabled(!can_create).on_select(cx.listener(move |this, _, window, cx| {
                this.add_menu = None;
                this.active_pane = target;
                run(this, target, window, cx);
                cx.notify();
            }))
        };
        let items = vec![
            item("add-folder", "New folder", "folder-plus", |this, _, window, cx| this.create_folder(&CreateFolder, window, cx), cx),
            item("add-file", "New file", "file-plus", |this, _, window, cx| this.create_file(&CreateFile, window, cx), cx),
        ];
        let mut items = items;
        items.extend(self.template_menu(target, |this| this.add_menu = None, cx));
        items.extend([
            MenuItem::Separator,
            item("add-files", if local { "Copy files here…" } else { "Upload files" }, "download", |this, target, _, cx| this.import_with_picker(target, false, cx), cx),
            item("add-folders", if local { "Copy folder here…" } else { "Upload folder" }, "folder-open", |this, target, _, cx| this.import_with_picker(target, true, cx), cx),
        ]);
        // AddMenu sits 4px under the button's left edge.
        deferred(
            anchored().position_mode(AnchoredPositionMode::Local).position(point(px(0.), px(32.))).snap_to_window().child(
                div()
                    .id("add-menu")
                    .occlude()
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.add_menu = None;
                        cx.notify();
                    }))
                    .child(ContextMenu::new(items).on_submenu(self.submenu_listener(cx))),
            ),
        )
        .with_priority(1)
    }

    /// The Back or Forward menu: one row per place, nearest first.
    pub(crate) fn render_history_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(menu) = &self.history_menu else { return div().into_any_element() };
        let (target, back) = (menu.target, menu.back);
        let items = menu
            .places
            .iter()
            .enumerate()
            .map(|(index, place)| {
                MenuItem::action(("history", index), self.place_label(target, place))
                    .icon(if place.is_empty() { "server" } else { "folder" })
                    .on_select(cx.listener(move |this, _, _, cx| {
                        this.history_menu = None;
                        this.travel(target, back, index + 1, cx);
                    }))
            })
            .collect();
        let close = |this: &mut Self, cx: &mut Context<Self>| {
            this.history_menu = None;
            cx.notify();
        };
        div()
            .absolute()
            .inset_0()
            .id("history-dismiss")
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, _, _, cx| close(this, cx)))
            .on_mouse_down(gpui::MouseButton::Right, cx.listener(move |this, _, _, cx| close(this, cx)))
            .child(
                anchored().position(menu.position).snap_to_window().child(
                    div()
                        .id("history-menu")
                        .debug_selector(|| "history-menu".into())
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(ContextMenu::new(items)),
                ),
            )
            .into_any_element()
    }

    pub(crate) fn render_toolbar(
        &self,
        target: PaneTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pane = self.pane_by_id(target.0).unwrap_or_else(|| self.pane());
        let toolbar_width = self.browser_width(window);
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .border_b_1()
            .border_color(c(BORDER_SECONDARY))
            .bg(self.chrome_background())
            .px(px(12.))
            .py(px(6.))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(px(2.))
                    .child(
                        self.nav_button("back", "chevron-left", 18., pane.can_go_back())
                            .debug_selector(|| "nav-back".into())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.back(&Back, window, cx)),
                            )
                            // Right-click lists the places Back can go to.
                            .on_mouse_down(gpui::MouseButton::Right, cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                this.open_history_menu(target, true, event.position, cx);
                            })),
                    )
                    .child(
                        self.nav_button("forward", "chevron-right", 18., pane.can_go_forward())
                            .debug_selector(|| "nav-forward".into())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.forward(&Forward, window, cx)
                            }))
                            .on_mouse_down(gpui::MouseButton::Right, cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                this.open_history_menu(target, false, event.position, cx);
                            })),
                    ),
            )
            .child(self.render_path_bar(target, window, cx))
            .children((!pane.is_at_root()).then(|| self.render_search(target, toolbar_width, window, cx)))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(px(4.))
                    // At This PC nothing can be created, so the add button is disabled.
                    .child(
                        div()
                            .relative()
                            .child(self.nav_button("add", "plus", 16., !pane.is_at_root()).when(!pane.is_at_root(), |button| {
                                button.on_click(cx.listener(move |this, _, _, cx| {
                                    this.add_menu = if this.add_menu == Some(target) { None } else { Some(target) };
                                    this.open_submenu = None;
                                    this.refresh_templates(cx);
                                    cx.notify();
                                }))
                            }))
                            .when(self.add_menu == Some(target), |slot| slot.child(self.render_add_menu(target, cx))),
                    )
                    .child(
                        self.nav_button("refresh", "refresh-cw", 16., true)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.refresh(&Refresh, window, cx)
                            })),
                    )
                    .child(self.nav_button("settings", "settings", 16., true).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.open_settings(&OpenSettings, window, cx)
                        }),
                    )),
            )
    }
}
