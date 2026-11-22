//! Customize Folder: a preview, the folder's colour, the emblem's colour, and a
//! searchable grid of every Lucide icon to put on the folder.

use gpui::{div, prelude::*, px, uniform_list, AnyElement, Context, MouseButton, SharedString, Window};

use super::{icon, thin_icon, TextScale};
use crate::components::{emblemed, Button, ButtonVariant, Input, Modal, ModalSize, Tooltip};
use crate::emblems;
use crate::theme::*;
use crate::workspace::folder_looks::{EmblemChoice, FolderLook, COLORS};
use crate::workspace::Workspace;

const COLUMNS: usize = 13;
const CELL: f32 = 40.;
const GRID_ROWS: f32 = 6.;
const PREVIEW: f32 = 88.;

/// What folders are usually for, first in the grid before every other icon.
const COMMON: [&str; 39] = [
    "code", "terminal", "gamepad-2", "briefcase", "book", "graduation-cap", "file-text", "download", "image", "camera",
    "music", "headphones", "video", "film", "house", "monitor", "layout-template", "share-2", "cloud", "globe", "mail",
    "users", "heart", "star", "sparkles", "bot", "cpu", "database", "package", "box", "archive", "wrench", "flask-conical",
    "palette", "wallet", "lock", "git-branch", "server", "notebook-pen",
];

