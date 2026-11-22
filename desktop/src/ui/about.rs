//! Settings → About: the logo with the app's name and version beside it; when
//! and where it was installed; then the project on GitHub with Star, Sponsor
//! and Report an issue.

use gpui::{div, img, prelude::*, px, Context};

use super::TextScale;
use crate::components::{Button, ButtonSize, ButtonVariant};
use crate::format::full_date;
use crate::theme::*;
use crate::workspace::about::{version_label, ISSUES, LOGO_SIZE, REPOSITORY, SPONSOR};
use crate::workspace::{Load, Workspace};

fn value(text: impl Into<gpui::SharedString>) -> gpui::Div {
    div().text_13().text_color(c(TEXT_PRIMARY)).child(text.into())
}

fn link(id: &'static str, label: &'static str, glyph: &'static str, variant: ButtonVariant, url: &'static str) -> Button {
    Button::new(id).label(label).icon(glyph).variant(variant).size(ButtonSize::Sm).on_click(move |_, _, cx| cx.open_url(url))
}

/// The page's column: wide enough for the longest fact, centred in the panel.
const COLUMN: f32 = 420.;

/// A fact: the label muted at the left, the value at the right.
fn fact(label: &'static str, text: impl Into<gpui::SharedString>, last: bool) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.))
        .px(px(14.))
        .py(px(9.))
        .when(!last, |row| row.border_b_1().border_color(c(BORDER_SECONDARY)))
        .child(div().flex_none().text_13().text_color(c(TEXT_SECONDARY)).child(label))
        .child(value(text).flex_1().min_w_0().text_right().truncate())
}

impl Workspace {
    pub(crate) fn render_about(&self, _: &mut Context<Self>) -> impl IntoElement {
        let (logo, installed, system) = match &self.about {
            Some(Load::Loaded(info)) => (
                info.logo.clone(),
                info.installed.map_or_else(|| "Unknown".to_string(), |millis| full_date(Some(millis))),
                info.system.clone().unwrap_or_else(|| "Linux".into()),
            ),
            _ => (None, "…".to_string(), "…".to_string()),
        };
        let accent = self.accent_hex();
        // The logo at the left and, on the same line, the name with its version
        // badge over what it is.
        let badge = div()
            .flex_none()
            .px(px(8.))
            .py(px(2.))
            .rounded(px(999.))
            .bg(alpha(accent, 0.14))
            .border_1()
            .border_color(alpha(accent, 0.3))
            .text_xs_tw()
            .text_color(c(TEXT_PRIMARY))
            .child(version_label(env!("BOXBOX_VERSION")));
        let header = div()
            .w_full()
            .flex()
            .items_center()
            .gap(px(16.))
            .child(div().size(px(LOGO_SIZE)).flex_none().when_some(logo, |slot, logo| slot.child(img(logo).size_full())))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .gap(px(4.))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(8.))
                            .child(div().text_size(px(18.)).font_weight(gpui::FontWeight::SEMIBOLD).text_color(c(TEXT_PRIMARY)).child("BoxBox Files"))
                            .child(badge),
                    )
                    .child(div().text_13().text_color(c(TEXT_SECONDARY)).child("A native file manager for Linux")),
            );
        let facts = div()
            .w_full()
            .rounded(px(8.))
            .border_1()
            .border_color(c(BORDER_SECONDARY))
            .bg(c(SURFACE_SECONDARY))
            .overflow_hidden()
            .child(fact("Installed", installed, false))
            .child(fact("System", system, true));
        let project = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(10.))
            .child(div().text_xs_tw().text_color(c(TEXT_SECONDARY)).text_center().child("Free and open source. A star or a sponsorship keeps it going."))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_center()
                    .gap(px(8.))
                    .child(link("about-star", "Star on GitHub", "star", ButtonVariant::Secondary, REPOSITORY))
                    .child(link("about-sponsor", "Sponsor", "heart", ButtonVariant::Primary, SPONSOR))
                    .child(link("about-issues", "Report an issue", "external-link", ButtonVariant::Secondary, ISSUES)),
            );
        // The project's links sit at the foot of the page.
        div().size_full().flex().justify_center().py(px(24.)).child(
            div().w(px(COLUMN)).max_w_full().h_full().flex().flex_col().items_center().gap(px(20.)).child(header).child(facts).child(project.mt_auto()),
        )
    }
}
