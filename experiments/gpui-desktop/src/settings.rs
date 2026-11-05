//! `settings/SettingsModal.svelte`, General section: the preferences the browse UI reads.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString, Window};

use crate::dialogs::Dialog;
use crate::preferences::{Preferences, SortBy, SortDirPref, ViewPref};
use crate::theme::*;
use crate::ui::modal::modal;
use crate::ui::TextScale;
use crate::workspace::Workspace;

/// A name as listed: without its extension when extensions are hidden (files only).
pub fn display_name(name: &str, is_dir: bool, show_extensions: bool) -> &str {
    if show_extensions || is_dir {
        return name;
    }
    match name.rfind('.') {
        Some(dot) if dot > 0 => &name[..dot],
        _ => name,
    }
}

type Setter = fn(&mut Preferences);

impl Workspace {
    pub(crate) fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.dialog = Some(Dialog::Settings);
        cx.notify();
    }

    fn settings_row(&self, label: &'static str, control: AnyElement) -> AnyElement {
        div()
            .flex()
            .min_h(px(40.))
            .items_center()
            .justify_between()
            .gap(px(16.))
            .py(px(6.))
            .border_b_1()
            .border_color(c(BORDER_SECONDARY))
            .text_13()
            .child(div().text_color(c(TEXT_PRIMARY)).child(label))
            .child(control)
            .into_any_element()
    }

    fn toggle(
        &self,
        id: &'static str,
        on: bool,
        flip: Setter,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(id)
            .relative()
            .w(px(40.))
            .h(px(20.))
            .rounded_full()
            .cursor_pointer()
            .bg(c(if on { ACCENT_SOLID } else { SURFACE_ELEVATED }))
            .child(
                div()
                    .absolute()
                    .top(px(2.))
                    .left(px(if on { 22. } else { 2. }))
                    .size(px(16.))
                    .rounded_full()
                    .bg(c(WHITE)),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.update_preferences(flip, cx)))
            .into_any_element()
    }

    fn segmented(
        &self,
        id: &'static str,
        options: Vec<(&'static str, bool, Setter)>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .flex()
            .p(px(2.))
            .gap(px(2.))
            .rounded(px(6.))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .children(
                options
                    .into_iter()
                    .enumerate()
                    .map(|(index, (label, active, set))| {
                        div()
                            .id((id, index))
                            .px(px(10.))
                            .py(px(3.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .text_xs_tw()
                            .when(active, |chip| {
                                chip.bg(c(SURFACE_ELEVATED)).text_color(c(TEXT_PRIMARY))
                            })
                            .when(!active, |chip| chip.text_color(c(TEXT_SECONDARY)))
                            .child(SharedString::from(label))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.update_preferences(set, cx)),
                            )
                    }),
            )
            .into_any_element()
    }

    pub(crate) fn render_settings(
        &self,
        cx: &mut Context<Self>,
        on_close: impl Fn(&mut Window, &mut gpui::App) + 'static,
    ) -> AnyElement {
        let prefs = &self.preferences;
        let rows = vec![
            self.settings_row(
                "Show hidden files",
                self.toggle(
                    "hidden",
                    prefs.show_hidden,
                    |p| p.show_hidden = !p.show_hidden,
                    cx,
                ),
            ),
            self.settings_row(
                "Show file extensions",
                self.toggle(
                    "extensions",
                    prefs.show_extensions,
                    |p| p.show_extensions = !p.show_extensions,
                    cx,
                ),
            ),
            self.settings_row(
                "Confirm before delete",
                self.toggle(
                    "confirm",
                    prefs.confirm_delete,
                    |p| p.confirm_delete = !p.confirm_delete,
                    cx,
                ),
            ),
            self.settings_row(
                "Preview on single click",
                self.toggle(
                    "single-click",
                    prefs.preview_on_single_click,
                    |p| p.preview_on_single_click = !p.preview_on_single_click,
                    cx,
                ),
            ),
            self.settings_row(
                "Default sort",
                self.segmented(
                    "sort-by",
                    vec![
                        ("Name", prefs.default_sort_by == SortBy::Name, |p| {
                            p.default_sort_by = SortBy::Name
                        }),
                        ("Size", prefs.default_sort_by == SortBy::Size, |p| {
                            p.default_sort_by = SortBy::Size
                        }),
                        ("Modified", prefs.default_sort_by == SortBy::ModTime, |p| {
                            p.default_sort_by = SortBy::ModTime
                        }),
                        ("Type", prefs.default_sort_by == SortBy::Type, |p| {
                            p.default_sort_by = SortBy::Type
                        }),
                    ],
                    cx,
                ),
            ),
            self.settings_row(
                "Default sort direction",
                self.segmented(
                    "sort-dir",
                    vec![
                        (
                            "Ascending",
                            prefs.default_sort_dir == SortDirPref::Asc,
                            |p| p.default_sort_dir = SortDirPref::Asc,
                        ),
                        (
                            "Descending",
                            prefs.default_sort_dir == SortDirPref::Desc,
                            |p| p.default_sort_dir = SortDirPref::Desc,
                        ),
                    ],
                    cx,
                ),
            ),
            self.settings_row(
                "Default view",
                self.segmented(
                    "view",
                    vec![
                        ("List", prefs.default_view_mode == ViewPref::List, |p| {
                            p.default_view_mode = ViewPref::List
                        }),
                        ("Grid", prefs.default_view_mode == ViewPref::Grid, |p| {
                            p.default_view_mode = ViewPref::Grid
                        }),
                    ],
                    cx,
                ),
            ),
        ];
        let body = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .pb(px(8.))
                    .text_11()
                    .text_color(c(TEXT_MUTED))
                    .child("GENERAL"),
            )
            .children(rows);
        modal(
            "Settings",
            576.,
            body.into_any_element(),
            Vec::new(),
            on_close,
        )
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_only_file_extensions() {
        assert_eq!(
            display_name("report.final.pdf", false, false),
            "report.final"
        );
        assert_eq!(display_name(".bashrc", false, false), ".bashrc");
        assert_eq!(display_name("photos.2024", true, false), "photos.2024");
        assert_eq!(display_name("a.txt", false, true), "a.txt");
    }
}
