//! `settings/SettingsRow.svelte` and the section buttons in `SettingsModal.svelte`.

use gpui::{div, prelude::*, px, AnyElement, App, ClickEvent, ElementId, Rgba, SharedString, Window};

use super::{Type, ROUNDED};
use crate::theme::*;

/// A label and its control. Rows in a group take `last` so the final one has no rule.
#[derive(IntoElement)]
pub struct SettingsRow {
    label: SharedString,
    control: Option<AnyElement>,
    last: bool,
}

impl SettingsRow {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self { label: label.into(), control: None, last: false }
    }

    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.control = Some(control.into_any_element());
        self
    }

    /// `last:border-b-0`
    pub fn last(mut self, last: bool) -> Self {
        self.last = last;
        self
    }
}

impl RenderOnce for SettingsRow {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        // flex min-h-10 items-center justify-between gap-x-4 border-b py-1.5 text-[13px]
        div()
            .flex()
            .min_h(px(40.))
            .items_center()
            .justify_between()
            .gap(px(16.))
            .when(!self.last, |row| row.border_b_1().border_color(c(BORDER_SECONDARY)))
            .py(px(6.))
            .type_13()
            .child(div().text_color(c(TEXT_PRIMARY)).child(self.label))
            .children(self.control)
    }
}

/// Marks the last row of a group.
pub fn settings_rows(rows: Vec<SettingsRow>) -> impl IntoElement {
    let count = rows.len();
    div().flex().flex_col().children(rows.into_iter().enumerate().map(move |(index, row)| row.last(index + 1 == count)))
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A section button in the settings nav, or its quieter "Reset to defaults".
#[derive(IntoElement)]
pub struct SettingsTab {
    id: ElementId,
    label: SharedString,
    active: bool,
    selection: Rgba,
    quiet: bool,
    on_click: Option<ClickHandler>,
}

impl SettingsTab {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, selection: Rgba) -> Self {
        Self { id: id.into(), label: label.into(), active: false, selection, quiet: false, on_click: None }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// `text-xs`, for the reset button at the foot of the nav.
    pub fn quiet(mut self) -> Self {
        self.quiet = true;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for SettingsTab {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let selector = self.id.to_string();
        // h-8 rounded px-2.5 text-left text-[13px]; active bg-selection text-white
        div()
            .id(self.id)
            .debug_selector(move || selector)
            .flex()
            .flex_none()
            .items_center()
            .h(px(32.))
            .rounded(px(ROUNDED))
            .px(px(10.))
            .whitespace_nowrap()
            .cursor_pointer()
            .map(|tab| if self.quiet { tab.type_xs() } else { tab.type_13() })
            .map(|tab| match self.active {
                true => tab.bg(self.selection).text_color(c(WHITE)),
                false => tab
                    .text_color(c(TEXT_SECONDARY))
                    .hover(|style| style.bg(c(SURFACE_ELEVATED)).text_color(c(TEXT_PRIMARY))),
            })
            .child(self.label)
            .when_some(self.on_click, |tab, handler| tab.on_click(handler))
    }
}
