//! The List and Grid buttons of the status bar, also in the file chooser.

use std::rc::Rc;

use gpui::{div, prelude::*, px, App, ElementId, SharedString, Window};

use crate::pane::ViewMode;
use crate::theme::*;
use crate::ui::icon;

type Pick = Rc<dyn Fn(ViewMode, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct ViewSwitch {
    /// Prefixes the buttons' ids and test selectors: `{prefix}-list`, `{prefix}-grid`.
    prefix: &'static str,
    mode: ViewMode,
    on_pick: Pick,
}

impl ViewSwitch {
    pub fn new(prefix: &'static str, mode: ViewMode, on_pick: impl Fn(ViewMode, &mut Window, &mut App) + 'static) -> Self {
        Self { prefix, mode, on_pick: Rc::new(on_pick) }
    }
}

impl RenderOnce for ViewSwitch {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let button = |name: &str, glyph: &'static str, target: ViewMode| {
            let id = SharedString::from(format!("{}-{name}", self.prefix));
            let active = self.mode == target;
            let on_pick = self.on_pick.clone();
            let selector = id.to_string();
            let button = div()
                .id(id.clone())
                .debug_selector(move || selector)
                .size(px(22.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(2.))
                .cursor_pointer()
                .when(active, |button| button.bg(c(SURFACE_ELEVATED)))
                .child(icon(glyph, 14., c(if active { TEXT_PRIMARY } else { TEXT_MUTED })))
                .on_click(move |_, window, cx| on_pick(target, window, cx));
            super::motion::interact(ElementId::Name(format!("{id}-feedback").into()), button).fill(c(SURFACE_SECONDARY)).rounded(2.)
        };
        div()
            .flex()
            .flex_none()
            .gap(px(2.))
            .rounded(px(4.))
            .bg(c(SURFACE_SECONDARY))
            .p(px(2.))
            .child(button("list", "list", ViewMode::List))
            .child(button("grid", "layout-grid", ViewMode::Grid))
    }
}
