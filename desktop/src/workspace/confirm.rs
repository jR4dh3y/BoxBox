//! A question before something that cannot be undone, in the app's own Modal
//! rather than a system prompt, as `BrowseDialogs.svelte` asks.

use gpui::{Context, SharedString};

use super::Workspace;

type Action = Box<dyn FnOnce(&mut Workspace, &mut Context<Workspace>)>;

pub(crate) struct Confirm {
    pub title: SharedString,
    pub message: SharedString,
    /// Names shown under the message when several items are affected.
    pub items: Vec<SharedString>,
    pub confirm_label: SharedString,
    pub danger: bool,
    action: Action,
    /// A second answer beside the main one, such as Open beside Run.
    pub secondary: Option<(SharedString, Action)>,
}

impl Confirm {
    pub fn new(
        title: impl Into<SharedString>,
        message: impl Into<SharedString>,
        confirm_label: impl Into<SharedString>,
        action: impl FnOnce(&mut Workspace, &mut Context<Workspace>) + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            items: Vec::new(),
            confirm_label: confirm_label.into(),
            danger: true,
            action: Box::new(action),
            secondary: None,
        }
    }

    /// The main action is not destructive.
    pub fn safe(mut self) -> Self {
        self.danger = false;
        self
    }

    pub fn secondary(mut self, label: impl Into<SharedString>, action: impl FnOnce(&mut Workspace, &mut Context<Workspace>) + 'static) -> Self {
        self.secondary = Some((label.into(), Box::new(action)));
        self
    }

    pub fn items(mut self, items: Vec<SharedString>) -> Self {
        self.items = items;
        self
    }
}

impl Workspace {
    pub(crate) fn ask(&mut self, confirm: Confirm, cx: &mut Context<Self>) {
        self.confirm = Some(confirm);
        cx.notify();
    }

    pub(crate) fn answer(&mut self, confirmed: bool, cx: &mut Context<Self>) {
        if let Some(confirm) = self.confirm.take() {
            if confirmed {
                (confirm.action)(self, cx);
            }
            cx.notify();
        }
    }

    pub(crate) fn answer_secondary(&mut self, cx: &mut Context<Self>) {
        if let Some((_, action)) = self.confirm.take().and_then(|confirm| confirm.secondary) {
            action(self, cx);
            cx.notify();
        }
    }
}
