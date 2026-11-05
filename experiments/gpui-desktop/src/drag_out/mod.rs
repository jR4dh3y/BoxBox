//! Dragging files out to other apps. GPUI 0.2 only receives external drops, so the source side
//! talks to the platform directly. Only X11 is implemented.

#[cfg(target_os = "linux")]
mod x11;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result};
use gpui::{div, prelude::*, px, App, Context, Entity, SharedString, WeakEntity, Window};

use crate::api::{Api, FileInfo};
use crate::theme::*;
use crate::workspace::Workspace;

/// The in-window drag value: the index of the row or tile, resolved to items when a drag starts.
pub struct DraggedItem(pub usize);

pub struct DragPreview {
    label: SharedString,
}

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(4.))
            .border_1()
            .border_color(c(BORDER_PRIMARY))
            .bg(c(SURFACE_SECONDARY))
            .text_size(px(13.))
            .text_color(c(TEXT_PRIMARY))
            .child(self.label.clone())
    }
}

/// Makes the item at `index` draggable within the window and out to other apps.
pub fn draggable<E: StatefulInteractiveElement>(
    element: E,
    index: usize,
    workspace: WeakEntity<Workspace>,
    api: Arc<Api>,
) -> E {
    element.on_drag(
        DraggedItem(index),
        move |item: &DraggedItem, _, _: &mut Window, cx: &mut App| -> Entity<DragPreview> {
            let files = workspace
                .upgrade()
                .map(|workspace| workspace.read(cx).pane().drag_items(item.0))
                .unwrap_or_default();
            let label = match files.as_slice() {
                [item] => item.name.clone(),
                items => format!("{} items", items.len()),
            };
            if !files.is_empty() {
                start(files, api.clone(), cx);
            }
            cx.new(|_| DragPreview {
                label: label.into(),
            })
        },
    )
}

fn start(files: Vec<FileInfo>, api: Arc<Api>, cx: &App) {
    #[cfg(target_os = "linux")]
    if cx.compositor_name() == "X11" {
        log::info!("dragging {} item(s) out over X11", files.len());
        std::thread::spawn(move || {
            let prepare = Box::new(move || stage(&api, &files));
            if let Err(error) = x11::drag(prepare) {
                log::warn!("drag to another app failed: {error:#}");
            }
        });
        return;
    }
    let _ = (files, api);
    log::info!(
        "dragging to other apps is not implemented on {}",
        cx.compositor_name()
    );
}

/// Other apps need local files, so the dragged items are downloaded to a fresh temp folder.
fn stage(api: &Api, files: &[FileInfo]) -> Result<Vec<PathBuf>> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let dir = std::env::temp_dir()
        .join("boxbox-gpui-drag")
        .join(format!("{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
    files.iter().map(|item| api.download(item, &dir)).collect()
}

/// `text/uri-list`: one percent-encoded `file://` URI per line, CRLF-terminated.
pub fn uri_list(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| format!("file://{}\r\n", encode_path(path)))
        .collect()
}

fn encode_path(path: &Path) -> String {
    #[cfg(unix)]
    let bytes = std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str()).to_vec();
    #[cfg(not(unix))]
    let bytes = path.to_string_lossy().replace('\\', "/").into_bytes();
    bytes
        .into_iter()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_list_encodes_each_path() {
        let paths = [
            PathBuf::from("/tmp/a b/été #1.txt"),
            PathBuf::from("/tmp/x.zip"),
        ];
        assert_eq!(
            uri_list(&paths),
            "file:///tmp/a%20b/%C3%A9t%C3%A9%20%231.txt\r\nfile:///tmp/x.zip\r\n"
        );
    }
}
