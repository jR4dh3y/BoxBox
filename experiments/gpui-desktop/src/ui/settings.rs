//! Settings modal for the GPUI desktop experiment.

use gpui::{
    div, prelude::*, px, AnyElement, ClipboardItem, Context, KeyDownEvent, MouseButton,
    PathPromptOptions, Window,
};

use super::{icon, TextScale};
use crate::api::{Api, SharePermissions, ShareSummary};
use crate::settings_state::{
    validate_connection, AccentColor, ConnectionField, DesktopSettings, SavedConnection,
    SettingsDialog, ShareExpiry, ShareField, ShareListState, WallpaperFit,
};
use crate::theme::*;
use crate::workspace::{Preferences, SettingsSection, Workspace};

#[derive(Clone, Copy)]
enum SettingsInput {
    AccentColor,
    ConnectionName,
    ConnectionUrl,
    ConnectionToken,
    SharePath,
    ShareUploadLimit,
}

#[derive(Clone, Copy)]
enum SharePermissionField {
    Upload,
    Delete,
    Manage,
}

impl Workspace {
    fn confirm_reset_preferences(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let answer = window.prompt(
            gpui::PromptLevel::Warning,
            "Reset all preferences?",
            Some("This removes your favorites and saved local wallpaper."),
            &["Cancel", "Reset"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if answer.await != Ok(1) { return; }
            let _ = this.update(cx, |this, cx| {
                this.preferences = Preferences::default();
                this.commit_preferences(cx);
                let connections = std::mem::take(&mut this.desktop_settings.connections);
                this.desktop_settings = DesktopSettings { connections, ..DesktopSettings::default() };
                this.favorites.entries.clear();
                this.favorites_error = this.favorites.save().err().map(|error| error.to_string());
                this.reload_wallpaper(cx);
                this.save_desktop_settings(cx);
            });
        }).detach();
    }

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
            .debug_selector(|| id.to_string())
            .w_full()
            .rounded(px(4.))
            .px(px(10.))
            .py(px(7.))
            .text_13()
            .cursor_pointer()
            .text_color(c(if active { WHITE } else { TEXT_SECONDARY }))
            .when(active, |item| item.bg(self.selection_color(false)))
            .when(!active, |item| {
                item.hover(|style| style.bg(c(SURFACE_ELEVATED)).text_color(c(TEXT_PRIMARY)))
            })
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_settings_section(section, cx);
                if section == SettingsSection::Links {
                    this.load_shares(cx);
                }
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
            .debug_selector(|| id.to_string())
            .w(px(38.))
            .h(px(22.))
            .rounded(px(12.))
            .flex()
            .items_center()
            .px(px(3.))
            .cursor_pointer()
            .bg(c(if checked { self.accent_hex() } else { SURFACE_ELEVATED }))
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
        label: String,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Workspace, &mut Context<Workspace>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .debug_selector(|| id.to_string())
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
                this.commit_preferences(cx);
            }),
        ));
        content = content.child(self.setting_row(
            "Show file extensions",
            self.toggle("setting-extensions", p.show_file_extensions, cx, |this, cx| {
                this.preferences.show_file_extensions = !this.preferences.show_file_extensions;
                this.commit_preferences(cx);
            }),
        ));
        content = content.child(self.setting_row(
            "Confirm before delete",
            self.toggle("setting-confirm-delete", p.confirm_delete, cx, |this, cx| {
                this.preferences.confirm_delete = !this.preferences.confirm_delete;
                this.commit_preferences(cx);
            }),
        ));
        content = content.child(self.setting_row(
            "Preview on single click",
            self.toggle("setting-preview", p.preview_on_single_click, cx, |this, cx| {
                this.preferences.preview_on_single_click = !this.preferences.preview_on_single_click;
                this.commit_preferences(cx);
            }),
        ));
        content = content.child(self.setting_row(
            "Selection checkboxes",
            self.toggle("setting-checkboxes", p.selection_checkboxes, cx, |this, cx| {
                this.preferences.selection_checkboxes = !this.preferences.selection_checkboxes;
                this.commit_preferences(cx);
            }),
        ));
        content = content.child(self.setting_row(
            "Default sort",
            self.cycle_button("setting-sort", p.default_sort.clone(), cx, |this, cx| {
                this.preferences.default_sort = match this.preferences.default_sort.as_str() {
                    "Name" => "Size",
                    "Size" => "Date modified",
                    "Date modified" => "Type",
                    _ => "Name",
                }.into();
                this.commit_preferences(cx);
            }),
        ));
        content = content.child(self.setting_row(
            "Default sort direction",
            self.cycle_button("setting-sort-direction", p.default_sort_direction.clone(), cx, |this, cx| {
                this.preferences.default_sort_direction = if this.preferences.default_sort_direction == "Ascending" { "Descending" } else { "Ascending" }.into();
                this.commit_preferences(cx);
            }),
        ));
        content.child(self.setting_row(
            "Default view",
            self.cycle_button("setting-view", p.default_view.clone(), cx, |this, cx| {
                this.preferences.default_view = if this.preferences.default_view == "List" { "Grid" } else { "List" }.into();
                this.commit_preferences(cx);
            }),
        ))
    }

    fn render_appearance_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let preferences = &self.preferences;
        let wallpaper_path = self
            .desktop_settings
            .wallpaper
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "No wallpaper selected".to_string());
        let mut wallpaper_controls = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(self.settings_button(
                "wallpaper-choose".to_string(),
                "Choose…".to_string(),
                true,
                cx,
                |this, cx| this.choose_wallpaper(cx),
            ));
        if self.desktop_settings.wallpaper.is_some() {
            wallpaper_controls = wallpaper_controls.child(self.settings_button(
                "wallpaper-remove".to_string(),
                "Remove".to_string(),
                true,
                cx,
                |this, cx| {
                    this.desktop_settings.wallpaper = None;
                    this.reload_wallpaper(cx);
                    this.save_desktop_settings(cx);
                },
            ));
        }

        div()
            .flex()
            .flex_col()
            .w_full()
            .child(self.setting_row("Wallpaper", wallpaper_controls))
            .child(
                div()
                    .py(px(5.))
                    .text_xs_tw()
                    .text_color(c(TEXT_MUTED))
                    .child(wallpaper_path),
            )
            .child(self.setting_row(
                "Wallpaper scaling",
                self.cycle_button(
                    "setting-wallpaper-fit",
                    self.desktop_settings.wallpaper_fit.label().to_string(),
                    cx,
                    |this, cx| {
                        let current = this.desktop_settings.wallpaper_fit;
                        let current_index = WallpaperFit::ALL
                            .iter()
                            .position(|fit| *fit == current)
                            .unwrap_or(0);
                        this.desktop_settings.wallpaper_fit =
                            WallpaperFit::ALL[(current_index + 1) % WallpaperFit::ALL.len()];
                        this.save_desktop_settings(cx);
                    },
                ),
            ))
            .child(self.setting_row("Accent color source", self.cycle_button("accent-source", if self.desktop_settings.accent_from_wallpaper { "From wallpaper" } else { "Custom" }.into(), cx, |this, cx| {
                this.desktop_settings.accent_from_wallpaper = !this.desktop_settings.accent_from_wallpaper;
                this.save_desktop_settings(cx);
            })))
            .child(self.setting_row("Custom color", self.settings_button("accent-custom".into(), format!("#{:06x}", self.accent_hex()), true, cx, |this, cx| {
                this.settings_panel.dialog = Some(SettingsDialog::EditAccent { color: String::new() }); cx.notify();
            })))
            .child(self.setting_row(
                "Accent color",
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .children(AccentColor::ALL.into_iter().enumerate().map(|(index, accent)| {
                        let selected = self.desktop_settings.accent == accent;
                        div()
                            .id(("accent", index))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(c(if selected {
                                WHITE
                            } else {
                                BORDER_PRIMARY
                            }))
                            .bg(c(accent.hex()))
                            .px(px(7.))
                            .py(px(4.))
                            .text_xs_tw()
                            .text_color(c(WHITE))
                            .cursor_pointer()
                            .child(accent.label())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.desktop_settings.accent = accent;
                                this.desktop_settings.custom_accent = None;
                                this.desktop_settings.accent_from_wallpaper = false;
                                this.save_desktop_settings(cx);
                            }))
                    })),
            ))
            .when_some(match &self.wallpaper {
                crate::workspace::appearance::WallpaperState::Failed(error) => Some(error.clone()),
                _ => None,
            }, |panel, error| panel.child(div().text_color(c(DANGER)).child(error)))
            .child(self.setting_row("Tint components from wallpaper", self.toggle("setting-tint", preferences.wallpaper_tint, cx, |this, cx| {
                this.preferences.wallpaper_tint = !this.preferences.wallpaper_tint;
                this.commit_preferences(cx);
            })))
            .child(self.setting_row("Frosted glass", self.toggle("setting-frost", preferences.frosted_glass, cx, |this, cx| {
                this.preferences.frosted_glass = !this.preferences.frosted_glass;
                this.commit_preferences(cx);
            })))
            .child(self.setting_row(
                "Hardware acceleration",
                div()
                    .max_w(px(280.))
                    .text_13()
                    .text_color(c(TEXT_SECONDARY))
                    .child("GPUI uses GPU rendering; this build has no hardware-acceleration switch."),
            ))
            .child(self.setting_row(
                "Compact rows",
                self.toggle("setting-compact", preferences.compact_rows, cx, |this, cx| {
                    this.preferences.compact_rows = !this.preferences.compact_rows;
                    this.commit_preferences(cx);
                }),
            ))
    }

    fn settings_button(
        &self,
        id: String,
        label: String,
        enabled: bool,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Workspace, &mut Context<Workspace>) + 'static,
    ) -> impl IntoElement {
        div()
            .debug_selector(|| id.clone())
            .id(gpui::SharedString::from(id))
            .rounded(px(4.))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_SECONDARY))
            .px(px(9.))
            .py(px(5.))
            .text_13()
            .text_color(c(if enabled { TEXT_PRIMARY } else { TEXT_DISABLED }))
            .when(enabled, |button| {
                button
                    .cursor_pointer()
                    .hover(|style| style.bg(c(SURFACE_ELEVATED)))
            })
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                if enabled {
                    on_click(this, cx);
                }
            }))
    }

    fn settings_text_field(
        &self,
        id: &'static str,
        label: &'static str,
        value: &str,
        placeholder: &'static str,
        masked: bool,
        field: SettingsInput,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let shown = if masked {
            "•".repeat(value.chars().count())
        } else {
            value.to_string()
        };
        let has_value = !value.is_empty();
        let focused = self.settings_input_focused(field);
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(div().text_xs_tw().text_color(c(TEXT_SECONDARY)).child(label))
            .child(
                div()
                    .id(id)
            .debug_selector(|| id.to_string())
                    .w_full()
                    .min_h(px(34.))
                    .flex()
                    .items_center()
                    .rounded(px(4.))
                    .border_1()
                    .border_color(c(if focused { self.accent_hex() } else { BORDER_PRIMARY }))
                    .bg(c(SURFACE_SECONDARY))
                    .px(px(10.))
                    .text_13()
                    .text_color(c(if has_value { TEXT_PRIMARY } else { TEXT_MUTED }))
                    .cursor_text()
                    .child(if has_value {
                        shown
                    } else {
                        placeholder.to_string()
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.focus_settings_input(field);
                            window.focus(&this.operation_focus);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
    }

    fn settings_input_focused(&self, field: SettingsInput) -> bool {
        match (&self.settings_panel.dialog, field) {
            (Some(SettingsDialog::EditAccent { .. }), SettingsInput::AccentColor) => true,
            (
                Some(SettingsDialog::EditConnection { focused, .. }),
                SettingsInput::ConnectionName,
            ) => *focused == ConnectionField::Name,
            (
                Some(SettingsDialog::EditConnection { focused, .. }),
                SettingsInput::ConnectionUrl,
            ) => *focused == ConnectionField::Url,
            (
                Some(SettingsDialog::ConnectSaved { focused, .. }),
                SettingsInput::ConnectionToken,
            ) => *focused == ConnectionField::Token,
            (Some(SettingsDialog::CreateShare { focused, .. }), SettingsInput::SharePath) => {
                *focused == ShareField::Path
            }
            (
                Some(SettingsDialog::EditShare { focused, .. }),
                SettingsInput::ShareUploadLimit,
            ) => *focused == ShareField::UploadLimit,
            _ => false,
        }
    }

    fn focus_settings_input(&mut self, field: SettingsInput) {
        self.settings_panel.input_selected = false;
        match (&mut self.settings_panel.dialog, field) {
            (
                Some(SettingsDialog::EditConnection { focused, .. }),
                SettingsInput::ConnectionName,
            ) => *focused = ConnectionField::Name,
            (
                Some(SettingsDialog::EditConnection { focused, .. }),
                SettingsInput::ConnectionUrl,
            ) => *focused = ConnectionField::Url,
            (
                Some(SettingsDialog::ConnectSaved { focused, .. }),
                SettingsInput::ConnectionToken,
            ) => *focused = ConnectionField::Token,
            (Some(SettingsDialog::CreateShare { focused, .. }), SettingsInput::SharePath) => {
                *focused = ShareField::Path
            }
            (
                Some(SettingsDialog::EditShare { focused, .. }),
                SettingsInput::ShareUploadLimit,
            ) => *focused = ShareField::UploadLimit,
            _ => {}
        }
    }

    fn active_settings_input(&mut self) -> Option<&mut String> {
        match self.settings_panel.dialog.as_mut()? {
            SettingsDialog::EditAccent { color } => Some(color),
            SettingsDialog::EditConnection { name, url, focused, .. } => match focused {
                ConnectionField::Name => Some(name), ConnectionField::Url => Some(url), ConnectionField::Token => None,
            },
            SettingsDialog::ConnectSaved { token, .. } => Some(token),
            SettingsDialog::CreateShare { path, .. } => Some(path),
            SettingsDialog::EditShare { max_upload_bytes, .. } => Some(max_upload_bytes),
            _ => None,
        }
    }

    fn edit_settings_input(&mut self, value: &str) {
        let selected = std::mem::take(&mut self.settings_panel.input_selected);
        if let Some(input) = self.active_settings_input() {
            if selected { input.clear(); }
            input.extend(value.chars().filter(|character| !character.is_control()));
        }
    }

    fn backspace_settings_input(&mut self) {
        let selected = std::mem::take(&mut self.settings_panel.input_selected);
        if let Some(input) = self.active_settings_input() {
            if selected { input.clear(); } else { input.pop(); }
        }
    }

    fn settings_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        if keystroke.modifiers.control || keystroke.modifiers.platform {
            if keystroke.key.eq_ignore_ascii_case("v") {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.edit_settings_input(&text);
                    cx.notify();
                }
            }
            cx.stop_propagation();
            return;
        }
        match keystroke.key.as_str() {
            "backspace" => {
                self.backspace_settings_input();
                cx.notify();
            }
            "enter" => self.submit_settings_dialog(cx),
            "escape" => self.cancel_settings_dialog(window, cx),
            _ => {
                if let Some(text) = &keystroke.key_char {
                    self.edit_settings_input(text);
                    cx.notify();
                }
            }
        }
        cx.stop_propagation();
    }

    fn cancel_settings_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_panel.dialog = None;
        self.settings_panel.error = None;
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub(crate) fn save_desktop_settings(&mut self, cx: &mut Context<Self>) {
        match self.desktop_settings.save() {
            Ok(()) => {
                self.desktop_settings.load_error = None;
                self.settings_panel.error = None;
            }
            Err(error) => self.settings_panel.error = Some(error),
        }
        cx.notify();
    }

    fn choose_wallpaper(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose a wallpaper image".into()),
        });
        cx.spawn(async move |this, cx| {
            let result = receiver.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            this.install_wallpaper(path, cx);
                        }
                    }
                    Ok(Err(error)) => {
                        this.settings_panel.error =
                            Some(format!("Could not open the wallpaper picker: {error}"));
                        cx.notify();
                    }
                    Ok(Ok(None)) | Err(_) => {}
                }
            })
            .ok();
        })
        .detach();
    }

    fn render_settings_notices(&self) -> impl IntoElement {
        let error = self
            .settings_panel
            .error
            .as_deref()
            .or(self.desktop_settings.load_error.as_deref());
        let mut notices = div().flex().flex_col().gap(px(5.));
        if let Some(error) = error {
            notices = notices.child(
                div()
                    .rounded(px(4.))
                    .bg(alpha(DANGER, 0.16))
                    .px(px(9.))
                    .py(px(6.))
                    .text_13()
                    .text_color(c(DANGER))
                    .child(error.to_string()),
            );
        }
        if let Some(notice) = self.settings_panel.notice.as_deref() {
            notices = notices.child(
                div()
                    .text_13()
                    .text_color(c(TEXT_SECONDARY))
                    .child(notice.to_string()),
            );
        }
        notices
    }

    fn render_connections(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let local = div()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .py(px(8.))
            .border_b_1()
            .border_color(c(BORDER_SECONDARY))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().text_13().text_color(c(TEXT_PRIMARY)).child("Local computer"))
                    .child(
                        div()
                            .text_xs_tw()
                            .text_color(c(TEXT_MUTED))
                            .child("Browse files on this computer"),
                    ),
            )
            .child(self.settings_button(
                "connection-open-local".to_string(),
                "Open".to_string(),
                true,
                cx,
                |this, cx| this.open_local_provider(cx),
            ));

        let connections = self
            .desktop_settings
            .connections
            .iter()
            .enumerate()
            .map(|(index, connection)| {
                let connection_id = connection.id.clone();
                let name = connection.name.clone();
                let url = connection.url.clone();
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(c(BORDER_SECONDARY))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_13().text_color(c(TEXT_PRIMARY)).child(name.clone()))
                            .child(
                                div()
                                    .text_xs_tw()
                                    .text_color(c(TEXT_MUTED))
                                    .truncate()
                                    .child(url),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .child(self.settings_button(
                                format!("connection-open-{connection_id}"),
                                "Open".to_string(),
                                true,
                                cx,
                                move |this, cx| this.connect_saved_connection(index, cx),
                            ))
                            .child(self.settings_button(
                                format!("connection-edit-{connection_id}"),
                                "Edit".to_string(),
                                true,
                                cx,
                                move |this, cx| this.edit_saved_connection(index, cx),
                            ))
                            .child(self.settings_button(
                                format!("connection-remove-{connection_id}"),
                                "Remove".to_string(),
                                true,
                                cx,
                                move |this, cx| this.confirm_remove_connection(index, cx),
                            )),
                    )
            })
            .collect::<Vec<_>>();
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .w_full()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm_tw()
                            .text_color(c(TEXT_PRIMARY))
                            .child("Saved server connections"),
                    )
                    .child(self.settings_button(
                        "connection-add".to_string(),
                        "Add server".to_string(),
                        true,
                        cx,
                        |this, cx| {
                            this.settings_panel.error = None;
                            this.settings_panel.dialog = Some(SettingsDialog::EditConnection {
                                index: None,
                                name: String::new(),
                                url: String::new(),
                                focused: ConnectionField::Name,
                            });
                            cx.notify();
                        },
                    )),
            )
            .child(
                div()
                    .text_xs_tw()
                    .text_color(c(TEXT_MUTED))
                    .child("Bearer tokens are requested when opening a server and are never saved."),
            )
            .child(local)
            .children(connections)
    }
    fn open_local_provider(&mut self, cx: &mut Context<Self>) {
        match Api::local() {
            Ok(api) => {
                self.settings_panel.dialog = None;
                self.settings_open = false;
                self.open_provider(api, cx);
            }
            Err(error) => {
                self.settings_panel.error = Some(format!("Could not open local files: {error:#}"));
                cx.notify();
            }
        }
    }

    fn edit_saved_connection(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(connection) = self.desktop_settings.connections.get(index) else {
            return;
        };
        self.settings_panel.error = None;
        self.settings_panel.dialog = Some(SettingsDialog::EditConnection {
            index: Some(index),
            name: connection.name.clone(),
            url: connection.url.clone(),
            focused: ConnectionField::Name,
        });
        cx.notify();
    }

    fn connect_saved_connection(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(connection) = self.desktop_settings.connections.get(index).cloned() else {
            return;
        };
        self.settings_panel.error = None;
        self.settings_panel.dialog = Some(SettingsDialog::ConnectSaved {
            connection,
            token: String::new(),
            focused: ConnectionField::Token,
        });
        cx.notify();
    }

    fn confirm_remove_connection(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(connection) = self.desktop_settings.connections.get(index) else {
            return;
        };
        self.settings_panel.dialog = Some(SettingsDialog::RemoveConnection {
            index,
            name: connection.name.clone(),
        });
        cx.notify();
    }

    fn remove_saved_connection(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.desktop_settings.connections.len() {
            self.settings_panel.dialog = None;
            self.settings_panel.error = Some("That saved connection no longer exists.".into());
        } else {
            self.desktop_settings.connections.remove(index);
            self.settings_panel.dialog = None;
            self.save_desktop_settings(cx);
        }
        cx.notify();
    }

    fn save_saved_connection(
        &mut self,
        index: Option<usize>,
        name: String,
        url: String,
        focused: ConnectionField,
        cx: &mut Context<Self>,
    ) {
        let connection = match validate_connection(&name, &url) {
            Ok(connection) => connection,
            Err(error) => {
                self.settings_panel.error = Some(error);
                self.settings_panel.dialog = Some(SettingsDialog::EditConnection {
                    index,
                    name,
                    url,
                    focused,
                });
                cx.notify();
                return;
            }
        };
        if self.desktop_settings.connections.iter().enumerate().any(
            |(existing_index, existing)| {
                Some(existing_index) != index
                    && existing.name.eq_ignore_ascii_case(&connection.name)
            },
        ) {
            self.settings_panel.error =
                Some("A saved connection already uses that name.".into());
            self.settings_panel.dialog = Some(SettingsDialog::EditConnection {
                index,
                name,
                url,
                focused,
            });
            cx.notify();
            return;
        }

        match index {
            Some(index) => {
                let Some(existing) = self.desktop_settings.connections.get(index) else {
                    self.settings_panel.error =
                        Some("That saved connection no longer exists.".into());
                    self.settings_panel.dialog = Some(SettingsDialog::EditConnection {
                        index: Some(index),
                        name,
                        url,
                        focused,
                    });
                    cx.notify();
                    return;
                };
                let mut connection = connection;
                connection.id = existing.id.clone();
                self.desktop_settings.connections[index] = connection;
            }
            None => self.desktop_settings.connections.push(connection),
        }
        self.settings_panel.dialog = None;
        self.save_desktop_settings(cx);
    }

    fn submit_settings_dialog(&mut self, cx: &mut Context<Self>) {
        match self.settings_panel.dialog.as_ref() {
            Some(SettingsDialog::EditAccent { color }) => {
                let hex = color.trim().strip_prefix('#').unwrap_or(color.trim());
                if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    self.settings_panel.error = Some("Enter a six-digit hex color, such as #4a9eff.".into());
                    cx.notify(); return;
                }
                if let Ok(value) = u32::from_str_radix(hex, 16) {
                    self.desktop_settings.custom_accent = Some(value);
                    self.desktop_settings.accent_from_wallpaper = false;
                    self.settings_panel.dialog = None;
                    self.save_desktop_settings(cx);
                }
            },
            Some(SettingsDialog::EditConnection { .. }) => {
                if let Some(SettingsDialog::EditConnection {
                    index,
                    name,
                    url,
                    focused,
                }) = self.settings_panel.dialog.take()
                {
                    self.save_saved_connection(index, name, url, focused, cx);
                }
            }
            Some(SettingsDialog::ConnectSaved { .. }) => {
                if let Some(SettingsDialog::ConnectSaved {
                    connection,
                    token,
                    ..
                }) = self.settings_panel.dialog.take()
                {
                    self.open_saved_connection(connection, token, cx);
                }
            }
            Some(SettingsDialog::CreateShare { .. }) => self.create_share(cx),
            Some(SettingsDialog::EditShare { .. }) => self.save_share_permissions(cx),
            Some(SettingsDialog::RemoveConnection { index, .. }) => {
                let index = *index;
                self.remove_saved_connection(index, cx);
            }
            Some(SettingsDialog::RevokeShare { .. }) => self.revoke_share(cx),
            _ => {}
        }
    }

    fn open_saved_connection(
        &mut self,
        connection: SavedConnection,
        token: String,
        cx: &mut Context<Self>,
    ) {
        let connection = match validate_connection(&connection.name, &connection.url) {
            Ok(validated) => SavedConnection {
                id: connection.id,
                ..validated
            },
            Err(error) => {
                self.settings_panel.error = Some(error);
                cx.notify();
                return;
            }
        };
        let token = (!token.is_empty()).then_some(token);
        let api = Api::new(&connection.url, token);
        self.settings_panel.dialog = None;
        self.settings_open = false;
        self.open_provider(api, cx);
    }


    fn start_share_mutation(
        &mut self,
    ) -> Option<(std::sync::Arc<Api>, String, u64)> {
        let api = self.active_api();
        if api.is_local() {
            self.settings_panel.error =
                Some("Public sharing requires a connected BoxBox server.".into());
            return None;
        }
        let Some(server) = api.server_url().map(str::to_owned) else {
            self.settings_panel.error =
                Some("No remote server is available for share management.".into());
            return None;
        };
        self.settings_panel.share_request = self.settings_panel.share_request.wrapping_add(1);
        self.settings_panel.busy = true;
        self.settings_panel.error = None;
        self.settings_panel.notice = None;
        Some((api, server, self.settings_panel.share_request))
    }
    fn render_share_management(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let api = self.active_api();
        if api.is_local() {
            return div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .text_sm_tw()
                        .text_color(c(TEXT_PRIMARY))
                        .child("Shared links require a connected BoxBox server."),
                )
                .child(
                    div()
                        .text_13()
                        .text_color(c(TEXT_SECONDARY))
                        .child("The local provider does not host a reachable public sharing service. No public links can be created in local mode."),
                );
        }

        let server = api.server_url().unwrap_or_default().to_string();
        let shares: AnyElement = match &self.settings_panel.shares {
            ShareListState::Loading {
                server: loaded,
                provider,
            } if loaded == &server && std::sync::Arc::ptr_eq(provider, &api) => div()
                .py(px(12.))
                .text_13()
                .text_color(c(TEXT_SECONDARY))
                .child("Loading shared links…")
                .into_any_element(),
            ShareListState::Loaded {
                server: loaded,
                provider,
                shares,
            } if loaded == &server && std::sync::Arc::ptr_eq(provider, &api) => {
                if shares.is_empty() {
                    div()
                        .py(px(12.))
                        .text_13()
                        .text_color(c(TEXT_SECONDARY))
                        .child("No shared links on this server.")
                        .into_any_element()
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .children(
                            shares
                                .iter()
                                .map(|share| self.render_share_row(share, cx)),
                        )
                        .into_any_element()
                }
            }
            ShareListState::Failed {
                server: loaded,
                provider,
                error,
            } if loaded == &server && std::sync::Arc::ptr_eq(provider, &api) => div()
                .py(px(12.))
                .text_13()
                .text_color(c(DANGER))
                .child(error.clone())
                .into_any_element(),
            _ => div()
                .py(px(12.))
                .text_13()
                .text_color(c(TEXT_SECONDARY))
                .child("Load shared links from the active server.")
                .into_any_element(),
        };

        div()
            .flex()
            .flex_col()
            .gap(px(9.))
            .w_full()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .min_w_0()
                            .text_xs_tw()
                            .text_color(c(TEXT_MUTED))
                            .truncate()
                            .child(server),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(self.settings_button(
                                "shares-refresh".to_string(),
                                if self.settings_panel.busy {
                                    "Loading…".to_string()
                                } else {
                                    "Refresh".to_string()
                                },
                                !self.settings_panel.busy,
                                cx,
                                |this, cx| this.load_shares(cx),
                            ))
                            .child(self.settings_button(
                                "shares-create".to_string(),
                                "Create share".to_string(),
                                !self.settings_panel.busy,
                                cx,
                                |this, cx| {
                                    this.settings_panel.error = None;
                                    this.settings_panel.dialog =
                                        Some(SettingsDialog::CreateShare {
                                            path: String::new(),
                                            permissions: SharePermissions {
                                                view: true,
                                                download: true,
                                                upload: false,
                                                delete: false,
                                                manage: false,
                                            },
                                            expires: ShareExpiry::default(),
                                            focused: ShareField::Path,
                                        });
                                    cx.notify();
                                },
                            )),
                    ),
            )
            .child(
                div()
                    .text_xs_tw()
                    .text_color(c(TEXT_MUTED))
                    .child("Expiry is selected when a link is created; existing expiry cannot be changed by this server API."),
            )
            .child(shares)
    }

    fn render_share_row(&self, share: &ShareSummary, cx: &mut Context<Self>) -> impl IntoElement {
        let copy_id = share.id.clone();
        let edit_id = share.id.clone();
        let revoke_id = share.id.clone();
        let label = share.file_name.clone();
        let edit_label = label.clone();
        let revoke_label = label.clone();
        let permissions = share.permissions.clone();
        let edit_permissions = permissions.clone();
        let edit_max_upload_bytes = share.max_upload_bytes;
        let expiry = share.expires_at.as_deref().unwrap_or("Never");
        let upload_limit = if permissions.upload {
            format!(" · {} bytes per file", share.max_upload_bytes)
        } else {
            String::new()
        };
        div()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(10.))
            .py(px(9.))
            .border_b_1()
            .border_color(c(BORDER_SECONDARY))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_13()
                            .text_color(c(TEXT_PRIMARY))
                            .truncate()
                            .child(label.clone()),
                    )
                    .child(
                        div()
                            .text_xs_tw()
                            .text_color(c(TEXT_MUTED))
                            .truncate()
                            .child(format!(
                                "{} · {}{} · expires {}",
                                if share.is_folder { "Folder" } else { "File" },
                                share_permission_summary(&permissions),
                                upload_limit,
                                expiry
                            )),
                    )
                    .child(
                        div()
                            .text_xs_tw()
                            .text_color(c(TEXT_MUTED))
                            .truncate()
                            .child(share.path.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .child(self.settings_button(
                        format!("share-copy-{}", share.id),
                        "Copy link".to_string(),
                        !self.settings_panel.busy,
                        cx,
                        move |this, cx| this.copy_share_link(&copy_id, cx),
                    ))
                    .when(share.is_folder, |row| {
                        row.child(self.settings_button(
                            format!("share-edit-{}", share.id),
                            "Permissions".to_string(),
                            !self.settings_panel.busy,
                            cx,
                            move |this, cx| {
                                this.edit_share_permissions(
                                    &edit_id,
                                    &edit_label,
                                    edit_permissions.clone(),
                                    edit_max_upload_bytes,
                                    cx,
                                )
                            },
                        ))
                    })
                    .child(self.settings_button(
                        format!("share-revoke-{}", share.id),
                        "Revoke".to_string(),
                        !self.settings_panel.busy,
                        cx,
                        move |this, cx| {
                            this.confirm_revoke_share(&revoke_id, &revoke_label, cx)
                        },
                    )),
            )
    }

    fn load_shares(&mut self, cx: &mut Context<Self>) {
        let api = self.active_api();
        if api.is_local() {
            self.settings_panel.share_request = self.settings_panel.share_request.wrapping_add(1);
            self.settings_panel.shares = ShareListState::Idle;
            self.settings_panel.busy = false;
            self.settings_panel.error = None;
            self.settings_panel.notice = None;
            cx.notify();
            return;
        }
        let Some(server) = api.server_url().map(str::to_owned) else {
            self.settings_panel.shares = ShareListState::Failed {
                server: String::new(),
                provider: api,
                error: "No remote server is available for share management.".into(),
            };
            self.settings_panel.busy = false;
            cx.notify();
            return;
        };
        self.settings_panel.share_request = self.settings_panel.share_request.wrapping_add(1);
        let request = self.settings_panel.share_request;
        self.settings_panel.shares = ShareListState::Loading {
            server: server.clone(),
            provider: api.clone(),
        };
        self.settings_panel.busy = true;
        self.settings_panel.error = None;
        self.settings_panel.notice = None;
        cx.notify();
        let request_api = api.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { request_api.list_shares() })
                .await;
            this.update(cx, |this, cx| {
                let active = this.active_api();
                if this.settings_panel.share_request != request
                    || !std::sync::Arc::ptr_eq(&active, &api)
                {
                    return;
                }
                this.settings_panel.busy = false;
                this.settings_panel.shares = match result {
                    Ok(shares) => ShareListState::Loaded {
                        server,
                        provider: api.clone(),
                        shares,
                    },
                    Err(error) => ShareListState::Failed {
                        server,
                        provider: api.clone(),
                        error: format!("{error:#}"),
                    },
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    fn copy_share_link(&mut self, share_id: &str, cx: &mut Context<Self>) {
        let api = self.active_api();
        let Some(server) = api.server_url() else {
            self.settings_panel.error =
                Some("A connected server is required to copy this share link.".into());
            cx.notify();
            return;
        };
        let share_url = match &self.settings_panel.shares {
            ShareListState::Loaded {
                server: loaded,
                provider,
                shares,
            } if loaded == server && std::sync::Arc::ptr_eq(provider, &api) => shares
                .iter()
                .find(|share| share.id == share_id)
                .map(|share| share.url.as_str()),
            _ => None,
        };
        let Some(link) = share_url.and_then(|url| absolute_share_url(server, url)) else {
            self.settings_panel.error =
                Some("The share link is unavailable or invalid for this server.".into());
            cx.notify();
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(link));
        self.settings_panel.error = None;
        self.settings_panel.notice = Some("Share link copied to the clipboard.".into());
        cx.notify();
    }

    fn edit_share_permissions(
        &mut self,
        id: &str,
        label: &str,
        mut permissions: SharePermissions,
        max_upload_bytes: i64,
        cx: &mut Context<Self>,
    ) {
        permissions.view = true;
        permissions.download = true;
        let focused = if permissions.upload {
            ShareField::UploadLimit
        } else {
            ShareField::Path
        };
        self.settings_panel.dialog = Some(SettingsDialog::EditShare {
            id: id.to_string(),
            label: label.to_string(),
            permissions,
            max_upload_bytes: max_upload_bytes.to_string(),
            focused,
        });
        self.settings_panel.error = None;
        cx.notify();
    }

    fn confirm_revoke_share(&mut self, id: &str, label: &str, cx: &mut Context<Self>) {
        self.settings_panel.dialog = Some(SettingsDialog::RevokeShare {
            id: id.to_string(),
            label: label.to_string(),
        });
        self.settings_panel.error = None;
        cx.notify();
    }

    fn toggle_share_permission(&mut self, field: SharePermissionField, cx: &mut Context<Self>) {
        let permissions = match self.settings_panel.dialog.as_mut() {
            Some(SettingsDialog::CreateShare { permissions, .. })
            | Some(SettingsDialog::EditShare { permissions, .. }) => permissions,
            _ => return,
        };
        match field {
            SharePermissionField::Upload => {
                permissions.upload = !permissions.upload;
                if !permissions.upload {
                    permissions.delete = false;
                    permissions.manage = false;
                }
            }
            SharePermissionField::Delete => {
                permissions.delete = !permissions.delete;
                if permissions.delete {
                    permissions.upload = true;
                } else {
                    permissions.manage = false;
                }
            }
            SharePermissionField::Manage => {
                permissions.manage = !permissions.manage;
                if permissions.manage {
                    permissions.upload = true;
                    permissions.delete = true;
                }
            }
        }
        permissions.view = true;
        permissions.download = true;
        if let Some(SettingsDialog::EditShare {
            permissions,
            focused,
            ..
        }) = &mut self.settings_panel.dialog
        {
            if !permissions.upload {
                *focused = ShareField::Path;
            }
        }
        cx.notify();
    }

    fn share_permission_row(
        &self,
        id: &'static str,
        label: &'static str,
        checked: bool,
        field: SharePermissionField,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.setting_row(
            label,
            self.toggle(id, checked, cx, move |this, cx| {
                this.toggle_share_permission(field, cx);
            }),
        )
    }

    fn run_share_mutation(
        &mut self,
        api: std::sync::Arc<Api>,
        server: String,
        request: u64,
        action: &'static str,
        operation: impl FnOnce(&Api) -> Result<(), String> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
        cx.spawn(async move |this, cx| {
            let operation_api = api.clone();
            let result = cx
                .background_executor()
                .spawn(async move { operation(&operation_api) })
                .await;
            this.update(cx, |this, cx| {
                let active = this.active_api();
                if this.settings_panel.share_request != request
                    || !std::sync::Arc::ptr_eq(&active, &api)
                {
                    return;
                }
                this.settings_panel.busy = false;
                match result {
                    Ok(()) => this.load_shares(cx),
                    Err(error) => {
                        this.settings_panel.error = Some(format!("Could not {action}: {error}"));
                        if !matches!(
                            &this.settings_panel.shares,
                            ShareListState::Loaded {
                                server: loaded,
                                provider,
                                ..
                            } if loaded == &server
                                && std::sync::Arc::ptr_eq(provider, &api)
                        ) {
                            this.settings_panel.shares = ShareListState::Failed {
                                server,
                                provider: api.clone(),
                                error: format!("Could not {action}: {error}"),
                            };
                        }
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn create_share(&mut self, cx: &mut Context<Self>) {
        let Some(SettingsDialog::CreateShare {
            path,
            mut permissions,
            expires,
            focused,
        }) = self.settings_panel.dialog.take()
        else {
            return;
        };
        let path = path.trim().to_string();
        if path.is_empty() || path.contains('\0') {
            self.settings_panel.error = Some("Enter a valid file or folder path.".into());
            self.settings_panel.dialog = Some(SettingsDialog::CreateShare {
                path,
                permissions,
                expires,
                focused,
            });
            cx.notify();
            return;
        }
        permissions.view = true;
        permissions.download = true;
        if !permissions.upload {
            permissions.delete = false;
            permissions.manage = false;
        }
        if !permissions.delete {
            permissions.manage = false;
        }
        let Some((api, server, request)) = self.start_share_mutation() else {
            self.settings_panel.dialog = Some(SettingsDialog::CreateShare {
                path,
                permissions,
                expires,
                focused,
            });
            cx.notify();
            return;
        };
        let expiry = expires.seconds();
        self.run_share_mutation(
            api,
            server,
            request,
            "create share",
            move |api| {
                api.create_share(&path, permissions, expiry)
                    .map(|_| ())
                    .map_err(|error| format!("{error:#}"))
            },
            cx,
        );
    }

    fn save_share_permissions(&mut self, cx: &mut Context<Self>) {
        let Some(SettingsDialog::EditShare {
            id,
            label,
            mut permissions,
            max_upload_bytes,
            focused,
        }) = self.settings_panel.dialog.take()
        else {
            return;
        };
        let raw_upload_limit = max_upload_bytes.clone();
        permissions.view = true;
        permissions.download = true;
        if !permissions.upload {
            permissions.delete = false;
            permissions.manage = false;
        }
        if !permissions.delete {
            permissions.manage = false;
        }
        let max_upload_bytes = if permissions.upload && !max_upload_bytes.trim().is_empty() {
            match max_upload_bytes.trim().parse::<i64>() {
                Ok(limit) if limit > 0 => Some(limit),
                _ => {
                    self.settings_panel.error =
                        Some("Upload limit must be a positive number of bytes.".into());
                    self.settings_panel.dialog = Some(SettingsDialog::EditShare {
                        id,
                        label,
                        permissions,
                        max_upload_bytes,
                        focused: ShareField::UploadLimit,
                    });
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };
        let Some((api, server, request)) = self.start_share_mutation() else {
            self.settings_panel.dialog = Some(SettingsDialog::EditShare {
                id,
                label,
                permissions,
                max_upload_bytes: raw_upload_limit,
                focused,
            });
            cx.notify();
            return;
        };
        self.settings_panel.dialog = None;
        self.run_share_mutation(
            api,
            server,
            request,
            "update share permissions",
            move |api| {
                api.update_share(&id, permissions, max_upload_bytes)
                    .map(|_| ())
                    .map_err(|error| format!("{error:#}"))
            },
            cx,
        );
    }

    fn revoke_share(&mut self, cx: &mut Context<Self>) {
        let Some(SettingsDialog::RevokeShare { id, label }) = self.settings_panel.dialog.take() else {
            return;
        };
        let Some((api, server, request)) = self.start_share_mutation() else {
            self.settings_panel.dialog = Some(SettingsDialog::RevokeShare { id, label });
            cx.notify();
            return;
        };
        self.run_share_mutation(
            api,
            server,
            request,
            "revoke share",
            move |api| api.revoke_share(&id).map_err(|error| format!("{error:#}")),
            cx,
        );
    }
    fn render_settings_dialog(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(dialog) = &self.settings_panel.dialog else {
            return div().into_any_element();
        };

        let (title, body, confirm): (&'static str, AnyElement, AnyElement) = match dialog {
            SettingsDialog::EditAccent { color } => (
                "Custom accent color",
                self.settings_text_field("accent-color-input", "Hex color", color, "#4a9eff", false, SettingsInput::AccentColor, cx).into_any_element(),
                self.settings_button("accent-save".into(), "Apply".into(), true, cx, |this, cx| this.submit_settings_dialog(cx)).into_any_element(),
            ),
            SettingsDialog::EditConnection {
                index,
                name,
                url,
                ..
            } => {
                let title = if index.is_some() {
                    "Edit server connection"
                } else {
                    "Add server connection"
                };
                let body = div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(self.settings_text_field(
                        "settings-connection-name",
                        "Name",
                        name,
                        "Server name",
                        false,
                        SettingsInput::ConnectionName,
                        cx,
                    ))
                    .child(self.settings_text_field(
                        "settings-connection-url",
                        "Server URL",
                        url,
                        "https://files.example.com",
                        false,
                        SettingsInput::ConnectionUrl,
                        cx,
                    ))
                    .child(
                        div()
                            .text_xs_tw()
                            .text_color(c(TEXT_MUTED))
                            .child("Use http:// or https://. Credentials, query strings, and fragments are not allowed."),
                    )
                    .child(
                        div()
                            .text_xs_tw()
                            .text_color(c(TEXT_MUTED))
                            .child("Enter saves · Escape cancels · Ctrl/⌘+V pastes"),
                    )
                    .into_any_element();
                let confirm = self.settings_button(
                    "settings-connection-save".to_string(),
                    "Save".to_string(),
                    true,
                    cx,
                    |this, cx| this.submit_settings_dialog(cx),
                );
                (title, body, confirm.into_any_element())
            }
            SettingsDialog::ConnectSaved {
                connection,
                token,
                ..
            } => {
                let body = div()
                    .flex()
                    .flex_col()
                    .gap(px(9.))
                    .child(
                        div()
                            .text_13()
                            .text_color(c(TEXT_PRIMARY))
                            .child(connection.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs_tw()
                            .text_color(c(TEXT_MUTED))
                            .child(connection.url.clone()),
                    )
                    .child(self.settings_text_field(
                        "settings-connection-token",
                        "Bearer token (optional)",
                        token,
                        "Paste token; it will not be saved",
                        true,
                        SettingsInput::ConnectionToken,
                        cx,
                    ))
                    .child(
                        div()
                            .text_xs_tw()
                            .text_color(c(TEXT_MUTED))
                            .child("Enter opens this server · Escape cancels · Ctrl/⌘+V pastes"),
                    )
                    .into_any_element();
                let confirm = self.settings_button(
                    "settings-connection-open".to_string(),
                    "Open".to_string(),
                    true,
                    cx,
                    |this, cx| this.submit_settings_dialog(cx),
                );
                ("Open server connection", body, confirm.into_any_element())
            }
            SettingsDialog::RemoveConnection { index, name } => {
                let body = div()
                    .text_13()
                    .text_color(c(TEXT_SECONDARY))
                    .child(format!("Remove the saved connection “{name}”? Its open tabs are not affected."))
                    .into_any_element();
                let index = *index;
                let confirm = self.settings_button(
                    "settings-connection-confirm-remove".to_string(),
                    "Remove".to_string(),
                    true,
                    cx,
                    move |this, cx| this.remove_saved_connection(index, cx),
                );
                ("Remove server connection", body, confirm.into_any_element())
            }
            SettingsDialog::CreateShare {
                path,
                permissions,
                expires,
                ..
            } => {
                let current_expiry = *expires;
                let body = div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(self.settings_text_field(
                        "settings-share-path",
                        "File or folder path",
                        path,
                        "Example: Documents/report.pdf",
                        false,
                        SettingsInput::SharePath,
                        cx,
                    ))
                    .child(
                        div()
                            .text_xs_tw()
                            .text_color(c(TEXT_MUTED))
                            .child("View and download are always included. Upload permissions apply to folders."),
                    )
                    .child(self.share_permission_row(
                        "share-create-upload",
                        "Allow uploads",
                        permissions.upload,
                        SharePermissionField::Upload,
                        cx,
                    ))
                    .child(self.share_permission_row(
                        "share-create-delete",
                        "Allow deleting uploaded files",
                        permissions.delete,
                        SharePermissionField::Delete,
                        cx,
                    ))
                    .child(self.share_permission_row(
                        "share-create-manage",
                        "Full access",
                        permissions.manage,
                        SharePermissionField::Manage,
                        cx,
                    ))
                    .child(self.setting_row(
                        "Link expiry",
                        self.cycle_button(
                            "share-create-expiry",
                            current_expiry.label().to_string(),
                            cx,
                            |this, cx| {
                                if let Some(SettingsDialog::CreateShare { expires, .. }) =
                                    &mut this.settings_panel.dialog
                                {
                                    let index = ShareExpiry::ALL
                                        .iter()
                                        .position(|expiry| expiry == expires)
                                        .unwrap_or(0);
                                    *expires = ShareExpiry::ALL
                                        [(index + 1) % ShareExpiry::ALL.len()];
                                }
                                cx.notify();
                            },
                        ),
                    ))
                    .child(
                        div()
                            .text_xs_tw()
                            .text_color(c(TEXT_MUTED))
                            .child("Enter creates the link · Escape cancels · Ctrl/⌘+V pastes"),
                    )
                    .into_any_element();
                let confirm = self.settings_button(
                    "settings-share-create-confirm".to_string(),
                    "Create link".to_string(),
                    !self.settings_panel.busy,
                    cx,
                    |this, cx| this.create_share(cx),
                );
                ("Create shared link", body, confirm.into_any_element())
            }
            SettingsDialog::EditShare {
                label,
                permissions,
                max_upload_bytes,
                ..
            } => {
                let body = div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_13()
                            .text_color(c(TEXT_PRIMARY))
                            .child(label.clone()),
                    )
                    .child(
                        div()
                            .text_xs_tw()
                            .text_color(c(TEXT_MUTED))
                            .child("View and download are always included. Expiry is fixed after creation."),
                    )
                    .child(self.share_permission_row(
                        "share-edit-upload",
                        "Allow uploads",
                        permissions.upload,
                        SharePermissionField::Upload,
                        cx,
                    ))
                    .child(self.share_permission_row(
                        "share-edit-delete",
                        "Allow deleting uploaded files",
                        permissions.delete,
                        SharePermissionField::Delete,
                        cx,
                    ))
                    .child(self.share_permission_row(
                        "share-edit-manage",
                        "Full access",
                        permissions.manage,
                        SharePermissionField::Manage,
                        cx,
                    ))
                    .when(permissions.upload, |body| {
                        body.child(self.settings_text_field(
                            "settings-share-upload-limit",
                            "Maximum upload per file (bytes)",
                            max_upload_bytes,
                            "e.g. 104857600",
                            false,
                            SettingsInput::ShareUploadLimit,
                            cx,
                        ))
                        .child(
                            div()
                                .text_xs_tw()
                                .text_color(c(TEXT_MUTED))
                                .child("Leave blank to keep the server's current limit."),
                        )
                    })
                    .into_any_element();
                let confirm = self.settings_button(
                    "settings-share-save-permissions".to_string(),
                    "Save permissions".to_string(),
                    !self.settings_panel.busy,
                    cx,
                    |this, cx| this.save_share_permissions(cx),
                );
                ("Edit shared-link permissions", body, confirm.into_any_element())
            }
            SettingsDialog::RevokeShare { label, .. } => {
                let body = div()
                    .text_13()
                    .text_color(c(TEXT_SECONDARY))
                    .child(format!("Revoke the link for “{label}”? Anyone using it will lose access."))
                    .into_any_element();
                let confirm = self.settings_button(
                    "settings-share-revoke-confirm".to_string(),
                    "Revoke link".to_string(),
                    !self.settings_panel.busy,
                    cx,
                    |this, cx| this.revoke_share(cx),
                );
                ("Revoke shared link", body, confirm.into_any_element())
            }
        };

        div()
            .id("settings-dialog-overlay")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .key_context("SettingsInput")
            .track_focus(&self.operation_focus)
            .on_key_down(cx.listener(Self::settings_key_down))
            .on_action(cx.listener(|this, _: &crate::workspace::SettingsSubmit, _, cx| this.submit_settings_dialog(cx)))
            .on_action(cx.listener(|this, _: &crate::workspace::SettingsCancel, window, cx| this.cancel_settings_dialog(window, cx)))
            .on_action(cx.listener(|this, _: &crate::workspace::SettingsBackspace, _, cx| { this.backspace_settings_input(); cx.notify(); }))
            .on_action(cx.listener(|this, _: &crate::workspace::SettingsSelectAll, _, cx| { this.settings_panel.input_selected = true; cx.notify(); }))
            .on_action(cx.listener(|this, _: &crate::workspace::SettingsPaste, _, cx| {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) { this.edit_settings_input(&text); cx.notify(); }
            }))
            .flex()
            .items_center()
            .justify_center()
            .bg(alpha(0x000000, 0.55))
            .child(
                div()
                    .id("settings-dialog")
                    .w(px(440.))
                    .max_w(px(640.))
                    .max_h(px(540.))
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(c(BORDER_PRIMARY))
                    .bg(c(SURFACE_PRIMARY))
                    .shadow_lg()
                    .p(px(18.))
                    .child(
                        div()
                            .text_sm_tw()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(c(TEXT_PRIMARY))
                            .child(title),
                    )
                    .child(
                        div()
                            .id("settings-dialog-body")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .child(body)
                            .child(self.render_settings_notices()),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.))
                            .child(self.settings_button(
                                "settings-dialog-cancel".to_string(),
                                "Cancel".to_string(),
                                true,
                                cx,
                                |this, cx| {
                                    this.settings_panel.dialog = None;
                                    this.settings_panel.error = None;
                                    cx.notify();
                                },
                            ))
                            .child(confirm),
                    ),
            )
            .into_any_element()
    }
    pub(crate) fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let content: AnyElement = match self.settings_section {
            SettingsSection::General => self.render_general_settings(cx).into_any_element(),
            SettingsSection::Appearance => self.render_appearance_settings(cx).into_any_element(),
            SettingsSection::Links => self.render_share_management(cx).into_any_element(),
            SettingsSection::Connections => self.render_connections(cx).into_any_element(),
        };

        div()
            .id("settings-overlay")
            .when(self.settings_panel.dialog.is_none(), |panel| panel.track_focus(&self.operation_focus).key_context("SettingsPanel"))
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
                    .max_w(gpui::relative(0.95))
                    .h(px(520.))
                    .max_h(gpui::relative(0.9))
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
                                    .debug_selector(|| "close-settings".into())
                                    .size(px(28.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(c(SURFACE_ELEVATED)))
                                    .child(icon("x", 16., c(TEXT_SECONDARY)))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_settings(&crate::workspace::CloseSettings, window, cx);
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
                                            .id("settings-reset")
                                            .flex_1()
                                            .child(
                                                div()
                                                    .id("settings-reset-button")
                                                    .debug_selector(|| "settings-reset-button".into())
                                                    .child("Reset to defaults")
                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                        this.confirm_reset_preferences(window, cx);
                                                    })),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .id("settings-content")
                                    .flex_1()
                                    .min_w_0()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .p(px(20.))
                                    .child(content)
                                    .child(self.render_settings_notices()),
                    ),
            )
            )
            .child(self.render_settings_dialog(cx))
    }
}

fn absolute_share_url(server: &str, share_url: &str) -> Option<String> {
    if share_url.starts_with("http://") || share_url.starts_with("https://") {
        return crate::settings_state::validate_server_url(share_url).ok();
    }
    let Some(token) = share_url.strip_prefix("/s/") else {
        return None;
    };
    if token.is_empty()
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return None;
    }
    let server = crate::settings_state::validate_server_url(server).ok()?;
    Some(format!("{}{share_url}", server.trim_end_matches('/')))
}

fn share_permission_summary(permissions: &SharePermissions) -> String {
    let mut labels = Vec::with_capacity(5);
    if permissions.view {
        labels.push("view");
    }
    if permissions.download {
        labels.push("download");
    }
    if permissions.upload {
        labels.push("upload");
    }
    if permissions.delete {
        labels.push("delete");
    }
    if permissions.manage {
        labels.push("full access");
    }
    labels.join(", ")
}
