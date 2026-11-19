//! `lib/components/ui/ContextMenu.svelte`: the menu panel. Views place it
//! (`anchored`) and close it. A submenu opens beside its row on hover; the view
//! keeps which one is open, since the menu is rebuilt every frame.

use std::{rc::Rc, sync::Arc};

use gpui::{anchored, deferred, div, img, point, prelude::*, px, AnchoredPositionMode, AnyElement, App, ClickEvent, ElementId, RenderImage, SharedString, Window};

use super::{motion, Type, ROUNDED_LG};
use crate::theme::*;
use crate::ui::icon;

type SelectHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
/// Told which submenu the pointer opened, or `None` when it rests on a plain row.
type SubmenuHandler = Rc<dyn Fn(Option<SharedString>, &mut Window, &mut App) + 'static>;

/// A named glyph from the icon set, or a picture such as an app's icon.
pub enum MenuIcon {
    Glyph(SharedString),
    Image(Arc<RenderImage>),
}

pub enum MenuItem {
    Action {
        id: ElementId,
        label: SharedString,
        icon: Option<MenuIcon>,
        shortcut: Option<SharedString>,
        disabled: bool,
        on_select: Option<SelectHandler>,
    },
    Submenu {
        id: SharedString,
        label: SharedString,
        icon: Option<MenuIcon>,
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

    pub fn icon(self, name: impl Into<SharedString>) -> Self {
        self.with_icon(MenuIcon::Glyph(name.into()))
    }

    pub fn image(self, image: Arc<RenderImage>) -> Self {
        self.with_icon(MenuIcon::Image(image))
    }

    fn with_icon(mut self, value: MenuIcon) -> Self {
        match &mut self {
            Self::Action { icon, .. } | Self::Submenu { icon, .. } => *icon = Some(value),
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
        // A 14px glyph or a 16px picture; an empty 16px slot without one, so labels line up.
        let glyph = |name: Option<MenuIcon>, disabled: bool| {
            div().flex().flex_none().items_center().justify_center().size(px(16.)).when_some(name, |el, name| match name {
                MenuIcon::Glyph(name) => el.child(icon(name.as_ref(), 14., c(if disabled { TEXT_DISABLED } else { TEXT_SECONDARY }))),
                MenuIcon::Image(image) => el.child(img(image).size(px(16.)).when(disabled, |image| image.opacity(0.5))),
            })
        };
        // 24px rows with 12px text, the density of desktop menus. No w-full: a
        // percentage of the content-sized panel resolves to the row's own width, so
        // rows stretch in the column instead and shortcuts line up. `flex_none` keeps
        // rows whole when the panel scrolls.
        let row = |id: ElementId, disabled: bool| {
            div()
                .debug_selector({
                    let id = id.to_string();
                    move || id
                })
                .id(id)
                .flex()
                .flex_none()
                .items_center()
                .gap(px(8.))
                .px(px(10.))
                .py(px(4.))
                .type_xs()
                .whitespace_nowrap()
                .text_color(c(if disabled { TEXT_DISABLED } else { TEXT_PRIMARY }))
                .when(!disabled, |el| el.cursor_pointer())
        };
        // The hover fill eases in and out (`motion`), behind an open submenu's own background.
        let feedback = |id: ElementId, row: gpui::Stateful<gpui::Div>, disabled: bool| {
            motion::interact(id, row).fill(motion::hover_fill()).disabled(disabled).into_any_element()
        };
        // A long name (an app, a template) ends in an ellipsis inside the panel's max width.
        let label = |text: SharedString| div().flex_1().min_w_0().truncate().child(text);
        match self {
            // mx-2 my-1 h-px bg-border-secondary
            Self::Separator => div().flex_none().mx(px(8.)).my(px(4.)).h(px(1.)).bg(c(BORDER_SECONDARY)).into_any_element(),
            Self::Action { id, label: text, icon: name, shortcut, disabled, on_select } => feedback(id.clone(), row(id, disabled)
                .child(glyph(name, disabled))
                .child(label(text))
                .when_some(shortcut, |el, keys| el.child(div().type_11().text_color(c(TEXT_MUTED)).child(keys)))
                .when_some(on_submenu.clone(), |el, handler| {
                    el.on_hover(move |hovered, window, cx| {
                        if *hovered {
                            handler(None, window, cx);
                        }
                    })
                })
                .when_some(on_select.filter(|_| !disabled), |el, handler| el.on_click(handler)), disabled),
            Self::Submenu { id, label: text, icon: name, items, open } => {
                let opened = id.clone();
                let open_on = on_submenu.clone();
                let element_id = ElementId::Name(id.clone());
                feedback(element_id.clone(), row(element_id, false)
                    .relative()
                    .when(open, |el| el.bg(c(SURFACE_SECONDARY)))
                    .child(glyph(name, false))
                    .child(label(text))
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
                        // Deferred drawing escapes the parent panel's scroll clip.
                        el.child(
                            div().absolute().top_0().left_full().child(
                                deferred(
                                    anchored()
                                        .position_mode(AnchoredPositionMode::Local)
                                        .position(point(px(0.), px(-4.)))
                                        .snap_to_window_with_margin(MENU_MARGIN)
                                        .child(div().occlude().child(ContextMenu::new(items))),
                                )
                                .with_priority(2),
                            ),
                        )
                    }), false)
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

/// The gap a menu keeps from the window's edges; a taller menu scrolls inside it.
pub const MENU_MARGIN: gpui::Pixels = px(8.);

impl RenderOnce for ContextMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let on_submenu = self.on_submenu;
        // `snap_to_window` moves a menu inside the window but never shrinks it, so a
        // menu taller than the window scrolls instead of losing its last rows.
        let max_height = (window.viewport_size().height - MENU_MARGIN * 2.).max(px(1.));
        // max-w-70 min-w-45 rounded-lg border border-border-primary bg-surface-context-menu py-1 shadow-xl
        let panel = div()
            .id("menu-panel")
            .flex()
            .flex_col()
            .min_w(px(180.))
            .max_w(px(280.))
            .max_h(max_height)
            .overflow_y_scroll()
            .rounded(px(ROUNDED_LG))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_CONTEXT_MENU))
            .py(px(4.))
            .shadow_xl()
            .children(self.items.into_iter().map(|item| item.render(&on_submenu)));
        // Fades in; a menu is rebuilt every frame but keeps its state while shown.
        motion::enter("menu-enter", panel, motion::MENU, cx)
    }
}
