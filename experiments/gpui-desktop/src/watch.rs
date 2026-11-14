//! `browse/desktopWatch.ts`: refresh a native folder when it changes on disk. While the watch
//! stream is down the folder is polled every 15 s.

use std::time::Duration;

use futures::StreamExt;
use gpui::{Context, Task};

use crate::api::encode_query;
use crate::sse::{self, SseMessage};
use crate::workspace::Workspace;

const POLL: Duration = Duration::from_secs(15);

pub struct Watch {
    pub path: String,
    _task: Task<()>,
    _poll: Option<Task<()>>,
}

impl Workspace {
    /// Starts or replaces the pane's watch when its folder changed; This PC has none.
    pub(crate) fn ensure_watch(&mut self, pane_id: usize, cx: &mut Context<Self>) {
        let Some(pane) = self.pane_by_id(pane_id) else {
            return;
        };
        let path = pane.path.clone();
        if path.is_empty() {
            self.watches.remove(&pane_id);
            return;
        }
        if self
            .watches
            .get(&pane_id)
            .is_some_and(|watch| watch.path == path)
        {
            return;
        }
        let mut events = sse::subscribe(
            self.api.clone(),
            format!("/desktop/watch?path={}", encode_query(&path)),
        );
        let task = cx.spawn(async move |this, cx| {
            while let Some(message) = events.next().await {
                let reconnected = matches!(&message, SseMessage::Event { name, .. } if name == "ready");
                let changed = matches!(&message, SseMessage::Event { name, .. } if name == "ready" || name == "change");
                let disconnected = message == SseMessage::Disconnected;
                let alive = this.update(cx, |this, cx| {
                    if reconnected {
                        if let Some(watch) = this.watches.get_mut(&pane_id) {
                            watch._poll = None;
                        }
                    }
                    if disconnected {
                        this.start_poll(pane_id, cx);
                    }
                    if changed && this.pane_by_id(pane_id).is_some_and(|pane| !pane.path.is_empty()) {
                        this.load_pane(pane_id, cx);
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        self.watches.insert(
            pane_id,
            Watch {
                path,
                _task: task,
                _poll: None,
            },
        );
    }

    fn start_poll(&mut self, pane_id: usize, cx: &mut Context<Self>) {
        let Some(watch) = self.watches.get_mut(&pane_id) else {
            return;
        };
        if watch._poll.is_some() {
            return;
        }
        watch._poll = Some(cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(POLL).await;
            if this
                .update(cx, |this, cx| this.load_pane(pane_id, cx))
                .is_err()
            {
                break;
            }
        }));
    }
}
