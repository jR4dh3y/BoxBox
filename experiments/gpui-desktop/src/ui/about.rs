//! Settings → About: the app, its version, when and where it was installed,
//! then the project on GitHub with Star, Sponsor and Report an issue.

use gpui::{div, prelude::*, px, Context};

use super::{icon, TextScale};
use crate::components::{settings_rows, Button, ButtonSize, ButtonVariant, SettingsRow};
use crate::format::full_date;
use crate::theme::*;
use crate::workspace::about::{version_label, ISSUES, REPOSITORY, SPONSOR};
use crate::workspace::{Load, Workspace};

fn value(text: impl Into<gpui::SharedString>) -> gpui::Div {
    div().text_13().text_color(c(TEXT_PRIMARY)).child(text.into())
}

fn link(id: &'static str, label: &'static str, glyph: &'static str, variant: ButtonVariant, url: &'static str) -> Button {
    Button::new(id).label(label).icon(glyph).variant(variant).size(ButtonSize::Sm).on_click(move |_, _, cx| cx.open_url(url))
}

impl Workspace {
    pub(crate) fn render_about(&self, _: &mut Context<Self>) -> impl IntoElement {
        let (installed, system) = match &self.about {
            Some(Load::Loaded(info)) => (
                info.installed.map_or_else(|| "Unknown".to_string(), |millis| full_date(Some(millis))),
                info.system.clone().unwrap_or_else(|| "Linux".into()),
            ),
            _ => ("…".to_string(), "…".to_string()),
        };
        let accent = self.accent_hex();
        let header = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .py(px(12.))
            .child(
                div()
                    .size(px(44.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(10.))
                    .bg(alpha(accent, 0.18))
                    .child(icon("folder", 24., c(accent))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(div().text_size(px(16.)).font_weight(gpui::FontWeight::SEMIBOLD).text_color(c(TEXT_PRIMARY)).child("BoxBox Files"))
                    .child(div().text_xs_tw().text_color(c(TEXT_SECONDARY)).child("A native file manager for Linux")),
            );
        let banner = div()
            .mt(px(16.))
            .p(px(14.))
            .rounded(px(8.))
            .border_1()
            .border_color(c(BORDER_SECONDARY))
            .bg(c(SURFACE_SECONDARY))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().text_13().font_weight(gpui::FontWeight::MEDIUM).text_color(c(TEXT_PRIMARY)).child("BoxBox is free and open source"))
                    .child(div().text_xs_tw().text_color(c(TEXT_SECONDARY)).child("A star helps others find it; sponsoring keeps it going.")),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .child(link("about-star", "Star on GitHub", "star", ButtonVariant::Secondary, REPOSITORY))
                    .child(link("about-sponsor", "Sponsor", "heart", ButtonVariant::Primary, SPONSOR))
                    .child(link("about-issues", "Report an issue", "external-link", ButtonVariant::Ghost, ISSUES)),
            );
        div()
            .flex()
            .flex_col()
            .child(header)
            .child(div().border_t_1().border_color(c(BORDER_SECONDARY)).child(settings_rows(vec![
                SettingsRow::new("Version").control(value(version_label(env!("BOXBOX_VERSION")))),
                SettingsRow::new("Installed").control(value(installed)),
                SettingsRow::new("System").control(value(system)),
            ])))
            .child(banner)
    }
}
