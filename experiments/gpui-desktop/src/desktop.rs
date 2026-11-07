//! Desktop integration: open, open in terminal, Open With (`desktop/OpenWithDialog.svelte`, with
//! PR #211's app icons and "Use as default"), favorites and preference persistence.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    div, img, prelude::*, px, AnyElement, Context, Entity, Focusable, Image, ImageFormat,
    Subscription, Window,
};
use serde::Deserialize;

use crate::api::FileInfo;
use crate::dialogs::Dialog;
use crate::notifications::ToastKind;
use crate::preferences::{Favorite, Preferences};
use crate::theme::*;
use crate::ui::modal::{button, input_frame, modal, ButtonKind};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{icon, TextScale};
use crate::workspace::{Load, Workspace};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Apps {
    #[serde(default)]
    pub recommended: Vec<App>,
    #[serde(default)]
    pub other: Vec<App>,
}

pub struct OpenWith {
    pub item: FileInfo,
    pub apps: Load<Apps>,
    pub selected: Option<String>,
    pub make_default: bool,
    pub search: Entity<TextInput>,
    pub _events: Subscription,
}

/// The app the dialog preselects: the default, else the first recommended.
pub fn initial_choice(apps: &Apps) -> Option<String> {
    apps.recommended
        .iter()
        .find(|app| app.is_default)
        .or(apps.recommended.first())
        .map(|app| app.id.clone())
}

pub fn matches(app: &App, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || app.name.to_lowercase().contains(&query)
        || app.id.to_lowercase().contains(&query)
}

#[derive(Default)]
pub struct AppIcons {
    icons: HashMap<String, Option<Arc<Image>>>,
}

impl Workspace {
    pub(crate) fn load_preferences(&mut self, cx: &mut Context<Self>) {
        self.background(
            |api| api.preferences(),
            |this, result, cx| match result {
                Ok(Some(preferences)) => this.apply_preferences(preferences, cx),
                Ok(None) => {}
                Err(error) => this.toast(
                    ToastKind::Error,
                    format!("Desktop preferences could not load: {error:#}"),
                    cx,
                ),
            },
            cx,
        );
    }

    pub(crate) fn save_preferences(&mut self, cx: &mut Context<Self>) {
        let preferences = self.preferences.clone();
        self.background(
            move |api| api.save_preferences(&preferences),
            |this, result, cx| {
                if let Err(error) = result {
                    this.error(&error, cx);
                }
            },
            cx,
        );
    }

    pub(crate) fn update_preferences(
        &mut self,
        change: impl FnOnce(&mut Preferences),
        cx: &mut Context<Self>,
    ) {
        let mut next = self.preferences.clone();
        change(&mut next);
        self.apply_preferences(next, cx);
        self.save_preferences(cx);
    }

    pub(crate) fn pin(&mut self, name: String, path: String, cx: &mut Context<Self>) {
        let message = format!("{name} pinned to favorites");
        self.update_preferences(
            |prefs| {
                if !prefs.is_favorite(&path) {
                    prefs.favorite_folders.push(Favorite { name, path });
                }
            },
            cx,
        );
        self.toast(ToastKind::Success, message, cx);
    }

    pub(crate) fn unpin(&mut self, path: &str, cx: &mut Context<Self>) {
        let name = self
            .preferences
            .favorite_folders
            .iter()
            .find(|f| f.path == path)
            .map(|f| f.name.clone())
            .unwrap_or_default();
        self.update_preferences(
            |prefs| {
                prefs
                    .favorite_folders
                    .retain(|favorite| favorite.path != path)
            },
            cx,
        );
        self.toast(
            ToastKind::Success,
            format!("{name} unpinned from favorites"),
            cx,
        );
    }

    pub(crate) fn open_item(&mut self, item: FileInfo, cx: &mut Context<Self>) {
        if item.is_dir {
            self.navigate(item.path, cx);
            return;
        }
        self.background(
            move |api| api.open(&item.path),
            |this, result, cx| {
                if let Err(error) = result {
                    this.error(&error, cx);
                }
            },
            cx,
        );
    }

    pub(crate) fn open_terminal(&mut self, path: String, cx: &mut Context<Self>) {
        self.background(
            move |api| api.terminal(&path),
            |this, result, cx| {
                if let Err(error) = result {
                    this.error(&error, cx);
                }
            },
            cx,
        );
    }

