//! `lib/components/ui/Modal.svelte`: backdrop, panel, title bar, body and footer.

use gpui::{div, prelude::*, px, rgba, AnyElement, App, ClickEvent, FontWeight, SharedString, Window};

use super::{Type, ROUNDED, ROUNDED_LG};
use crate::theme::*;
use crate::ui::icon;

type CloseHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy)]
pub enum ModalSize {
    /// `max-w-md`
    Md,
    /// `max-w-3xl`
    Lg,
    /// `max-w-2xl`
    Xl,
}

#[derive(IntoElement)]
pub struct Modal {
    title: Option<SharedString>,
    size: ModalSize,
    padded: bool,
    body: Vec<AnyElement>,
    footer: Vec<AnyElement>,
    on_close: Option<CloseHandler>,
    close_selector: Option<&'static str>,
}

impl Modal {
    pub fn new() -> Self {
        Self { title: None, size: ModalSize::Md, padded: true, body: Vec::new(), footer: Vec::new(), on_close: None, close_selector: None }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn size(mut self, size: ModalSize) -> Self {
        self.size = size;
        self
    }

    /// Content that manages its own spacing, like a two-pane layout, sets this false.
    pub fn padded(mut self, padded: bool) -> Self {
        self.padded = padded;
        self
    }

    pub fn body(mut self, element: impl IntoElement) -> Self {
        self.body.push(element.into_any_element());
        self
    }

    pub fn footer(mut self, element: impl IntoElement) -> Self {
        self.footer.push(element.into_any_element());
        self
    }

    /// A test selector for the close button.
    pub fn close_selector(mut self, selector: &'static str) -> Self {
        self.close_selector = Some(selector);
        self
    }

    pub fn on_close(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Modal {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let max_width = match self.size {
            ModalSize::Md => 448.,
            ModalSize::Xl => 672.,
            ModalSize::Lg => 768.,
        };
        let close = div()
            .id("modal-close")
            .flex()
            .items_center()
            .justify_center()
            .size(px(28.))
            .rounded(px(ROUNDED))
            .text_color(c(TEXT_SECONDARY))
            .cursor_pointer()
            .hover(|style| style.bg(c(SURFACE_ELEVATED)).text_color(c(TEXT_PRIMARY)))
            .child(icon("x", 16., c(TEXT_SECONDARY)))
            .when_some(self.close_selector, |el, selector| el.debug_selector(move || selector.to_string()))
            .when_some(self.on_close, |el, handler| el.on_click(handler));
        // fixed inset-0 flex items-center justify-center bg-black/55
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x0000008c))
            .child(
                // mx-4 max-h-[90vh] w-full max-w-* flex-col overflow-hidden rounded-lg border
                // border-border-primary bg-surface-secondary shadow-2xl
                div()
                    .mx(px(16.))
                    .w_full()
                    .max_w(px(max_width))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded(px(ROUNDED_LG))
                    .border_1()
                    .border_color(c(BORDER_PRIMARY))
                    .bg(c(SURFACE_SECONDARY))
                    .shadow_2xl()
                    .when_some(self.title, |el, title| {
                        // h-11 items-center justify-between gap-3 border-b border-border-secondary pr-2 pl-4
                        el.child(
                            div()
                                .flex()
                                .flex_none()
                                .h(px(44.))
                                .items_center()
                                .justify_between()
                                .gap(px(12.))
                                .border_b_1()
                                .border_color(c(BORDER_SECONDARY))
                                .pr(px(8.))
                                .pl(px(16.))
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .type_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(c(TEXT_PRIMARY))
                                        .child(title),
                                )
                                .child(close),
                        )
                    })
                    .child(div().flex().flex_col().when(self.padded, |el| el.p(px(16.))).children(self.body))
                    .when(!self.footer.is_empty(), |el| {
                        // flex justify-end gap-2 border-t border-border-secondary px-4 py-3
                        el.child(
                            div()
                                .flex()
                                .justify_end()
                                .gap(px(8.))
                                .border_t_1()
                                .border_color(c(BORDER_SECONDARY))
                                .px(px(16.))
                                .py(px(12.))
                                .children(self.footer),
                        )
                    }),
            )
    }
}
