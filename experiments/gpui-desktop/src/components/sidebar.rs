//! `lib/components/Sidebar.svelte`: a collapsible section and a nav row.

use gpui::{div, prelude::*, px, AnyElement, App, ClickEvent, ElementId, FontWeight, SharedString, Window};

use super::{Type, ROUNDED};
use crate::theme::*;
use crate::ui::icon;

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A titled block with a bottom border. With `on_toggle` the title is the
/// collapsing header (Drives, Places, Favorites); without it, a plain label (Devices).
#[derive(IntoElement)]
pub struct SidebarSection {
    id: ElementId,
    title: &'static str,
    collapsed: bool,
    on_toggle: Option<ClickHandler>,
    children: Vec<AnyElement>,
}

impl SidebarSection {
    pub fn new(id: impl Into<ElementId>, title: &'static str) -> Self {
        Self { id: id.into(), title, collapsed: false, on_toggle: None, children: Vec::new() }
    }

    pub fn collapsible(
        mut self,
        collapsed: bool,
        on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.collapsed = collapsed;
        self.on_toggle = Some(Box::new(on_toggle));
        self
    }
}

impl ParentElement for SidebarSection {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for SidebarSection {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let section = div().flex().flex_col().border_b_1().border_color(c(BORDER_SECONDARY));
        match self.on_toggle {
            // px-3 py-2.5 text-[11px] font-medium tracking-wide uppercase; 14px chevron, gap-1.5
            Some(on_toggle) => section
                .child(
                    div()
                        .id(self.id)
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .px(px(12.))
                        .py(px(10.))
                        .type_11()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c(TEXT_SECONDARY))
                        .cursor_pointer()
                        .hover(|style| style.text_color(c(TEXT_PRIMARY)))
                        .child(icon(
                            if self.collapsed { "chevron-right" } else { "chevron-down" },
                            14.,
                            c(TEXT_SECONDARY),
                        ))
                        .child(self.title.to_uppercase())
                        .on_click(on_toggle),
                )
                .when(!self.collapsed, |section| section.child(div().pb(px(8.)).children(self.children))),
            // py-2; the label is px-3 pb-1 text-[11px] font-medium uppercase
            None => section.py(px(8.)).child(
                div()
                    .px(px(12.))
                    .pb(px(4.))
                    .type_11()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(TEXT_SECONDARY))
                    .child(self.title.to_uppercase()),
            )
            .children(self.children),
        }
    }
}

/// `navItemClass`: a 13px row with a 16px icon, indented to line up under the section title.
#[derive(IntoElement)]
pub struct NavItem {
    id: ElementId,
    label: SharedString,
    icon: &'static str,
    dim_icon: bool,
    active: bool,
    disabled: bool,
    on_click: Option<ClickHandler>,
    trailing: Option<(&'static str, SharedString, ClickHandler)>,
}

impl NavItem {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, icon: &'static str) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon,
            dim_icon: true,
            active: false,
            disabled: false,
            on_click: None,
            trailing: None,
        }
    }

    /// Drive, place and favorite icons are `opacity-80`; Trash and device icons are not.
    pub fn dim_icon(mut self, dim: bool) -> Self {
        self.dim_icon = dim;
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
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

    /// A small action at the row's end that shows on hover (unpin, eject).
    pub fn trailing(
        mut self,
        icon: &'static str,
        label: impl Into<SharedString>,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.trailing = Some((icon, label.into(), Box::new(handler)));
        self
    }
}

impl RenderOnce for NavItem {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        // `navItemActiveClass` adds bg-selection, but the built CSS lets the base
        // bg-transparent win, so only the white text shows. Match what renders.
        let text = if self.active { WHITE } else { TEXT_PRIMARY };
        let group = SharedString::from(format!("nav-{}", self.id));
        // w-full flex items-center gap-2.5 py-1.5 px-3 pl-5 text-[13px] (pr-9 with an action)
        div()
            .id(self.id)
            .group(group.clone())
            .relative()
            .w_full()
            .flex()
            .items_center()
            .gap(px(10.))
            .py(px(6.))
            .pl(px(20.))
            .pr(px(if self.trailing.is_some() { 36. } else { 12. }))
            .type_13()
            .text_color(c(text))
            .when(self.disabled, |row| row.opacity(0.5))
            .when(!self.disabled, |row| row.cursor_pointer().hover(|style| style.bg(c(SURFACE_SECONDARY))))
            .child(icon(self.icon, 16., if self.dim_icon { alpha(text, 0.8) } else { c(text) }))
            .child(div().flex_1().min_w_0().truncate().child(self.label))
            .when_some(self.trailing, |row, (glyph, label, handler)| {
                // absolute top-1 right-2 h-6 w-6 rounded text-text-muted, shown on group hover
                row.child(
                    div()
                        .id(label)
                        .absolute()
                        .top(px(4.))
                        .right(px(8.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(24.))
                        .rounded(px(ROUNDED))
                        .invisible()
                        .group_hover(group, |style| style.visible())
                        .cursor_pointer()
                        .hover(|style| style.bg(c(SURFACE_SECONDARY)))
                        .child(icon(glyph, 14., c(TEXT_MUTED)))
                        .on_click(move |event, window, cx| {
                            cx.stop_propagation();
                            handler(event, window, cx);
                        }),
                )
            })
            .when_some(self.on_click.filter(|_| !self.disabled), |row, handler| row.on_click(handler))
    }
}