    pub(crate) fn open_with_dialog(
        &mut self,
        item: FileInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let search = cx.new(|cx| TextInput::new("Search applications", cx));
        let events = cx.subscribe_in(&search, window, |this, _, event, window, cx| match event {
            InputEvent::Submit => this.confirm_open_with(cx),
            InputEvent::Cancel => this.close_dialog(window, cx),
            InputEvent::Changed => {
                // A selection hidden by the filter is deselected, so it cannot be opened.
                if let Some(Dialog::OpenWith(dialog)) = &mut this.dialog {
                    let query = dialog.search.read(cx).text().to_string();
                    if let Load::Loaded(apps) = &dialog.apps {
                        let visible = apps.recommended.iter().chain(&apps.other).any(|app| {
                            Some(&app.id) == dialog.selected.as_ref() && matches(app, &query)
                        });
                        if !visible {
                            dialog.selected = None;
                        }
                    }
                }
                cx.notify();
            }
        });
        window.focus(&search.focus_handle(cx));
        self.dialog = Some(Dialog::OpenWith(OpenWith {
            item: item.clone(),
            apps: Load::Loading,
            selected: None,
            make_default: false,
            search,
            _events: events,
        }));
        self.load_apps(item.path, cx);
    }

    fn load_apps(&mut self, path: String, cx: &mut Context<Self>) {
        self.background(
            move |api| api.apps(&path),
            |this, result, cx| {
                let mut wanted = Vec::new();
                if let Some(Dialog::OpenWith(dialog)) = &mut this.dialog {
                    dialog.make_default = false;
                    match result {
                        Ok(apps) => {
                            dialog.selected = initial_choice(&apps);
                            wanted = apps
                                .recommended
                                .iter()
                                .chain(&apps.other)
                                .filter(|app| app.icon.is_some())
                                .map(|app| app.id.clone())
                                .collect();
                            dialog.apps = Load::Loaded(apps);
                        }
                        Err(error) => dialog.apps = Load::Failed(format!("{error:#}")),
                    }
                }
                for id in wanted {
                    this.load_app_icon(id, cx);
                }
            },
            cx,
        );
    }

    fn load_app_icon(&mut self, id: String, cx: &mut Context<Self>) {
        if self.app_icons.icons.contains_key(&id) {
            return;
        }
        self.app_icons.icons.insert(id.clone(), None);
        let key = id.clone();
        self.background(
            move |api| api.app_icon(&id),
            move |this, result, _| {
                // A missing icon keeps the generic app glyph.
                if let Ok(bytes) = result {
                    this.app_icons.icons.insert(
                        key,
                        Some(Arc::new(Image::from_bytes(ImageFormat::Png, bytes))),
                    );
                }
            },
            cx,
        );
    }

    pub(crate) fn confirm_open_with(&mut self, cx: &mut Context<Self>) {
        let Some(Dialog::OpenWith(dialog)) = &self.dialog else {
            return;
        };
        let Some(app) = dialog.selected.clone() else {
            return;
        };
        let (path, make_default) = (dialog.item.path.clone(), dialog.make_default);
        self.background(
            move |api| {
                if make_default {
                    api.set_default_app(&path, &app)?;
                }
                api.open_with(&path, &app)
            },
            |this, result, cx| match result {
                Ok(()) => {
                    this.dialog = None;
                    cx.notify();
                }
                Err(error) => {
                    if let Some(Dialog::OpenWith(dialog)) = &mut this.dialog {
                        dialog.apps = Load::Failed(format!("{error:#}"));
                    }
                }
            },
            cx,
        );
    }

