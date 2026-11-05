//! `SearchBar.svelte`: recursive name search under the current folder, active from two
//! trimmed characters, debounced 300 ms; Enter searches at once, Esc or × clears.

use std::time::Duration;

use gpui::{div, prelude::*, px, Context, Entity, Focusable, Subscription, Window};

use crate::theme::*;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{icon, TextScale};
use crate::workspace::Workspace;

const DEBOUNCE: Duration = Duration::from_millis(300);

/// The query a search runs with, or None when it is too short to search.
pub fn active_query(text: &str) -> Option<String> {
    let query = text.trim();
    (query.chars().count() >= 2).then(|| query.to_string())
}

pub struct SearchField {
    pub input: Entity<TextInput>,
    epoch: u64,
    _events: Subscription,
}

impl Workspace {
    pub(crate) fn search_field(
        &mut self,
        pane_id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> SearchField {
        let input = cx.new(|cx| TextInput::new("Search files and folders...", cx));
        let events =
            cx.subscribe_in(
                &input,
                window,
                move |this, input, event, window, cx| match event {
                    InputEvent::Changed => {
                        this.debounce_search(pane_id, input.read(cx).text().to_string(), cx)
                    }
                    InputEvent::Submit => {
                        let text = input.read(cx).text().trim().to_string();
                        this.set_query(pane_id, (!text.is_empty()).then_some(text), cx);
                    }
                    InputEvent::Cancel => {
                        input.update(cx, |input, cx| input.set_text("", 0..0, cx));
                        this.set_query(pane_id, None, cx);
                        window.focus(&this.focus_handle);
                    }
                },
            );
        SearchField {
            input,
            epoch: 0,
            _events: events,
        }
    }

    fn debounce_search(&mut self, pane_id: usize, text: String, cx: &mut Context<Self>) {
        let Some(field) = self.search_fields.get_mut(&pane_id) else {
            return;
        };
        field.epoch += 1;
        let epoch = field.epoch;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            this.update(cx, |this, cx| {
                if this
                    .search_fields
                    .get(&pane_id)
                    .is_some_and(|field| field.epoch == epoch)
                {
                    this.set_query(pane_id, active_query(&text), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn set_query(&mut self, pane_id: usize, query: Option<String>, cx: &mut Context<Self>) {
        let query = query.unwrap_or_default();
        let Some(pane) = self.pane_by_id_mut(pane_id) else {
            return;
        };
        if pane.query == query {
            return;
        }
        pane.query = query;
        self.load_pane(pane_id, cx);
    }

    /// Navigating clears the query and its text.
    pub(crate) fn clear_search_text(&mut self, pane_id: usize, cx: &mut Context<Self>) {
        if let Some(field) = self.search_fields.get_mut(&pane_id) {
            field.epoch += 1;
            field
                .input
                .update(cx, |input, cx| input.set_text("", 0..0, cx));
        }
    }

    pub(crate) fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.pane().id;
        if let Some(field) = self.search_fields.get(&id) {
            window.focus(&field.input.focus_handle(cx));
        }
    }

    pub(crate) fn render_search(
        &self,
        pane_id: usize,
        toolbar_width: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // `w-28 @lg:w-48 @3xl:w-64 @5xl:w-96` against the toolbar's container width.
        let width = match toolbar_width {
            w if w >= 1024. => 384.,
            w if w >= 768. => 256.,
            w if w >= 512. => 192.,
            _ => 112.,
        };
        let field = self.search_fields.get(&pane_id);
        let has_text = field.is_some_and(|field| !field.input.read(cx).text().is_empty());
        div()
            .w(px(width))
            .flex_shrink()
            .min_w(px(112.))
            .h(px(28.))
            .flex()
            .items_center()
            .gap(px(8.))
            .pl(px(12.))
            .pr(px(4.))
            .rounded(px(4.))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_SECONDARY))
            .text_sm_tw()
            .overflow_hidden()
            .child(icon("search", 16., c(TEXT_MUTED)))
            .children(field.map(|field| {
                div()
                    .flex_1()
                    .min_w_0()
                    .pl(px(6.))
                    .child(field.input.clone())
            }))
            .when(has_text, |bar| {
                bar.child(
                    div()
                        .id("search-clear")
                        .size(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .hover(|style| style.bg(c(SURFACE_ELEVATED)))
                        .child(icon("x", 14., c(TEXT_MUTED)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.clear_search_text(pane_id, cx);
                            this.set_query(pane_id, None, cx);
                        })),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_two_trimmed_characters() {
        assert_eq!(active_query(" a "), None);
        assert_eq!(active_query(" ab "), Some("ab".into()));
        assert_eq!(active_query("é"), None);
    }
}
