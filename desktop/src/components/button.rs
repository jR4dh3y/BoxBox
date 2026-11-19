//! `lib/components/ui/Button.svelte`.

use gpui::{div, prelude::*, px, App, ClickEvent, ElementId, FontWeight, SharedString, Window};

use super::{Type, ROUNDED};
use crate::theme::*;
use crate::ui::icon;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonSize {
    Sm,
    Md,
    Lg,
    /// A square icon button.
    Icon,
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    icon: Option<SharedString>,
    variant: ButtonVariant,
    size: ButtonSize,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            icon: None,
            variant: ButtonVariant::Primary,
            size: ButtonSize::Md,
            disabled: false,
            on_click: None,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// A Lucide icon, drawn at 16px before the label.
    pub fn icon(mut self, icon: impl Into<SharedString>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        // (background, hover background, text, hover text, border)
        let (bg, hover_bg, text, hover_text, border) = match self.variant {
            ButtonVariant::Primary => (Some(ACCENT_SOLID), Some(ACCENT_SOLID_HOVER), WHITE, WHITE, None),
            ButtonVariant::Secondary => (
                Some(SURFACE_SECONDARY),
                Some(SURFACE_TERTIARY),
                TEXT_SECONDARY,
                TEXT_PRIMARY,
                Some(BORDER_PRIMARY),
            ),
            ButtonVariant::Ghost => (None, Some(SURFACE_SECONDARY), TEXT_SECONDARY, TEXT_PRIMARY, None),
            ButtonVariant::Danger => (Some(DANGER), Some(DANGER_HOVER), WHITE, WHITE, None),
        };
        let enabled = !self.disabled;
        let id = self.id.clone();
        let base = div()
            .debug_selector({
                let id = self.id.to_string();
                move || id
            })
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .font_weight(FontWeight::MEDIUM)
            .rounded(px(ROUNDED))
            .text_color(c(text))
            .when_some(border, |el, border| el.border_1().border_color(c(border)))
            .when(enabled, |el| el.cursor_pointer().hover(move |style| style.text_color(c(hover_text))))
            .when(self.disabled, |el| el.opacity(0.5));
        // sm: h-7 px-2.5 text-xs gap-1 · md: h-8 px-3 text-[13px] gap-2
        // lg: h-9 px-5 text-sm gap-2 · icon: w-7 h-7 p-0
        let sized = match self.size {
            ButtonSize::Sm => base.h(px(28.)).px(px(10.)).type_xs().gap(px(4.)),
            ButtonSize::Md => base.h(px(32.)).px(px(12.)).type_13().gap(px(8.)),
            ButtonSize::Lg => base.h(px(36.)).px(px(20.)).type_sm().gap(px(8.)),
            ButtonSize::Icon => base.size(px(28.)),
        };
        let button = sized
            .when_some(self.icon, |el, name: SharedString| el.child(icon(name.as_ref(), 16., c(text))))
            .when_some(self.label, |el, label| el.child(label))
            .when_some(self.on_click.filter(|_| enabled), |el, handler| el.on_click(handler));
        // The background eases to its hover colour, and a press brightens it (`motion`).
        let disabled_alpha = |color: gpui::Rgba| if enabled { color } else { gpui::Rgba { a: color.a * 0.5, ..color } };
        let feedback = super::motion::interact(id, button).rounded(ROUNDED).disabled(!enabled);
        let feedback = match bg {
            Some(bg) => feedback.base(disabled_alpha(c(bg))),
            None => feedback,
        };
        match hover_bg {
            Some(hover) => feedback.fill(c(hover)),
            None => feedback,
        }
    }
}
