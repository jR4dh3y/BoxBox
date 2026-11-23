//! A question before something that cannot be undone, in the app's own Modal
//! rather than a system prompt, as `BrowseDialogs.svelte` asks.

use gpui::{Context, SharedString};

use super::Workspace;

type Action = Box<dyn FnOnce(&mut Workspace, &mut Context<Workspace>)>;

/// How the dialog asks.
pub(crate) enum Ask {
    /// A titled dialog that explains, with a named action.
    Explained { title: SharedString, message: SharedString, confirm_label: SharedString },
    /// One short question answered No or Yes, such as "Delete 5 items?".
    Question(SharedString),
}

#[cfg(test)]
impl Ask {
    /// What the dialog asks first: the title, or the question.
    pub fn heading(&self) -> &str {
        match self {
            Self::Explained { title, .. } => title,
            Self::Question(question) => question,
        }
    }
}

pub(crate) struct Confirm {
    pub ask: Ask,
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
            ask: Ask::Explained { title: title.into(), message: message.into(), confirm_label: confirm_label.into() },
            danger: true,
            action: Box::new(action),
            secondary: None,
        }
    }

    /// A destructive question answered No or Yes.
    pub fn question(question: impl Into<SharedString>, action: impl FnOnce(&mut Workspace, &mut Context<Workspace>) + 'static) -> Self {
        Self { ask: Ask::Question(question.into()), danger: true, action: Box::new(action), secondary: None }
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
