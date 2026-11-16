//! Dragging files out to other apps. GPUI 0.2 only receives external drops, so the source side
//! talks to the platform directly: XDND on X11, `wl_data_device` on Wayland.

#[cfg(target_os = "linux")]
mod wayland;
#[cfg(target_os = "linux")]
mod x11;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result};
use gpui::{
    div, point, prelude::*, px, App, Bounds, Context, DragMoveEvent, Entity, Global, Pixels, Point,
    SharedString, WeakEntity, Window,
};

use crate::api::{Api, FileInfo};
use crate::theme::*;
use crate::workspace::operations::PaneTarget;
use crate::workspace::Workspace;

/// Resolves the dragged items to real paths. It can download, so it never runs on the UI thread.
pub type Prepare = Box<dyn FnOnce() -> Result<Vec<PathBuf>> + Send>;

/// The in-window drag value: the pane and index of the row or tile, resolved to items when a
/// drag starts.
pub struct DraggedItem {
    pub pane: PaneTarget,
    pub index: usize,
}

pub struct DragPreview {
    pub label: SharedString,
    /// Where in the dragged row the pointer took hold. GPUI draws the preview at
    /// the row's corner, so this much padding puts the label beside the pointer.
    pub grip: Point<Pixels>,
}

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().pl(self.grip.x + px(12.)).pt(self.grip.y + px(12.)).child(
            div()
                .when_some(cx.try_global::<crate::components::UiFont>(), |el, font| el.font_family(font.0.clone()))
                .px(px(8.))
                .py(px(4.))
                .rounded(px(4.))
                .border_1()
                .border_color(c(BORDER_PRIMARY))
                .bg(c(SURFACE_SECONDARY))
                .text_size(px(13.))
                .text_color(c(TEXT_PRIMARY))
                .child(self.label.clone()),
        )
    }
}

/// Makes the item at `index` of `pane` draggable within the window and out to other apps.
pub fn draggable<E: StatefulInteractiveElement>(
    element: E,
    pane: PaneTarget,
    index: usize,
    workspace: WeakEntity<Workspace>,
    api: Arc<Api>,
) -> E {
    element.on_drag(
        DraggedItem { pane, index },
        move |item: &DraggedItem, grip, _: &mut Window, cx: &mut App| -> Entity<DragPreview> {
            let files = workspace.update(cx, |workspace, _| workspace.begin_drag(item.pane, item.index)).unwrap_or_default();
            let label = match files.as_slice() {
                [item] => item.name.clone(),
                items => format!("{} items", items.len()),
            };
            start(files, api.clone(), cx);
            cx.new(|_| DragPreview { label: label.into(), grip })
        },
    )
}

fn start(files: Vec<FileInfo>, api: Arc<Api>, cx: &mut App) {
    #[cfg(target_os = "linux")]
    if let Some(drag) = wayland_drag(cx) {
        drag.pending = None;
    }
    if files.is_empty() {
        return;
    }
    let prepare: Prepare = Box::new(move || stage(&api, &files));
    #[cfg(target_os = "linux")]
    match cx.compositor_name() {
        "X11" => {
            std::thread::spawn(move || {
                if let Err(error) = x11::drag(prepare) {
                    log::warn!("drag to another app failed: {error:#}");
                }
            });
        }
        // Held until the pointer leaves the window; see `moved`.
        "Wayland" => {
            if let Some(drag) = wayland_drag(cx) {
                drag.pending = Some(prepare);
            }
        }
        name => log::info!("dragging to other apps is not implemented on {name}"),
    }
    #[cfg(not(target_os = "linux"))]
    drop(prepare);
}

/// The Wayland drag source of this app, and the drag that has not left the window yet.
#[cfg(target_os = "linux")]
struct WaylandDrag {
    source: wayland::Source,
    pending: Option<Prepare>,
}

#[cfg(target_os = "linux")]
impl Global for WaylandDrag {}

#[cfg(target_os = "linux")]
fn wayland_drag(cx: &mut App) -> Option<&mut WaylandDrag> {
    cx.has_global::<WaylandDrag>()
        .then(|| cx.global_mut::<WaylandDrag>())
}

/// Call once the window exists. On Wayland this starts listening for the pointer presses a drag
/// has to name.
pub fn init(window: &Window, cx: &mut App) {
    #[cfg(target_os = "linux")]
    if cx.compositor_name() == "Wayland" {
        match wayland::Source::connect(window) {
            Ok(source) => cx.set_global(WaylandDrag {
                source,
                pending: None,
            }),
            Err(error) => log::warn!("dragging to other apps is unavailable: {error:#}"),
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (window, cx);
}

/// Wayland keeps sending the pointer to the pressed window while the button is held, so a drag
/// stays in the window until it is handed to the compositor. That happens when it crosses the
/// window edge, and it ends the in-window drag: GPUI never sees the button come up.
pub fn moved(event: &DragMoveEvent<DraggedItem>, window: &mut Window, cx: &mut App) {
    #[cfg(target_os = "linux")]
    {
        let viewport = Bounds::new(point(px(0.), px(0.)), window.viewport_size());
        if viewport.contains(&event.event.position) {
            return;
        }
        let Some(drag) = wayland_drag(cx) else {
            return;
        };
        let Some(prepare) = drag.pending.take() else {
            return;
        };
        if let Err(error) = drag.source.start(window, wayland::Payload::new(prepare)) {
            log::warn!("drag to another app failed: {error:#}");
        }
        cx.stop_active_drag(window);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (event, window, cx);
}

/// Remote files are staged for other apps; local files keep their original filesystem paths.
fn stage(api: &Api, files: &[FileInfo]) -> Result<Vec<PathBuf>> {
    if api.is_local() {
        return files
            .iter()
            .map(|item| {
                api.local_path(&item.path)?
                    .context("local file has no filesystem path")
            })
            .collect();
    }
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
