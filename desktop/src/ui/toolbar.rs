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
use crate::components::{ContextMenu, MenuItem};
use crate::workspace::drag_drop::{drop_zone, DropTarget};
use crate::workspace::{Back, CreateFile, CreateFolder, Forward, OpenSettings, Refresh, ToggleDetails, Workspace};

pub(crate) struct Crumb {
    pub label: String,
    pub path: String,
}

/// How many leading crumbs to hide behind "…" so the rest fit in `available`.
/// The current folder always shows.
/// Empty space kept after the crumbs, which a click turns into the location field.
const PATH_BAR_SPACE: f32 = 12.;

/// The "/" between crumbs, which takes a drag for the crumb before it.
fn separator(zone: Option<(DropTarget, gpui::WeakEntity<Workspace>)>) -> gpui::Div {
    let zone_name = match &zone {
        Some((DropTarget::Crumb { path, .. }, _)) => Some(path.clone()),
        _ => None,
    };
    let glyph = div()
        .debug_selector(move || match &zone_name {
            Some(path) => format!("crumb-separator-{path}"),
            None => "crumb-separator".into(),
        }).flex_none().h_full().flex().items_center().px(px(2.)).text_xs_tw().text_color(c(TEXT_MUTED)).child("/");
    match zone {
        Some((zone, workspace)) => drop_zone(glyph, zone, workspace),
        None => glyph,
    }
}

fn hidden_crumbs(widths: &[f32], available: f32) -> usize {
    const SEPARATOR: f32 = 7. + 4.;
    const OVERFLOW: f32 = 15. + SEPARATOR;
    let need = |from: usize| -> f32 {
        let shown: f32 = widths[from..].iter().map(|width| width + 8.).sum::<f32>() + SEPARATOR * (widths.len() - from - 1) as f32;
        shown + if from > 0 { OVERFLOW } else { 0. }
    };
    (0..widths.len().saturating_sub(1)).find(|&from| need(from) <= available).unwrap_or(widths.len().saturating_sub(1))
}

/// A toolbar button's eased hover fill and press (`motion`).
fn pressable(id: &'static str, enabled: bool, button: gpui::Stateful<gpui::Div>) -> crate::components::motion::Interact {
    crate::components::motion::interact(gpui::ElementId::Name(format!("{id}-feedback").into()), button)
        .fill(c(SURFACE_ELEVATED))
        .rounded(4.)
        .disabled(!enabled)
}

fn under(path: &str, root: &str) -> bool {
    path == root || path.strip_prefix(root).is_some_and(|rest| rest.starts_with('/'))
}

/// `root`, then a crumb for each folder of `rest` below it.
fn trail(root: Crumb, rest: &str) -> Vec<Crumb> {
    let prefix = root.path.clone();
    trail_segments(vec![root], prefix, rest)
}

