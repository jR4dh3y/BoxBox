//! `EditablePathBar.svelte` editing: Ctrl+L or a click on the bar turns it into a text field.
//! Enter goes to a BoxBox path (`home/Documents`) or an absolute local path (`/etc`).

use gpui::{AppContext as _, Context, Entity, Focusable, Subscription, Window};

use crate::ui::text_input::{InputEvent, TextInput};
use crate::workspace::Workspace;

pub struct LocationEdit {
    pub pane: usize,
    pub input: Entity<TextInput>,
    _events: Subscription,
}

/// Typed text as a BoxBox path; absolute local paths go through the `/` mount.
pub fn parse_location(text: &str) -> String {
    let text = text.trim();
    let path = match text.strip_prefix('/') {
        Some("") => return "root".into(),
        Some(rest) => format!("root/{rest}"),
        None => text.to_string(),
    };
    path.trim_end_matches('/').to_string()
}

impl Workspace {
    pub(crate) fn edit_location(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pane = self.pane().id;
        let path = self.pane().path.clone();
        let text = if path.starts_with('@') {
            String::new()
        } else {
            path
        };
        let len = text.len();
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Type a path", cx);
            input.set_text(text, 0..len, cx);
            input
        });
        let events = cx.subscribe_in(
            &input,
            window,
            |this, input, event, window, cx| match event {
                InputEvent::Submit => {
                    let path = parse_location(input.read(cx).text());
                    this.location = None;
                    window.focus(&this.focus_handle);
                    this.navigate(path, cx);
                }
                InputEvent::Cancel => {
                    this.location = None;
                    window.focus(&this.focus_handle);
                    cx.notify();
                }
                InputEvent::Changed => {}
            },
        );
        window.focus(&input.focus_handle(cx));
        self.location = Some(LocationEdit {
            pane,
            input,
            _events: events,
        });
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_absolute_paths_to_the_root_mount() {
        assert_eq!(parse_location(" home/Documents/ "), "home/Documents");
        assert_eq!(parse_location("/etc/"), "root/etc");
        assert_eq!(parse_location("/"), "root");
    }
}
