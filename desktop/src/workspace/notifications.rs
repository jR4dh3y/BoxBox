//! `stores/toast.svelte.ts`: result notices. At most four show at once, and a
//! notice's five seconds start only once it is shown. A dismissed notice
//! leaves for `motion::LEAVE` before it is gone, so it can slide away.

use std::rc::Rc;
use std::time::{Duration, Instant};

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
    /// When it was dismissed; it is drawn leaving until `purge` drops it.
    pub leaving: Option<Instant>,
}

pub(crate) type NoticeAction = Rc<dyn Fn(&mut Workspace, &mut Context<Workspace>)>;

#[derive(Default)]
pub(crate) struct Notices {
    next_id: u64,
    entries: Vec<Notice>,
}

impl Notices {
    /// The notices on show, oldest first; leaving ones are not among them.
    pub fn visible(&self) -> Vec<&Notice> {
        self.entries.iter().filter(|notice| notice.leaving.is_none()).take(MAX_VISIBLE).collect()
    }

    /// What the stack draws: the notices on show and those still leaving, in order.
    pub fn drawn(&self) -> impl Iterator<Item = &Notice> {
        let shown: Vec<u64> = self.visible().iter().map(|notice| notice.id).collect();
        self.entries.iter().filter(move |notice| notice.leaving.is_some() || shown.contains(&notice.id))
    }

    fn push(&mut self, kind: ToastKind, message: String, action: Option<(SharedString, NoticeAction)>) {
        self.next_id += 1;
        self.entries.push(Notice { id: self.next_id, kind, message, action, timed: false, leaving: None });
    }

    /// Starts `id` leaving; false if it is not on show.
    fn remove(&mut self, id: u64) -> bool {
        match self.entries.iter_mut().find(|notice| notice.id == id && notice.leaving.is_none()) {
            Some(notice) => {
                notice.leaving = Some(Instant::now());
                true
            }
            None => false,
        }
    }

    /// Drops `id` once it has left.
    fn purge(&mut self, id: u64) {
        self.entries.retain(|notice| notice.id != id);
    }

    /// Visible notices whose lifetime has not started; marks them started.
    fn start_timers(&mut self) -> Vec<u64> {
        self.entries
            .iter_mut()
            .filter(|notice| notice.leaving.is_none())
            .take(MAX_VISIBLE)
            .filter(|notice| !notice.timed)
            .map(|notice| {
                notice.timed = true;
                notice.id
            })
            .collect()
    }
}

impl Workspace {
    /// Ends an operation's busy state and shows its failure. Success is silent, as
    /// in other file managers: the new folder, the opened app or the pasted files
    /// are the result. Outcomes nothing on screen shows call `notify` themselves.
    pub(crate) fn report(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        self.operation_status = super::operations::OperationStatus::Idle;
        if let Err(message) = result {
            self.notify(ToastKind::Error, message, cx);
        }
        cx.notify();
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
        if !self.notices.remove(id) {
            return;
        }
        if crate::components::motion::enabled(cx) {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(crate::components::motion::LEAVE).await;
                this.update(cx, |this, cx| {
                    this.notices.purge(id);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        } else {
            self.notices.purge(id);
        }
        self.start_notice_timers(cx);
        cx.notify();
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
