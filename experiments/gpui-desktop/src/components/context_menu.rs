//! `lib/components/ui/ContextMenu.svelte`: the menu panel. Views place it
//! (`anchored`) and close it.

use gpui::{div, prelude::*, px, AnyElement, App, ClickEvent, ElementId, SharedString, Window};

use super::{Type, ROUNDED_LG};
use crate::theme::*;
use crate::ui::icon;

type SelectHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

pub enum MenuItem {
    Action {
        id: ElementId,
        label: SharedString,
        icon: Option<SharedString>,
        shortcut: Option<SharedString>,
        disabled: bool,
        on_select: Option<SelectHandler>,
    },
    Separator,
}

impl MenuItem {
    pub fn action(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self::Action { id: id.into(), label: label.into(), icon: None, shortcut: None, disabled: false, on_select: None }
    }

    pub fn icon(mut self, name: impl Into<SharedString>) -> Self {
        if let Self::Action { icon, .. } = &mut self {
            *icon = Some(name.into());
        }
        self
    }

    pub fn shortcut(mut self, keys: impl Into<SharedString>) -> Self {
        if let Self::Action { shortcut, .. } = &mut self {
            *shortcut = Some(keys.into());
        }
        self
    }

    pub fn disabled(mut self, value: bool) -> Self {
        if let Self::Action { disabled, .. } = &mut self {
            *disabled = value;
        }
        self
    }

    pub fn on_select(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        if let Self::Action { on_select, .. } = &mut self {
            *on_select = Some(Box::new(handler));
        }
        self
    }

    fn render(self) -> AnyElement {
        match self {
            // mx-2 my-1 h-px bg-border-secondary
            Self::Separator => div().mx(px(8.)).my(px(4.)).h(px(1.)).bg(c(BORDER_SECONDARY)).into_any_element(),
            // flex w-full items-center gap-3 px-3 py-2 text-[13px]
            Self::Action { id, label, icon: glyph, shortcut, disabled, on_select } => div()
                .debug_selector({
                    let id = id.to_string();
                    move || id
                })
                .id(id)
                // No w-full: a percentage of the content-sized panel resolves to the row's own
                // width, so rows stretch in the column instead and shortcuts line up.
                .flex()
                .items_center()
                .gap(px(12.))
                .px(px(12.))
                .py(px(8.))
                .type_13()
                .whitespace_nowrap()
                .text_color(c(if disabled { TEXT_DISABLED } else { TEXT_PRIMARY }))
                .when(!disabled, |el| el.cursor_pointer().hover(|style| style.bg(c(SURFACE_SECONDARY))))
                .child(
                    // h-4 w-4 text-text-secondary, a 14px icon; w-4 spacer without one
                    div().flex().flex_none().items_center().justify_center().size(px(16.)).when_some(glyph, |el, name: SharedString| {
                        el.child(icon(name.as_ref(), 14., c(if disabled { TEXT_DISABLED } else { TEXT_SECONDARY })))
                    }),
                )
                .child(div().flex_1().child(label))
                .when_some(shortcut, |el, keys| el.child(div().type_11().text_color(c(TEXT_MUTED)).child(keys)))
                .when_some(on_select.filter(|_| !disabled), |el, handler| el.on_click(handler))
                .into_any_element(),
        }
    }
}

#[derive(IntoElement)]
pub struct ContextMenu {
    items: Vec<MenuItem>,
}

impl ContextMenu {
    pub fn new(items: Vec<MenuItem>) -> Self {
        Self { items }
    }
}

impl RenderOnce for ContextMenu {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        // max-w-70 min-w-45 rounded-lg border border-border-primary bg-surface-context-menu py-1 shadow-xl
        div()
            .flex()
            .flex_col()
            .min_w(px(180.))
            .max_w(px(280.))
            .rounded(px(ROUNDED_LG))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_CONTEXT_MENU))
            .py(px(4.))
            .shadow_xl()
            .children(self.items.into_iter().map(MenuItem::render))
    }
}
