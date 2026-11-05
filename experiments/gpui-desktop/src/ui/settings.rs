//! Settings modal for the GPUI desktop experiment.

use gpui::{div, prelude::*, px, AnyElement, Context};

use super::{icon, TextScale};
use crate::theme::*;
use crate::workspace::{Preferences, SettingsSection, Workspace};

impl Workspace {
    fn settings_tab(
        &self,
        id: &'static str,
        label: &'static str,
        section: SettingsSection,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.settings_section == section;
        div()
            .id(id)
            .w_full()
            .rounded(px(4.))
            .px(px(10.))
            .py(px(7.))
            .text_13()
            .cursor_pointer()
            .text_color(c(if active { WHITE } else { TEXT_SECONDARY }))
            .when(active, |item| item.bg(c(SELECTION)))
            .when(!active, |item| {
                item.hover(|style| style.bg(c(SURFACE_ELEVATED)).text_color(c(TEXT_PRIMARY)))
            })
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_settings_section(section, cx);
            }))
    }

    fn setting_row(
        &self,
        label: &'static str,
        value: impl IntoElement,
    ) -> impl IntoElement {
        div()
            .w_full()
            .min_h(px(40.))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(18.))
            .border_b_1()
            .border_color(c(BORDER_SECONDARY))
            .child(div().text_13().text_color(c(TEXT_PRIMARY)).child(label))
            .child(value)
    }

    fn toggle(
        &self,
        id: &'static str,
        checked: bool,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Workspace, &mut Context<Workspace>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .w(px(38.))
            .h(px(22.))
            .rounded(px(12.))
            .flex()
            .items_center()
            .px(px(3.))
            .cursor_pointer()
            .bg(c(if checked { ACCENT } else { SURFACE_ELEVATED }))
            .child(
                div()
                    .size(px(16.))
                    .rounded(px(9.))
                    .bg(c(WHITE))
                    .when(checked, |knob| knob.ml(px(16.))),
            )
            .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
    }

    fn cycle_button(
        &self,
        id: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Workspace, &mut Context<Workspace>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .min_w(px(118.))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .rounded(px(4.))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_SECONDARY))
            .px(px(9.))
            .py(px(5.))
            .text_13()
            .text_color(c(TEXT_PRIMARY))
            .cursor_pointer()
            .hover(|style| style.bg(c(SURFACE_ELEVATED)))
            .child(label)
            .child(icon("chevron-down", 14., c(TEXT_SECONDARY)))
            .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx)))
    }

    fn render_general_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = &self.preferences;
        let mut content = div().flex().flex_col().w_full();
        content = content.child(self.setting_row(
            "Show hidden files",
            self.toggle("setting-hidden", p.show_hidden_files, cx, |this, cx| {
                this.preferences.show_hidden_files = !this.preferences.show_hidden_files;
                cx.notify();
            }),
        ));
        content = content.child(self.setting_row(
            "Show file extensions",
            self.toggle("setting-extensions", p.show_file_extensions, cx, |this, cx| {
                this.preferences.show_file_extensions = !this.preferences.show_file_extensions;
                cx.notify();
            }),
        ));
        content = content.child(self.setting_row(
            "Confirm before delete",
            self.toggle("setting-confirm-delete", p.confirm_delete, cx, |this, cx| {
                this.preferences.confirm_delete = !this.preferences.confirm_delete;
                cx.notify();
            }),
        ));
        content = content.child(self.setting_row(
            "Preview on single click",
            self.toggle("setting-preview", p.preview_on_single_click, cx, |this, cx| {
                this.preferences.preview_on_single_click = !this.preferences.preview_on_single_click;
                cx.notify();
            }),
        ));
        content = content.child(self.setting_row(
            "Selection checkboxes",
            self.toggle("setting-checkboxes", p.selection_checkboxes, cx, |this, cx| {
                this.preferences.selection_checkboxes = !this.preferences.selection_checkboxes;
                cx.notify();
            }),
        ));
        content = content.child(self.setting_row(
            "Default sort",
            self.cycle_button("setting-sort", p.default_sort, cx, |this, cx| {
                this.preferences.default_sort = match this.preferences.default_sort {
                    "Name" => "Size",
                    "Size" => "Date modified",
                    "Date modified" => "Type",
                    _ => "Name",
                };
                cx.notify();
            }),
        ));
        content = content.child(self.setting_row(
            "Default sort direction",
            self.cycle_button("setting-sort-direction", p.default_sort_direction, cx, |this, cx| {
                this.preferences.default_sort_direction = if this.preferences.default_sort_direction == "Ascending" { "Descending" } else { "Ascending" };
                cx.notify();
            }),
        ));
        content.child(self.setting_row(
            "Default view",
            self.cycle_button("setting-view", p.default_view, cx, |this, cx| {
                this.preferences.default_view = if this.preferences.default_view == "List" { "Grid" } else { "List" };
                cx.notify();
            }),
        ))
    }

    fn render_appearance_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = &self.preferences;
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(self.setting_row(
                "Hardware acceleration",
                self.toggle("setting-hardware", p.hardware_acceleration, cx, |this, cx| {
                    this.preferences.hardware_acceleration = !this.preferences.hardware_acceleration;
                    cx.notify();
                }),
            ))
            .child(
                div()
                    .py(px(10.))
                    .text_xs_tw()
                    .text_color(c(TEXT_MUTED))
                    .child("Restart BoxBox to apply hardware acceleration changes."),
            )
            .child(self.setting_row(
                "Compact rows",
                self.toggle("setting-compact", p.compact_rows, cx, |this, cx| {
                    this.preferences.compact_rows = !this.preferences.compact_rows;
                    cx.notify();
                }),
            ))
            .child(self.setting_row(
                "Tint components from wallpaper",
                self.toggle("setting-wallpaper-tint", p.wallpaper_tint, cx, |this, cx| {
                    this.preferences.wallpaper_tint = !this.preferences.wallpaper_tint;
                    cx.notify();
                }),
            ))
            .child(self.setting_row(
                "Frosted glass",
                self.toggle("setting-frosted", p.frosted_glass, cx, |this, cx| {
                    this.preferences.frosted_glass = !this.preferences.frosted_glass;
                    cx.notify();
                }),
            ))
            .child(
                div()
                    .py(px(12.))
                    .text_sm_tw()
                    .text_color(c(TEXT_SECONDARY))
                    .child("Wallpaper and accent color controls will be added with native file picking."),
            )
    }

    fn render_unavailable(&self, title: &'static str, message: &'static str) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .py(px(12.))
            .child(div().text_sm_tw().text_color(c(TEXT_PRIMARY)).child(title))
            .child(div().text_13().text_color(c(TEXT_SECONDARY)).child(message))
    }

    pub(crate) fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let content: AnyElement = match self.settings_section {
            SettingsSection::General => self.render_general_settings(cx).into_any_element(),
            SettingsSection::Appearance => self.render_appearance_settings(cx).into_any_element(),
            SettingsSection::Links => self
                .render_unavailable(
                    "Shared links",
                    "Shared-link management is available in the web and Electrobun clients, but not yet in this native experiment.",
                )
                .into_any_element(),
            SettingsSection::Connections => self
                .render_unavailable(
                    "Connections",
                    "This GPUI build currently connects to one BOXBOX_URL. Saved server connections are not implemented yet.",
                )
                .into_any_element(),
        };

        div()
            .id("settings-overlay")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(alpha(0x000000, 0.58))
            .child(
                div()
                    .id("settings-modal")
                    .w(px(720.))
                    .max_w(px(900.))
                    .h(px(520.))
                    .max_h(px(700.))
                    .flex()
                    .flex_col()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(c(BORDER_PRIMARY))
                    .bg(c(SURFACE_PRIMARY))
                    .shadow_lg()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .border_b_1()
                            .border_color(c(BORDER_SECONDARY))
                            .px(px(18.))
                            .py(px(13.))
                            .child(div().text_sm_tw().font_weight(gpui::FontWeight::SEMIBOLD).child("Settings"))
                            .child(
                                div()
                                    .id("close-settings")
                                    .size(px(28.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(c(SURFACE_ELEVATED)))
                                    .child(icon("x", 16., c(TEXT_SECONDARY)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.settings_open = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_h_0()
                            .child(
                                div()
                                    .w(px(156.))
                                    .flex_none()
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.))
                                    .border_r_1()
                                    .border_color(c(BORDER_SECONDARY))
                                    .p(px(10.))
                                    .child(self.settings_tab("settings-general", "General", SettingsSection::General, cx))
                                    .child(self.settings_tab("settings-appearance", "Appearance", SettingsSection::Appearance, cx))
                                    .child(self.settings_tab("settings-links", "Shared links", SettingsSection::Links, cx))
                                    .child(self.settings_tab("settings-connections", "Connections", SettingsSection::Connections, cx))
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(
                                                div()
                                                    .id("reset-settings")
                                                    .rounded(px(4.))
                                                    .px(px(10.))
                                                    .py(px(7.))
                                                    .text_13()
                                                    .text_color(c(TEXT_SECONDARY))
                                                    .cursor_pointer()
                                                    .hover(|style| style.bg(c(SURFACE_ELEVATED)).text_color(c(TEXT_PRIMARY)))
                                                    .child("Reset to defaults")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.preferences = Preferences::default();
                                                        cx.notify();
                                                    })),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .min_h_0()
                                    .p(px(20.))
                                    .child(content),
                            ),
                    ),
            )
    }
}
