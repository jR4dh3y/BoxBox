//! `lib/components/ui/ContextMenu.svelte`: the menu panel. Views place it
//! (`anchored`) and close it. A submenu opens beside its row on hover; the view
//! keeps which one is open, since the menu is rebuilt every frame.

use std::rc::Rc;

use gpui::{anchored, deferred, div, point, prelude::*, px, AnchoredPositionMode, AnyElement, App, ClickEvent, ElementId, SharedString, Window};

use super::{Type, ROUNDED_LG};
use crate::theme::*;
use crate::ui::icon;

type SelectHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
/// Told which submenu the pointer opened, or `None` when it rests on a plain row.
type SubmenuHandler = Rc<dyn Fn(Option<SharedString>, &mut Window, &mut App) + 'static>;

pub enum MenuItem {
    Action {
        id: ElementId,
        label: SharedString,
        icon: Option<SharedString>,
        shortcut: Option<SharedString>,
        disabled: bool,
        on_select: Option<SelectHandler>,
    },
    Submenu {
        id: SharedString,
        label: SharedString,
        icon: Option<SharedString>,
        items: Vec<MenuItem>,
        open: bool,
    },
    Separator,
}

impl MenuItem {
    pub fn action(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self::Action { id: id.into(), label: label.into(), icon: None, shortcut: None, disabled: false, on_select: None }
    }

    /// A row that shows `items` beside it while `open`.
    pub fn submenu(id: impl Into<SharedString>, label: impl Into<SharedString>, items: Vec<MenuItem>, open: bool) -> Self {
        Self::Submenu { id: id.into(), label: label.into(), icon: None, items, open }
    }

    pub fn icon(mut self, name: impl Into<SharedString>) -> Self {
        match &mut self {
            Self::Action { icon, .. } | Self::Submenu { icon, .. } => *icon = Some(name.into()),
            Self::Separator => {}
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

    fn render(self, on_submenu: &Option<SubmenuHandler>) -> AnyElement {
        // h-4 w-4 text-text-secondary, a 14px icon; w-4 spacer without one
        let glyph = |name: Option<SharedString>, disabled: bool| {
            div().flex().flex_none().items_center().justify_center().size(px(16.)).when_some(name, |el, name: SharedString| {
                el.child(icon(name.as_ref(), 14., c(if disabled { TEXT_DISABLED } else { TEXT_SECONDARY })))
            })
        };
        // flex w-full items-center gap-3 px-3 py-2 text-[13px]. No w-full: a percentage of
        // the content-sized panel resolves to the row's own width, so rows stretch in the
        // column instead and shortcuts line up.
        let row = |id: ElementId, disabled: bool| {
            div()
                .debug_selector({
                    let id = id.to_string();
                    move || id
                })
                .id(id)
                .flex()
                .items_center()
                .gap(px(12.))
                .px(px(12.))
                .py(px(8.))
                .type_13()
                .whitespace_nowrap()
                .text_color(c(if disabled { TEXT_DISABLED } else { TEXT_PRIMARY }))
                .when(!disabled, |el| el.cursor_pointer().hover(|style| style.bg(c(SURFACE_SECONDARY))))
        };
        match self {
            // mx-2 my-1 h-px bg-border-secondary
            Self::Separator => div().mx(px(8.)).my(px(4.)).h(px(1.)).bg(c(BORDER_SECONDARY)).into_any_element(),
            Self::Action { id, label, icon: name, shortcut, disabled, on_select } => row(id, disabled)
                .child(glyph(name, disabled))
                .child(div().flex_1().child(label))
                .when_some(shortcut, |el, keys| el.child(div().type_11().text_color(c(TEXT_MUTED)).child(keys)))
                .when_some(on_submenu.clone(), |el, handler| {
                    el.on_hover(move |hovered, window, cx| {
                        if *hovered {
                            handler(None, window, cx);
                        }
                    })
                })
                .when_some(on_select.filter(|_| !disabled), |el, handler| el.on_click(handler))
                .into_any_element(),
            Self::Submenu { id, label, icon: name, items, open } => {
                let opened = id.clone();
                let open_on = on_submenu.clone();
                row(ElementId::Name(id.clone()), false)
                    .relative()
                    .when(open, |el| el.bg(c(SURFACE_SECONDARY)))
                    .child(glyph(name, false))
                    .child(div().flex_1().child(label))
                    .child(icon("chevron-right", 14., c(TEXT_MUTED)))
                    .when_some(open_on.clone(), |el, handler| {
                        let opened = opened.clone();
                        el.on_hover(move |hovered, window, cx| {
                            if *hovered {
                                handler(Some(opened.clone()), window, cx);
                            }
                        })
                        .on_click({
                            let handler = open_on.clone().unwrap();
                            let opened = id.clone();
                            move |_, window, cx| handler(Some(opened.clone()), window, cx)
                        })
                    })
                    .when(open, |el| {
                        // The panel starts at the row's right edge, 4px up so its first row
                        // lines up, and moves inside the window when it would leave it.
                        el.child(
                            div().absolute().top_0().left_full().child(
                                deferred(
                                    anchored()
                                        .position_mode(AnchoredPositionMode::Local)
                                        .position(point(px(0.), px(-4.)))
                                        .snap_to_window()
                                        .child(div().occlude().child(ContextMenu { items, on_submenu: None })),
                                )
                                .with_priority(2),
                            ),
                        )
                    })
                    .into_any_element()
            }
        }
    }
}

#[derive(IntoElement)]
pub struct ContextMenu {
    items: Vec<MenuItem>,
    on_submenu: Option<SubmenuHandler>,
}

impl ContextMenu {
    pub fn new(items: Vec<MenuItem>) -> Self {
        Self { items, on_submenu: None }
    }

    /// Needed when the menu has submenus: says which one the pointer opened.
    pub fn on_submenu(mut self, handler: impl Fn(Option<SharedString>, &mut Window, &mut App) + 'static) -> Self {
        self.on_submenu = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ContextMenu {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let on_submenu = self.on_submenu;
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
            .children(self.items.into_iter().map(|item| item.render(&on_submenu)))
    }
}
