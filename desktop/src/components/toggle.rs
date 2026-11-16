//! `lib/components/ui/Toggle.svelte`.

use gpui::{div, prelude::*, px, App, ClickEvent, ElementId, Window};

use crate::theme::*;

type ChangeHandler = Box<dyn Fn(bool, &ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct Toggle {
    id: ElementId,
    checked: bool,
    disabled: bool,
    on_change: Option<ChangeHandler>,
}

impl Toggle {
    pub fn new(id: impl Into<ElementId>, checked: bool) -> Self {
        Self { id: id.into(), checked, disabled: false, on_change: None }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called with the new state.
    pub fn on_change(mut self, handler: impl Fn(bool, &ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let checked = self.checked;
        // relative h-5 w-10 rounded-full; knob: absolute top-0.5 left-0.5 h-4 w-4, translate-x-5 when on
        div()
            .debug_selector({
                let id = self.id.to_string();
                move || id
            })
            .id(self.id)
            .relative()
            .flex_none()
            .h(px(20.))
            .w(px(40.))
            .rounded_full()
            .bg(c(if checked { ACCENT_SOLID } else { SURFACE_ELEVATED }))
            .when(self.disabled, |el| el.opacity(0.5))
            .when(!self.disabled, |el| el.cursor_pointer())
            .child(
                div()
                    .absolute()
                    .top(px(2.))
                    .left(px(if checked { 22. } else { 2. }))
                    .size(px(16.))
                    .rounded_full()
                    // CSS dims the disabled control as one layer; GPUI dims each element,
                    // so the knob blends with the track (a known gallery exception).
                    .bg(c(WHITE)),
            )
            .when_some(self.on_change.filter(|_| !self.disabled), |el, handler| {
                el.on_click(move |event, window, cx| handler(!checked, event, window, cx))
            })
    }
}