fn trail_segments(mut crumbs: Vec<Crumb>, mut prefix: String, rest: &str) -> Vec<Crumb> {
    for segment in rest.split('/').filter(|segment| !segment.is_empty()) {
        prefix = if prefix.is_empty() { segment.to_string() } else { format!("{prefix}/{segment}") };
        crumbs.push(Crumb { label: segment.to_string(), path: prefix.clone() });
    }
    crumbs
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
    /// A place such as Downloads starts with the folders that hold it, from Home
    /// or the drive, so the trail is the folder's whole path.
    pub(crate) fn breadcrumbs(&self, target: PaneTarget) -> Vec<Crumb> {
        let pane = self.pane_by_id(target.0).unwrap_or_else(|| self.pane());
        let path = &pane.path;
        // A mounted device or share starts the trail with its name, not its mount folder.
        let volume = self
            .devices
            .volumes()
            .filter_map(|volume| Some((self.volume_name(volume).to_string(), pane.api.virtual_path(volume.mount_point()?)?)))
            .filter(|(_, root)| under(path, root))
            .max_by_key(|(_, root)| root.len());
        if let Some((label, root)) = volume {
            return trail(Crumb { label, path: root.clone() }, &path[root.len()..]);
        }
        let Some((label, root)) = self.provider_root(target, path) else {
            return trail_segments(Vec::new(), String::new(), path);
        };
        let above = pane.api.place_parent(&root).and_then(|parent| {
            let (label, holder) = self.provider_root(target, &parent)?;
            Some(trail(Crumb { label, path: holder.clone() }, &parent[holder.len()..]))
        });
        let mut crumbs = above.unwrap_or_default();
        crumbs.extend(trail(Crumb { label, path: root.clone() }, &path[root.len()..]));
        crumbs
    }

    /// The drive or place of the pane's provider that holds `path`, and its name.
    fn provider_root(&self, target: PaneTarget, path: &str) -> Option<(String, String)> {
        let pane = self.pane_by_id(target.0).unwrap_or_else(|| self.pane());
        let crate::pane::Load::Loaded(roots) = &pane.roots else { return None };
        roots.iter().find(|root| under(path, &root.name)).map(|root| (self.root_name(&pane.api, root).to_string(), root.name.clone()))
    }

    fn nav_button(
        &self,
        id: &'static str,
        glyph: &'static str,
        size: f32,
        enabled: bool,
    ) -> gpui::Stateful<gpui::Div> {
        // Translucent rather than TEXT_DISABLED's fixed grey, which vanishes on a wallpaper.
        let glyph = icon(glyph, size, if enabled { c(TEXT_SECONDARY) } else { alpha(WHITE, 0.3) });
        Self::nav_shell(id, enabled).child(glyph)
    }

    /// A toolbar button around any icon.
    fn nav_shell(id: &'static str, enabled: bool) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .debug_selector(move || format!("toolbar-{id}"))
            .size(px(28.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .when(enabled, |button| button.cursor_pointer())
    }

    /// Refresh, whose icon turns while what it asked for loads.
    fn refresh_button(&self, target: PaneTarget, enabled: bool, window: &mut Window) -> gpui::Stateful<gpui::Div> {
        let Some(turn) = self.refresh_turn(target).filter(|_| enabled) else {
            return self.nav_button("refresh", "refresh-cw", 16., enabled);
        };
        window.request_animation_frame();
        let glyph = icon("refresh-cw", 16., c(TEXT_PRIMARY)).with_transformation(Transformation::rotate(percentage(turn)));
        Self::nav_shell("refresh", enabled).child(glyph)
    }

    fn render_path_bar(&self, target: PaneTarget, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.location_editor.as_ref().is_some_and(|editor| editor.target_id == target.0) {
            // The same box as the breadcrumbs, holding the text field.
            return div()
                .key_context("LocationField")
                .relative()
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .rounded(px(4.))
                .border_1()
                .border_color(c(BORDER_FOCUS))
                .bg(self.chrome_background())
                .h(px(28.))
                .px(px(8.))
                .text_13()
                .text_color(c(TEXT_PRIMARY))
                .child(self.location_field.clone())
                .children(self.render_location_completion(cx))
                .into_any_element();
        }
        let crumbs = self.breadcrumbs(target);
        let content: AnyElement = if self.trash_open {
            div().text_13().whitespace_nowrap().child("Trash").into_any_element()
        } else if crumbs.is_empty() {
            div()
                .id("path-bar-space")
                .flex_1()
                .text_13()
                .whitespace_nowrap()
                .cursor_text()
                .on_click(cx.listener(move |this, _, window, cx| this.edit_location(target, window, cx)))
                .child("Overview")
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
            .h(px(28.))
            .pl(px(8.))
            .pr(px(4.))
            .child(
                div()
                    .id("overview")
                    .size(px(18.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .child(icon("layout-dashboard", 14., c(TEXT_SECONDARY)))
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(String::new(), cx))),
            )
            .child(content).into_any_element()
    }

    /// The folders Tab can complete, under the location field while typing.
    fn render_location_completion(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let completion = self.location_completion.as_ref()?;
        let text = self.location_field.read(cx).text();
        // Only while it fits what is typed, and not just the folder already entered.
        if !completion.is_for(text) || completion.names.is_empty() || (completion.names.len() == 1 && completion.stepped_to(text)) {
            return None;
        }
        const ROW: f32 = 28.;
        let rows = completion.names.iter().enumerate().map(|(index, name)| {
            let chosen = completion.index == Some(index);
            let filled = format!("{}{name}/", completion.base);
            div()
                .id(("completion", index))
                .debug_selector(move || format!("completion-{index}"))
                .h(px(ROW))
                .flex()
                .items_center()
                .px(px(8.))
                .rounded(px(4.))
                .truncate()
                .cursor_pointer()
                .when(chosen, |row| row.bg(alpha(self.accent_hex(), 0.25)))
                .hover(|style| style.bg(c(SURFACE_ELEVATED)))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.fill_location(filled.clone(), true, window, cx);
                    this.location_field.read(cx).focus(window);
                }))
                .child(gpui::SharedString::from(name.clone()))
        });
        let shown = crate::workspace::location_completion::SHOWN as f32;
        let list = div()
            .id("location-completion")
            .debug_selector(|| "location-completion".into())
            .occlude()
            .min_w(px(220.))
            .max_w(px(420.))
            .max_h(px(ROW * shown + 10.))
            .overflow_y_scroll()
            .track_scroll(&self.location_scroll)
            .p(px(4.))
            .rounded(px(6.))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_SECONDARY))
            .shadow_lg()
            .text_13()
            .text_color(c(TEXT_PRIMARY))
            .children(rows);
        // From the field's top-left, below it, drawn over the panes.
        Some(div().absolute().top_0().left_0().child(
            gpui::deferred(gpui::anchored().position_mode(gpui::AnchoredPositionMode::Local).position(gpui::point(px(0.), px(32.))).snap_to_window().child(list)).with_priority(2),
        ))
    }

    /// The crumbs that fit, after a "…" that lists the ones that do not. Each opens
    /// on click, opens a tab on middle-click, has a menu, takes drops, opens when a
    /// drag rests on it, and shows its full path on hover.
    fn render_crumbs(&self, target: PaneTarget, crumbs: Vec<Crumb>, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let last = crumbs.len() - 1;
        let font = gpui::font(self.fonts.sans.clone());
        let widths: Vec<f32> = crumbs
            .iter()
            .map(|crumb| {
                let run = gpui::TextRun { len: crumb.label.len(), font: font.clone(), color: gpui::black(), background_color: None, underline: None, strikethrough: None };
                f32::from(window.text_system().shape_line(crumb.label.clone().into(), px(13.), &[run], None).width)
            })
            .collect();
        let hidden = self.path_bar_widths.borrow().get(&target.0).map_or(0, |&available| hidden_crumbs(&widths, available - PATH_BAR_SPACE));
        let hover = self.drop_hover(cx).cloned();
        // No gaps: the crumbs and their separators fill the row, so a drag held anywhere on it is over one.
        let mut row = div().relative().flex().flex_1().min_w_0().h_full().overflow_hidden().whitespace_nowrap();
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
                .child(separator(None));
        }
        for (index, crumb) in crumbs.iter().enumerate().skip(hidden) {
            let path = crumb.path.clone();
            let zone = DropTarget::Crumb { pane: target, path: path.clone() };
            if index > hidden {
                // A drag on the separator is over the crumb before it.
                let before = DropTarget::Crumb { pane: target, path: crumbs[index - 1].path.clone() };
                row = row.child(separator(Some((before, cx.entity().downgrade()))));
            }
            let dropping = hover.as_ref() == Some(&zone);
            let (open, tab, menu, lit) = (path.clone(), path.clone(), path.clone(), path.clone());
            let element = div()
                .id(("crumb", index))
                .debug_selector(move || format!("crumb-{}-{index}", target.0))
                .min_w_0()
                .h_full()
                .flex()
                .items_center()
                .px(px(4.))
                .rounded(px(4.))
                .text_13()
                .whitespace_nowrap()
                .overflow_hidden()
                .when(dropping, |crumb| crumb.bg(alpha(ACCENT, 0.25)))
                .map(|crumb| match index == last {
                    // Clicking the current folder, like the space after it, edits the path.
                    true => crumb.flex_shrink().cursor_text().on_click(cx.listener(move |this, _, window, cx| this.edit_location(target, window, cx))),
                    false => crumb
                        .flex_none()
                        .text_color(c(TEXT_SECONDARY))
                        .cursor_pointer()
                        .active(crate::components::pressed)
                        .on_click(cx.listener(move |this, _, _, cx| this.navigate_in(target, open.clone(), cx))),
                })
                .on_mouse_down(gpui::MouseButton::Middle, cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    this.middle_click_folder(target, tab.clone(), event, cx);
                }))
                .on_mouse_down(gpui::MouseButton::Right, cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.open_place_menu(crate::workspace::place_menu::Place::Folder { pane: target, path: menu.clone() }, event.position, cx);
                }))
                // Text is laid out before hover is known, so the lit label is drawn from state.
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    let crumb = (target.0, lit.clone());
                    let now = if *hovered { Some(crumb) } else { this.hovered_crumb.take().filter(|held| *held != crumb) };
                    if this.hovered_crumb != now {
                        this.hovered_crumb = now;
                        cx.notify();
                    }
                }))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        // A click opens it: its name lights up and is underlined.
                        .when(index != last && self.hovered_crumb.as_ref().is_some_and(|(pane, held)| *pane == target.0 && *held == path), |label| {
                            label.text_color(c(WHITE)).underline()
                        })
                        .child(crumb.label.clone()),
                );
            row = row.child(drop_zone(element, zone, cx.entity().downgrade()));
        }
        // Empty space after the crumbs edits the path, as in Nautilus and Explorer.
        row = row.child(
            div()
                .id("path-bar-space")
                .debug_selector(move || format!("path-bar-space-{}", target.0))
                .flex_1()
                .min_w(px(PATH_BAR_SPACE))
                .h(px(18.))
                .cursor_text()
                .on_click(cx.listener(move |this, _, window, cx| this.edit_location(target, window, cx))),
        );
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
                    .icon(if place.path.is_empty() { "layout-dashboard" } else { "folder" })
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
            .occlude()
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
        // Trash keeps the toolbar; what does not apply there is greyed out.
        let trash = self.trash_open;
        let can_add = !pane.is_at_root() && !trash;
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
                    .child(pressable(
                        "back",
                        trash || pane.can_go_back(),
                        drop_zone(self.nav_button("back", "chevron-left", 18., trash || pane.can_go_back()), DropTarget::Back(target), cx.entity().downgrade())
                            .when(self.drop_hover(cx) == Some(&DropTarget::Back(target)), |button| button.bg(alpha(ACCENT, 0.25)))
                            .debug_selector(|| "nav-back".into())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.back(&Back, window, cx)),
                            )
                            // Right-click lists the places Back can go to.
                            .on_mouse_down(gpui::MouseButton::Right, cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                this.open_history_menu(target, true, event.position, cx);
                            })),
                    ))
                    .child(pressable(
                        "forward",
                        !trash && pane.can_go_forward(),
                        self.nav_button("forward", "chevron-right", 18., !trash && pane.can_go_forward())
                            .debug_selector(|| "nav-forward".into())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.forward(&Forward, window, cx)
                            }))
                            .on_mouse_down(gpui::MouseButton::Right, cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                this.open_history_menu(target, false, event.position, cx);
                            })),
                    )),
            )
            .child(self.render_path_bar(target, window, cx))
            .children((!pane.is_at_root() && !self.trash_open).then(|| self.render_search(target, toolbar_width, window, cx)))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .gap(px(4.))
                    // At the Overview nothing can be created, so the add button is disabled.
                    .child(
                        div()
                            .relative()
                            .child(pressable("add", can_add, self.nav_button("add", "plus", 16., can_add).when(can_add, |button| {
                                button.on_click(cx.listener(move |this, _, _, cx| {
                                    this.add_menu = if this.add_menu == Some(target) { None } else { Some(target) };
                                    this.open_submenu = None;
                                    this.refresh_templates(cx);
                                    cx.notify();
                                }))
                            })))
                            .when(self.add_menu == Some(target), |slot| slot.child(div().absolute().top_0().left_0().child(self.render_add_menu(target, cx)))),
                    )
                    // Trash is watched, so it needs no Refresh.
                    .child(pressable("refresh", !trash, self.refresh_button(target, !trash, window).when(!trash, |button| {
                        button.on_click(cx.listener(|this, _, window, cx| this.refresh(&Refresh, window, cx)))
                    })))
                    .when(trash, |row| row.child(self.restore_trash_button(cx)).child(self.empty_trash_button(cx)))
                    .child(pressable(
                        "details",
                        true,
                        self.nav_button("details", "info", 16., true)
                            .when(self.preferences.details_panel, |button| button.bg(c(SURFACE_ELEVATED)))
                            .tooltip(crate::components::Tooltip::text("Details (F11)"))
                            .on_click(cx.listener(|this, _, window, cx| this.toggle_details(&ToggleDetails, window, cx))),
                    ))
                    .child(pressable(
                        "settings",
                        true,
                        self.nav_button("settings", "settings", 16., true).on_click(cx.listener(|this, _, window, cx| this.open_settings(&OpenSettings, window, cx))),
                    )),
            )
    }
}
