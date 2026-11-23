//! `lib/components/ui/Modal.svelte`: backdrop, panel, title bar, body and footer.

use gpui::{div, prelude::*, px, rgba, Animation, AnimationExt as _, AnyElement, App, ClickEvent, FontWeight, Window};

use super::{motion, Type, ROUNDED, ROUNDED_LG};
use crate::theme::*;
use crate::ui::icon;

type CloseHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy)]
pub enum ModalSize {
    /// A short question and its answers.
    Sm,
    /// `max-w-md`
    Md,
    /// `max-w-3xl`
    Lg,
    /// `max-w-2xl`
    Xl,
}

#[derive(IntoElement)]
pub struct Modal {
    title: Option<AnyElement>,
    /// Drawn before the title, such as the icon of the item a dialog is about.
    leading: Option<AnyElement>,
    size: ModalSize,
    padded: bool,
    body: Vec<AnyElement>,
    footer: Vec<AnyElement>,
    on_close: Option<CloseHandler>,
    close_selector: Option<&'static str>,
}

impl Modal {
    pub fn new() -> Self {
        Self { title: None, leading: None, size: ModalSize::Md, padded: true, body: Vec::new(), footer: Vec::new(), on_close: None, close_selector: None }
    }

    /// Text, or an element such as selectable text.
    pub fn title(mut self, title: impl IntoElement) -> Self {
        self.title = Some(title.into_any_element());
        self
    }

    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading = Some(element.into_any_element());
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let max_width: f32 = match self.size {
            ModalSize::Sm => 340.,
            ModalSize::Md => 448.,
            ModalSize::Xl => 672.,
            ModalSize::Lg => 768.,
        };
        let width = max_width.min(f32::from(window.viewport_size().width) - 32.).max(0.);
        // max-h-[90vh] with the mx-4 margin: a short window scrolls the body, so the
        // title bar and the footer's buttons stay in it.
        let max_height = (f32::from(window.viewport_size().height) - 32.).max(0.);
        let close = div()
            .id("modal-close")
            .flex()
            .items_center()
            .justify_center()
            .size(px(28.))
            .rounded(px(ROUNDED))
            .text_color(c(TEXT_SECONDARY))
            .cursor_pointer()
            .hover(|style| style.text_color(c(TEXT_PRIMARY)))
            .child(icon("x", 16., c(TEXT_SECONDARY)))
            .when_some(self.close_selector, |el, selector| el.debug_selector(move || selector.to_string()))
            .when_some(self.on_close, |el, handler| el.on_click(handler));
        let close = motion::interact("modal-close-feedback", close).fill(c(SURFACE_ELEVATED)).rounded(ROUNDED);
        // fixed inset-0 flex items-center justify-center bg-black/55
        let backdrop = div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x0000008c))
            // What is behind the dialog takes no hover or clicks while it is open.
            .occlude();
        // The shade darkens in as the panel fades in.
        let backdrop = match motion::enabled(cx) {
            true => backdrop
                .with_animation("modal-backdrop", Animation::new(motion::DIALOG).with_easing(motion::ease_out), |backdrop, t| {
                    backdrop.bg(gpui::Rgba { r: 0., g: 0., b: 0., a: 0.55 * t })
                })
                .into_any_element(),
            false => backdrop.into_any_element(),
        };
        let panel = 
                // mx-4 max-h-[90vh] w-full max-w-* flex-col overflow-hidden rounded-lg border
                // border-border-primary bg-surface-secondary shadow-2xl
                // A definite width, not w-full capped by max-w: a percentage leaves wrapped
                // text measured as one line, and the footer would fall outside the panel.
                div()
                    .debug_selector(|| "modal-panel".into())
                    .mx(px(16.))
                    .w(px(width))
                    .max_h(px(max_height))
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
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .min_w_0()
                                        .children(self.leading)
                                        .child(
                                            div()
                                                .min_w_0()
                                                .truncate()
                                                .type_sm()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(c(TEXT_PRIMARY))
                                                .child(title),
                                        ),
                                )
                                .child(close),
                        )
                    })
                    .child(
                        div()
                            .id("modal-body")
                            .flex()
                            .flex_col()
                            .min_h_0()
                            .overflow_y_scroll()
                            .when(self.padded, |el| el.p(px(16.)))
                            .children(self.body),
                    )
                    .when(!self.footer.is_empty(), |el| {
                        // flex justify-end gap-2 border-t border-border-secondary px-4 py-3
                        el.child(
                            div()
                                .flex()
                                .flex_none()
                                .justify_end()
                                .items_center()
                                .gap(px(8.))
                                .border_t_1()
                                .border_color(c(BORDER_SECONDARY))
                                .px(px(16.))
                                .py(px(12.))
                                .children(self.footer),
                        )
                    });
        div().absolute().inset_0().child(backdrop).child(
            div().absolute().inset_0().flex().items_center().justify_center().child(motion::enter_from("modal-enter", panel, motion::DIALOG, (0., 12.), cx)),
        )
    }
}