    pub(crate) fn render_open_with(
        &self,
        dialog: &OpenWith,
        cx: &mut Context<Self>,
        on_close: impl Fn(&mut Window, &mut gpui::App) + 'static,
    ) -> AnyElement {
        let query = dialog.search.read(cx).text().to_string();
        let body: AnyElement = match &dialog.apps {
            Load::Loading => div()
                .py(px(24.))
                .text_13()
                .text_color(c(TEXT_MUTED))
                .child("Loading applications…")
                .into_any_element(),
            Load::Failed(error) => {
                let path = dialog.item.path.clone();
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .text_13()
                    .child(div().text_color(c(DANGER)).child(error.clone()))
                    .child(button("apps-reload", "Reload", ButtonKind::Secondary, {
                        let this = cx.entity().downgrade();
                        move |_, _, cx| {
                            let path = path.clone();
                            this.update(cx, |this, cx| {
                                if let Some(Dialog::OpenWith(dialog)) = &mut this.dialog {
                                    dialog.apps = Load::Loading;
                                }
                                this.load_apps(path, cx);
                            })
                            .ok();
                        }
                    }))
                    .into_any_element()
            }
            Load::Loaded(apps) => {
                let mut section = |title: &'static str, list: &[App]| {
                    let rows: Vec<AnyElement> = list
                        .iter()
                        .filter(|app| matches(app, &query))
                        .map(|app| {
                            self.app_row(app, dialog.selected.as_deref() == Some(&app.id), cx)
                        })
                        .collect();
                    (!rows.is_empty()).then(|| {
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .px(px(4.))
                                    .pt(px(8.))
                                    .pb(px(4.))
                                    .text_11()
                                    .text_color(c(TEXT_MUTED))
                                    .child(title.to_uppercase()),
                            )
                            .children(rows)
                    })
                };
                div()
                    .id("apps")
                    .flex()
                    .flex_col()
                    .max_h(px(320.))
                    .overflow_y_scroll()
                    .children(section("Recommended", &apps.recommended))
                    .children(section("Other applications", &apps.other))
                    .into_any_element()
            }
        };
        let checked = dialog.make_default;
        let checkbox = div()
            .id("use-default")
            .flex()
            .items_center()
            .gap(px(8.))
            .mr_auto()
            .text_13()
            .text_color(c(TEXT_SECONDARY))
            .cursor_pointer()
            .child(icon(
                if checked { "square-check" } else { "square" },
                16.,
                c(if checked { ACCENT } else { TEXT_SECONDARY }),
            ))
            .child("Use as default")
            .on_click(cx.listener(|this, _, _, cx| {
                if let Some(Dialog::OpenWith(dialog)) = &mut this.dialog {
                    dialog.make_default = !dialog.make_default;
                    cx.notify();
                }
            }));
        let open = {
            let this = cx.entity().downgrade();
            button("open-with", "Open", ButtonKind::Primary, move |_, _, cx| {
                this.update(cx, |this, cx| this.confirm_open_with(cx)).ok();
            })
        };
        modal(
            format!("Open “{}” with", dialog.item.name),
            448.,
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(input_frame(dialog.search.clone()))
                .child(body)
                .into_any_element(),
            vec![checkbox.into_any_element(), open.into_any_element()],
            on_close,
        )
        .into_any_element()
    }

    fn app_row(&self, app: &App, selected: bool, cx: &mut Context<Self>) -> AnyElement {
        let id = app.id.clone();
        let glyph: AnyElement = match self.app_icons.icons.get(&app.id) {
            Some(Some(image)) => img(image.clone()).size(px(24.)).into_any_element(),
            _ => icon("app-window", 20., c(TEXT_SECONDARY)).into_any_element(),
        };
        div()
            .id(gpui::SharedString::from(format!("app-{}", app.id)))
            .flex()
            .items_center()
            .gap(px(10.))
            .h(px(32.))
            .px(px(8.))
            .rounded(px(4.))
            .cursor_pointer()
            .when(selected, |row| row.bg(c(SELECTION)))
            .when(!selected, |row| {
                row.hover(|style| style.bg(c(SURFACE_TERTIARY)))
            })
            .child(
                div()
                    .size(px(14.))
                    .rounded_full()
                    .border_1()
                    .border_color(c(if selected { ACCENT } else { TEXT_MUTED }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(selected, |radio| {
                        radio.child(div().size(px(6.)).rounded_full().bg(c(ACCENT)))
                    }),
            )
            .child(
                div()
                    .size(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(glyph),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_13()
                    .child(app.name.clone()),
            )
            .when(app.is_default, |row| {
                row.child(div().text_11().text_color(c(TEXT_MUTED)).child("Default"))
            })
            .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                if let Some(Dialog::OpenWith(dialog)) = &mut this.dialog {
                    dialog.selected = Some(id.clone());
                }
                if event.click_count() >= 2 {
                    this.confirm_open_with(cx);
                }
                cx.notify();
            }))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, default: bool) -> App {
        App {
            id: id.into(),
            name: id.to_uppercase(),
            icon: None,
            is_default: default,
        }
    }

    #[test]
    fn preselects_default_then_first_recommended() {
        let apps = Apps {
            recommended: vec![app("gedit", false), app("kate", true)],
            other: vec![],
        };
        assert_eq!(initial_choice(&apps).as_deref(), Some("kate"));
        let apps = Apps {
            recommended: vec![app("gedit", false)],
            ..apps
        };
        assert_eq!(initial_choice(&apps).as_deref(), Some("gedit"));
        assert!(
            matches(&app("org.gnome.Gedit", false), "gedit")
                && !matches(&app("kate", false), "vim")
        );
    }
}
