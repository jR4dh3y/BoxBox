//! Read-only text the mouse can select and Ctrl+C copies, for the Details
//! panel and Properties. The workspace keeps the one selection.

use gpui::{div, prelude::*, Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, SharedString};

use crate::components::{index_at, selectable_text, TextSelection};
use crate::workspace::Workspace;

impl Workspace {
    /// Text the mouse can select and Ctrl+C copies; a double-click takes it all.
    pub(crate) fn selectable(&self, id: impl Into<SharedString>, text: impl Into<SharedString>, cx: &mut Context<Self>) -> impl IntoElement {
        let (id, text): (SharedString, SharedString) = (id.into(), text.into());
        let range = self.text_selection.as_ref().filter(|selection| selection.is_of(&id, &text)).map(TextSelection::range);
        let styled = selectable_text(text.clone(), range);
        let (pressed, moved) = (styled.layout().clone(), styled.layout().clone());
        let (pressed_text, moved_text) = (text.clone(), text);
        let (pressed_id, moved_id, selector) = (id.clone(), id.clone(), id.to_string());
        div()
            .id(id)
            .debug_selector(move || selector)
            .cursor_text()
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                let at = index_at(&pressed, &pressed_text, event.position);
                this.text_selection = Some(TextSelection::start(pressed_id.clone(), pressed_text.clone(), at, event.click_count >= 2));
                // So Ctrl+C reaches the workspace rather than a text field.
                this.focus_handle.focus(window);
                cx.stop_propagation();
                cx.notify();
            }))
            .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                let at = index_at(&moved, &moved_text, event.position);
                if let Some(selection) = this.text_selection.as_mut().filter(|selection| selection.dragging && selection.is_of(&moved_id, &moved_text)) {
                    selection.extend(at);
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, _| stop_dragging(this)))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, _, _| stop_dragging(this)))
            .child(styled)
    }
}

fn stop_dragging(workspace: &mut Workspace) {
    if let Some(selection) = workspace.text_selection.as_mut() {
        selection.dragging = false;
    }
}