/// A cell of the icon grid.
#[derive(Clone, Copy, PartialEq)]
enum Choice {
    Without,
    /// The special folder's own emblem.
    Own(&'static str),
    Icon(&'static str),
}

impl Choice {
    fn of(look: &FolderLook, own: Option<&'static str>) -> Self {
        match (&look.emblem, own) {
            (EmblemChoice::Own, Some(own)) => Self::Own(own),
            (EmblemChoice::Own, None) | (EmblemChoice::Without, _) => Self::Without,
            (EmblemChoice::Icon(name), _) => emblems::find(name).map_or(Self::Without, |icon| Self::Icon(icon.name)),
        }
    }

    fn emblem(self) -> EmblemChoice {
        match self {
            Self::Without => EmblemChoice::Without,
            Self::Own(_) => EmblemChoice::Own,
            Self::Icon(name) => EmblemChoice::Icon(name.to_string()),
        }
    }

    fn label(self) -> String {
        match self {
            Self::Without => "No icon".into(),
            Self::Own(name) => format!("Default ({name})"),
            Self::Icon(name) => name.into(),
        }
    }
}

impl Workspace {
    /// With no search: no icon, the folder's own when it has one, the common
    /// ones, then every other. With one, the icons that match.
    fn customize_choices(&self, cx: &gpui::App) -> Vec<Choice> {
        let Some(dialog) = &self.customize else { return Vec::new() };
        let query = self.customize_search.read(cx).text().to_string();
        if !query.trim().is_empty() {
            return emblems::search(&query).into_iter().map(|icon| Choice::Icon(icon.name)).collect();
        }
        let common: Vec<&'static str> = COMMON.iter().filter_map(|name| emblems::find(name)).map(|icon| icon.name).collect();
        let mut choices = vec![Choice::Without];
        choices.extend(dialog.own.map(Choice::Own));
        choices.extend(common.iter().map(|&name| Choice::Icon(name)));
        choices.extend(emblems::all().iter().filter(|icon| !common.contains(&icon.name)).map(|icon| Choice::Icon(icon.name)));
        choices
    }

    fn swatch(&self, id: SharedString, color: u32, selected: bool, label: &'static str, pick: impl Fn(&mut FolderLook) + 'static, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id(id.clone())
            .debug_selector(move || id.to_string())
            .size(px(22.))
            .flex_none()
            .rounded_full()
            .border_2()
            .border_color(if selected { c(TEXT_PRIMARY) } else { alpha(TEXT_PRIMARY, 0.) })
            .p(px(2.))
            .cursor_pointer()
            .tooltip(Tooltip::text(label))
            .child(div().size_full().rounded_full().bg(c(color)))
            .on_click(cx.listener(move |this, _, _, cx| this.change_look(&pick, cx)))
    }

    fn swatch_row(&self, title: &'static str, swatches: Vec<AnyElement>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(div().text_xs_tw().text_color(c(TEXT_MUTED)).child(title))
            .child(div().flex().flex_wrap().gap(px(4.)).children(swatches))
    }

    pub(crate) fn render_customize_folder(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(dialog) = &self.customize else { return div().into_any_element() };
        let look = dialog.look.clone();
        let default_tone = self.kind_tint(crate::format::Kind::Folder).unwrap_or(FOLDER);
        let tone = look.color.unwrap_or(default_tone);
        let emblem = Self::look_emblem(Some(&look), dialog.own);
        let preview = div()
            .size(px(PREVIEW + 24.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.))
            .bg(c(SURFACE_SECONDARY))
            .child(emblemed(thin_icon("folder", PREVIEW, c(tone)), emblem, PREVIEW, c(tone)));

        let mut folder_colors = vec![self
            .swatch("folder-color-default".into(), default_tone, look.color.is_none(), "Default", |look| look.color = None, cx)
            .into_any_element()];
        folder_colors.extend(COLORS.iter().map(|&(color, label)| {
            self.swatch(format!("folder-color-{color:06x}").into(), color, look.color == Some(color), label, move |look| look.color = Some(color), cx)
                .into_any_element()
        }));
        let mut emblem_colors = vec![self
            .swatch("emblem-color-same".into(), tone, look.emblem_color.is_none(), "Same as folder", |look| look.emblem_color = None, cx)
            .into_any_element()];
        emblem_colors.extend(COLORS.iter().map(|&(color, label)| {
            self.swatch(format!("emblem-color-{color:06x}").into(), color, look.emblem_color == Some(color), label, move |look| look.emblem_color = Some(color), cx)
                .into_any_element()
        }));

        let chosen = Choice::of(&look, dialog.own);
        let count = self.customize_choices(cx).len();
        let accent = self.accent_hex();
        let grid = uniform_list(
            "customize-icons",
            count.div_ceil(COLUMNS),
            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                let choices = this.customize_choices(cx);
                range
                    .map(|row| {
                        let cells = choices.iter().enumerate().skip(row * COLUMNS).take(COLUMNS).map(|(index, &choice)| {
                            let selected = choice == chosen;
                            let glyph: AnyElement = match choice {
                                Choice::Without => icon("x", 18., c(TEXT_MUTED)).into_any_element(),
                                Choice::Own(name) | Choice::Icon(name) => gpui::svg()
                                    .path(emblems::path(name))
                                    .size(px(20.))
                                    .text_color(c(if selected { accent } else { TEXT_SECONDARY }))
                                    .into_any_element(),
                            };
                            let id = match choice {
                                Choice::Without => "emblem-none".to_string(),
                                Choice::Own(_) => "emblem-own".to_string(),
                                Choice::Icon(name) => format!("emblem-{name}"),
                            };
                            div()
                                .id(("emblem-cell", index))
                                .debug_selector(move || id.clone())
                                .size(px(CELL))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(6.))
                                .cursor_pointer()
                                .map(|cell| match selected {
                                    true => cell.bg(alpha(accent, 0.2)).border_1().border_color(alpha(accent, 0.6)),
                                    false => cell.hover(|style| style.bg(c(SURFACE_SECONDARY))),
                                })
                                .tooltip(Tooltip::text(choice.label()))
                                .child(glyph)
                                .on_click(cx.listener(move |this, _, _, cx| this.change_look(|look| look.emblem = choice.emblem(), cx)))
                        });
                        div().flex().h(px(CELL)).children(cells).into_any_element()
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .track_scroll(self.customize_scroll.clone())
        .h(px(CELL * GRID_ROWS))
        .w(px(CELL * COLUMNS as f32));

        let search = self.customize_search.clone();
        let focused = search.read(cx).is_focused(window);
        let body = div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
                div()
                    .flex()
                    .gap(px(16.))
                    .child(preview)
                    .child(div().flex_1().min_w_0().flex().flex_col().gap(px(12.)).child(self.swatch_row("Folder colour", folder_colors)).child(self.swatch_row("Icon colour", emblem_colors))),
            )
            .child(Input::new("").focused(focused).editor(search))
            .child(
                div()
                    .flex()
                    .justify_center()
                    .rounded(px(4.))
                    .border_1()
                    .border_color(c(BORDER_SECONDARY))
                    .py(px(4.))
                    .map(|frame| match count {
                        0 => frame.child(div().h(px(CELL * GRID_ROWS)).flex().items_center().text_13().text_color(c(TEXT_SECONDARY)).child("No icon matches.")),
                        _ => frame.child(grid),
                    }),
            );
        div()
            .id("customize-overlay")
            .absolute()
            .inset_0()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Modal::new()
                    .size(ModalSize::Xl)
                    .title(format!("Customize “{}”", dialog.folder.name))
                    .on_close(cx.listener(|this, _, window, cx| this.close_customize_folder(window, cx)))
                    .body(body)
                    .footer(
                        div().mr_auto().child(
                            Button::new("customize-reset")
                                .label("Reset")
                                .variant(ButtonVariant::Secondary)
                                .disabled(look == FolderLook::default())
                                .on_click(cx.listener(|this, _, _, cx| this.change_look(|look| *look = FolderLook::default(), cx))),
                        ),
                    )
                    .footer(Button::new("customize-done").label("Done").on_click(cx.listener(|this, _, window, cx| this.save_customize_folder(window, cx)))),
            )
            .into_any_element()
    }
}
