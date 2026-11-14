//! `stores/toast.svelte.ts`: result notices. At most four show at once, and a
//! notice's five seconds start only once it is shown.

use std::rc::Rc;
use std::time::Duration;

use gpui::{Context, SharedString};

use super::Workspace;
use crate::components::ToastKind;

const MAX_VISIBLE: usize = 4;
const LIFETIME: Duration = Duration::from_secs(5);

pub(crate) struct Notice {
    pub id: u64,
    pub kind: ToastKind,
    pub message: String,
    /// A follow-up the notice offers, such as showing a restored item.
    pub action: Option<(SharedString, NoticeAction)>,
    timed: bool,
}

pub(crate) type NoticeAction = Rc<dyn Fn(&mut Workspace, &mut Context<Workspace>)>;

#[derive(Default)]
pub(crate) struct Notices {
    next_id: u64,
    entries: Vec<Notice>,
}

impl Notices {
    pub fn visible(&self) -> &[Notice] {
        &self.entries[..self.entries.len().min(MAX_VISIBLE)]
    }

    fn push(&mut self, kind: ToastKind, message: String, action: Option<(SharedString, NoticeAction)>) {
        self.next_id += 1;
        self.entries.push(Notice { id: self.next_id, kind, message, action, timed: false });
    }

    fn remove(&mut self, id: u64) -> bool {
        let before = self.entries.len();
        self.entries.retain(|notice| notice.id != id);
        self.entries.len() != before
    }

    /// Visible notices whose lifetime has not started; marks them started.
    fn start_timers(&mut self) -> Vec<u64> {
        let count = self.entries.len().min(MAX_VISIBLE);
        self.entries[..count]
            .iter_mut()
            .filter(|notice| !notice.timed)
            .map(|notice| {
                notice.timed = true;
                notice.id
            })
            .collect()
    }
}

impl Workspace {
    /// Shows the outcome of an operation and ends its busy state.
    pub(crate) fn report(&mut self, result: Result<String, String>, cx: &mut Context<Self>) {
        self.operation_status = super::operations::OperationStatus::Idle;
        match result {
            Ok(message) => self.notify(ToastKind::Success, message, cx),
            Err(message) => self.notify(ToastKind::Error, message, cx),
        }
    }

    pub(crate) fn notify(&mut self, kind: ToastKind, message: impl Into<String>, cx: &mut Context<Self>) {
        self.notices.push(kind, message.into(), None);
        self.start_notice_timers(cx);
        cx.notify();
    }

    /// A notice with a button that runs `action` and closes the notice.
    pub(crate) fn notify_with_action(
        &mut self,
        kind: ToastKind,
        message: impl Into<String>,
        label: impl Into<SharedString>,
        action: impl Fn(&mut Workspace, &mut Context<Workspace>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.notices.push(kind, message.into(), Some((label.into(), Rc::new(action))));
        self.start_notice_timers(cx);
        cx.notify();
    }

    pub(crate) fn run_notice_action(&mut self, id: u64, cx: &mut Context<Self>) {
        let action = self.notices.visible().iter().find(|notice| notice.id == id).and_then(|notice| notice.action.as_ref().map(|(_, action)| action.clone()));
        self.dismiss_notice(id, cx);
        if let Some(action) = action {
            action(self, cx);
        }
    }

    pub(crate) fn dismiss_notice(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.notices.remove(id) {
            self.start_notice_timers(cx);
            cx.notify();
        }
    }

    fn start_notice_timers(&mut self, cx: &mut Context<Self>) {
        for id in self.notices.start_timers() {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(LIFETIME).await;
                this.update(cx, |this, cx| this.dismiss_notice(id, cx)).ok();
            })
            .detach();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_shown_notices_start_their_lifetime() {
        let mut notices = Notices::default();
        for index in 0..6 {
            notices.push(ToastKind::Info, index.to_string(), None);
        }
        assert_eq!(notices.start_timers(), [1, 2, 3, 4]);
        assert!(notices.start_timers().is_empty());
        assert!(notices.remove(2));
        assert_eq!(notices.start_timers(), [5]);
        assert_eq!(notices.visible().len(), 4);
    }
}
