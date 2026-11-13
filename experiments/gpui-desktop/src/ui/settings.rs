//! Settings modal for the GPUI desktop experiment.

use gpui::{
    div, prelude::*, px, AnyElement, App, ClipboardItem, Context, MouseButton, PathPromptOptions, Window,
};

use super::TextScale;
use crate::components::{
    InputEvent, TextField,
    settings_rows, Button, ButtonSize, ButtonVariant, Input, Modal, ModalSize, SettingsRow, SettingsTab, Toggle,
};
use crate::pane::ViewMode;
use crate::sort::{SortDir, SortField};
use crate::api::{Api, SharePermissions, ShareSummary};
use crate::settings_state::{
    validate_connection, AccentColor, DesktopSettings, SavedConnection, SettingsDialog, ShareExpiry,
    ShareListState, WallpaperFit,
};
use crate::theme::*;
use crate::workspace::{Preferences, SettingsSection, Workspace};

/// Which settings text field a dialog input uses.
#[derive(Clone, Copy)]
pub(crate) enum SettingsField {
    /// A connection's name, a share path or an upload limit.
    First,
    /// A connection's URL.
    Second,
    /// A bearer token, masked.
    Token,
}

#[derive(Clone, Copy)]
enum SharePermissionField {
    Upload,
    Delete,
    Manage,
}

impl Workspace {
    fn confirm_reset_preferences(&mut self, cx: &mut Context<Self>) {
        let confirm = crate::workspace::confirm::Confirm::new(
            "Reset all preferences?",
            "This removes your favorites and saved local wallpaper. Saved servers are kept.",
            "Reset",
            |this, cx| {
                this.preferences = Preferences::default();
                this.commit_preferences(cx);
                let connections = std::mem::take(&mut this.desktop_settings.connections);
                this.desktop_settings = DesktopSettings { connections, ..DesktopSettings::default() };
                this.favorites.entries.clear();
                this.save_favorites(None, cx);
                this.reload_wallpaper(cx);
                this.save_desktop_settings(cx);
            },
        );
        self.ask(confirm, cx);
    }

    fn settings_tab(&self, id: &'static str, label: &'static str, section: SettingsSection, cx: &mut Context<Self>) -> SettingsTab {
        SettingsTab::new(id, label, self.selection_color(false)).active(self.settings_section == section).on_click(cx.listener(
            move |this, _, _, cx| {
                this.select_settings_section(section, cx);
                if section == SettingsSection::Links {
                    this.load_shares(cx);
                }
            },
        ))
    }

    /// A toggle for one boolean preference.
    fn preference_toggle(&self, id: &'static str, checked: bool, field: fn(&mut Preferences) -> &mut bool, cx: &mut Context<Self>) -> Toggle {
        let workspace = cx.entity().downgrade();
        Toggle::new(id, checked).on_change(move |value, _, _, cx| {
            workspace
                .update(cx, |this, cx| {
                    *field(&mut this.preferences) = value;
                    this.commit_preferences(cx);
                })
                .ok();
        })
    }

    /// Applies the accent field as it is typed, once it holds a colour.
    pub(crate) fn on_accent_field(&mut self, event: &InputEvent, cx: &mut Context<Self>) {
        if !matches!(event, InputEvent::Changed | InputEvent::Submit) {
            return;
        }
        let text = self.accent_field.read(cx).text().to_string();
        match parse_hex_color(&text) {
            Some(color) => {
                self.accent_invalid = false;
                self.desktop_settings.custom_accent = Some(color);
                self.desktop_settings.accent_from_wallpaper = false;
                self.save_desktop_settings(cx);
            }
            None => self.accent_invalid = !text.trim().is_empty(),
        }
        cx.notify();
    }

