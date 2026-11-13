//! `lib/components/ui/Input.svelte`: the field's frame and text. An editor
//! element, when the field is being edited, goes in as its child.

use gpui::{div, prelude::*, px, AnyElement, App, SharedString, Window};

use super::{Type, ROUNDED};
use crate::theme::*;

#[derive(IntoElement)]
pub struct Input {
    value: SharedString,
    placeholder: SharedString,
    disabled: bool,
    focused: bool,
    invalid: bool,
    editor: Option<AnyElement>,
}

impl Input {
    pub fn new(value: impl Into<SharedString>) -> Self {
        Self { value: value.into(), placeholder: SharedString::default(), disabled: false, focused: false, invalid: false, editor: None }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// `focus:border-border-focus`
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// `border-danger`, for a value that will not be accepted.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Draw this element in place of the static text.
    pub fn editor(mut self, editor: impl IntoElement) -> Self {
        self.editor = Some(editor.into_any_element());
        self
    }
}

impl RenderOnce for Input {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        // h-8 w-full rounded border border-border-primary bg-surface-primary px-3 text-[13px]
        let empty = self.value.is_empty();
        // Chromium centres an input's 19.5px line in its 30px content box on
        // whole pixels, 6px down; GPUI's centring rounds up a pixel earlier.
        div()
            .h(px(32.))
            .w_full()
            .flex()
            .items_start()
            .pt(px(6.))
            .overflow_hidden()
            .rounded(px(ROUNDED))
            .border_1()
            .border_color(c(if self.invalid { DANGER } else if self.focused { BORDER_FOCUS } else { BORDER_PRIMARY }))
            .bg(c(SURFACE_PRIMARY))
            .px(px(12.))
            .type_13()
            .whitespace_nowrap()
            .when(self.disabled, |el| el.opacity(0.5))
            .map(|el| match self.editor {
                Some(editor) => el.text_color(c(TEXT_PRIMARY)).child(editor),
                None if empty => el.text_color(c(TEXT_MUTED)).child(self.placeholder),
                None => el.text_color(c(TEXT_PRIMARY)).child(self.value),
            })
    }
}
