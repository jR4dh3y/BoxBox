//! The hover hint for a control: the web app's `title`, drawn in the app's colours.

use gpui::{div, prelude::*, px, AnyView, App, Context, Global, SharedString, Window};

use super::{Type, ROUNDED};
use crate::theme::*;

/// The app's sans family, for views drawn outside the workspace's element tree.
pub struct UiFont(pub SharedString);

impl Global for UiFont {}

pub struct Tooltip {
    text: SharedString,
}

impl Tooltip {
    /// The builder `.tooltip()` takes. Line breaks in `text` are kept.
    pub fn text(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        let text = text.into();
        move |_, cx| cx.new(|_| Tooltip { text: text.clone() }).into()
    }
}

impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .when_some(cx.try_global::<UiFont>(), |el, font| el.font_family(font.0.clone()))
            .max_w(px(360.))
            .px(px(8.))
            .py(px(4.))
            .rounded(px(ROUNDED))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_ELEVATED))
            .type_xs()
            .text_color(c(TEXT_PRIMARY))
            .children(self.text.lines().map(|line| div().child(line.to_string())))
    }
}