    fn render_general_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = &self.preferences;
        settings_rows(vec![
            SettingsRow::new("Show hidden files")
                .control(self.preference_toggle("setting-hidden", p.show_hidden_files, |p| &mut p.show_hidden_files, cx)),
            SettingsRow::new("Show file extensions")
                .control(self.preference_toggle("setting-extensions", p.show_file_extensions, |p| &mut p.show_file_extensions, cx)),
            SettingsRow::new("Confirm before delete")
                .control(self.preference_toggle("setting-confirm-delete", p.confirm_delete, |p| &mut p.confirm_delete, cx)),
            SettingsRow::new("Preview on single click")
                .control(self.preference_toggle("setting-preview", p.preview_on_single_click, |p| &mut p.preview_on_single_click, cx)),
            SettingsRow::new("Selection checkboxes")
                .control(self.preference_toggle("setting-checkboxes", p.selection_checkboxes, |p| &mut p.selection_checkboxes, cx)),
            // Selects are w-40.
            SettingsRow::new("Default sort").control(self.select("setting-sort", p.default_sort, &SortField::OPTIONS, 160., cx, |this, value, cx| {
                this.preferences.default_sort = value;
                this.commit_preferences(cx);
            })),
            SettingsRow::new("Default sort direction").control(self.select(
                "setting-sort-direction",
                p.default_sort_direction,
                &SortDir::OPTIONS,
                160.,
                cx,
                |this, value, cx| {
                    this.preferences.default_sort_direction = value;
                    this.commit_preferences(cx);
                },
            )),
            SettingsRow::new("Default view").control(self.select("setting-view", p.default_view, &ViewMode::OPTIONS, 160., cx, |this, value, cx| {
                this.preferences.default_view = value;
                this.commit_preferences(cx);
            })),
        ])
    }

    fn render_appearance_settings(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        const ACCENT_SOURCES: [(bool, &str); 2] = [(true, "From wallpaper"), (false, "Custom")];
        let preferences = &self.preferences;
        let settings = &self.desktop_settings;
        let has_wallpaper = settings.wallpaper.is_some();
        let wallpaper_label =
            settings.wallpaper.as_ref().and_then(|path| path.file_name()).map(|name| name.to_string_lossy().into_owned());
        let mut rows = vec![SettingsRow::new("Accent color").control(self.select(
            "accent-source",
            settings.accent_from_wallpaper,
            &ACCENT_SOURCES,
            176.,
            cx,
            |this, from_wallpaper, cx| {
                this.desktop_settings.accent_from_wallpaper = from_wallpaper;
                this.save_desktop_settings(cx);
            },
        ))];
        let accent_note = settings.accent_from_wallpaper.then(|| match &self.wallpaper {
            _ if !has_wallpaper => Some("Choose a wallpaper first".to_string()),
            crate::workspace::appearance::WallpaperState::Ready { accent: Some(_), .. } => None,
            crate::workspace::appearance::WallpaperState::Ready { accent: None, .. } => Some(format!(
                "No usable color; using {}",
                if settings.custom_accent.is_some() { "your custom color" } else { "default blue" }
            )),
            _ => Some("Matching wallpaper…".to_string()),
        }).flatten();
        if !settings.accent_from_wallpaper {
            let field = self.accent_field.clone();
            let focused = field.read(cx).is_focused(window);
            rows.push(SettingsRow::new("Custom color").control(
                // A swatch, the hex field (w-24) and Default.
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().size(px(20.)).rounded(px(4.)).border_1().border_color(c(BORDER_PRIMARY)).bg(c(self.accent_hex())))
                    .child(
                        div()
                            .id("accent-color-input")
                            .debug_selector(|| "accent-color-input".into())
                            .w(px(96.))
                            .child(Input::new("").placeholder("#4a9eff").focused(focused).invalid(self.accent_invalid).editor(field)),
                    )
                    .child(
                        Button::new("accent-default")
                            .label("Default")
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .disabled(settings.custom_accent.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.desktop_settings.custom_accent = None;
                                this.desktop_settings.accent = AccentColor::default();
                                this.accent_field.update(cx, |field, cx| field.set_text("", 0..0, cx));
                                this.accent_invalid = false;
                                this.save_desktop_settings(cx);
                            })),
                    ),
            ));
        }
        rows.push(SettingsRow::new("Compact rows").control(self.preference_toggle(
            "setting-compact",
            preferences.compact_rows,
            |p| &mut p.compact_rows,
            cx,
        )));
        rows.push(SettingsRow::new("Wallpaper").control(
            div()
                .flex()
                .min_w_0()
                .items_center()
                .gap(px(8.))
                .when_some(wallpaper_label, |row, label| {
                    row.child(div().max_w(px(144.)).truncate().text_xs_tw().text_color(c(TEXT_SECONDARY)).child(label))
                })
                .child(
                    Button::new("wallpaper-choose")
                        .label(if has_wallpaper { "Change" } else { "Choose" })
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Sm)
                        .on_click(cx.listener(|this, _, _, cx| this.choose_wallpaper(cx))),
                )
                .when(has_wallpaper, |row| {
                    row.child(
                        Button::new("wallpaper-remove")
                            .label("Remove")
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Sm)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.desktop_settings.wallpaper = None;
                                this.reload_wallpaper(cx);
                                this.save_desktop_settings(cx);
                            })),
                    )
                }),
        ));
        if has_wallpaper {
            rows.push(SettingsRow::new("Wallpaper fit").control(self.select(
                "setting-wallpaper-fit",
                settings.wallpaper_fit,
                &WallpaperFit::OPTIONS,
                160.,
                cx,
                |this, fit, cx| {
                    this.desktop_settings.wallpaper_fit = fit;
                    this.save_desktop_settings(cx);
                },
            )));
            rows.push(SettingsRow::new("Tint components from wallpaper").control(self.preference_toggle(
                "setting-tint",
                preferences.wallpaper_tint,
                |p| &mut p.wallpaper_tint,
                cx,
            )));
            rows.push(SettingsRow::new("Frosted glass").control(self.preference_toggle(
                "setting-frost",
                preferences.frosted_glass,
                |p| &mut p.frosted_glass,
                cx,
            )));
        }
        let failure = match &self.wallpaper {
            crate::workspace::appearance::WallpaperState::Failed(error) => Some(error.clone()),
            _ => None,
        };
        div()
            .flex()
            .flex_col()
            .child(settings_rows(rows.drain(..1).collect()))
            // The wallpaper accent's state sits under its select, as on the web.
            .when_some(accent_note, |panel, note| panel.child(div().py(px(8.)).text_xs_tw().text_color(c(TEXT_SECONDARY)).child(note)))
            .child(div().border_t_1().border_color(c(BORDER_SECONDARY)).child(settings_rows(rows)))
            .when_some(failure, |panel, error| panel.child(div().py(px(8.)).text_xs_tw().text_color(c(DANGER)).child(error)))
    }

    fn settings_button(
        &self,
        id: String,
        label: String,
        enabled: bool,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut Workspace, &mut Context<Workspace>) + 'static,
    ) -> impl IntoElement {
        let selector = id.clone();
        div().debug_selector(move || selector).child(
            Button::new(gpui::SharedString::from(id))
                .label(label)
                .variant(ButtonVariant::Secondary)
                .size(ButtonSize::Sm)
                .disabled(!enabled)
                .on_click(cx.listener(move |this, _, _, cx| on_click(this, cx))),
        )
    }

    pub(crate) fn settings_field(&self, field: SettingsField) -> &gpui::Entity<TextField> {
        match field {
            SettingsField::First => &self.settings_fields[0],
            SettingsField::Second => &self.settings_fields[1],
            SettingsField::Token => &self.settings_fields[2],
        }
    }

    /// Opens a settings dialog with its fields' starting text, focusing the first.
    fn open_settings_dialog(&mut self, dialog: SettingsDialog, texts: &[(SettingsField, String)], cx: &mut Context<Self>) {
        self.settings_panel.error = None;
        self.settings_panel.dialog = Some(dialog);
        for (field, text) in texts {
            let (text, end) = (text.clone(), text.len());
            self.settings_field(*field).clone().update(cx, |field, cx| field.set_text(text, end..end, cx));
        }
        self.pending_field_focus = texts.first().map(|(field, _)| *field);
        cx.notify();
    }

    /// The new-share dialog, starting from `path`.
    pub(crate) fn open_create_share(&mut self, path: String, cx: &mut Context<Self>) {
        let permissions = SharePermissions { view: true, download: true, ..Default::default() };
        self.open_settings_dialog(
            SettingsDialog::CreateShare { permissions, expires: ShareExpiry::default() },
            &[(SettingsField::First, path)],
            cx,
        );
    }

    /// A labelled field (`text-xs` label over the input).
    fn settings_text_field(&self, id: &'static str, label: &'static str, field: SettingsField, window: &Window, cx: &App) -> impl IntoElement {
        let entity = self.settings_field(field).clone();
        let focused = entity.read(cx).is_focused(window);
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(div().text_xs_tw().text_color(c(TEXT_SECONDARY)).child(label))
            .child(div().id(id).debug_selector(move || id.to_string()).child(Input::new("").focused(focused).editor(entity)))
    }

    /// Enter submits and Escape cancels the open settings dialog.
    pub(crate) fn on_settings_field(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Submit => self.submit_settings_dialog(cx),
            InputEvent::Cancel => self.cancel_settings_dialog(window, cx),
            InputEvent::Changed => {}
        }
    }

    fn cancel_settings_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_panel.dialog = None;
        self.settings_panel.error = None;
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub(crate) fn save_desktop_settings(&mut self, cx: &mut Context<Self>) {
        match self.desktop_settings.encode() {
            Ok(bytes) => self.persist(&crate::persist::DESKTOP_SETTINGS, bytes, cx, |this, result, _| match result {
                Ok(()) => {
                    this.desktop_settings.load_error = None;
                    this.settings_panel.error = None;
                }
                Err(error) => this.settings_panel.error = Some(format!("Could not save desktop settings: {error}")),
            }),
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
                            this.open_settings_dialog(
                                SettingsDialog::EditConnection { index: None },
                                &[(SettingsField::First, String::new()), (SettingsField::Second, String::new())],
                                cx,
                            );
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
        let texts = [(SettingsField::First, connection.name.clone()), (SettingsField::Second, connection.url.clone())];
        self.open_settings_dialog(SettingsDialog::EditConnection { index: Some(index) }, &texts, cx);
    }

    fn connect_saved_connection(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(connection) = self.desktop_settings.connections.get(index).cloned() else {
            return;
        };
        self.open_settings_dialog(SettingsDialog::ConnectSaved { connection }, &[(SettingsField::Token, String::new())], cx);
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

    fn save_saved_connection(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        let name = self.settings_field(SettingsField::First).read(cx).text().to_string();
        let url = self.settings_field(SettingsField::Second).read(cx).text().to_string();
        let connection = match validate_connection(&name, &url) {
            Ok(connection) => connection,
            Err(error) => {
                self.settings_panel.error = Some(error);
                self.settings_panel.dialog = Some(SettingsDialog::EditConnection { index });
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
            self.settings_panel.dialog = Some(SettingsDialog::EditConnection { index });
            cx.notify();
            return;
        }

        match index {
            Some(index) => {
                let Some(existing) = self.desktop_settings.connections.get(index) else {
                    self.settings_panel.error =
                        Some("That saved connection no longer exists.".into());
                    self.settings_panel.dialog = Some(SettingsDialog::EditConnection { index: Some(index) });
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
            Some(SettingsDialog::EditConnection { .. }) => {
                if let Some(SettingsDialog::EditConnection { index }) = self.settings_panel.dialog.take() {
                    self.save_saved_connection(index, cx);
                }
            }
            Some(SettingsDialog::ConnectSaved { .. }) => {
                if let Some(SettingsDialog::ConnectSaved { connection }) = self.settings_panel.dialog.take() {
                    let token = self.settings_field(SettingsField::Token).read(cx).text().to_string();
                    // The token is never kept once used.
                    self.settings_field(SettingsField::Token).clone().update(cx, |field, cx| field.set_text("", 0..0, cx));
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
                                |this, cx| this.open_create_share(String::new(), cx),
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
        self.open_settings_dialog(
            SettingsDialog::EditShare { id: id.to_string(), label: label.to_string(), permissions },
            &[(SettingsField::First, max_upload_bytes.to_string())],
            cx,
        );
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
        cx.notify();
    }

    fn share_permission_row(
        &self,
        id: &'static str,
        label: &'static str,
        checked: bool,
        field: SharePermissionField,
        cx: &mut Context<Self>,
    ) -> SettingsRow {
        let workspace = cx.entity().downgrade();
        SettingsRow::new(label).control(Toggle::new(id, checked).on_change(move |_, _, _, cx| {
            workspace.update(cx, |this, cx| this.toggle_share_permission(field, cx)).ok();
        }))
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
        let Some(SettingsDialog::CreateShare { mut permissions, expires }) = self.settings_panel.dialog.take() else {
            return;
        };
        let path = self.settings_field(SettingsField::First).read(cx).text().trim().to_string();
        if path.is_empty() || path.contains('\0') {
            self.settings_panel.error = Some("Enter a valid file or folder path.".into());
            self.settings_panel.dialog = Some(SettingsDialog::CreateShare { permissions, expires });
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
            self.settings_panel.dialog = Some(SettingsDialog::CreateShare { permissions, expires });
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
        let Some(SettingsDialog::EditShare { id, label, mut permissions }) = self.settings_panel.dialog.take() else {
            return;
        };
        let max_upload_bytes = self.settings_field(SettingsField::First).read(cx).text().to_string();
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
                    self.settings_panel.dialog = Some(SettingsDialog::EditShare { id, label, permissions });
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };
        let Some((api, server, request)) = self.start_share_mutation() else {
            self.settings_panel.dialog = Some(SettingsDialog::EditShare { id, label, permissions });
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
    fn render_settings_dialog(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(dialog) = &self.settings_panel.dialog else {
            return div().into_any_element();
        };
        let busy = self.settings_panel.busy;
        let hint = |text: &'static str| div().text_xs_tw().text_color(c(TEXT_MUTED)).child(text);
        let confirm = |id: &'static str, label: &'static str, enabled: bool, variant: ButtonVariant, cx: &mut Context<Self>| {
            Button::new(id)
                .label(label)
                .variant(variant)
                .disabled(!enabled)
                .on_click(cx.listener(|this, _, _, cx| this.submit_settings_dialog(cx)))
                .into_any_element()
        };
        let (title, body, confirm): (&'static str, AnyElement, AnyElement) = match dialog {
            SettingsDialog::EditConnection { index } => (
                if index.is_some() { "Edit server connection" } else { "Add server connection" },
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(self.settings_text_field("settings-connection-name", "Name", SettingsField::First, window, cx))
                    .child(self.settings_text_field("settings-connection-url", "Server URL", SettingsField::Second, window, cx))
                    .child(hint("Use http:// or https://. Credentials, query strings, and fragments are not allowed."))
                    .into_any_element(),
                confirm("settings-connection-save", "Save", true, ButtonVariant::Primary, cx),
            ),
            SettingsDialog::ConnectSaved { connection } => (
                "Open server connection",
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(div().text_13().text_color(c(TEXT_PRIMARY)).child(connection.name.clone()))
                    .child(div().text_xs_tw().text_color(c(TEXT_MUTED)).child(connection.url.clone()))
                    .child(self.settings_text_field("settings-connection-token", "Bearer token (optional)", SettingsField::Token, window, cx))
                    .into_any_element(),
                confirm("settings-connection-open", "Open", true, ButtonVariant::Primary, cx),
            ),
            SettingsDialog::RemoveConnection { name, .. } => (
                "Remove server connection",
                div()
                    .text_13()
                    .text_color(c(TEXT_SECONDARY))
                    .child(format!("Remove the saved connection “{name}”? Its open tabs are not affected."))
                    .into_any_element(),
                confirm("settings-connection-confirm-remove", "Remove", true, ButtonVariant::Danger, cx),
            ),
            SettingsDialog::CreateShare { permissions, expires } => (
                "Create shared link",
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(self.settings_text_field("settings-share-path", "File or folder path", SettingsField::First, window, cx))
                    .child(hint("View and download are always included. Upload permissions apply to folders."))
                    .child(settings_rows(vec![
                        self.share_permission_row("share-create-upload", "Allow uploads", permissions.upload, SharePermissionField::Upload, cx),
                        self.share_permission_row(
                            "share-create-delete",
                            "Allow deleting uploaded files",
                            permissions.delete,
                            SharePermissionField::Delete,
                            cx,
                        ),
                        self.share_permission_row("share-create-manage", "Full access", permissions.manage, SharePermissionField::Manage, cx),
                        SettingsRow::new("Link expiry").control(self.select(
                            "share-create-expiry",
                            *expires,
                            &ShareExpiry::OPTIONS,
                            160.,
                            cx,
                            |this, value, cx| {
                                if let Some(SettingsDialog::CreateShare { expires, .. }) = &mut this.settings_panel.dialog {
                                    *expires = value;
                                }
                                cx.notify();
                            },
                        )),
                    ]))
                    .into_any_element(),
                confirm("settings-share-create-confirm", "Create link", !busy, ButtonVariant::Primary, cx),
            ),
            SettingsDialog::EditShare { label, permissions, .. } => (
                "Edit shared-link permissions",
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(div().text_13().text_color(c(TEXT_PRIMARY)).child(label.clone()))
                    .child(hint("View and download are always included. Expiry is fixed after creation."))
                    .child(settings_rows(vec![
                        self.share_permission_row("share-edit-upload", "Allow uploads", permissions.upload, SharePermissionField::Upload, cx),
                        self.share_permission_row(
                            "share-edit-delete",
                            "Allow deleting uploaded files",
                            permissions.delete,
                            SharePermissionField::Delete,
                            cx,
                        ),
                        self.share_permission_row("share-edit-manage", "Full access", permissions.manage, SharePermissionField::Manage, cx),
                    ]))
                    .when(permissions.upload, |body| {
                        body.child(self.settings_text_field(
                            "settings-share-upload-limit",
                            "Maximum upload per file (bytes)",
                            SettingsField::First,
                            window,
                            cx,
                        ))
                        .child(hint("Leave blank to keep the server's current limit."))
                    })
                    .into_any_element(),
                confirm("settings-share-save-permissions", "Save permissions", !busy, ButtonVariant::Primary, cx),
            ),
            SettingsDialog::RevokeShare { label, .. } => (
                "Revoke shared link",
                div()
                    .text_13()
                    .text_color(c(TEXT_SECONDARY))
                    .child(format!("Revoke the link for “{label}”? Anyone using it will lose access."))
                    .into_any_element(),
                confirm("settings-share-revoke-confirm", "Revoke link", !busy, ButtonVariant::Danger, cx),
            ),
        };
        div()
            .id("settings-dialog-overlay")
            .absolute()
            .inset_0()
            .key_context("SettingsDialog")
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Modal::new()
                    .title(title)
                    .on_close(cx.listener(|this, _, window, cx| this.cancel_settings_dialog(window, cx)))
                    .body(div().flex().flex_col().gap(px(10.)).child(body).child(self.render_settings_notices()))
                    .footer(
                        Button::new("settings-dialog-cancel")
                            .label("Cancel")
                            .variant(ButtonVariant::Secondary)
                            .on_click(cx.listener(|this, _, window, cx| this.cancel_settings_dialog(window, cx))),
                    )
                    .footer(confirm),
            )
            .into_any_element()
    }

    /// `SettingsModal.svelte`: a 26rem body with the section nav on the left.
    pub(crate) fn render_settings(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content: AnyElement = match self.settings_section {
            SettingsSection::General => self.render_general_settings(cx).into_any_element(),
            SettingsSection::Appearance => self.render_appearance_settings(window, cx).into_any_element(),
            SettingsSection::Links => self.render_share_management(cx).into_any_element(),
            SettingsSection::Connections => self.render_connections(cx).into_any_element(),
        };
        // nav: w-40 flex-col gap-0.5 border-r p-2, with Reset to defaults at the foot
        let nav = div()
            .w(px(160.))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(2.))
            .border_r_1()
            .border_color(c(BORDER_SECONDARY))
            .p(px(8.))
            .child(self.settings_tab("settings-general", "General", SettingsSection::General, cx))
            .child(self.settings_tab("settings-appearance", "Appearance", SettingsSection::Appearance, cx))
            .child(self.settings_tab("settings-links", "Shared links", SettingsSection::Links, cx))
            .child(self.settings_tab("settings-connections", "Connections", SettingsSection::Connections, cx))
            .child(div().flex_1())
            .child(
                SettingsTab::new("settings-reset-button", "Reset to defaults", self.selection_color(false))
                    .quiet()
                    .on_click(cx.listener(|this, _, _, cx| this.confirm_reset_preferences(cx))),
            );
        let body = div()
            .flex()
            .h(px(416.))
            .child(nav)
            .child(
                // flex-1 overflow-y-auto px-5 py-2
                div()
                    .id("settings-content")
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(20.))
                    .py(px(8.))
                    .child(content)
                    .child(self.render_settings_notices()),
            );
        div()
            .id("settings-overlay")
            .when(self.settings_panel.dialog.is_none(), |panel| panel.track_focus(&self.settings_focus).key_context("SettingsPanel"))
            .absolute()
            .inset_0()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Modal::new()
                    .title("Settings")
                    .size(ModalSize::Xl)
                    .padded(false)
                    .close_selector("close-settings")
                    .on_close(cx.listener(|this, _, window, cx| this.close_settings(&crate::workspace::CloseSettings, window, cx)))
                    .body(body),
            )
            .child(self.render_settings_dialog(window, cx))
    }
}

/// `#4a9eff` or `4a9eff`.
fn parse_hex_color(text: &str) -> Option<u32> {
    let hex = text.trim();
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    (hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())).then(|| u32::from_str_radix(hex, 16).ok()).flatten()
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
