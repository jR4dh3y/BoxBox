//! One notice from `lib/components/ui/Toast.svelte`. The view stacks them in
//! a 380px column with 8px gaps, as `Notifications.svelte` does.

use gpui::{div, prelude::*, px, App, ClickEvent, ElementId, SharedString, Window};

use super::{Type, ROUNDED, ROUNDED_LG};
use crate::theme::*;
use crate::ui::icon;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToastKind {
    Success,
    Error,
    Info,
    Warning,
}

type DismissHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Toast {
    id: ElementId,
    kind: ToastKind,
    message: SharedString,
    on_dismiss: Option<DismissHandler>,
    action: Option<(SharedString, DismissHandler)>,
}

impl Toast {
    pub fn new(id: impl Into<ElementId>, kind: ToastKind, message: impl Into<SharedString>) -> Self {
        Self { id: id.into(), kind, message: message.into(), on_dismiss: None, action: None }
    }

    /// A text button before the close button, for the notice's follow-up.
    pub fn action(mut self, label: impl Into<SharedString>, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.action = Some((label.into(), Box::new(handler)));
        self
    }

    pub fn on_dismiss(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Toast {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let (glyph, color) = match self.kind {
            ToastKind::Success => ("check-circle", SUCCESS),
            ToastKind::Error => ("x-circle", DANGER),
            ToastKind::Info => ("info", ACCENT),
            ToastKind::Warning => ("alert-triangle", WARNING),
        };
        // flex items-start gap-3 rounded-lg border border-border-primary bg-surface-secondary px-4 py-3 shadow-lg
        div()
            .flex()
            .items_start()
            .gap(px(12.))
            .rounded(px(ROUNDED_LG))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_SECONDARY))
            .px(px(16.))
            .py(px(12.))
            .shadow_lg()
            // mt-0.5 shrink-0, a 16px icon
            .child(div().mt(px(2.)).flex_none().child(icon(glyph, 16., c(color))))
            // flex-1 pr-2 text-[13px] text-text-primary
            .child(div().flex_1().pr(px(8.)).type_13().text_color(c(TEXT_PRIMARY)).child(self.message))
            .when_some(self.action, |el, (label, handler)| {
                // text-[13px] font-medium text-accent, as a link-like button
                el.child(
                    div()
                        .id("toast-action")
                        .debug_selector(|| "toast-action".into())
                        .flex_none()
                        .type_13()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(c(ACCENT))
                        .cursor_pointer()
                        .hover(|style| style.underline())
                        .child(label)
                        .on_click(handler),
                )
            })
            .child(
                // shrink-0 rounded p-0.5 text-text-muted, a 14px X
                div()
                    .id(self.id)
                    .flex_none()
                    .rounded(px(ROUNDED))
                    .p(px(2.))
                    .cursor_pointer()
                    .hover(|style| style.bg(c(SURFACE_ELEVATED)))
                    .child(icon("x", 14., c(TEXT_MUTED)))
                    .when_some(self.on_dismiss, |el, handler| el.on_click(handler)),
            )
    }
}
