//! `lib/components/Sidebar.svelte`: a collapsible section and a nav row.

use gpui::{div, prelude::*, px, AnyElement, AnyView, App, ClickEvent, ElementId, FontWeight, SharedString, Window};

use super::skeleton::{self, Wait};
use super::{Type, ROUNDED};
use crate::theme::*;
use crate::ui::icon;

type CardBuilder = Box<dyn Fn(&mut Window, &mut App) -> AnyView + 'static>;
type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A titled block with a bottom border. With `on_toggle` the title is the
/// collapsing header (Drives, Places, Favorites); without it, a plain label (Devices).
#[derive(IntoElement)]
pub struct SidebarSection {
    id: ElementId,
    title: &'static str,
    collapsed: bool,
    /// When it last folded open or shut, so it can ease there.
    toggled: Option<std::time::Instant>,
    on_toggle: Option<ClickHandler>,
    children: Vec<AnyElement>,
}

impl SidebarSection {
    pub fn new(id: impl Into<ElementId>, title: &'static str) -> Self {
        Self { id: id.into(), title, collapsed: false, toggled: None, on_toggle: None, children: Vec::new() }
    }

    pub fn collapsible(
        mut self,
        collapsed: bool,
        toggled: Option<std::time::Instant>,
        on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.collapsed = collapsed;
        self.toggled = toggled;
        self.on_toggle = Some(Box::new(on_toggle));
        self
    }
}

impl ParentElement for SidebarSection {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

/// A row's height, to know how tall a folding section's rows stand.
const ROW: f32 = 34.;

impl RenderOnce for SidebarSection {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let section = div().flex().flex_col().border_b_1().border_color(c(BORDER_SECONDARY));
        // How open the section is, easing from where it was toggled.
        let t = super::motion::progress(self.toggled, super::motion::FOLD, window, cx);
        let open = if self.collapsed { 1. - t } else { t };
        let chevron = icon("chevron-right", 14., c(TEXT_SECONDARY)).with_transformation(gpui::Transformation::rotate(gpui::radians(open * std::f32::consts::FRAC_PI_2)));
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
                        .hover(|style| style.text_color(c(TEXT_PRIMARY))).active(crate::components::pressed)
                        .child(chevron)
                        .child(self.title.to_uppercase())
                        .on_click(on_toggle),
                )
                .when(open > 0., |section| {
                    let rows = div().pb(px(8.));
                    // While folding, the rows show through a window that grows or shrinks
                    // with them; the reckoned height runs a little over, so it opens whole.
                    let tall = self.children.len() as f32 * ROW + 8.;
                    let rows = if open < 1. { rows.max_h(px(tall * open)).overflow_hidden().opacity(open) } else { rows };
                    section.child(rows.children(self.children))
                }),
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
    muted: bool,
    wait: Option<Wait>,
    drop_target: bool,
    badge: Option<&'static str>,
    detail: Option<SharedString>,
    tooltip: Option<SharedString>,
    /// A richer card than `tooltip`, which it replaces.
    card: Option<CardBuilder>,
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
            muted: false,
            wait: None,
            drop_target: false,
            badge: None,
            detail: None,
            tooltip: None,
            card: None,
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

    /// A dragged item is over this entry and would be dropped into its folder.
    pub fn drop_target(mut self, drop_target: bool) -> Self {
        self.drop_target = drop_target;
        self
    }

    /// A remembered entry whose details are on their way: its name pulses.
    pub fn waiting(mut self, wait: Wait) -> Self {
        self.wait = Some(wait);
        self
    }

    /// Dims the row: a device that is present but not mounted.
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// A small glyph after the label, such as the read-only mark.
    pub fn badge(mut self, icon: &'static str) -> Self {
        self.badge = Some(icon);
        self
    }

    /// Muted text at the row's end, such as the server a drive is on.
    pub fn detail(mut self, text: impl Into<SharedString>) -> Self {
        self.detail = Some(text.into());
        self
    }

    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// A view shown after the pointer rests, in place of the text tooltip.
    pub fn hover_card(mut self, card: impl Fn(&mut Window, &mut App) -> AnyView + 'static) -> Self {
        self.card = Some(Box::new(card));
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
        let detail_selector = format!("{}-detail", self.id);
        let pulse_id = ElementId::Name(format!("{}-pulse", self.id).into());
        let (feedback_id, disabled) = (self.id.clone(), self.disabled);
        // w-full flex items-center gap-2.5 py-1.5 px-3 pl-5 text-[13px] (pr-9 with an action)
        let row = div()
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
            .when(!self.disabled, |row| row.cursor_pointer())
            .when(self.drop_target, |row| row.bg(alpha(ACCENT, 0.25)))
            .child(icon(self.icon, 16., alpha(text, if self.muted { 0.5 } else if self.dim_icon { 0.8 } else { 1. })))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    // With a detail the name takes the room left, so the detail sits at
                    // the row's end however long the name is.
                    .child({
                        let label = div()
                            .min_w_0()
                            .truncate()
                            .when(self.detail.is_some() && self.badge.is_none(), |label| label.flex_1())
                            .when(self.muted, |label| label.text_color(c(TEXT_MUTED)))
                            .child(self.label);
                        match self.wait {
                            Some(wait) => skeleton::pulse(pulse_id, label, wait),
                            None => label.into_any_element(),
                        }
                    })
                    .when_some(self.badge, |label, glyph| label.child(icon(glyph, 12., c(TEXT_MUTED))))
                    .when_some(self.detail, |label, detail| {
                        label.child(div().ml_auto().flex_none().max_w(px(80.)).truncate().type_11().text_color(c(TEXT_MUTED)).debug_selector(move || detail_selector).child(detail))
                    }),
            )
            .map(|row| match (self.card, self.tooltip) {
                (Some(card), _) => row.tooltip(card),
                (None, Some(text)) => row.tooltip(super::Tooltip::text(text)),
                (None, None) => row,
            })
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
                        .hover(|style| style.bg(c(SURFACE_SECONDARY))).active(crate::components::pressed)
                        .child(icon(glyph, 14., c(TEXT_MUTED)))
                        .on_click(move |event, window, cx| {
                            cx.stop_propagation();
                            handler(event, window, cx);
                        }),
                )
            })
            .when_some(self.on_click.filter(|_| !self.disabled), |row, handler| row.on_click(handler));
        // The hover fill eases in and out, and a click brightens the row (`motion`).
        super::motion::interact(feedback_id, row).fill(super::motion::hover_fill()).disabled(disabled)
    }
}
