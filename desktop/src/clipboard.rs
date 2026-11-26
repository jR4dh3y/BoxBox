//! Orders clipboard writes from the workspace and its text controls.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_lock::Mutex;
use gpui::{App, ClipboardItem, Global};

#[derive(Default)]
pub(crate) struct Clipboard {
    writer: Arc<Writer>,
    text: Option<TextCopy>,
}

enum TextCopy {
    Pending(String),
    Published,
}

impl Global for Clipboard {}

#[derive(Default)]
pub(crate) struct Writer {
    generation: AtomicU64,
    lock: Mutex<()>,
}

impl Writer {
    fn begin(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub(crate) fn current(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    pub(crate) async fn commit<T>(&self, generation: u64, write: impl FnOnce() -> T) -> Option<T> {
        let _guard = self.lock.lock().await;
        (self.current() == generation).then(write)
    }
}

pub(crate) fn writer(cx: &mut App) -> Arc<Writer> {
    cx.default_global::<Clipboard>().writer.clone()
}

pub(crate) fn begin_file_write(cx: &mut App) -> u64 {
    let clipboard = cx.global_mut::<Clipboard>();
    clipboard.text = None;
    clipboard.writer.begin()
}

pub(crate) fn text_is_current(cx: &App) -> bool {
    cx.try_global::<Clipboard>()
        .is_some_and(|clipboard| clipboard.text.is_some())
}

pub(crate) fn pending_text(cx: &App) -> Option<&str> {
    match cx.try_global::<Clipboard>()?.text.as_ref()? {
        TextCopy::Pending(text) => Some(text),
        TextCopy::Published => None,
    }
}

pub(crate) fn read_text(cx: &App) -> Option<String> {
    pending_text(cx)
        .map(str::to_owned)
        .or_else(|| cx.read_from_clipboard().and_then(|item| item.text()))
}

pub(crate) fn copy_text(text: String, cx: &mut App) {
    let clipboard = cx.default_global::<Clipboard>();
    let writer = clipboard.writer.clone();
    let generation = writer.begin();

    // Wayland's GPUI write needs window focus and queues the selection request.
    #[cfg(target_os = "linux")]
    let native = !cfg!(test) && crate::os_clipboard::wayland();
    #[cfg(windows)]
    let native = false;

    if !native {
        if let Some(_guard) = writer.lock.try_lock() {
            clipboard.text = Some(TextCopy::Published);
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            return;
        }
    }

    clipboard.text = Some(TextCopy::Pending(text.clone()));
    cx.spawn(async move |cx| {
        let _guard = writer.lock.lock().await;
        if writer.current() != generation {
            return;
        }
        #[cfg(target_os = "linux")]
        let native_written = if native {
            let text = text.clone();
            cx.background_executor()
                .spawn(async move { crate::os_clipboard::publish_wayland_text(&text) })
                .await
                .is_ok()
        } else {
            false
        };
        #[cfg(windows)]
        let native_written = false;

        let _ = cx.update(|cx| {
            if writer.current() != generation {
                return;
            }
            if !native_written {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
            cx.global_mut::<Clipboard>().text = Some(TextCopy::Published);
        });
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[gpui::test]
    fn delayed_file_copy_cannot_replace_a_newer_clipboard_choice(cx: &mut TestAppContext) {
        let writer = cx.update(writer);
        let slow = cx.update(begin_file_write);
        let newer = cx.update(begin_file_write);
        cx.executor().block_test(async {
            assert_eq!(
                writer.commit(newer, || "newer files").await,
                Some("newer files")
            );
            assert_eq!(writer.commit(slow, || "older files").await, None);
        });
    }

    #[gpui::test]
    fn text_waits_for_an_in_flight_file_write(cx: &mut TestAppContext) {
        let writer = cx.update(writer);
        let generation = writer.begin();
        let guard = writer.lock.try_lock().unwrap();

        cx.update(|cx| copy_text("new text".into(), cx));
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(read_text(cx).as_deref(), Some("new text"));
            assert!(cx.read_from_clipboard().is_none());
            cx.write_to_clipboard(ClipboardItem::new_string("older file write".into()));
        });
        assert_ne!(writer.current(), generation);
        drop(guard);

        cx.run_until_parked();
        assert_eq!(
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .as_deref(),
            Some("new text")
        );
    }

    #[gpui::test]
    fn newer_file_copy_supersedes_queued_text(cx: &mut TestAppContext) {
        let writer = cx.update(writer);
        let guard = writer.lock.try_lock().unwrap();
        cx.update(|cx| copy_text("queued text".into(), cx));
        cx.update(begin_file_write);
        drop(guard);
        cx.run_until_parked();
        cx.update(|cx| assert_eq!(read_text(cx), None));
    }
}
