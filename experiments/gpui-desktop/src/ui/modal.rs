//! `ui/Modal.svelte` and `ui/Button.svelte`.

use gpui::{div, prelude::*, px, AnyElement, App, ClickEvent, ElementId, FontWeight, Window};

use super::{icon, TextScale};
use crate::theme::*;

#[derive(Clone, Copy)]
pub enum ButtonKind {
    Primary,
    Secondary,
    Danger,
}

pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<gpui::SharedString>,
    kind: ButtonKind,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let (bg, hover, text, border) = match kind {
        ButtonKind::Primary => (ACCENT_SOLID, ACCENT_SOLID_HOVER, WHITE, None),
        ButtonKind::Danger => (DANGER, DANGER_HOVER, WHITE, None),
        ButtonKind::Secondary => (
            SURFACE_SECONDARY,
            SURFACE_TERTIARY,
            TEXT_SECONDARY,
            Some(BORDER_PRIMARY),
        ),
    };
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(32.))
        .px(px(12.))
        .rounded(px(4.))
        .text_13()
        .font_weight(FontWeight::MEDIUM)
        .bg(c(bg))
        .text_color(c(text))
        .when_some(border, |button, border| {
            button.border_1().border_color(c(border))
        })
        .cursor_pointer()
        .hover(move |style| style.bg(c(hover)))
        .child(label.into())
        .on_click(on_click)
}

/// A centred dialog over a dimmed backdrop; clicking the backdrop or × closes it.
pub fn modal(
    title: impl Into<gpui::SharedString>,
    width: f32,
    body: AnyElement,
    footer: Vec<AnyElement>,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let on_close = std::rc::Rc::new(on_close);
    let close_backdrop = on_close.clone();
    div()
        .id("modal-backdrop")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(alpha(0x000000, 0.55))
        .occlude()
        .on_click(move |_, window, cx| close_backdrop(window, cx))
        .child(
            div()
                .id("modal")
                .mx(px(16.))
                .w(px(width))
                .max_w_full()
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(8.))
                .border_1()
                .border_color(c(BORDER_PRIMARY))
                .bg(c(SURFACE_SECONDARY))
                .shadow_2xl()
                // Clicks inside the dialog must not reach the backdrop.
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .h(px(44.))
                        .items_center()
                        .justify_between()
                        .gap(px(12.))
                        .pl(px(16.))
                        .pr(px(8.))
                        .border_b_1()
                        .border_color(c(BORDER_SECONDARY))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_sm_tw()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(c(TEXT_PRIMARY))
                                .child(title.into()),
                        )
                        .child(
                            div()
                                .id("modal-close")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(4.))
                                .cursor_pointer()
                                .hover(|style| style.bg(c(SURFACE_ELEVATED)))
                                .child(icon("x", 16., c(TEXT_SECONDARY)))
                                .on_click(move |_, window, cx| on_close(window, cx)),
                        ),
                )
                .child(div().p(px(16.)).child(body))
                .when(!footer.is_empty(), |dialog| {
                    dialog.child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.))
                            .px(px(16.))
                            .py(px(12.))
                            .border_t_1()
                            .border_color(c(BORDER_SECONDARY))
                            .children(footer),
                    )
                }),
        )
}

/// `ui/Input.svelte` frame around a text input entity.
pub fn input_frame(input: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .h(px(32.))
        .w_full()
        .px(px(12.))
        .rounded(px(4.))
        .border_1()
        .border_color(c(BORDER_PRIMARY))
        .bg(c(SURFACE_PRIMARY))
        .text_13()
        .child(input)
}
